//! Builds the 2D cross-section of a prim: its outline, minus any cut and hollow.

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::Vec2;

use super::HollowShape;

/// Number of sides used for circular outlines.
pub const CIRCLE_SIDES: usize = 32;

/// Cut fractions start at this angle (pointing down −Y) and run counter-clockwise.
const START_ANGLE: f32 = -FRAC_PI_2;

/// A cross-section as closed loops. The first loop is the outer boundary, counter-clockwise; any
/// further loops are holes, clockwise.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub loops: Vec<Vec<Vec2>>,
}

/// The convex outline of `shape`, counter-clockwise, inside the unit square around the origin.
fn outline(shape: HollowShape) -> Vec<Vec2> {
    match shape {
        HollowShape::Square | HollowShape::Same => vec![
            Vec2::new(-0.5, -0.5),
            Vec2::new(0.5, -0.5),
            Vec2::new(0.5, 0.5),
            Vec2::new(-0.5, 0.5),
        ],
        HollowShape::Triangle => {
            vec![
                Vec2::new(-0.5, -0.5),
                Vec2::new(0.5, -0.5),
                Vec2::new(0.0, 0.5),
            ]
        }
        HollowShape::Circle => (0..CIRCLE_SIDES)
            .map(|i| {
                let a = START_ANGLE + TAU * i as f32 / CIRCLE_SIDES as f32;
                snap(Vec2::new(a.cos(), a.sin()) * 0.5)
            })
            .collect(),
    }
}

/// Rounds coordinates that are within float noise of zero to exactly zero, so points that should
/// sit on an axis do, and later merge cleanly.
fn snap(p: Vec2) -> Vec2 {
    let s = |v: f32| if v.abs() < 1e-6 { 0.0 } else { v };
    Vec2::new(s(p.x), s(p.y))
}

/// The cut fraction (0..1) at which a point lies, measured from [`START_ANGLE`].
fn fraction_of(p: Vec2) -> f32 {
    let a = (p.y.atan2(p.x) - START_ANGLE).rem_euclid(TAU);
    a / TAU
}

/// Where the ray from the origin at cut fraction `f` leaves the convex polygon.
fn point_at(poly: &[Vec2], f: f32) -> Vec2 {
    let a = START_ANGLE + TAU * f;
    let dir = Vec2::new(a.cos(), a.sin());
    let mut best: Option<Vec2> = None;
    for i in 0..poly.len() {
        let p = poly[i];
        let q = poly[(i + 1) % poly.len()];
        let edge = q - p;
        let denom = dir.perp_dot(edge);
        if denom.abs() < 1e-12 {
            continue;
        }
        let t = p.perp_dot(edge) / denom;
        let s = p.perp_dot(dir) / denom;
        if t > 0.0 && (-1e-5..=1.0 + 1e-5).contains(&s) {
            let hit = dir * t;
            if best.is_none_or(|b| hit.length_squared() < b.length_squared()) {
                best = Some(hit);
            }
        }
    }
    snap(best.unwrap_or(Vec2::ZERO))
}

/// The largest scale at which `inner` still fits inside `outer`, capped at 1.
///
/// A square hole in a circle would otherwise poke through at the corners, so hollow is measured
/// against the biggest hole of that shape that fits. Both outlines are star-shaped around the
/// origin and piecewise linear, so checking the directions of every corner is enough.
fn fit_scale(outer: &[Vec2], inner: &[Vec2]) -> f32 {
    outer
        .iter()
        .chain(inner)
        .map(|&p| {
            let f = fraction_of(p);
            let r_outer = point_at(outer, f).length();
            let r_inner = point_at(inner, f).length();
            if r_inner > 0.0 {
                r_outer / r_inner
            } else {
                1.0
            }
        })
        .fold(1.0, f32::min)
}

/// The outline points between cut fractions `begin` and `end`, in counter-clockwise order,
/// including the two cut points themselves.
fn arc(poly: &[Vec2], begin: f32, end: f32) -> Vec<Vec2> {
    const EPS: f32 = 1e-5;
    let mut corners: Vec<(f32, Vec2)> = poly
        .iter()
        .map(|&p| (fraction_of(p), p))
        .filter(|&(f, _)| f > begin + EPS && f < end - EPS)
        .collect();
    corners.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::with_capacity(corners.len() + 2);
    out.push(point_at(poly, begin));
    out.extend(corners.into_iter().map(|(_, p)| p));
    out.push(point_at(poly, end));
    out
}

/// Builds a profile from an outline shape, a cut range and a hollow.
///
/// A full cut (0..1) gives the closed outline, plus a hole when hollow. A partial cut gives one
/// loop: the outline arc, then either the hollow's arc back again or the centre point.
pub fn build(shape: HollowShape, cut: [f32; 2], hollow: f32, hollow_shape: HollowShape) -> Profile {
    let outer = outline(shape);
    let inner_shape = if hollow_shape == HollowShape::Same {
        shape
    } else {
        hollow_shape
    };
    let inner_outline = outline(inner_shape);
    let fit = fit_scale(&outer, &inner_outline);
    let inner: Vec<Vec2> = inner_outline
        .into_iter()
        .map(|p| p * hollow * fit)
        .collect();
    let full = cut[0] <= 0.0 && cut[1] >= 1.0;

    if full {
        let mut loops = vec![outer];
        if hollow > 0.0 {
            loops.push(inner.into_iter().rev().collect());
        }
        return Profile { loops };
    }

    let mut ring = arc(&outer, cut[0], cut[1]);
    if hollow > 0.0 {
        ring.extend(arc(&inner, cut[0], cut[1]).into_iter().rev());
    } else {
        ring.push(Vec2::ZERO);
    }
    Profile { loops: vec![ring] }
}

/// Twice the signed area of a loop; positive when counter-clockwise.
pub fn signed_area2(ring: &[Vec2]) -> f32 {
    (0..ring.len())
        .map(|i| ring[i].perp_dot(ring[(i + 1) % ring.len()]))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_square_is_one_ccw_loop() {
        let p = build(HollowShape::Square, [0.0, 1.0], 0.0, HollowShape::Same);
        assert_eq!(p.loops.len(), 1);
        assert!(signed_area2(&p.loops[0]) > 0.0);
    }

    #[test]
    fn hollow_adds_a_clockwise_hole() {
        let p = build(HollowShape::Circle, [0.0, 1.0], 0.5, HollowShape::Square);
        assert_eq!(p.loops.len(), 2);
        assert!(signed_area2(&p.loops[1]) < 0.0);
    }

    #[test]
    fn square_hollow_fits_inside_a_circle() {
        let p = build(HollowShape::Circle, [0.0, 1.0], 0.95, HollowShape::Square);
        assert!(p.loops[1].iter().all(|q| q.length() < 0.5 * 0.95 + 1e-5));
    }

    #[test]
    fn quarter_cut_square_keeps_three_quarters() {
        // Cutting from 0.25 to 1.0 removes one quadrant: area 0.75 of the unit square.
        let p = build(HollowShape::Square, [0.25, 1.0], 0.0, HollowShape::Same);
        let area = signed_area2(&p.loops[0]) * 0.5;
        assert!((area - 0.75).abs() < 1e-4, "area {area}");
    }

    #[test]
    fn cut_points_lie_on_the_outline() {
        let square = outline(HollowShape::Square);
        for f in [0.0, 0.1, 0.33, 0.5, 0.9] {
            let p = point_at(&square, f);
            assert!((p.x.abs().max(p.y.abs()) - 0.5).abs() < 1e-5, "{f}: {p}");
        }
    }
}
