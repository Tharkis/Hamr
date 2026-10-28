# Hamr

Hamr is a desktop 3D modeling tool for building models out of primitives, in the spirit of the
Second Life build tools. Models are meant for 3D printing and games: Hamr exports watertight STL,
OBJ and glTF. It runs natively on Windows and Linux.

Hamr is an independent project. It reproduces how the Second Life build tools behave but contains
no code from the Second Life viewer.

## Status

Early development. The current build opens a window with a 3D viewport, a ground grid and camera
controls. Prims, editing tools and export are next.

| Milestone | Contents | State |
| --- | --- | --- |
| 1. Foundation | Window, viewport, camera, grid, units, CI | In progress |
| 2. Prim engine | Every SL prim type and parameter, hole-cutting prims, watertight tests | Planned |
| 3. Build tools | Move/rotate/stretch, snapping, linking, mirror and array, undo, saving, custom prim library | Planned |
| 4. Export | STL, OBJ, glTF, merge into one solid, printability check | Planned |

## Building

Install Rust (1.85 or newer) from <https://rustup.rs>, then:

```sh
cargo run --release
```

Run the tests with `cargo test`.

## Camera controls

| Action | Mouse | Second Life style |
| --- | --- | --- |
| Orbit | Right-drag | Ctrl+Alt+drag |
| Orbit sideways and zoom | | Alt+drag |
| Pan | Middle-drag | Ctrl+Alt+Shift+drag |
| Zoom | Wheel | |
| Focus on a point | | Alt+click |
| Frame everything | F | |
| Reset camera | Home | |

## Layout

- `crates/hamr-core`: geometry, camera and units, with no UI or GPU code, so it can be tested on its
  own.
- `crates/hamr-app`: the desktop application (egui for the interface, wgpu for 3D rendering).

All lengths are stored in millimetres. The world is right-handed with Z pointing up.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your
option.
