# Agent notes

Detail behind [CLAUDE.md](CLAUDE.md), which holds the rules, working-efficiently and testing
guidance; read it first. Agents that don't read CLAUDE.md should treat it as part of this file. Keep
this file to facts that save a session from rediscovering them, and update both files when a change
makes them wrong.

## Repo map

| Path | What lives there |
| --- | --- |
| `crates/hamr-core` | UI-free logic: units, camera, grid, meshes (and later prims, booleans, export) |
| `crates/hamr-core/src/units.rs` | `Unit` and mm conversions; lengths are always stored in mm |
| `crates/hamr-core/src/camera.rs` | `OrbitCamera`, SL-style orbit / pan / zoom / focus |
| `crates/hamr-core/src/mesh.rs` | `Mesh` and `Vertex`: render-ready, vertices split per corner for shading |
| `crates/hamr-core/src/solid.rs` | `Solid`: indexed closed surface, `check_closed`, `to_mesh` for shading |
| `crates/hamr-core/src/prim/` | `PrimShape` (SL Object-tab parameters), `profile.rs` cross-sections, `sweep.rs` path sweep and caps, `tests.rs` watertightness grid |
| `crates/hamr-core/src/scene.rs` | `Scene` and `Object` (prim + position, rotation, size in mm, colour, negative flag) |
| `crates/hamr-core/src/history.rs` | `History`: snapshot undo/redo; `commit` turns changes since the last commit into one step |
| `crates/hamr-core/src/project.rs` | `.hamr` project files: versioned JSON of the scene, `to_json` / `from_json` |
| `crates/hamr-core/src/gizmo.rs` | Move/rotate/stretch handles: their lines, screen-space hit test, and `Drag` maths with snapping |
| `crates/hamr-app` | The `hamr` binary: eframe/egui UI and wgpu viewport |
| `crates/hamr-app/src/app.rs` | Window layout, menus, shortcuts, Build panel, viewport input, handles, picking, undo commits |
| `crates/hamr-app/src/files.rs` | Open/save dialogs (rfd) and atomic project file writes |
| `crates/hamr-app/src/object_panel.rs` | Object panel: placement and shape parameters of the selected prim |
| `crates/hamr-app/src/renderer.rs`, `*.wgsl` | Viewport rendering (opaque, translucent negative prims, grid, overlay) and shaders |
| `.github/workflows/ci.yml` | fmt + clippy, then test and release build on Ubuntu and Windows |

Plan: 1) foundation, viewport, CI 2) prim engine 3) build tools 4) export. See the README status table.

## Commands

```sh
cargo build                                   # debug build of the workspace
cargo run --release                           # launch the app
cargo test --all                              # all tests
cargo test -p hamr-core                       # core only, no window needed
cargo test -p hamr-core camera::              # tests in one module
cargo fmt --all --check                       # CI formatting check
cargo clippy --all-targets -- -D warnings     # CI lint; warnings fail the build
```

## Running the app in a cloud container

There is no display or GPU, so the app needs a virtual X server and a software Vulkan driver:

```sh
sudo apt-get install -y xvfb libxkbcommon-x11-0 mesa-vulkan-drivers xdotool imagemagick
Xvfb :99 -screen 0 1280x800x24 &
DISPLAY=:99 cargo run --release &
DISPLAY=:99 import -window root /tmp/hamr.png  # screenshot
```

## Prim geometry

- Prims are built in the unit box (−0.5..0.5) and scaled by `Object::transform`. Box, cylinder and
  prism sweep straight up; sphere, torus, tube and ring revolve around Z. SL's "path cut" is a
  profile cut on the linear types and a path cut on the circular ones; see `prim/mod.rs`.
- Check prims with `Solid::check_closed`, which uses shared indices. `Mesh::check_watertight`
  welds by position, so it wrongly fails prims whose separate surfaces touch (a twisted sphere
  crosses its own axis).
- `sweep.rs` only merges vertices that are the same profile point in neighbouring rings, or
  neighbouring points in a ring. Don't weld by position globally: it fuses touching surfaces into
  non-manifold edges.
- Full tapers stop at `MIN_TAPER_SCALE` and ring-like profiles stay just off the axis, so walls
  never fold onto each other. Twisted profiles get extra points along long edges
  (`MAX_TWISTED_EDGE`), and caps fan from a centre point to include them.
- Negative prims only carry a flag for now; the subtraction happens at export (step 4).

## Editing and undo

- Edits change `Scene` directly. `app.rs` calls `History::commit` once nothing is held: no handle
  drag, no mouse button down and no focused widget. A whole drag or a typed value becomes one undo
  step, so don't commit from inside individual edits.
- Move and rotate handles use world axes, stretch handles the prim's own axes. Handle maths lives in
  `gizmo.rs` so it is tested without a window; the app only hit-tests and feeds it rays.
- Unsaved changes are `scene != saved`, so undoing back to the saved state clears the `*` in the
  title.

## Known traps

- Run `xdotool windowfocus <id>` before `xdotool key ...`, or the keys go nowhere.
- Without `mesa-vulkan-drivers` wgpu finds no Vulkan adapter in the container.
- CI runs on every push to `main` and every PR; keep fmt and clippy clean before pushing.
- Windows is a supported target: avoid Unix-only paths, shell calls and APIs in app code.
- `hamr-core` is built with `opt-level = 2` in dev (root `Cargo.toml`); without it the prim test
  grid takes about 30 s instead of 2 s.
- `xdotool windowclose` destroys the window instead of asking it to close, so it skips the
  unsaved-changes prompt; test that through File > Quit.
- egui `DragValue`s can be set from xdotool by clicking one, then `ctrl+a`, then
  `type -- "<value>"` and `Return`. The `--` matters for negative numbers.
