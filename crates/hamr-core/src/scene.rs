//! The objects being built and their placement in the world.

use glam::{EulerRot, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::prim::{PrimKind, PrimShape};

/// Default edge length of a new prim, in millimetres.
pub const DEFAULT_SIZE: f32 = 50.0;

/// A single prim placed in the world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    /// Stable identifier, unique within a scene.
    pub id: u64,
    pub name: String,
    pub shape: PrimShape,
    /// Centre of the object's bounding box, in millimetres.
    pub position: Vec3,
    pub rotation: Quat,
    /// Size of the bounding box along the object's own axes, in millimetres.
    pub size: Vec3,
    /// Linear RGBA colour.
    pub color: [f32; 4],
    /// A negative prim cuts its volume out of the positive prims it overlaps when the model is
    /// merged for export, instead of adding to it.
    pub negative: bool,
}

impl Object {
    /// Maps the prim's unit box to the world.
    pub fn transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.size, self.rotation, self.position)
    }

    /// Rotation as X, Y, Z Euler angles in degrees, the way the SL Object tab shows it.
    pub fn rotation_degrees(&self) -> Vec3 {
        let (x, y, z) = self.rotation.to_euler(EulerRot::XYZ);
        // Adding zero turns −0.0 into 0.0, so the panel never shows "-0.0°".
        Vec3::new(x, y, z) * (180.0 / std::f32::consts::PI) + Vec3::ZERO
    }

    pub fn set_rotation_degrees(&mut self, degrees: Vec3) {
        let r = degrees * (std::f32::consts::PI / 180.0);
        self.rotation = Quat::from_euler(EulerRot::XYZ, r.x, r.y, r.z);
    }
}

/// Default size of a new prim of `kind`. Torus, tube and ring are flat so their default profile
/// comes out round, as in SL.
pub fn default_size(kind: PrimKind) -> Vec3 {
    if kind.is_ring_like() {
        Vec3::new(DEFAULT_SIZE, DEFAULT_SIZE, DEFAULT_SIZE * 0.25)
    } else {
        Vec3::splat(DEFAULT_SIZE)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub objects: Vec<Object>,
    next_id: u64,
}

impl Scene {
    /// A scene holding `objects`. New objects get ids above the highest one already used.
    pub fn from_objects(objects: Vec<Object>) -> Self {
        let next_id = objects.iter().map(|o| o.id).max().unwrap_or(0);
        Self { objects, next_id }
    }

    /// Copies an object, moves the copy by `offset` and returns the copy's id.
    pub fn duplicate(&mut self, id: u64, offset: Vec3) -> Option<u64> {
        let mut copy = self.get(id)?.clone();
        self.next_id += 1;
        copy.id = self.next_id;
        copy.position += offset;
        let new_id = copy.id;
        self.objects.push(copy);
        Some(new_id)
    }

    /// Adds a default prim of `kind` resting on the ground at `at` (Z is ignored) and returns its
    /// id.
    pub fn add_prim(&mut self, kind: PrimKind, at: Vec3) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let size = default_size(kind);
        self.objects.push(Object {
            id,
            name: format!("{} {id}", kind.name()),
            shape: PrimShape::new(kind),
            position: Vec3::new(at.x, at.y, size.z * 0.5),
            rotation: Quat::IDENTITY,
            size,
            color: [0.85, 0.62, 0.25, 1.0],
            negative: false,
        });
        id
    }

    pub fn get(&self, id: u64) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Object> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    pub fn remove(&mut self, id: u64) {
        self.objects.retain(|o| o.id != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_prims_rest_on_the_ground_with_unique_ids() {
        let mut scene = Scene::default();
        let a = scene.add_prim(PrimKind::Box, Vec3::new(10.0, 20.0, 99.0));
        let b = scene.add_prim(PrimKind::Torus, Vec3::ZERO);
        assert_ne!(a, b);
        let box_ = scene.get(a).unwrap();
        assert_eq!(box_.position, Vec3::new(10.0, 20.0, DEFAULT_SIZE * 0.5));
        // The bottom of the transformed unit box sits at Z = 0.
        let bottom = box_.transform().transform_point3(Vec3::new(0.0, 0.0, -0.5));
        assert!(bottom.z.abs() < 1e-4);
    }

    #[test]
    fn duplicates_get_fresh_ids_after_loading() {
        // A loaded scene must not hand out an id that is already taken.
        let mut scene = Scene::default();
        let a = scene.add_prim(PrimKind::Box, Vec3::ZERO);
        let b = scene.add_prim(PrimKind::Box, Vec3::ZERO);
        let mut loaded = Scene::from_objects(scene.objects.clone());
        let c = loaded.duplicate(a, Vec3::X).unwrap();
        assert!(c != a && c != b);
        let original = loaded.get(a).unwrap().position;
        assert_eq!(loaded.get(c).unwrap().position, original + Vec3::X);
    }

    #[test]
    fn euler_degrees_round_trip() {
        let mut scene = Scene::default();
        let id = scene.add_prim(PrimKind::Box, Vec3::ZERO);
        let o = scene.get_mut(id).unwrap();
        o.set_rotation_degrees(Vec3::new(30.0, -45.0, 120.0));
        let back = o.rotation_degrees();
        assert!(
            (back - Vec3::new(30.0, -45.0, 120.0)).abs().max_element() < 1e-3,
            "{back}"
        );
    }
}
