//! Sweeps a profile along a straight or circular path and closes it into a solid.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use super::profile::{self, Profile};
use super::{PrimKind, PrimShape};
use crate::mesh::position_key;
use crate::Solid;
/// Path segments per full turn of a circular path.
const SEGMENTS_PER_TURN: f32 = 32.0;
/// Twist is split into steps of at most this many degrees. Twisted flat faces are curved surfaces,
/// so they need fine steps to shade smoothly.
const DEGREES_PER_TWIST_STEP: f32 = 3.0;

/// How one point of the path places the profile in 3D.
trait Path {
    /// Maps a profile point to the unit box at path parameter `t` (0..1 over the full path).
    fn place(&self, p: Vec2, t: f32) -> Vec3;
}

/// The smallest scale a full taper reaches. Shrinking a hollow or cut profile all the way to a
/// line would fold its walls onto each other and leave edges shared by four faces, which slicers
/// reject. A thousandth of the prim (0.05 mm on a 50 mm prim) is invisible but keeps it manifold.
const MIN_TAPER_SCALE: f32 = 0.001;

/// Shrinks one end of the path: positive taper shrinks the end, negative the beginning.
fn taper_scale(taper: f32, t: f32) -> f32 {
    let scale = if taper >= 0.0 {
        1.0 - taper * t
    } else {
        1.0 + taper * (1.0 - t)
    };
    scale.max(MIN_TAPER_SCALE)
}

/// Applies the twist, taper and shear that every path shares to a profile point.
fn shape_profile(s: &PrimShape, p: Vec2, t: f32) -> Vec2 {
    let scale = Vec2::new(taper_scale(s.taper.x, t), taper_scale(s.taper.y, t));
    let angle = (s.twist[0] + (s.twist[1] - s.twist[0]) * t).to_radians();
    Vec2::from_angle(angle).rotate(p * scale) + s.top_shear * (t - 0.5)
}

struct Linear<'a>(&'a PrimShape);

impl Path for Linear<'_> {
    fn place(&self, p: Vec2, t: f32) -> Vec3 {
        shape_profile(self.0, p, t).extend(t - 0.5)
    }
}

/// Revolves the profile around Z. The profile's X becomes the distance from the axis and its Y the
/// height.
struct Circular<'a> {
    shape: &'a PrimShape,
    /// Scales the unit profile before it is placed.
    profile_scale: Vec2,
    /// Distance of the profile centre from the Z axis at the start.
    radius: f32,
}

impl Path for Circular<'_> {
    fn place(&self, p: Vec2, t: f32) -> Vec3 {
        let s = self.shape;
        let q = shape_profile(s, p * self.profile_scale, t);
        let turns = s.revolutions;
        // Coils climb so that each turn sits on top of the previous one.
        let climb = (t - 0.5) * (1.0 - 1.0 / turns);
        let rho = self.radius * (1.0 + s.radius_offset * t) + q.x;
        let (sin, cos) = (TAU * turns * t).sin_cos();
        Vec3::new(rho * cos + s.skew * (t - 0.5), rho * sin, q.y + climb)
    }
}

/// Sampled path parameters, and whether the path wraps back onto its start.
fn path_samples(s: &PrimShape, range: [f32; 2], closed: bool) -> Vec<f32> {
    let span = range[1] - range[0];
    let twist_steps = ((s.twist[1] - s.twist[0]).abs() * span / DEGREES_PER_TWIST_STEP).ceil();
    let turn_steps = if s.kind.is_linear() {
        1.0
    } else {
        (SEGMENTS_PER_TURN * s.revolutions * span).ceil()
    };
    let steps = twist_steps.max(turn_steps).max(1.0) as usize;
    let count = if closed { steps } else { steps + 1 };
    (0..count)
        .map(|i| range[0] + span * i as f32 / steps as f32)
        .collect()
}

