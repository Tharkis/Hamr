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
| `crates/hamr-core/src/mesh.rs` | `Mesh` and `Vertex` |
| `crates/hamr-app` | The `hamr` binary: eframe/egui UI and wgpu viewport |
| `crates/hamr-app/src/renderer.rs`, `*.wgsl` | Viewport rendering and shaders |
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

## Known traps

- Run `xdotool windowfocus <id>` before `xdotool key ...`, or the keys go nowhere.
- Without `mesa-vulkan-drivers` wgpu finds no Vulkan adapter in the container.
- CI runs on every push to `main` and every PR; keep fmt and clippy clean before pushing.
- Windows is a supported target: avoid Unix-only paths, shell calls and APIs in app code.
