//! Closed triangle surfaces with shared vertices: the form prims are built in and that export
//! and booleans will work on.

use std::collections::HashMap;

use glam::Vec3;

use crate::mesh::{Mesh, Vertex, WatertightError};

/// An indexed surface. Unlike [`Mesh`], which splits vertices for shading, a solid shares each
/// vertex between all the triangles that touch it, so its connectivity can be checked and kept.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Solid {
    pub positions: Vec<Vec3>,
    /// Counter-clockwise when seen from outside.
    pub triangles: Vec<[u32; 3]>,
}

impl Solid {
    /// Checks that every edge joins exactly two triangles that wind in opposite directions, and
    /// that no triangle uses the same vertex twice.
    ///
    /// This works on the shared indices rather than on positions, so two separate parts of a
    /// surface that merely touch (as a twisted sphere does on its axis) are not mistaken for one.
    pub fn check_closed(&self) -> Result<(), WatertightError> {
        let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
        for &[a, b, c] in &self.triangles {
            if [a, b, c]
                .iter()
                .any(|&i| i as usize >= self.positions.len())
            {
                return Err(WatertightError::IndexOutOfRange);
            }
            if a == b || b == c || a == c {
                return Err(WatertightError::Degenerate);
            }
            for edge in [(a, b), (b, c), (c, a)] {
                *edges.entry(edge).or_default() += 1;
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

    /// Enclosed volume; positive when the triangles face outwards.
    pub fn signed_volume(&self) -> f32 {
        self.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| self.positions[i as usize]);
                a.dot(b.cross(c))
            })
            .sum::<f32>()
            / 6.0
    }

    /// Turns the surface inside out.
    pub fn flip(&mut self) {
        for t in &mut self.triangles {
            t.swap(1, 2);
        }
    }

    /// Splits vertices for shading. Each corner gets the average normal of the faces around its
    /// vertex whose angle to its own face is below `crease_degrees`, so curved surfaces look
    /// smooth and corners stay sharp.
    pub fn to_mesh(&self, crease_degrees: f32) -> Mesh {
        // Area-weighted face normals, and the faces touching each vertex.
        let face_normals: Vec<Vec3> = self
            .triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| self.positions[i as usize]);
                (b - a).cross(c - a)
            })
            .collect();
        let mut around: Vec<Vec<u32>> = vec![Vec::new(); self.positions.len()];
        for (f, tri) in self.triangles.iter().enumerate() {
            for &v in tri {
                around[v as usize].push(f as u32);
            }
        }

        let cos_crease = crease_degrees.to_radians().cos();
        let mut mesh = Mesh::default();
        for (f, tri) in self.triangles.iter().enumerate() {
            let own = face_normals[f].normalize_or_zero();
            for &v in tri {
                let normal: Vec3 = around[v as usize]
                    .iter()
                    .map(|&g| face_normals[g as usize])
                    .filter(|n| n.normalize_or_zero().dot(own) >= cos_crease)
                    .sum();
                let normal = normal.try_normalize().unwrap_or(own);
                mesh.vertices.push(Vertex {
                    position: self.positions[v as usize].into(),
                    normal: normal.into(),
                });
                mesh.indices.push(mesh.indices.len() as u32);
            }
        }
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tetrahedron() -> Solid {
        Solid {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
            triangles: vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [0, 3, 2]],
        }
    }

    #[test]
    fn tetrahedron_is_closed_with_positive_volume() {
        let t = tetrahedron();
        assert_eq!(t.check_closed(), Ok(()));
        assert!((t.signed_volume() - 1.0 / 6.0).abs() < 1e-6);
    }

    #[test]
    fn touching_copies_are_still_two_closed_parts() {
        // Two tetrahedra sharing a corner position but not a vertex index are fine; a position
        // based check would see a non-manifold vertex there.
        let mut t = tetrahedron();
        let other = tetrahedron();
        let base = t.positions.len() as u32;
        t.positions.extend(other.positions.iter().map(|p| -*p));
        t.triangles.extend(
            other
                .triangles
                .iter()
                .map(|tri| [tri[0] + base, tri[2] + base, tri[1] + base]),
        );
        assert_eq!(t.check_closed(), Ok(()));
    }

    #[test]
    fn missing_face_is_open() {
        let mut t = tetrahedron();
        t.triangles.pop();
        assert_eq!(t.check_closed(), Err(WatertightError::OpenEdge));
    }
}
