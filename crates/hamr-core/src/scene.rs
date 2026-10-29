//! The objects being built and their placement in the world.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use glam::{EulerRot, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::mesh::Mesh;
use crate::prim::{PrimKind, PrimShape, CREASE_DEGREES};
use crate::Solid;

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
    /// The root prim of the link set this prim belongs to. `None` for a root or an unlinked prim.
    #[serde(default)]
    pub link: Option<u64>,
    /// The prim is mirrored along its own X axis, as left by the mirror tool. Its transform then
    /// flips handedness, so triangles must be reversed to keep facing outwards.
    #[serde(default)]
    pub mirrored: bool,
    /// A fixed surface in the unit box that replaces the prim shape, for custom prims saved as
    /// a baked mesh. Shared, since history snapshots copy objects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baked: Option<Arc<Solid>>,
}

impl Object {
    /// The object's surface in its unit box: the baked mesh if it has one, else the prim.
    pub fn solid(&self) -> Solid {
        match &self.baked {
            Some(solid) => Solid::clone(solid),
            None => self.shape.solid(),
        }
    }

    /// [`Object::solid`] with vertices split for shading.
    pub fn mesh(&self) -> Mesh {
        match &self.baked {
            Some(solid) => solid.to_mesh(CREASE_DEGREES),
            None => self.shape.mesh(),
        }
    }

