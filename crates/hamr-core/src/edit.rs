//! Edits that act on several prims at once: moving a selection as one, mirroring and arrays.

use glam::{Mat3, Quat, Vec3};

use crate::prim::{PrimKind, PrimShape};
use crate::{Object, Scene};

/// The box around `objects`, aligned with `rotation`, as an object the handles can work on.
/// `None` if `objects` is empty.
pub fn group_frame(objects: &[Object], rotation: Quat) -> Option<Object> {
    let to_frame = rotation.inverse();
    let corners = objects.iter().flat_map(|o| {
        let m = o.transform();
        crate::mesh::unit_box_corners().map(move |c| to_frame * m.transform_point3(c))
    });
    let (lo, hi) = corners.fold(None, |acc: Option<(Vec3, Vec3)>, p| {
        Some(acc.map_or((p, p), |(lo, hi)| (lo.min(p), hi.max(p))))
    })?;
    Some(Object {
        id: 0,
        name: String::new(),
        shape: PrimShape::new(PrimKind::Box),
        position: rotation * ((lo + hi) * 0.5),
        rotation,
        size: (hi - lo).max(Vec3::splat(crate::gizmo::MIN_SIZE_MM)),
        color: [1.0; 4],
        negative: false,
        link: None,
        mirrored: false,
    })
}

/// `objects` moved, turned and scaled together the way the frame `from` became `to`.
///
/// Prims can be turned any way inside the group, so a group can only be scaled evenly: the axis
/// whose size changed most sets the scale for all three. The face opposite a stretched face
/// stays put, as it does for a single prim.
pub fn transform_group(objects: &[Object], from: &Object, to: &Object) -> Vec<Object> {
    let ratio = to.size / from.size;
    let axis = (0..3)
        .max_by(|&a, &b| (ratio[a] - 1.0).abs().total_cmp(&(ratio[b] - 1.0).abs()))
        .unwrap_or(0);
    let scale = ratio[axis];
    // Where the frame's centre ends up, keeping the stretched axis exactly where the handle put
    // it and scaling the other two about the old centre.
    let mut local_shift = Vec3::ZERO;
    local_shift[axis] = (from.rotation.inverse() * (to.position - from.position))[axis];
    let turn = to.rotation * from.rotation.inverse();
    let moved = if scale == 1.0 {
        to.position
    } else {
        from.position + from.rotation * local_shift
    };
    objects
        .iter()
        .map(|o| {
            let mut o = o.clone();
            o.position = moved + turn * ((o.position - from.position) * scale);
            o.rotation = (turn * o.rotation).normalize();
            o.size *= scale;
            o
        })
        .collect()
}

/// Mirrors `ids` across the plane through `center` at right angles to world axis `axis`.
pub fn mirror(scene: &mut Scene, ids: &[u64], axis: usize, center: Vec3) {
    let mut flip = Vec3::ONE;
    flip[axis] = -1.0;
    let world = Mat3::from_diagonal(flip);
    let local = Mat3::from_diagonal(Vec3::new(-1.0, 1.0, 1.0));
    for &id in ids {
        let Some(o) = scene.get_mut(id) else { continue };
        o.position = center + (o.position - center) * flip;
        // The mirrored placement is world · rotation · local-flip, which is a proper rotation;
        // the flip itself is carried by the `mirrored` flag.
        o.rotation = Quat::from_mat3(&(world * Mat3::from_quat(o.rotation) * local)).normalize();
        o.mirrored = !o.mirrored;
    }
}

/// Adds `count - 1` copies of `ids`, each `offset` further on, and returns the new ids.
pub fn array_linear(scene: &mut Scene, ids: &[u64], count: u32, offset: Vec3) -> Vec<u64> {
    (1..count)
        .flat_map(|i| scene.duplicate_all(ids, offset * i as f32))
        .collect()
}

