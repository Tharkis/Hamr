# Hamr: notes for coding agents

Hamr builds 3D models out of primitives, in the spirit of the Second Life build tools, and exports
watertight STL, OBJ and glTF. It must run natively on Windows and Linux.

This is an independent implementation. Reproduce how the Second Life build tools behave from public
documentation (the SL wiki and knowledge base), but never copy code, shaders or data from the Second
Life viewer, and don't paste its source into the repo or a PR. New dependencies must be permissively
licensed (MIT, Apache-2.0, BSD, Zlib or similar) to fit Hamr's MIT OR Apache-2.0 licence.

- Build: `cargo build`; run: `cargo run --release`; test: `cargo test --all`.
- Lint like CI: `cargo fmt --all --check` and `cargo clippy --all-targets -- -D warnings`.
- `hamr-core` holds geometry, prims, camera and units with no UI or GPU code; keep it that way so it
  stays testable without a window. `hamr-app` is the egui + wgpu desktop app.
- All lengths are millimetres. The world is right-handed with Z up.
- Every exported or merged mesh must be watertight (closed, manifold, consistent winding).
- Shared dependency versions go in `[workspace.dependencies]` in the root `Cargo.toml`.

Repo map, exact commands and known traps: [AGENTS.md](AGENTS.md). Keep this file and AGENTS.md up to
date in the same PR as any change that makes them wrong (new crate, command, trap or convention).

## Working efficiently

- Search with Grep or Glob before opening files, read only the line range you need, and don't reread
  files.
- Skip `target/`, `Cargo.lock` and generated or binary assets unless the task is about them.
- Prefer `cargo check -p <crate>` while iterating; build the app only when you need to see it run.
- Keep output short: don't restate the diff in replies, and show only the failing part of a build or
  test run.
- Put new logic in `hamr-core` where possible, so it can be tested without launching the app.

## Testing

Add tests for meaningful behavior or a reproducible bug, and say what failure each test catches.
Prefer extending existing tests, and skip tests that only repeat the implementation. Still run the
relevant existing tests.

- Run the narrowest relevant tests first (`cargo test -p hamr-core <name>`), and the full suite plus
  fmt and clippy before a PR.
- Geometry changes (prims, booleans, export) need a watertightness test: closed edges, no
  degenerate triangles, outward normals.
- UI changes need a manual check in the running app; see AGENTS.md for running it headless in the
  cloud and taking a screenshot.
- Docs-only changes need no test run.