    /// Maps the prim's unit box to the world.
    pub fn transform(&self) -> Mat4 {
        let flip = if self.mirrored {
            Vec3::new(-1.0, 1.0, 1.0)
        } else {
            Vec3::ONE
        };
        Mat4::from_scale_rotation_translation(self.size * flip, self.rotation, self.position)
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
    /// A scene holding `objects`. New objects get ids above the highest one already used. Links
    /// to prims that are missing or are not roots are dropped.
    pub fn from_objects(objects: Vec<Object>) -> Self {
        let next_id = objects.iter().map(|o| o.id).max().unwrap_or(0);
        let mut scene = Self { objects, next_id };
        let roots: HashSet<u64> = scene
            .objects
            .iter()
            .filter(|o| o.link.is_none())
            .map(|o| o.id)
            .collect();
        for o in &mut scene.objects {
            if o.link
                .is_some_and(|root| !roots.contains(&root) || root == o.id)
            {
                o.link = None;
            }
        }
        scene
    }

    /// Adds copies of `objects` with fresh ids and returns the new ids in the same order. Links
    /// between the copies are kept; a copy linked to a prim outside `objects` stays linked to it
    /// if that prim is in the scene.
    pub fn add_objects(&mut self, objects: Vec<Object>) -> Vec<u64> {
        let mut new_ids = HashMap::new();
        for o in &objects {
            self.next_id += 1;
            new_ids.insert(o.id, self.next_id);
        }
        let mut out = Vec::with_capacity(objects.len());
        for mut o in objects {
            o.id = new_ids[&o.id];
            o.link = match o.link {
                Some(root) => match new_ids.get(&root) {
                    Some(&copy) => Some(copy),
                    None => self
                        .get(root)
                        .is_some_and(|r| r.link.is_none())
                        .then_some(root),
                },
                None => None,
            };
            out.push(o.id);
            self.objects.push(o);
        }
        out
    }

    /// Copies an object, moves the copy by `offset` and returns the copy's id.
    pub fn duplicate(&mut self, id: u64, offset: Vec3) -> Option<u64> {
        self.duplicate_all(&[id], offset).first().copied()
    }

    /// Copies `ids`, moves the copies by `offset` and returns their ids in the same order. Copies
    /// of a whole link set form a new link set.
    pub fn duplicate_all(&mut self, ids: &[u64], offset: Vec3) -> Vec<u64> {
        let copies = ids
            .iter()
            .filter_map(|&id| self.get(id).cloned())
            .map(|mut o| {
                o.position += offset;
                o
            })
            .collect();
        self.add_objects(copies)
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
            link: None,
            mirrored: false,
            baked: None,
        });
        id
    }

    pub fn get(&self, id: u64) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Object> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    /// Removes a prim. If it was the root of a link set, the next prim in the set becomes root.
    pub fn remove(&mut self, id: u64) {
        self.unlink(&[id]);
        self.objects.retain(|o| o.id != id);
    }

    /// The root of the link set `id` belongs to; `id` itself for a root or an unlinked prim.
    pub fn root_of(&self, id: u64) -> u64 {
        self.get(id).and_then(|o| o.link).unwrap_or(id)
    }

    /// Every prim in the link set of `id`, root first, in scene order.
    pub fn link_set(&self, id: u64) -> Vec<u64> {
        let root = self.root_of(id);
        if self.get(root).is_none() {
            return Vec::new();
        }
        let children = self
            .objects
            .iter()
            .filter(|o| o.link == Some(root))
            .map(|o| o.id);
        std::iter::once(root).chain(children).collect()
    }

    /// Links every prim in the link sets of `ids` into one link set with `root`'s root as its
    /// root, as SL does with the last prim selected.
    pub fn link(&mut self, ids: &[u64], root: u64) {
        let root = self.root_of(root);
        let members: HashSet<u64> = ids
            .iter()
            .chain([&root])
            .flat_map(|&id| self.link_set(id))
            .collect();
        for o in &mut self.objects {
            if members.contains(&o.id) {
                o.link = (o.id != root).then_some(root);
            }
        }
    }

    /// Takes `ids` out of their link sets. When a root is taken out but some of its children are
    /// not, the first remaining child becomes the root of the rest.
    pub fn unlink(&mut self, ids: &[u64]) {
        let leaving: HashSet<u64> = ids.iter().copied().collect();
        for &id in ids {
            let Some(o) = self.get(id) else { continue };
            if o.link.is_some() {
                continue;
            }
            let staying: Vec<u64> = self
                .link_set(id)
                .into_iter()
                .skip(1)
                .filter(|c| !leaving.contains(c))
                .collect();
            if let Some((&new_root, rest)) = staying.split_first() {
                for o in &mut self.objects {
                    if o.id == new_root {
                        o.link = None;
                    } else if rest.contains(&o.id) {
                        o.link = Some(new_root);
                    }
                }
            }
        }
        for o in &mut self.objects {
            if leaving.contains(&o.id) || o.link.is_some_and(|r| leaving.contains(&r)) {
                o.link = None;
            }
        }
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

    fn ids(scene: &mut Scene, n: usize) -> Vec<u64> {
        (0..n)
            .map(|_| scene.add_prim(PrimKind::Box, Vec3::ZERO))
            .collect()
    }

    #[test]
    fn linking_merges_link_sets_under_the_chosen_root() {
        // Linking two link sets must leave one flat set: every child points straight at the root.
        let mut scene = Scene::default();
        let p = ids(&mut scene, 4);
        scene.link(&[p[0], p[1]], p[1]);
        scene.link(&[p[2], p[3]], p[3]);
        scene.link(&[p[0], p[3]], p[0]);
        assert_eq!(scene.root_of(p[3]), p[1]);
        let mut set = scene.link_set(p[2]);
        assert_eq!(set[0], p[1]);
        set.sort();
        assert_eq!(set, p);
        assert!(scene
            .objects
            .iter()
            .all(|o| o.link.is_none_or(|r| r == p[1])));
    }

    #[test]
    fn unlinking_or_removing_the_root_promotes_a_child() {
        // The rest of a link set must stay linked when its root leaves.
        let mut scene = Scene::default();
        let p = ids(&mut scene, 3);
        scene.link(&p, p[0]);
        let mut removed = scene.clone();
        removed.remove(p[0]);
        assert_eq!(removed.link_set(p[2]), vec![p[1], p[2]]);
        scene.unlink(&[p[0]]);
        assert_eq!(scene.link_set(p[0]), vec![p[0]]);
        assert_eq!(scene.link_set(p[2]), vec![p[1], p[2]]);
        scene.unlink(&[p[1], p[2]]);
        assert!(scene.objects.iter().all(|o| o.link.is_none()));
    }

    #[test]
    fn copies_of_a_link_set_form_their_own_set() {
        // Duplicating a link set must not link the copies to the original.
        let mut scene = Scene::default();
        let p = ids(&mut scene, 2);
        scene.link(&p, p[0]);
        let copies = scene.duplicate_all(&p, Vec3::X);
        assert_eq!(scene.link_set(copies[1]), copies);
        assert_eq!(scene.link_set(p[1]), p);
        // A lone child copied on its own stays in its original set.
        let child = scene.duplicate(p[1], Vec3::ZERO).unwrap();
        assert_eq!(scene.root_of(child), p[0]);
    }

    #[test]
    fn loading_drops_links_to_missing_or_non_root_prims() {
        // Hand-edited or damaged files must not create chains or dangling links.
        let mut scene = Scene::default();
        let p = ids(&mut scene, 3);
        let mut objects = scene.objects.clone();
        objects[1].link = Some(p[2]);
        objects[2].link = Some(p[0]);
        objects[0].link = Some(99);
        let loaded = Scene::from_objects(objects);
        assert!(loaded.objects.iter().all(|o| o.link.is_none()));
    }

    #[test]
    fn mirrored_transform_flips_handedness() {
        let mut scene = Scene::default();
        let id = scene.add_prim(PrimKind::Box, Vec3::ZERO);
        let o = scene.get_mut(id).unwrap();
        assert!(o.transform().determinant() > 0.0);
        o.mirrored = true;
        assert!(o.transform().determinant() < 0.0);
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