/// Adds `count - 1` copies of `ids` turned step by step around the line through `center` along
/// `axis`, and returns the new ids. A full circle spreads `count` items evenly; a smaller angle
/// puts the last copy at the end of it.
pub fn array_circular(
    scene: &mut Scene,
    ids: &[u64],
    count: u32,
    axis: Vec3,
    center: Vec3,
    total_degrees: f32,
) -> Vec<u64> {
    if count < 2 {
        return Vec::new();
    }
    let full = (total_degrees.abs() - 360.0).abs() < 1e-3;
    let steps = if full { count } else { count - 1 };
    let step = total_degrees.to_radians() / steps as f32;
    let axis = axis.normalize();
    let mut out = Vec::new();
    for i in 1..count {
        let turn = Quat::from_axis_angle(axis, step * i as f32);
        let copies = scene.duplicate_all(ids, Vec3::ZERO);
        for &id in &copies {
            if let Some(o) = scene.get_mut(id) {
                o.position = center + turn * (o.position - center);
                o.rotation = (turn * o.rotation).normalize();
            }
        }
        out.extend(copies);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mesh;

    fn world_points(o: &Object) -> Vec<Vec3> {
        let m = o.transform();
        o.shape
            .mesh()
            .vertices
            .iter()
            .map(|v| m.transform_point3(v.position.into()))
            .collect()
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).abs().max_element() < 1e-3
    }

    #[test]
    fn mirroring_reflects_every_vertex() {
        // Mirroring an asymmetric, turned prim must give exactly the reflected surface, which
        // fails if the rotation or the mirrored flag is wrong.
        let mut scene = Scene::default();
        let id = scene.add_prim(PrimKind::Cylinder, Vec3::new(30.0, 10.0, 0.0));
        {
            let o = scene.get_mut(id).unwrap();
            o.shape.twist = [0.0, 90.0];
            o.shape.top_shear = glam::Vec2::new(0.3, -0.2);
            o.shape.path_cut = [0.1, 0.7];
            o.set_rotation_degrees(Vec3::new(20.0, 35.0, -50.0));
            o.size = Vec3::new(20.0, 40.0, 60.0);
        }
        let before = world_points(scene.get(id).unwrap());
        for axis in 0..3 {
            let center = Vec3::new(5.0, -3.0, 7.0);
            let mut mirrored = scene.clone();
            mirror(&mut mirrored, &[id], axis, center);
            let after = world_points(mirrored.get(id).unwrap());
            let mut flip = Vec3::ONE;
            flip[axis] = -1.0;
            for (b, a) in before.iter().zip(&after) {
                assert!(close(center + (*b - center) * flip, *a), "axis {axis}");
            }
            // Mirroring twice gives back the original placement.
            mirror(&mut mirrored, &[id], axis, center);
            assert!(!mirrored.get(id).unwrap().mirrored);
            for (b, a) in before.iter().zip(world_points(mirrored.get(id).unwrap())) {
                assert!(close(*b, a));
            }
        }
    }

    #[test]
    fn mirrored_meshes_stay_watertight_once_flipped() {
        // A mirrored prim's world mesh, with its triangles reversed as the app and export do,
        // must still be closed and face outwards.
        let mut scene = Scene::default();
        let id = scene.add_prim(PrimKind::Torus, Vec3::ZERO);
        mirror(&mut scene, &[id], 1, Vec3::ZERO);
        let o = scene.get(id).unwrap();
        let mut mesh: Mesh = o.shape.mesh();
        let m = o.transform();
        for v in &mut mesh.vertices {
            v.position = m.transform_point3(v.position.into()).into();
        }
        for t in mesh.indices.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
        mesh.check_watertight().unwrap();
        assert!(mesh.signed_volume() > 0.0);
    }

    #[test]
    fn moving_a_group_keeps_prims_in_place_relative_to_each_other() {
        // Turning a group must turn each prim about the group centre, not about its own.
        let mut scene = Scene::default();
        let a = scene.add_prim(PrimKind::Box, Vec3::new(-40.0, 0.0, 0.0));
        let b = scene.add_prim(PrimKind::Box, Vec3::new(40.0, 0.0, 0.0));
        let objects = vec![scene.get(a).unwrap().clone(), scene.get(b).unwrap().clone()];
        let from = group_frame(&objects, Quat::IDENTITY).unwrap();
        assert!(close(from.size, Vec3::new(130.0, 50.0, 50.0)));
        let mut to = from.clone();
        to.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        to.position += Vec3::new(0.0, 0.0, 10.0);
        let moved = transform_group(&objects, &from, &to);
        assert!(close(moved[0].position, Vec3::new(0.0, -40.0, 35.0)));
        assert!(close(moved[1].position, Vec3::new(0.0, 40.0, 35.0)));
        assert!(close(moved[0].size, objects[0].size));
    }

    #[test]
    fn stretching_a_group_scales_evenly_from_the_opposite_face() {
        // The face opposite the dragged one stays put and all prims scale by the same amount.
        let mut scene = Scene::default();
        let a = scene.add_prim(PrimKind::Box, Vec3::new(-40.0, 0.0, 0.0));
        let b = scene.add_prim(PrimKind::Box, Vec3::new(40.0, 0.0, 0.0));
        let objects = vec![scene.get(a).unwrap().clone(), scene.get(b).unwrap().clone()];
        let from = group_frame(&objects, Quat::IDENTITY).unwrap();
        let mut to = from.clone();
        // Stretch the +X face out so the group doubles in length.
        to.size.x *= 2.0;
        to.position.x += from.size.x * 0.5;
        let scaled = transform_group(&objects, &from, &to);
        let after = group_frame(&scaled, Quat::IDENTITY).unwrap();
        assert!(close(after.size, from.size * 2.0), "{}", after.size);
        let left = |f: &Object| f.position.x - f.size.x * 0.5;
        assert!((left(&after) - left(&from)).abs() < 1e-3);
        assert!(close(scaled[0].size, Vec3::splat(100.0)));
    }

    #[test]
    fn arrays_place_copies_step_by_step() {
        let mut scene = Scene::default();
        let a = scene.add_prim(PrimKind::Box, Vec3::new(100.0, 0.0, 0.0));
        let linear = array_linear(&mut scene, &[a], 4, Vec3::new(0.0, 60.0, 0.0));
        assert_eq!(linear.len(), 3);
        assert!(close(
            scene.get(linear[2]).unwrap().position,
            Vec3::new(100.0, 180.0, 25.0)
        ));

        // Four copies around a full circle land a quarter turn apart, each turned to match.
        let ring = array_circular(&mut scene, &[a], 4, Vec3::Z, Vec3::ZERO, 360.0);
        assert_eq!(ring.len(), 3);
        let first = scene.get(ring[0]).unwrap();
        assert!(close(first.position, Vec3::new(0.0, 100.0, 25.0)));
        assert!((first.rotation_degrees().z - 90.0).abs() < 1e-3);
        // Over 180° the last copy sits at the end of the arc.
        let half = array_circular(&mut scene, &[a], 3, Vec3::Z, Vec3::ZERO, 180.0);
        assert!(close(
            scene.get(half[1]).unwrap().position,
            Vec3::new(-100.0, 0.0, 25.0)
        ));
    }
}
