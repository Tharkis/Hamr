//! Every prim, across the parameter ranges, must come out as a closed, outward-facing solid.

use glam::{Vec2, Vec3};

use super::*;

/// Fails with the shape that broke, so a regression points straight at the parameters.
fn assert_solid(shape: &PrimShape) {
    let solid = shape.solid();
    assert!(!solid.triangles.is_empty(), "empty mesh for {shape:?}");
    if let Err(e) = solid.check_closed() {
        panic!("{e:?} for {shape:?}");
    }
    assert!(solid.signed_volume() > 0.0, "inside out or flat: {shape:?}");
    let smallest = solid
        .triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| solid.positions[i as usize]);
            (b - a).cross(c - a).length() * 0.5
        })
        .fold(f32::MAX, f32::min);
    // Full tapers legitimately leave very small (but real) triangles at the tip, so only reject
    // triangles whose area is float noise.
    assert!(
        smallest > 1e-13,
        "sliver triangle of area {smallest} for {shape:?}"
    );
    for &p in &solid.positions {
        assert!(p.is_finite(), "non-finite vertex for {shape:?}");
        // Shear, skew and radius offset may push a prim past its box, but never far.
        assert!(
            p.abs().max_element() < 2.0,
            "vertex {p} far outside the box for {shape:?}"
        );
    }
}

/// Every cut, hollow and twist value worth trying, for each kind.
fn variations(kind: PrimKind) -> Vec<PrimShape> {
    let base = PrimShape::new(kind);
    let mut out = vec![base.clone()];
    let cuts = [
        [0.0, 1.0],
        [0.25, 1.0],
        [0.0, 0.5],
        [0.2, 0.8],
        [0.49, 0.51],
        [0.0, 0.02],
    ];
    let hollows = [0.0, 0.5, 0.95];
    for cut in cuts {
        for hollow in hollows {
            for hollow_shape in HollowShape::ALL {
                for twist in [[0.0, 0.0], [0.0, 90.0], [-180.0, 180.0]] {
                    let mut s = base.clone();
                    s.hollow = hollow;
                    s.hollow_shape = hollow_shape;
                    s.twist = twist;
                    match kind {
                        PrimKind::Sphere => s.dimple = cut,
                        k if k.is_ring_like() => s.profile_cut = cut,
                        _ => s.path_cut = cut,
                    }
                    out.push(s);
                }
            }
        }
    }
    // Path-wise parameters, combined with a hollow pie cut.
    for path in [[0.0, 1.0], [0.3, 0.9]] {
        for taper in [
            Vec2::ZERO,
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 0.5),
            Vec2::new(0.3, -0.7),
        ] {
            for shear in [Vec2::ZERO, Vec2::new(0.5, -0.5)] {
                let mut s = base.clone();
                s.hollow = 0.4;
                s.taper = taper;
                s.top_shear = shear;
                if kind.is_linear() {
                    s.path_cut = [0.1, 0.7];
                    s.slice = path;
                } else {
                    s.path_cut = path;
                }
                out.push(s);
            }
        }
    }
    if kind.is_ring_like() {
        for revolutions in [1.0, 1.5, 4.0] {
            for radius_offset in [0.0, -1.0, 1.0] {
                for skew in [0.0, 0.95] {
                    for hole in [
                        Vec2::new(1.0, 0.25),
                        Vec2::new(0.05, 0.05),
                        Vec2::new(1.0, 0.5),
                    ] {
                        let mut s = base.clone();
                        s.revolutions = revolutions;
                        s.radius_offset = radius_offset;
                        s.skew = skew;
                        s.hole_size = hole;
                        s.hollow = 0.3;
                        out.push(s);
                    }
                }
            }
        }
    }
    out
}

#[test]
fn every_variation_is_a_closed_solid() {
    for kind in PrimKind::ALL {
        for shape in variations(kind) {
            assert_solid(&shape);
        }
    }
}

#[test]
fn default_box_is_the_unit_cube() {
    // Catches scaling or placement drift: the default box must fill the unit box exactly.
    let mesh = PrimShape::new(PrimKind::Box).mesh();
    assert!((mesh.signed_volume() - 1.0).abs() < 1e-5);
    let (lo, hi) = mesh.bounds().unwrap();
    assert!((lo + Vec3::splat(0.5)).abs().max_element() < 1e-6);
    assert!((hi - Vec3::splat(0.5)).abs().max_element() < 1e-6);
}

#[test]
fn hollow_removes_the_expected_volume() {
    // A 50% square hollow in a box removes a quarter of its cross-section.
    let mut s = PrimShape::new(PrimKind::Box);
    s.hollow = 0.5;
    assert!((s.mesh().signed_volume() - 0.75).abs() < 1e-4);
}

#[test]
fn sphere_volume_is_close_to_the_true_sphere() {
    // Catches a sphere that is not closed at the poles or is revolved the wrong amount.
    let v = PrimShape::new(PrimKind::Sphere).mesh().signed_volume();
    let exact = 4.0 / 3.0 * std::f32::consts::PI * 0.125;
    assert!(
        (v - exact).abs() / exact < 0.03,
        "volume {v}, expected about {exact}"
    );
}

#[test]
fn half_slice_halves_a_cylinder() {
    let full = PrimShape::new(PrimKind::Cylinder).mesh().signed_volume();
    let mut s = PrimShape::new(PrimKind::Cylinder);
    s.slice = [0.0, 0.5];
    assert!((s.mesh().signed_volume() - full * 0.5).abs() < 1e-4);
}

#[test]
fn full_taper_makes_a_pyramid() {
    // A box tapered to a point has a third of the cube's volume (plus a sliver for the tiny top
    // the taper stops at).
    let mut s = PrimShape::new(PrimKind::Box);
    s.taper = Vec2::ONE;
    assert!((s.mesh().signed_volume() - 1.0 / 3.0).abs() < 1e-3);
}

#[test]
fn clamping_fixes_out_of_range_input() {
    let mut s = PrimShape::new(PrimKind::Torus);
    s.path_cut = [0.9, 0.1];
    s.hollow = 2.0;
    s.revolutions = f32::NAN;
    let c = s.clamped();
    assert!(c.path_cut[1] - c.path_cut[0] >= MIN_CUT_SPAN - 1e-6);
    assert_eq!(c.hollow, MAX_HOLLOW);
    assert_eq!(c.revolutions, 1.0);
    assert_solid(&s);
}

#[test]
fn smooth_cylinder_side_has_smooth_normals() {
    // The cylinder side should be shaded smooth: no side normal points straight along a facet.
    let mesh = PrimShape::new(PrimKind::Cylinder).mesh();
    let distinct: std::collections::HashSet<[u32; 3]> = mesh
        .vertices
        .iter()
        .filter(|v| v.normal[2].abs() < 0.5)
        .map(|v| v.normal.map(|c| (c * 1000.0).round().to_bits()))
        .collect();
    assert!(distinct.len() >= 32, "only {} side normals", distinct.len());
}
