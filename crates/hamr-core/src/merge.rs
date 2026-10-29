//! Merging prims into one watertight solid for export: overlapping prims are joined and negative
//! prims are cut out, using the Manifold boolean library.

use std::collections::HashMap;

use glam::Vec3;
use manifold3d::{Manifold, MeshGL64};

use crate::{Object, Solid};

/// Properties carried through the booleans for each vertex: position, then linear RGB colour.
const PROPS: usize = 6;

/// The merged model, in millimetres.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub solid: Solid,
    /// Linear RGB colour of each triangle corner, three per triangle, so colours stay sharp where
    /// prims of different colours meet.
    pub corner_colors: Vec<[f32; 3]>,
}

#[derive(Debug)]
pub enum MergeError {
    /// A prim could not be turned into a valid solid. This is a bug in the prim generator.
    InvalidPrim { name: String, reason: String },
    /// There are no positive prims, or the negative prims cut all of them away.
    Empty,
}

impl std::fmt::Display for MergeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MergeError::InvalidPrim { name, reason } => {
                write!(f, "{name} could not be merged ({reason})")
            }
            MergeError::Empty => write!(f, "there is nothing left to export"),
        }
    }
}

impl std::error::Error for MergeError {}

/// The object's surface in the world, wound outwards.
pub fn world_solid(object: &Object) -> Solid {
    let mut solid = object.solid();
    let m = object.transform();
    for p in &mut solid.positions {
        *p = m.transform_point3(*p);
    }
    if object.mirrored {
        solid.flip();
    }
    solid
}

fn to_manifold(object: &Object) -> Result<Manifold, MergeError> {
    let solid = world_solid(object);
    let [r, g, b, _] = object.color;
    let props: Vec<f64> = solid
        .positions
        .iter()
        .flat_map(|p| [p.x, p.y, p.z, r, g, b].map(f64::from))
        .collect();
    let indices: Vec<u64> = solid
        .triangles
        .iter()
        .flatten()
        .map(|&i| i as u64)
        .collect();
    Manifold::from_mesh_f64(&props, PROPS, &indices).map_err(|e| MergeError::InvalidPrim {
        name: object.name.clone(),
        reason: e.to_string(),
    })
}

/// Joins the positive prims among `objects` and cuts the negative ones out of them.
pub fn merge<'a>(objects: impl IntoIterator<Item = &'a Object>) -> Result<Model, MergeError> {
    let mut positive = Vec::new();
    let mut negative = Vec::new();
    for object in objects {
        let m = to_manifold(object)?;
        if object.negative {
            negative.push(m);
        } else {
            positive.push(m);
        }
    }
    let mut result = Manifold::batch_union(&positive);
    if !negative.is_empty() {
        result = result.difference(&Manifold::batch_union(&negative));
    }
    if result.is_empty() {
        return Err(MergeError::Empty);
    }
    Ok(from_meshgl(&result.to_meshgl64()))
}

