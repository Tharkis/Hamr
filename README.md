<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo/hamr-logo-dark.svg">
    <img alt="Hamr" src="assets/logo/hamr-logo-light.svg" width="372">
  </picture>
</p>

<p align="center"><i>Shape anything from primitives.</i></p>

# Hamr

Hamr is a desktop 3D modeling tool for building models out of primitives, in the spirit of the
Second Life build tools. Models are meant for 3D printing and games: Hamr exports watertight STL,
OBJ and glTF. It runs natively on Windows and Linux.

Hamr is an independent project. It reproduces how the Second Life build tools behave but contains
no code from the Second Life viewer.

## The name

In Old Norse, *hamr* is a shape, a skin, the outward form a thing takes. Shapeshifters changed
their hamr, and Freyja lent out her *valshamr*, a falcon-shape cloak, to anyone who needed wings.
Its close cousin *hamarr* is a hammer, which is why the icon is Thor's hammer Mjǫllnir, built the
way Hamr builds things: a box for the head, a cylinder for the handle and a sphere for the pommel.
The handle is short on purpose. In the Prose Edda, Loki, as a fly, bit the smith at the bellows,
and the handle came out too short.

## Install

Download the latest release from the
[releases page](https://github.com/Tharkis/Hamr/releases/latest):

| System | File |
| --- | --- |
| Windows 10 and 11 | `Hamr-<version>-windows-x64-setup.exe`, or the portable `.zip` |
| Arch Linux, CachyOS | `hamr-<version>-1-x86_64.pkg.tar.zst`: `sudo pacman -U <file>` |
| Debian, Ubuntu, Mint | `hamr_<version>-1_amd64.deb`: `sudo apt install ./<file>` |
| Fedora, openSUSE | `hamr-<version>-1.x86_64.rpm`: `sudo dnf install ./<file>` |
| Any Linux, sandboxed | `Hamr-<version>-x86_64.flatpak`: `flatpak install --user <file>` |
| Any Linux, no install | `Hamr-<version>-x86_64.AppImage`: make it executable and run it |

Hamr needs a graphics driver with Vulkan support (Mesa on Linux).

## Status

Version 1.0: the whole planned feature set works. You can create all seven Second Life
prim types and edit their position, size, rotation, colour and every SL shape parameter. Move,
rotate and stretch handles snap to the grid, every edit can be undone, and projects save to `.hamr`
files. Prims can be linked, mirrored and repeated in arrays, and any selection can be saved as a
custom prim, either as an editable group or as one baked mesh. Export merges everything into one
watertight solid, cutting out negative prims, and writes STL, OBJ or glTF. A printability check
warns about overhangs, floating pieces and models too big for the printer.

| Milestone | Contents | State |
| --- | --- | --- |
| 1. Foundation | Window, viewport, camera, grid, units, CI | Done |
| 2. Prim engine | Every SL prim type and parameter, hole-cutting prims, watertight tests | Done |
| 3. Build tools | Move/rotate/stretch, snapping, linking, mirror and array, undo, saving, custom prim library | Done |
| 4. Export | STL, OBJ, glTF, merge into one solid, baked custom prims, printability check | Done |

## Building

Install Rust (1.85 or newer) from <https://rustup.rs>, CMake, and a C++ compiler (Visual Studio
Build Tools on Windows, `g++` or `clang` on Linux). The first build downloads and compiles the
[Manifold](https://github.com/elalish/manifold) geometry library, which takes a few minutes. Then:

```sh
cargo run --release
```

Run the tests with `cargo test`. The installers and Linux packages are built by the Release
workflow (`.github/workflows/release.yml`) from the files in `packaging/`.

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
| Add to or remove from the selection | Shift+click |
| Link, unlink | Ctrl+L, Ctrl+Shift+L |
| Undo, redo | Ctrl+Z, Ctrl+Y or Ctrl+Shift+Z |
| New, open, save, save as | Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+Shift+S |
| Export | Ctrl+E |

Stretch handles resize from the opposite face, or around the centre with "Stretch both sides" on.
Turn off "Snap to grid" for free movement; snapping uses the grid spacing and 15° steps.

Clicking a linked prim selects its whole link set, and the last prim picked becomes the root when
linking, as in Second Life. Tick "Edit linked parts" to pick single prims inside a link set.
Several selected prims move, turn and scale together; a group scales evenly.

The Build panel also has:

- **Mirror**: flip the selection across a plane through the origin or through the selection, keeping
  the original or not.
- **Array**: add copies in a row (count and offset) or around an axis (count, centre and angle).
- **Custom prims**: save the selection under a name, as a group of editable prims or as one baked
  mesh, then click it to add it again like a prim. Right-click one to delete it from the library.

## Exporting

File > Export (Ctrl+E) merges every prim into one watertight solid, joining overlaps and cutting
out negative prims, and saves it in the format of the extension you pick:

- **STL**: binary, in millimetres, for slicers.
- **OBJ**: in millimetres, with vertex colours.
- **glTF** (`.glb`): in metres and Y up as glTF expects, with colours and normals, for game engines.

File > Export selection does the same for the selected prims only. Tools > Check printability
reports pieces that start in mid air, overhangs steeper than your printer manages, pieces too small
to print and models larger than the build volume.

## Layout

- `crates/hamr-core`: prims, geometry, scene, camera and units, with no UI or GPU code, so it can be
  tested on its own.
- `crates/hamr-app`: the desktop application (egui for the interface, wgpu for 3D rendering).
- `assets/`: the icon and logo. `packaging/`: installer and package definitions.

All lengths are stored in millimetres. The world is right-handed with Z pointing up.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your
option.
