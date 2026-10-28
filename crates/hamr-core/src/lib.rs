//! Core logic for Hamr: units, camera, meshes and helpers that do not depend on any UI or GPU code.
//!
//! Hamr uses a right-handed, Z-up coordinate system and stores every length in millimetres.

pub mod camera;
pub mod grid;
pub mod mesh;
pub mod units;

pub use camera::OrbitCamera;
pub use mesh::{Mesh, Vertex};
pub use units::Unit;