/// Builds the closed mesh for an already clamped shape.
pub fn build(s: &PrimShape) -> Solid {
    let kind = s.kind;
    let outline = kind.outline();
    let (profile, path_range) = match kind {
        PrimKind::Box | PrimKind::Cylinder | PrimKind::Prism => (
            profile::build(outline, s.path_cut, s.hollow, s.hollow_shape),
            s.slice,
        ),
        // A sphere is a half circle revolved around Z. Its profile is the right half of a circle,
        // and the dimple cuts within that half.
        PrimKind::Sphere => {
            let cut = s.dimple.map(|f| f * 0.5);
            (
                profile::build(outline, cut, s.hollow, s.hollow_shape),
                s.path_cut,
            )
        }
        PrimKind::Torus | PrimKind::Tube | PrimKind::Ring => (
            profile::build(outline, s.profile_cut, s.hollow, s.hollow_shape),
            s.path_cut,
        ),
    };

    if kind.is_linear() {
        let samples = path_samples(s, path_range, false);
        return sweep(&profile, &Linear(s), &samples, false, is_twisted(s));
    }

    let circular = if kind == PrimKind::Sphere {
        Circular {
            shape: s,
            profile_scale: Vec2::ONE,
            radius: 0.0,
        }
    } else {
        // At the maximum hole size the profile would touch the Z axis, and on a coil its inner
        // wall would collapse into a line. Stopping just short keeps the wall a real surface.
        let thickness = s.hole_size.y.min(0.5 - MIN_TAPER_SCALE);
        Circular {
            shape: s,
            profile_scale: Vec2::new(thickness, s.hole_size.x / s.revolutions),
            radius: 0.5 - thickness * 0.5,
        }
    };
    // The path closes on itself only when nothing changes between its two ends. A twist of a whole
    // turn ends where it began, so it closes too; capping it would put two caps in the same place.
    let closed = path_range == [0.0, 1.0]
        && s.revolutions == 1.0
        && (s.twist[1] - s.twist[0]).rem_euclid(360.0) == 0.0
        && s.taper == Vec2::ZERO
        && s.top_shear == Vec2::ZERO
        && s.radius_offset == 0.0
        && s.skew == 0.0;
    let samples = path_samples(s, path_range, closed);
    sweep(&profile, &circular, &samples, closed, is_twisted(s))
}

fn is_twisted(s: &PrimShape) -> bool {
    s.twist[0] != s.twist[1]
}

/// Twisted flat faces are curved, so long profile edges are split into pieces no longer than
/// this. Otherwise each twisted face is one strip of long thin triangles and shades in stripes.
const MAX_TWISTED_EDGE: f32 = 0.1;

/// The profile with long edges split, plus where each original corner ended up.
struct DenseProfile {
    loops: Vec<Vec<Vec2>>,
    /// corners[loop][i] = index in the dense loop of the profile's i-th point.
    corners: Vec<Vec<u32>>,
}

fn densify(profile: &Profile, max_edge: Option<f32>) -> DenseProfile {
    let mut loops = Vec::with_capacity(profile.loops.len());
    let mut corners = Vec::with_capacity(profile.loops.len());
    for ring in &profile.loops {
        let mut dense = Vec::new();
        let mut at = Vec::with_capacity(ring.len());
        for (i, &p) in ring.iter().enumerate() {
            at.push(dense.len() as u32);
            dense.push(p);
            let q = ring[(i + 1) % ring.len()];
            let pieces = max_edge.map_or(1, |m| ((q - p).length() / m).ceil().max(1.0) as usize);
            dense.extend((1..pieces).map(|k| p.lerp(q, k as f32 / pieces as f32)));
        }
        loops.push(dense);
        corners.push(at);
    }
    DenseProfile { loops, corners }
}

/// A corner of a cap triangle: a point of the swept rings, or one of the cap's extra points
/// inside the profile.
#[derive(Clone, Copy)]
enum CapPoint {
    Ring(usize, u32),
    Inside(usize),
}

/// A triangulated cap, the same at both ends of the path.
struct Cap {
    triangles: Vec<[CapPoint; 3]>,
    /// Profile positions of the [`CapPoint::Inside`] points.
    inside: Vec<Vec2>,
}

