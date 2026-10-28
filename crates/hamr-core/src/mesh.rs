//! Triangle meshes and the watertightness check that every prim and export must pass.

use std::collections::HashMap;

use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// An indexed triangle mesh. Triangles wind counter-clockwise when seen from outside.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// Why a mesh is not watertight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatertightError {
    /// An index points past the end of the vertex list.
    IndexOutOfRange,
    /// An edge belongs to only one triangle, so the surface has a hole.
    OpenEdge,
    /// An edge belongs to more than two triangles, or two neighbours wind the same way.
    NonManifoldEdge,
}

impl Mesh {
    /// An axis-aligned box centred on the origin with flat-shaded faces.
    pub fn cuboid(size: Vec3) -> Self {
        let h = size * 0.5;
        // Each face: normal, then the two axes spanning it so that u x v = normal.
        let faces = [
            (Vec3::X, Vec3::Y, Vec3::Z),
            (Vec3::NEG_X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::Z, Vec3::X),
            (Vec3::NEG_Y, Vec3::X, Vec3::Z),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (Vec3::NEG_Z, Vec3::Y, Vec3::X),
        ];
        let mut mesh = Mesh::default();
        for (normal, u, v) in faces {
            let base = mesh.vertices.len() as u32;
            for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let p = (normal + u * su + v * sv) * h;
                mesh.vertices.push(Vertex {
                    position: p.into(),
                    normal: normal.into(),
                });
            }
            mesh.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        mesh
    }

    /// Returns the minimum and maximum corners of the mesh, or `None` if it has no vertices.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut it = self.vertices.iter().map(|v| Vec3::from(v.position));
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }

    /// Distance along the ray to the nearest triangle it hits, if any. `dir` must be unit length.
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3) -> Option<f32> {
        let pos = |i: u32| Vec3::from(self.vertices[i as usize].position);
        self.indices
            .chunks_exact(3)
            .filter_map(|t| ray_triangle(origin, dir, pos(t[0]), pos(t[1]), pos(t[2])))
            .min_by(f32::total_cmp)
    }

    /// Checks that the surface is closed and consistently oriented, which is what slicers need.
    ///
    /// Vertices at the same position count as one, so flat-shaded meshes with split normals pass.
    pub fn check_watertight(&self) -> Result<(), WatertightError> {
        // Merge vertices that share a position.
        let mut ids: HashMap<[u32; 3], u32> = HashMap::new();
        let mut welded = Vec::with_capacity(self.vertices.len());
        for v in &self.vertices {
            let key = v.position.map(|c| (c + 0.0).to_bits());
            let next = ids.len() as u32;
            welded.push(*ids.entry(key).or_insert(next));
        }

        // Every directed edge must appear exactly once and be matched by its reverse.
        let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
        for tri in self.indices.chunks_exact(3) {
            let mut w = [0u32; 3];
            for (slot, &i) in w.iter_mut().zip(tri) {
                *slot = *welded
                    .get(i as usize)
                    .ok_or(WatertightError::IndexOutOfRange)?;
            }
            if w[0] == w[1] || w[1] == w[2] || w[0] == w[2] {
                continue; // Degenerate triangles add no area and no edges.
            }
            for k in 0..3 {
                *edges.entry((w[k], w[(k + 1) % 3])).or_default() += 1;
            }
        }
        for (&(a, b), &count) in &edges {
            if count != 1 {
                return Err(WatertightError::NonManifoldEdge);
            }
            match edges.get(&(b, a)) {
                Some(1) => {}
                Some(_) => return Err(WatertightError::NonManifoldEdge),
                None => return Err(WatertightError::OpenEdge),
            }
        }
        Ok(())
    }
}

/// Möller–Trumbore ray/triangle intersection, hitting both sides of the triangle.
fn ray_triangle(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 0.0).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuboid_is_watertight() {
        assert_eq!(
            Mesh::cuboid(Vec3::new(10.0, 20.0, 30.0)).check_watertight(),
            Ok(())
        );
    }

    #[test]
    fn cuboid_faces_point_outwards() {
        let mesh = Mesh::cuboid(Vec3::ONE);
        for tri in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(mesh.vertices[tri[k] as usize].position));
            let n = (b - a).cross(c - a);
            let centroid = (a + b + c) / 3.0;
            assert!(n.dot(centroid) > 0.0);
        }
    }

    #[test]
    fn ray_hits_the_near_face() {
        let mesh = Mesh::cuboid(Vec3::splat(2.0));
        let t = mesh
            .ray_hit(Vec3::new(0.0, 0.0, 10.0), Vec3::NEG_Z)
            .unwrap();
        assert!((t - 9.0).abs() < 1e-5);
        assert_eq!(mesh.ray_hit(Vec3::new(5.0, 0.0, 10.0), Vec3::NEG_Z), None);
    }

    #[test]
    fn bounds_of_cuboid() {
        let (lo, hi) = Mesh::cuboid(Vec3::new(2.0, 4.0, 6.0)).bounds().unwrap();
        assert_eq!(
            (lo, hi),
            (Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0))
        );
    }

    #[test]
    fn missing_face_is_an_open_edge() {
        let mut mesh = Mesh::cuboid(Vec3::ONE);
        mesh.indices.truncate(mesh.indices.len() - 6);
        assert_eq!(mesh.check_watertight(), Err(WatertightError::OpenEdge));
    }

    #[test]
    fn flipped_face_is_detected() {
        let mut mesh = Mesh::cuboid(Vec3::ONE);
        mesh.indices[..6].reverse();
        assert!(mesh.check_watertight().is_err());
    }
}
