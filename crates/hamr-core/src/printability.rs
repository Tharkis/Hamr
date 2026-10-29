//! Checks a merged model for things that commonly go wrong when it is 3D printed.

use glam::Vec3;

use crate::merge::Model;

/// Settings for the check. Lengths are in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Printer {
    /// Width, depth and height the printer can build.
    pub build_volume: Vec3,
    /// Faces leaning out further than this from vertical need support.
    pub max_overhang_degrees: f32,
    /// Parts thinner than this in every direction are likely too small to print.
    pub min_feature: f32,
}

impl Default for Printer {
    fn default() -> Self {
        Self {
            build_volume: Vec3::new(220.0, 220.0, 250.0),
            max_overhang_degrees: 45.0,
            min_feature: 0.8,
        }
    }
}

/// Something worth knowing before printing.
#[derive(Clone, Debug, PartialEq)]
pub enum Issue {
    /// The model is larger than the build volume.
    TooBig { size: Vec3 },
    /// The model is in several pieces that don't touch.
    SeparateParts { count: usize },
    /// Pieces whose lowest point is above the lowest point of the model, so they start in mid
    /// air.
    Floating { count: usize, highest_gap: f32 },
    /// Downward-facing surface steeper than the printer can bridge, that needs support.
    Overhang { area: f32, fraction: f32 },
    /// Pieces smaller than the smallest feature the printer can make.
    TooSmall { count: usize },
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Issue::TooBig { size } => write!(
                f,
                "The model ({:.1} × {:.1} × {:.1} mm) is larger than the build volume.",
                size.x, size.y, size.z
            ),
            Issue::SeparateParts { count } => {
                write!(
                    f,
                    "The model is in {count} separate pieces that don't touch."
                )
            }
            Issue::Floating { count, highest_gap } => write!(
                f,
                "{count} piece{} start{} in mid air, up to {highest_gap:.1} mm above the bed, \
                 and need support.",
                plural(*count),
                if *count == 1 { "s" } else { "" }
            ),
            Issue::Overhang { area, fraction } => write!(
                f,
                "{area:.0} mm² of downward-facing surface ({:.0}% of the surface) overhangs too \
                 far and needs support.",
                fraction * 100.0
            ),
            Issue::TooSmall { count } => write!(
                f,
                "{count} piece{} too small to print reliably.",
                if *count == 1 { " is" } else { "s are" }
            ),
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Lists the problems found, the most serious first. An empty list means the model looks
/// printable. The model is assumed to sit on the bed with its lowest point.
pub fn check(model: &Model, printer: &Printer) -> Vec<Issue> {
    let solid = &model.solid;
    let mut issues = Vec::new();
    let Some((lo, hi)) = bounds(solid.positions.iter().copied()) else {
        return issues;
    };
    let size = hi - lo;
    if size.cmpgt(printer.build_volume).any() {
        issues.push(Issue::TooBig { size });
    }

    let parts = parts(model);
    let bed = lo.z;
    let tolerance = 0.01;
    let mut floating = 0;
    let mut highest_gap = 0.0f32;
    let mut too_small = 0;
    for part in &parts {
        let gap = part.lo.z - bed;
        if gap > tolerance {
            floating += 1;
            highest_gap = highest_gap.max(gap);
        }
        if (part.hi - part.lo).max_element() < printer.min_feature {
            too_small += 1;
        }
    }
    if floating > 0 {
        issues.push(Issue::Floating {
            count: floating,
            highest_gap,
        });
    }
    if too_small > 0 {
        issues.push(Issue::TooSmall { count: too_small });
    }

    // A face needs support when it points down more steeply than the overhang limit allows,
    // unless it lies on the bed.
    let limit = -(printer.max_overhang_degrees.to_radians().sin());
    let mut overhang = 0.0;
    let mut total = 0.0;
    for tri in &solid.triangles {
        let [a, b, c] = tri.map(|i| solid.positions[i as usize]);
        let cross = (b - a).cross(c - a);
        let area = cross.length() * 0.5;
        total += area;
        let on_bed = [a, b, c].iter().all(|p| p.z - bed < tolerance);
        if !on_bed && area > 0.0 && cross.z / (2.0 * area) < limit {
            overhang += area;
        }
    }
    if overhang > 0.0 {
        issues.push(Issue::Overhang {
            area: overhang,
            fraction: overhang / total,
        });
    }

    if parts.len() > 1 {
        issues.push(Issue::SeparateParts { count: parts.len() });
    }
    issues
}

struct Part {
    lo: Vec3,
    hi: Vec3,
}

fn bounds(points: impl Iterator<Item = Vec3>) -> Option<(Vec3, Vec3)> {
    points.fold(None, |acc, p| {
        Some(acc.map_or((p, p), |(lo, hi): (Vec3, Vec3)| (lo.min(p), hi.max(p))))
    })
}

/// The connected pieces of the model, found by following shared vertices.
fn parts(model: &Model) -> Vec<Part> {
    let solid = &model.solid;
    let mut parent: Vec<u32> = (0..solid.positions.len() as u32).collect();
    fn find(parent: &mut [u32], mut i: u32) -> u32 {
        while parent[i as usize] != i {
            parent[i as usize] = parent[parent[i as usize] as usize];
            i = parent[i as usize];
        }
        i
    }
    for &[a, b, c] in &solid.triangles {
        for (x, y) in [(a, b), (b, c)] {
            let (rx, ry) = (find(&mut parent, x), find(&mut parent, y));
            parent[rx as usize] = ry;
        }
    }
    let mut groups: std::collections::HashMap<u32, Vec<Vec3>> = Default::default();
    let used: std::collections::HashSet<u32> = solid.triangles.iter().flatten().copied().collect();
    for v in used {
        let root = find(&mut parent, v);
        groups
            .entry(root)
            .or_default()
            .push(solid.positions[v as usize]);
    }
    groups
        .into_values()
        .filter_map(|points| bounds(points.into_iter()))
        .map(|(lo, hi)| Part { lo, hi })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{merge, PrimKind, Scene};

    fn check_scene(scene: &Scene) -> Vec<Issue> {
        check(&merge::merge(&scene.objects).unwrap(), &Printer::default())
    }

    #[test]
    fn a_box_on_the_bed_is_fine() {
        let mut scene = Scene::default();
        scene.add_prim(PrimKind::Box, Vec3::ZERO);
        assert_eq!(check_scene(&scene), vec![]);
    }

    #[test]
    fn finds_floating_pieces_and_their_overhangs() {
        // A second box hovering above the first is a separate piece, starts 30 mm up, and its
        // whole underside overhangs.
        let mut scene = Scene::default();
        scene.add_prim(PrimKind::Box, Vec3::ZERO);
        let b = scene.add_prim(PrimKind::Box, Vec3::new(100.0, 0.0, 0.0));
        scene.get_mut(b).unwrap().position.z += 30.0;
        let issues = check_scene(&scene);
        assert!(
            issues.contains(&Issue::SeparateParts { count: 2 }),
            "{issues:?}"
        );
        let Some(Issue::Floating { count, highest_gap }) =
            issues.iter().find(|i| matches!(i, Issue::Floating { .. }))
        else {
            panic!("{issues:?}")
        };
        assert_eq!(*count, 1);
        assert!((highest_gap - 30.0).abs() < 1e-3);
        let Some(Issue::Overhang { area, .. }) =
            issues.iter().find(|i| matches!(i, Issue::Overhang { .. }))
        else {
            panic!("{issues:?}")
        };
        assert!((area - 2500.0).abs() < 1.0, "{area}");
    }

    #[test]
    fn gentle_slopes_and_sphere_bottoms() {
        // A sphere's lower half overhangs more than 45° near its bottom, but a 30° chamfer-like
        // slope (a tapered box standing on its wide end) does not.
        let mut scene = Scene::default();
        let id = scene.add_prim(PrimKind::Box, Vec3::ZERO);
        scene.get_mut(id).unwrap().shape.taper = glam::Vec2::new(0.5, 0.5);
        assert_eq!(check_scene(&scene), vec![]);

        let mut scene = Scene::default();
        scene.add_prim(PrimKind::Sphere, Vec3::ZERO);
        let issues = check_scene(&scene);
        assert!(
            matches!(issues.as_slice(), [Issue::Overhang { .. }]),
            "{issues:?}"
        );
    }

    #[test]
    fn too_big_and_too_small() {
        let mut scene = Scene::default();
        let big = scene.add_prim(PrimKind::Box, Vec3::ZERO);
        scene.get_mut(big).unwrap().size = Vec3::new(300.0, 50.0, 50.0);
        let small = scene.add_prim(PrimKind::Box, Vec3::new(200.0, 0.0, 0.0));
        let s = scene.get_mut(small).unwrap();
        s.size = Vec3::splat(0.5);
        s.position.z = 0.25;
        let issues = check_scene(&scene);
        assert!(matches!(issues[0], Issue::TooBig { .. }), "{issues:?}");
        assert!(issues.contains(&Issue::TooSmall { count: 1 }), "{issues:?}");
    }
}
