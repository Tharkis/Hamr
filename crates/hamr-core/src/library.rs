//! Custom prims: a model saved so it can be added again later, like one of the built-in prims.

use std::sync::Arc;

use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::merge::{self, MergeError};
use crate::project::ProjectError;
use crate::{Object, PrimKind, PrimShape, Scene};

/// File extension of custom prim files, without the dot.
pub const EXTENSION: &str = "hamrprim";
const FORMAT: &str = "hamr-prim";
/// Bump when the format changes in a way older versions can't read, and add a migration.
const VERSION: u32 = 1;

/// A saved group of prims. They are stored as one link set, resting on the ground and centred on
/// the origin, so placing it works like adding a prim.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomPrim {
    pub name: String,
    pub objects: Vec<Object>,
}

#[derive(Serialize, Deserialize)]
struct CustomPrimFile {
    format: String,
    version: u32,
    name: String,
    objects: Vec<Object>,
}

impl CustomPrim {
    /// The prims `ids` (and nothing else) as a custom prim. `None` if none of them exist.
    pub fn from_selection(scene: &Scene, ids: &[u64], name: &str) -> Option<Self> {
        let mut objects: Vec<Object> = ids
            .iter()
            .filter_map(|&id| scene.get(id))
            .cloned()
            .collect();
        let frame = crate::edit::group_frame(&objects, Quat::IDENTITY)?;
        // Keep the root of the first link set as root, and link everything else to it.
        let root = objects
            .iter()
            .find(|o| o.link.is_none())
            .unwrap_or(&objects[0])
            .id;
        let shift = Vec3::new(
            frame.position.x,
            frame.position.y,
            frame.position.z - frame.size.z * 0.5,
        );
        for o in &mut objects {
            o.link = (o.id != root).then_some(root);
            o.position -= shift;
        }
        Some(Self {
            name: name.to_owned(),
            objects,
        })
    }