/// Converts Manifold's output, where vertices are split wherever their colours differ, into a
/// solid with one shared vertex per corner position plus per-corner colours.
fn from_meshgl(mesh: &MeshGL64) -> Model {
    let props = mesh.vert_properties();
    let n = mesh.num_prop();
    let tris = mesh.tri_verts();
    // Manifold lists which split vertices are really the same vertex.
    let merge: HashMap<u64, u64> = mesh
        .merge_from_vert()
        .into_iter()
        .zip(mesh.merge_to_vert())
        .collect();
    let mut index = HashMap::new();
    let mut model = Model::default();
    for corner in tris.chunks_exact(3) {
        let mut tri = [0u32; 3];
        for (k, &v) in corner.iter().enumerate() {
            let shared = merge.get(&v).copied().unwrap_or(v);
            tri[k] = *index.entry(shared).or_insert_with(|| {
                let p = &props[shared as usize * n..];
                model
                    .solid
                    .positions
                    .push(Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32));
                (model.solid.positions.len() - 1) as u32
            });
            let p = &props[v as usize * n..];
            model.corner_colors.push(if n >= PROPS {
                [p[3] as f32, p[4] as f32, p[5] as f32]
            } else {
                [0.8; 3]
            });
        }
        model.solid.triangles.push(tri);
    }
    model
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PrimKind, Scene};

    fn volume(model: &Model) -> f32 {
        model.solid.signed_volume()
    }

    #[test]
    fn overlapping_prims_merge_into_one_closed_solid() {
        // Two boxes overlapping by half must become one closed, outward-facing surface whose
        // volume counts the overlap once.
        let mut scene = Scene::default();
        scene.add_prim(PrimKind::Box, Vec3::ZERO);
        scene.add_prim(PrimKind::Box, Vec3::new(25.0, 0.0, 0.0));
        let model = merge(&scene.objects).unwrap();
        model.solid.check_closed().unwrap();
        assert!(
            (volume(&model) - 75.0 * 50.0 * 50.0).abs() < 1.0,
            "{}",
            volume(&model)
        );
        assert_eq!(model.corner_colors.len(), model.solid.triangles.len() * 3);
    }

    #[test]
    fn negative_prims_cut_holes() {
        // A negative cylinder through a box must remove its volume and leave a closed surface
        // with a hole through it.
        let mut scene = Scene::default();
        scene.add_prim(PrimKind::Box, Vec3::ZERO);
        let hole = scene.add_prim(PrimKind::Cylinder, Vec3::ZERO);
        let h = scene.get_mut(hole).unwrap();
        h.negative = true;
        h.size = Vec3::new(20.0, 20.0, 80.0);
        let model = merge(&scene.objects).unwrap();
        model.solid.check_closed().unwrap();
        let full = 50.0f32.powi(3);
        assert!(volume(&model) < full - 0.9 * std::f32::consts::PI * 100.0 * 50.0);
        assert!(volume(&model) > 0.0);
    }

    #[test]
    fn colours_stay_with_their_prims() {
        let mut scene = Scene::default();
        let a = scene.add_prim(PrimKind::Box, Vec3::ZERO);
        let b = scene.add_prim(PrimKind::Box, Vec3::new(40.0, 0.0, 0.0));
        scene.get_mut(a).unwrap().color = [1.0, 0.0, 0.0, 1.0];
        scene.get_mut(b).unwrap().color = [0.0, 0.0, 1.0, 1.0];
        let model = merge(&scene.objects).unwrap();
        let tris = &model.solid.triangles;
        // Every triangle is one colour, and its colour matches the side of the model it is on.
        for (t, tri) in tris.iter().enumerate() {
            let c = &model.corner_colors[t * 3..t * 3 + 3];
            assert!(c[0] == c[1] && c[1] == c[2]);
            let x = tri
                .iter()
                .map(|&i| model.solid.positions[i as usize].x)
                .sum::<f32>()
                / 3.0;
            if x < 10.0 {
                assert_eq!(c[0], [1.0, 0.0, 0.0]);
            } else if x > 20.0 {
                assert_eq!(c[0], [0.0, 0.0, 1.0]);
            }
        }
    }

    #[test]
    fn mirrored_and_every_kind_of_prim_merge() {
        // Every prim type, mirrored or not, must be accepted by the boolean engine.
        let mut scene = Scene::default();
        for (i, kind) in PrimKind::ALL.into_iter().enumerate() {
            let id = scene.add_prim(kind, Vec3::new(i as f32 * 30.0, 0.0, 0.0));
            let o = scene.get_mut(id).unwrap();
            o.shape.hollow = 0.3;
            o.shape.twist = [0.0, 90.0];
            o.mirrored = i % 2 == 0;
        }
        let model = merge(&scene.objects).unwrap();
        model.solid.check_closed().unwrap();
        assert!(volume(&model) > 0.0);
    }

    #[test]
    fn nothing_left_is_an_error() {
        let mut scene = Scene::default();
        let a = scene.add_prim(PrimKind::Box, Vec3::ZERO);
        assert!(matches!(merge(&[]), Err(MergeError::Empty)));
        scene.get_mut(a).unwrap().negative = true;
        assert!(matches!(merge(&scene.objects), Err(MergeError::Empty)));
    }
}