/// Places the profile at every sample, joins neighbouring rings with quads and caps open ends.
fn sweep(
    profile: &Profile,
    path: &dyn Path,
    samples: &[f32],
    closed: bool,
    twisted: bool,
) -> Solid {
    let dense = densify(profile, twisted.then_some(MAX_TWISTED_EDGE));
    let mut positions: Vec<Vec3> = Vec::new();
    // rings[sample][loop] = index of the loop's first vertex.
    let mut rings: Vec<Vec<u32>> = Vec::with_capacity(samples.len());
    for &t in samples {
        let mut starts = Vec::with_capacity(dense.loops.len());
        for ring in &dense.loops {
            starts.push(positions.len() as u32);
            positions.extend(ring.iter().map(|&p| path.place(p, t)));
        }
        rings.push(starts);
    }

    let mut tris: Vec<[u32; 3]> = Vec::new();
    let joins = if closed {
        samples.len()
    } else {
        samples.len() - 1
    };
    for k in 0..joins {
        let next = (k + 1) % samples.len();
        for (l, ring) in dense.loops.iter().enumerate() {
            let n = ring.len() as u32;
            for i in 0..n {
                let j = (i + 1) % n;
                let a = rings[k][l] + i;
                let b = rings[k][l] + j;
                let c = rings[next][l] + j;
                let d = rings[next][l] + i;
                tris.extend(split_quad(&positions, [a, b, c, d]));
            }
        }
    }

    if !closed {
        let cap = cap_triangles(profile, &dense);
        let last = samples.len() - 1;
        for (sample, flip) in [(0, true), (last, false)] {
            let t = samples[sample];
            let inside_start = positions.len() as u32;
            positions.extend(cap.inside.iter().map(|&q| path.place(q, t)));
            for tri in &cap.triangles {
                let [a, b, c] = tri.map(|p| match p {
                    CapPoint::Ring(l, i) => rings[sample][l] + i,
                    CapPoint::Inside(k) => inside_start + k as u32,
                });
                // The start cap faces back along the path, the end cap forwards.
                tris.push(if flip { [a, c, b] } else { [a, b, c] });
            }
        }
    }

    // Points that collapse onto each other, such as a sphere's poles or the centre of a cut
    // profile on the axis, are merged. Only the same profile point in neighbouring rings, or
    // neighbouring points in one ring, are merged: other surfaces that happen to touch stay
    // separate so the result stays manifold.
    let mut merge = UnionFind::new(positions.len());
    let same =
        |a: u32, b: u32| position_key(positions[a as usize]) == position_key(positions[b as usize]);
    for k in 0..samples.len() {
        for (l, ring) in dense.loops.iter().enumerate() {
            let n = ring.len() as u32;
            for i in 0..n {
                let here = rings[k][l] + i;
                let along = rings[k][l] + (i + 1) % n;
                if same(here, along) {
                    merge.union(here, along);
                }
                if k + 1 < samples.len() || closed {
                    let next = rings[(k + 1) % samples.len()][l] + i;
                    if same(here, next) {
                        merge.union(here, next);
                    }
                }
            }
        }
    }

    let mut solid = merge.compact(&positions, &tris);
    // Depending on the path the surface may come out inside-out; flip it so it faces outwards.
    if solid.signed_volume() < 0.0 {
        solid.flip();
    }
    solid
}

/// Groups vertex indices that should become one vertex.
struct UnionFind {
    parent: Vec<u32>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n as u32).collect(),
        }
    }

    fn find(&mut self, mut i: u32) -> u32 {
        while self.parent[i as usize] != i {
            let up = self.parent[self.parent[i as usize] as usize];
            self.parent[i as usize] = up;
            i = up;
        }
        i
    }

    fn union(&mut self, a: u32, b: u32) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra.max(rb) as usize] = ra.min(rb);
        }
    }

    /// Renumbers vertices so each group is one vertex, dropping triangles that collapse.
    fn compact(&mut self, positions: &[Vec3], triangles: &[[u32; 3]]) -> Solid {
        let mut new_index = vec![u32::MAX; positions.len()];
        let mut solid = Solid::default();
        for i in 0..positions.len() as u32 {
            let root = self.find(i);
            if new_index[root as usize] == u32::MAX {
                new_index[root as usize] = solid.positions.len() as u32;
                solid.positions.push(positions[root as usize]);
            }
            new_index[i as usize] = new_index[root as usize];
        }
        solid.triangles = triangles
            .iter()
            .map(|t| t.map(|i| new_index[i as usize]))
            .filter(|[a, b, c]| a != b && b != c && a != c)
            .collect();
        solid
    }
}

