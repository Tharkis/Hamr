# Agent notes

Detail behind [CLAUDE.md](CLAUDE.md), which holds the rules, working-efficiently and testing
guidance; read it first. Agents that don't read CLAUDE.md should treat it as part of this file. Keep
this file to facts that save a session from rediscovering them, and update both files when a change
makes them wrong.

## Repo map

| Path | What lives there |
| --- | --- |
| `crates/hamr-core` | UI-free logic: units, camera, grid, meshes, prims, booleans, export |
| `crates/hamr-core/src/units.rs` | `Unit` and mm conversions; lengths are always stored in mm |
| `crates/hamr-core/src/camera.rs` | `OrbitCamera`, SL-style orbit / pan / zoom / focus |
| `crates/hamr-core/src/mesh.rs` | `Mesh` and `Vertex`: render-ready, vertices split per corner for shading |
| `crates/hamr-core/src/solid.rs` | `Solid`: indexed closed surface, `check_closed`, `to_mesh` for shading |
| `crates/hamr-core/src/prim/` | `PrimShape` (SL Object-tab parameters), `profile.rs` cross-sections, `sweep.rs` path sweep and caps, `tests.rs` watertightness grid |
| `crates/hamr-core/src/scene.rs` | `Scene` and `Object` (prim + position, rotation, size in mm, colour, negative flag) |
| `crates/hamr-core/src/edit.rs` | Multi-prim edits: group frame and group transform for handles, mirror, linear and circular arrays |
| `crates/hamr-core/src/library.rs` | `CustomPrim`: a saved selection (`.hamrprim` JSON) placed back as one link set |
| `crates/hamr-core/src/merge.rs` | `merge`: union of positive prims minus negative ones via Manifold, into a `Model` (solid + corner colours) |
| `crates/hamr-core/src/export.rs` | STL (binary), OBJ (vertex colours) and GLB (metres, Y up) writers for a `Model` |
| `crates/hamr-core/src/printability.rs` | Printability check: build volume, floating and tiny pieces, overhangs |
| `crates/hamr-core/src/history.rs` | `History`: snapshot undo/redo; `commit` turns changes since the last commit into one step |
| `crates/hamr-core/src/project.rs` | `.hamr` project files: versioned JSON of the scene, `to_json` / `from_json` |
| `crates/hamr-core/src/gizmo.rs` | Move/rotate/stretch handles: their lines, screen-space hit test, and `Drag` maths with snapping |
| `crates/hamr-app` | The `hamr` binary: eframe/egui UI and wgpu viewport |
| `crates/hamr-app/src/about.rs` | About window and `ICON_PNG`, also used for the window icon in `main.rs` |
| `crates/hamr-app/build.rs` | Embeds `assets/icon/hamr.ico` in the Windows exe (winresource) |
| `crates/hamr-app/src/app.rs` | Window layout, menus, shortcuts, Build panel, viewport input, handles, picking, undo commits |
| `crates/hamr-app/src/arrange_panel.rs` | Mirror and Array section of the Build panel |
| `crates/hamr-app/src/library_panel.rs` | Custom prims section and the per-user library folder |
| `crates/hamr-app/src/print_check.rs` | Printability check window and printer settings |
| `crates/hamr-app/src/files.rs` | Open/save dialogs (rfd) and atomic project file writes |
| `crates/hamr-app/src/object_panel.rs` | Object panel: placement and shape parameters of the selected prim |
| `crates/hamr-app/src/renderer.rs`, `*.wgsl` | Viewport rendering (opaque, translucent negative prims, grid, overlay) and shaders |
| `.github/workflows/ci.yml` | fmt + clippy, then test and release build on Ubuntu and Windows |
| `.github/workflows/release.yml` | Builds every installer and package; run by hand on `main` to tag and publish a release |
| `assets/icon/` | App icon: `hamr.svg` (full), `hamr-small.svg` (no engraved rune, for 16 and 32 px), rendered PNGs and `hamr.ico` |
| `assets/logo/` | README logo (icon + rune-cut wordmark), light and dark versions |
| `packaging/linux/` | Desktop file, AppStream metainfo, `stage.sh` (tree every Linux package is made from), `build-appimage.sh` |
| `packaging/arch/PKGBUILD` | Arch/CachyOS package, repackaging the release tarball |
| `packaging/flatpak/` | Flatpak manifest, packaging the staged release build |
| `packaging/windows/hamr.iss` | Inno Setup installer script |
| `packaging/release-notes.md` | Install table put at the top of every GitHub release |

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
cargo deb -p hamr --no-build                  # .deb from target/release (needs cargo-deb)
cargo generate-rpm -p crates/hamr-app         # .rpm from target/release (needs cargo-generate-rpm)
packaging/linux/stage.sh                      # dist/hamr-linux, input to tarball, AppImage, Arch, Flatpak
```

## Releasing

The version lives in `[workspace.package]` in the root `Cargo.toml`; also add a `<release>` line
to `packaging/linux/io.github.tharkis.Hamr.metainfo.xml`. After that change is merged, run the
Release workflow on `main` (Actions > Release > Run workflow). It builds the Windows installer and
zip, `.deb`, `.rpm`, Arch package, Flatpak, AppImage and tarball, tags `v<version>` and publishes
the GitHub release. Pull requests that touch packaging run the same builds without publishing.

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
- Negative prims are only subtracted when merging (`merge.rs`), for export, the printability check
  and baked custom prims. The viewport just draws them translucent.

## Editing and undo

- Edits change `Scene` directly. `app.rs` calls `History::commit` once nothing is held: no handle
  drag, no mouse button down and no focused widget. A whole drag or a typed value becomes one undo
  step, so don't commit from inside individual edits.
- Move and rotate handles use world axes, stretch handles the prim's own axes. Handle maths lives in
  `gizmo.rs` so it is tested without a window; the app only hit-tests and feeds it rays.
- Link sets are flat: each child's `Object::link` names the root directly, never another child.
  Positions stay in world space, so moving a set means moving every member (`edit::transform_group`).
  `Scene::from_objects` drops links that would break this, so loaded files can't create chains.
- Several selected prims share one set of handles on `edit::group_frame`. A group can only scale
  evenly, since its prims may be turned any way.
- Mirroring sets `Object::mirrored` (a flip of the prim's own X axis) and adjusts the rotation. The
  transform's determinant is then negative, so anything that builds triangles from it (the
  renderer and `merge::world_solid`) must reverse their winding.
- Custom prims live in `%APPDATA%\Hamr\prims` on Windows and `$XDG_DATA_HOME/hamr/prims` (or
  `~/.local/share/hamr/prims`) elsewhere. A baked custom prim is one `Object` whose `baked` solid
  (in the unit box) replaces its prim shape; use `Object::solid` / `Object::mesh`, never
  `object.shape` directly, for geometry. Loading rejects baked meshes that aren't closed.
- Unsaved changes are `scene != saved`, so undoing back to the saved state clears the `*` in the
  title.

## Known traps

- The app ID `io.github.tharkis.Hamr` must match in `main.rs` (`APP_ID`), the desktop file, the
  metainfo, icon file names and the Flatpak manifest. Wayland finds the window's icon through it;
  the window icon set in `main.rs` only works on X11 and Windows.
- Regenerate the icon PNGs and `.ico` from the SVGs together when the icon changes; sizes 16 and
  32 come from `hamr-small.svg`.

- Run `xdotool windowfocus <id>` before `xdotool key ...`, or the keys go nowhere.
- Without `mesa-vulkan-drivers` wgpu finds no Vulkan adapter in the container.
- CI runs on every push to `main` and every PR; keep fmt and clippy clean before pushing.
- Windows is a supported target: avoid Unix-only paths, shell calls and APIs in app code.
- `hamr-core` is built with `opt-level = 2` in dev (root `Cargo.toml`); without it the prim test
  grid takes about 30 s instead of 2 s.
- The first build compiles the Manifold C++ library, so it needs CMake, a C++ compiler and network
  access (the `manifold-csg-sys` build script clones Manifold with git). It takes a few minutes;
  later builds are cached. TBB is off (`default-features = false`).
- `rfd` file dialogs don't open under Xvfb (no desktop portal), so export and open/save can't be
  driven headless; test the writers through `hamr-core` instead.
- `xdotool windowclose` destroys the window instead of asking it to close, so it skips the
  unsaved-changes prompt; test that through File > Quit.
- egui `DragValue`s can be set from xdotool by clicking one, then `ctrl+a`, then
  `type -- "<value>"` and `Return`. The `--` matters for negative numbers.
