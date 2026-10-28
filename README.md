# Hamr

Hamr is a desktop 3D modeling tool for building models out of primitives, in the spirit of the
Second Life build tools. Models are meant for 3D printing and games: Hamr exports watertight STL,
OBJ and glTF. It runs natively on Windows and Linux.

Hamr is an independent project. It reproduces how the Second Life build tools behave but contains
no code from the Second Life viewer.

## Status

Early development. You can create all seven Second Life prim types, select them, and edit their
position, size, rotation, colour and every SL shape parameter. Prims can be marked negative to cut
holes; the cut is applied when exporting, which comes in step 4. Move, rotate and stretch handles
snap to the grid, every edit can be undone, and projects save to `.hamr` files. Linking, mirror and
array, and the custom prim library come next.

| Milestone | Contents | State |
| --- | --- | --- |
| 1. Foundation | Window, viewport, camera, grid, units, CI | Done |
| 2. Prim engine | Every SL prim type and parameter, hole-cutting prims, watertight tests | Done |
| 3. Build tools | Move/rotate/stretch, snapping, linking, mirror and array, undo, saving, custom prim library | In progress |
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
| Select a prim | Click | |
| Delete the selected prim | Delete | |

## Editing

| Action | Shortcut |
| --- | --- |
| Move, rotate, stretch tool | W, E, R |
| Copy while moving | Shift+drag an arrow |
| Duplicate | Ctrl+D |
| Undo, redo | Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z |
| New, open, save, save as | Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+Shift+S |

Stretch handles resize from the opposite face, or around the centre with "Stretch both sides" on.
Turn off "Snap to grid" for free movement; snapping uses the grid spacing and 15° steps.

## Layout

- `crates/hamr-core`: prims, geometry, scene, camera and units, with no UI or GPU code, so it can be
  tested on its own.
- `crates/hamr-app`: the desktop application (egui for the interface, wgpu for 3D rendering).

All lengths are stored in millimetres. The world is right-handed with Z pointing up.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your
option.