/// Counter-clockwise cap triangles that use every point of the dense rings on the cap's edge, so
/// the caps meet the sides without gaps.
fn cap_triangles(profile: &Profile, dense: &DenseProfile) -> Cap {
    let mut out = Cap {
        triangles: Vec::new(),
        inside: Vec::new(),
    };
    for tri in triangulate(profile) {
        // Walk the triangle's edges, picking up the extra points on edges that follow the outline.
        let mut polygon: Vec<(CapPoint, Vec2)> = Vec::new();
        for k in 0..3 {
            let (l, i) = tri[k];
            let (m, j) = tri[(k + 1) % 3];
            let corners = &dense.corners[l];
            let n = corners.len() as u32;
            let dense_len = dense.loops[l].len() as u32;
            let start = corners[i as usize];
            let mut push =
                |d: u32| polygon.push((CapPoint::Ring(l, d), dense.loops[l][d as usize]));
            if l == m && j == (i + 1) % n {
                let end = corners[j as usize];
                let mut d = start;
                while d != end {
                    push(d);
                    d = (d + 1) % dense_len;
                }
            } else if l == m && i == (j + 1) % n {
                let end = corners[j as usize];
                let mut d = start;
                while d != end {
                    push(d);
                    d = (d + dense_len - 1) % dense_len;
                }
            } else {
                push(start);
            }
        }
        if polygon.len() == 3 {
            out.triangles
                .push([polygon[0].0, polygon[1].0, polygon[2].0]);
            continue;
        }
        // Points on the edges are collinear with the corners, so fan from the centre instead.
        let centre = polygon.iter().map(|p| p.1).sum::<Vec2>() / polygon.len() as f32;
        let centre_id = CapPoint::Inside(out.inside.len());
        out.inside.push(centre);
        for k in 0..polygon.len() {
            let next = polygon[(k + 1) % polygon.len()].0;
            out.triangles.push([centre_id, polygon[k].0, next]);
        }
    }
    out
}

/// Splits a quad into two triangles. The same diagonal is used everywhere so twisted surfaces shade
/// evenly, except where it would produce a triangle with no area: where a profile touches the Z
/// axis, one diagonal can put all three corners on a line.
fn split_quad(positions: &[Vec3], [a, b, c, d]: [u32; 4]) -> [[u32; 3]; 2] {
    let area = |[x, y, z]: [u32; 3]| {
        let [x, y, z] = [x, y, z].map(|i| positions[i as usize]);
        (y - x).cross(z - x).length_squared()
    };
    let first = [[a, b, c], [a, c, d]];
    let second = [[a, b, d], [b, c, d]];
    let worst = |pair: [[u32; 3]; 2]| area(pair[0]).min(area(pair[1]));
    if worst(first) < worst(second) * 1e-6 {
        second
    } else {
        first
    }
}

/// Triangulates the profile's area, returning counter-clockwise triangles as (loop, index) pairs.
fn triangulate(profile: &Profile) -> Vec<[(usize, u32); 3]> {
    let mut flat: Vec<[f32; 2]> = Vec::new();
    let mut owner: Vec<(usize, u32)> = Vec::new();
    let mut holes: Vec<u32> = Vec::new();
    for (l, ring) in profile.loops.iter().enumerate() {
        if l > 0 {
            holes.push(flat.len() as u32);
        }
        for (i, p) in ring.iter().enumerate() {
            flat.push(p.to_array());
            owner.push((l, i as u32));
        }
    }
    let mut out: Vec<u32> = Vec::new();
    earcut::Earcut::new().earcut(flat.iter().copied(), &holes, &mut out);
    out.chunks_exact(3)
        .map(|t| {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| owner[i as usize]);
            let pt = |(l, i): (usize, u32)| profile.loops[l][i as usize];
            if profile::signed_area2(&[pt(a), pt(b), pt(c)]) < 0.0 {
                [a, c, b]
            } else {
                [a, b, c]
            }
        })
        .collect()
}