    /// The prims `ids` merged into one solid, with negative prims cut out, as a custom prim that
    /// is a single baked mesh. It takes the colour of the first positive prim.
    pub fn baked_from_selection(
        scene: &Scene,
        ids: &[u64],
        name: &str,
    ) -> Result<Self, MergeError> {
        let objects: Vec<&Object> = ids.iter().filter_map(|&id| scene.get(id)).collect();
        let model = merge::merge(objects.iter().copied())?;
        let (lo, hi) = model
            .solid
            .positions
            .iter()
            .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), &p| {
                (lo.min(p), hi.max(p))
            });
        let size = (hi - lo).max(Vec3::splat(crate::gizmo::MIN_SIZE_MM));
        let center = (lo + hi) * 0.5;
        let mut unit = model.solid;
        for p in &mut unit.positions {
            *p = ((*p - center) / size).clamp(Vec3::splat(-0.5), Vec3::splat(0.5));
        }
        let color = objects
            .iter()
            .find(|o| !o.negative)
            .map_or([0.8, 0.8, 0.8, 1.0], |o| o.color);
        Ok(Self {
            name: name.to_owned(),
            objects: vec![Object {
                id: 1,
                name: name.to_owned(),
                shape: PrimShape::new(PrimKind::Box),
                position: Vec3::new(0.0, 0.0, size.z * 0.5),
                rotation: Quat::IDENTITY,
                size,
                color,
                negative: false,
                link: None,
                mirrored: false,
                baked: Some(Arc::new(unit)),
            }],
        })
    }

    /// Adds a copy to `scene` resting on the ground at `at` (Z is ignored) and returns the new
    /// ids, root first.
    pub fn place(&self, scene: &mut Scene, at: Vec3) -> Vec<u64> {
        let shift = Vec3::new(at.x, at.y, 0.0);
        let objects = self
            .objects
            .iter()
            .cloned()
            .map(|mut o| {
                o.position += shift;
                o
            })
            .collect();
        let ids = scene.add_objects(objects);
        ids.first().map_or_else(Vec::new, |&id| scene.link_set(id))
    }

    pub fn to_json(&self) -> String {
        let file = CustomPrimFile {
            format: FORMAT.into(),
            version: VERSION,
            name: self.name.clone(),
            objects: self.objects.clone(),
        };
        serde_json::to_string_pretty(&file).expect("custom prims always serialize")
    }

    pub fn from_json(text: &str) -> Result<Self, ProjectError> {
        #[derive(Deserialize)]
        struct Header {
            format: String,
            version: u32,
        }
        let header: Header = serde_json::from_str(text).map_err(ProjectError::Invalid)?;
        if header.format != FORMAT {
            return Err(ProjectError::NotAProject);
        }
        if header.version > VERSION {
            return Err(ProjectError::TooNew(header.version));
        }
        let file: CustomPrimFile = serde_json::from_str(text).map_err(ProjectError::Invalid)?;
        let objects = file
            .objects
            .into_iter()
            .map(|mut o| {
                o.shape = o.shape.clamped();
                o
            })
            .collect();
        let objects = Scene::from_objects(objects).objects;
        crate::project::check_baked(&objects)?;
        if objects.is_empty() {
            return Err(ProjectError::NotAProject);
        }
        Ok(Self {
            name: file.name,
            objects,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrimKind;

    #[test]
    fn saved_prims_place_as_one_link_set_on_the_ground() {
        // A custom prim made of two separate prims must come back as a single link set resting
        // on the ground where it is placed, with fresh ids.
        let mut scene = Scene::default();
        let a = scene.add_prim(PrimKind::Box, Vec3::new(100.0, 0.0, 0.0));
        let b = scene.add_prim(PrimKind::Sphere, Vec3::new(160.0, 0.0, 0.0));
        scene.get_mut(b).unwrap().position.z += 40.0;
        let custom = CustomPrim::from_selection(&scene, &[a, b], "Lamp").unwrap();

        let loaded = CustomPrim::from_json(&custom.to_json()).unwrap();
        assert_eq!(loaded, custom);

        let placed = loaded.place(&mut scene, Vec3::new(-20.0, 30.0, 500.0));
        assert_eq!(placed.len(), 2);
        assert!(placed.iter().all(|id| *id != a && *id != b));
        assert_eq!(scene.link_set(placed[1]), placed);
        let parts: Vec<Object> = placed
            .iter()
            .map(|&id| scene.get(id).unwrap().clone())
            .collect();
        let frame = crate::edit::group_frame(&parts, Quat::IDENTITY).unwrap();
        assert!((frame.position.x + 20.0).abs() < 1e-3);
        assert!((frame.position.y - 30.0).abs() < 1e-3);
        assert!((frame.position.z - frame.size.z * 0.5).abs() < 1e-3);
        // The prims keep their places relative to each other.
        let gap = scene.get(placed[1]).unwrap().position - scene.get(placed[0]).unwrap().position;
        assert!((gap - Vec3::new(60.0, 0.0, 40.0)).length() < 1e-3);
    }

    #[test]
    fn baked_prims_keep_the_merged_shape() {
        // A baked custom prim must reproduce the merged model (holes included) wherever it is
        // placed, survive saving, and still merge and export as a watertight solid.
        let mut scene = Scene::default();
        let a = scene.add_prim(PrimKind::Box, Vec3::ZERO);
        let b = scene.add_prim(PrimKind::Sphere, Vec3::new(30.0, 0.0, 0.0));
        let hole = scene.add_prim(PrimKind::Cylinder, Vec3::ZERO);
        let h = scene.get_mut(hole).unwrap();
        h.negative = true;
        h.size = Vec3::new(10.0, 10.0, 80.0);
        let ids = [a, b, hole];
        let original = merge::merge(ids.iter().map(|&id| scene.get(id).unwrap())).unwrap();

        let custom = CustomPrim::baked_from_selection(&scene, &ids, "Knob").unwrap();
        let custom = CustomPrim::from_json(&custom.to_json()).unwrap();
        assert_eq!(custom.objects.len(), 1);

        let mut other = Scene::default();
        let placed = custom.place(&mut other, Vec3::new(100.0, -50.0, 0.0));
        let baked = merge::merge(&other.objects).unwrap();
        baked.solid.check_closed().unwrap();
        let (v0, v1) = (original.solid.signed_volume(), baked.solid.signed_volume());
        assert!((v0 - v1).abs() < v0 * 1e-3, "{v0} vs {v1}");
        // It rests on the ground at the spot it was placed.
        let o = other.get(placed[0]).unwrap();
        assert!((o.position.z - o.size.z * 0.5).abs() < 1e-3);
        assert!((o.position.x - 100.0).abs() < 1e-3);
    }

    #[test]
    fn damaged_baked_meshes_are_rejected() {
        let mut scene = Scene::default();
        let a = scene.add_prim(PrimKind::Box, Vec3::ZERO);
        let mut custom = CustomPrim::baked_from_selection(&scene, &[a], "Cube").unwrap();
        let solid = Arc::make_mut(custom.objects[0].baked.as_mut().unwrap());
        solid.triangles.pop();
        assert!(matches!(
            CustomPrim::from_json(&custom.to_json()),
            Err(ProjectError::DamagedMesh(_))
        ));
    }

    #[test]
    fn rejects_other_files() {
        assert!(matches!(
            CustomPrim::from_json(r#"{"format":"hamr","version":1,"objects":[]}"#),
            Err(ProjectError::NotAProject)
        ));
        assert!(matches!(
            CustomPrim::from_json(r#"{"format":"hamr-prim","version":99}"#),
            Err(ProjectError::TooNew(99))
        ));
    }
}
