//! Core logic for Hamr: units, camera, meshes and helpers that do not depend on any UI or GPU code.
//!
//! Hamr uses a right-handed, Z-up coordinate system and stores every length in millimetres.

pub mod camera;
pub mod edit;
pub mod gizmo;
pub mod grid;
pub mod history;
pub mod library;
pub mod mesh;
pub mod prim;
pub mod project;
pub mod scene;
pub mod solid;
pub mod units;

pub use camera::OrbitCamera;
pub use history::History;
pub use mesh::{Mesh, Vertex};
pub use prim::{HollowShape, PrimKind, PrimShape};
pub use scene::{Object, Scene};
pub use solid::Solid;
pub use units::Unit;
