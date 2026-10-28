//! The move, rotate and stretch handles drawn on the selected prim, and what dragging them does.
//!
//! Like the SL build tools, moving and rotating work along the world axes and stretching works
//! along the prim's own axes. Handles keep the same size on screen however far away the camera is.

use glam::{Mat4, Quat, Vec2, Vec3};

use crate::Object;

/// Which handles are shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Move,
    Rotate,
    Stretch,
}

/// One grabbable part of the gizmo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// Arrow along world axis 0, 1 or 2.
    Axis(usize),
    /// Ring around world axis 0, 1 or 2.
    Ring(usize),
    /// Stretch handle on the face of the prim's own axis, on its positive or negative side.
    Face { axis: usize, positive: bool },
}

/// How big handles are, as a fraction of the distance from the camera.
const SCREEN_SCALE: f32 = 0.18;
/// How close (in pixels) the cursor must be to grab a handle.
pub const GRAB_RADIUS_PX: f32 = 9.0;
const RING_SEGMENTS: usize = 64;
/// Smallest size stretching can give a prim, in millimetres.
pub const MIN_SIZE_MM: f32 = 0.1;

const AXES: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];

/// Colour of each axis: X red, Y green, Z blue, as on the grid.
pub fn axis_color(axis: usize) -> [f32; 4] {
    [
        [0.95, 0.3, 0.3, 1.0],
        [0.35, 0.9, 0.35, 1.0],
        [0.35, 0.55, 1.0, 1.0],
    ][axis]
}

/// Line segments in world space that make up a handle.
pub struct HandleLines {
    pub handle: Handle,
    pub segments: Vec<(Vec3, Vec3)>,
}

/// World-space length of the arrows (and radius of the rings) for an object seen from `eye`.
pub fn gizmo_size(object: &Object, eye: Vec3) -> f32 {
    object.position.distance(eye) * SCREEN_SCALE
}

/// The handles of `tool` for `object`, as line segments.
pub fn handles(tool: Tool, object: &Object, eye: Vec3) -> Vec<HandleLines> {
    let size = gizmo_size(object, eye);
    let c = object.position;
    match tool {
        Tool::Move => (0..3)
            .map(|a| {
                let dir = AXES[a];
                let tip = c + dir * size;
                // A small arrow head made of two strokes.
                let side = AXES[(a + 1) % 3] * size * 0.08;
                let back = tip - dir * size * 0.15;
                HandleLines {
                    handle: Handle::Axis(a),
                    segments: vec![(c, tip), (tip, back + side), (tip, back - side)],
                }
            })
            .collect(),
        Tool::Rotate => (0..3)
            .map(|a| {
                let (u, v) = (AXES[(a + 1) % 3], AXES[(a + 2) % 3]);
                let point = |i: usize| {
                    let t = std::f32::consts::TAU * i as f32 / RING_SEGMENTS as f32;
                    c + (u * t.cos() + v * t.sin()) * size
                };
                HandleLines {
                    handle: Handle::Ring(a),
                    segments: (0..RING_SEGMENTS)
                        .map(|i| (point(i), point(i + 1)))
                        .collect(),
                }
            })
            .collect(),
        Tool::Stretch => {
            let box_half = size * 0.05;
            let mut out = Vec::new();
            for axis in 0..3 {
                for positive in [true, false] {
                    let centre = face_point(object, axis, positive);
                    out.push(HandleLines {
                        handle: Handle::Face { axis, positive },
                        segments: cube_edges(centre, box_half),
                    });
                }
            }
            out
        }
    }
}

/// Centre of one face of the object's box, in world space.
fn face_point(object: &Object, axis: usize, positive: bool) -> Vec3 {
    let sign = if positive { 0.5 } else { -0.5 };
    object.transform().transform_point3(AXES[axis] * sign)
}

fn cube_edges(c: Vec3, h: f32) -> Vec<(Vec3, Vec3)> {
    let corner = |i: usize| {
        c + Vec3::new(
            if i & 1 == 0 { -h } else { h },
            if i & 2 == 0 { -h } else { h },
            if i & 4 == 0 { -h } else { h },
        )
    };
    let mut out = Vec::with_capacity(12);
    for a in 0..8usize {
        for bit in [1, 2, 4] {
            if a & bit == 0 {
                out.push((corner(a), corner(a | bit)));
            }
        }
    }
    out
}

/// Projects a world point to pixel coordinates (origin top-left), or `None` behind the camera.
pub fn to_screen(view_proj: Mat4, viewport_px: Vec2, p: Vec3) -> Option<Vec2> {
    let clip = view_proj * p.extend(1.0);
    if clip.w <= 1e-6 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(Vec2::new(
        (ndc.x + 1.0) * 0.5 * viewport_px.x,
        (1.0 - ndc.y) * 0.5 * viewport_px.y,
    ))
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 {
        ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + ab * t)
}

/// The handle under the cursor (in pixels), if any is within [`GRAB_RADIUS_PX`].
pub fn hit_test(
    handles: &[HandleLines],
    view_proj: Mat4,
    viewport_px: Vec2,
    cursor_px: Vec2,
) -> Option<Handle> {
    let mut best: Option<(f32, Handle)> = None;
    for h in handles {
        for &(a, b) in &h.segments {
            let (Some(sa), Some(sb)) = (
                to_screen(view_proj, viewport_px, a),
                to_screen(view_proj, viewport_px, b),
            ) else {
                continue;
            };
            let d = distance_to_segment(cursor_px, sa, sb);
            if d <= GRAB_RADIUS_PX && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, h.handle));
            }
        }
    }
    best.map(|(_, h)| h)
}

/// Parameter along the line `point + dir * s` closest to the ray, or `None` when the ray runs
/// almost parallel to the line (dragging would be unstable).
fn line_param(point: Vec3, dir: Vec3, ray_origin: Vec3, ray_dir: Vec3) -> Option<f32> {
    let w0 = point - ray_origin;
    let b = dir.dot(ray_dir);
    let denom = 1.0 - b * b;
    if denom < 1e-4 {
        return None;
    }
    Some((b * ray_dir.dot(w0) - dir.dot(w0)) / denom)
}

/// Where the ray meets the plane through `point` with normal `normal`.
fn plane_hit(point: Vec3, normal: Vec3, ray_origin: Vec3, ray_dir: Vec3) -> Option<Vec3> {
    let denom = normal.dot(ray_dir);
    if denom.abs() < 1e-4 {
        return None;
    }
    let t = normal.dot(point - ray_origin) / denom;
    Some(ray_origin + ray_dir * t)
}

/// Snapping applied while dragging.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    /// Positions and sizes snap to multiples of this, in millimetres.
    pub grid: f32,
    /// Rotations snap to multiples of this, in degrees.
    pub angle_degrees: f32,
}

fn snap_to(value: f32, step: f32) -> f32 {
    if step > 0.0 {
        (value / step).round() * step
    } else {
        value
    }
}

/// A drag in progress: the handle, the object as it was when the drag began, and where on the
/// handle it was grabbed.
#[derive(Clone, Debug)]
pub struct Drag {
    pub handle: Handle,
    start: Object,
    grab: Grab,
}

#[derive(Clone, Copy, Debug)]
enum Grab {
    Along(f32),
    Around(Vec3),
}

impl Drag {
    /// Starts dragging `handle` with the cursor ray. Returns `None` if the handle is seen edge-on
    /// and can't be dragged from this angle.
    pub fn start(handle: Handle, object: &Object, ray_origin: Vec3, ray_dir: Vec3) -> Option<Self> {
        let (point, dir) = Self::line(handle, object);
        let grab = match handle {
            Handle::Ring(a) => Grab::Around(plane_hit(point, AXES[a], ray_origin, ray_dir)?),
            _ => Grab::Along(line_param(point, dir, ray_origin, ray_dir)?),
        };
        Some(Self {
            handle,
            start: object.clone(),
            grab,
        })
    }

    /// The line a handle slides along (or the axis a ring turns around).
    fn line(handle: Handle, object: &Object) -> (Vec3, Vec3) {
        match handle {
            Handle::Axis(a) | Handle::Ring(a) => (object.position, AXES[a]),
            Handle::Face { axis, .. } => (object.position, object.rotation * AXES[axis]),
        }
    }

    /// The object as the drag would leave it with the cursor on this ray. Keeps the last result
    /// when the ray gives nothing usable.
    pub fn update(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        snap: Option<Snap>,
        both_sides: bool,
    ) -> Option<Object> {
        let start = &self.start;
        let (point, dir) = Self::line(self.handle, start);
        let mut out = start.clone();
        match (self.handle, self.grab) {
            (Handle::Axis(a), Grab::Along(s0)) => {
                let s = line_param(point, dir, ray_origin, ray_dir)?;
                let mut value = start.position[a] + (s - s0);
                if let Some(snap) = snap {
                    value = snap_to(value, snap.grid);
                }
                out.position[a] = value;
            }
            (Handle::Ring(_), Grab::Around(p0)) => {
                let p = plane_hit(point, dir, ray_origin, ray_dir)?;
                let (from, to) = (p0 - point, p - point);
                if from.length_squared() < 1e-12 || to.length_squared() < 1e-12 {
                    return None;
                }
                let mut angle = from.cross(to).dot(dir).atan2(from.dot(to));
                if let Some(snap) = snap {
                    angle = snap_to(angle.to_degrees(), snap.angle_degrees).to_radians();
                }
                out.rotation = (Quat::from_axis_angle(dir, angle) * start.rotation).normalize();
            }
            (Handle::Face { axis, positive }, Grab::Along(s0)) => {
                let s = line_param(point, dir, ray_origin, ray_dir)?;
                let sign = if positive { 1.0 } else { -1.0 };
                let growth = (s - s0) * sign * if both_sides { 2.0 } else { 1.0 };
                let mut size = start.size[axis] + growth;
                if let Some(snap) = snap {
                    size = snap_to(size, snap.grid);
                }
                size = size.max(MIN_SIZE_MM);
                out.size[axis] = size;
                if !both_sides {
                    // Keep the opposite face where it was.
                    out.position = start.position + dir * sign * (size - start.size[axis]) * 0.5;
                }
            }
            _ => return None,
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PrimKind, Scene};

    fn cube() -> Object {
        let mut scene = Scene::default();
        let id = scene.add_prim(PrimKind::Box, Vec3::ZERO);
        let mut o = scene.get(id).unwrap().clone();
        o.position = Vec3::ZERO;
        o
    }

    /// A ray from above pointing straight down at `(x, y)`.
    fn down_at(x: f32, y: f32) -> (Vec3, Vec3) {
        (Vec3::new(x, y, 500.0), Vec3::NEG_Z)
    }

    #[test]
    fn moving_along_x_follows_the_cursor() {
        let o = cube();
        let (ro, rd) = down_at(5.0, 3.0);
        let drag = Drag::start(Handle::Axis(0), &o, ro, rd).unwrap();
        let (ro, rd) = down_at(25.0, 40.0);
        let moved = drag.update(ro, rd, None, false).unwrap();
        // Only X changes, by how far the cursor moved along X.
        assert!((moved.position - Vec3::new(20.0, 0.0, 0.0)).length() < 1e-3);
    }

    #[test]
    fn moving_snaps_to_the_grid() {
        let o = cube();
        let (ro, rd) = down_at(0.0, 0.0);
        let drag = Drag::start(Handle::Axis(1), &o, ro, rd).unwrap();
        let (ro, rd) = down_at(0.0, 13.0);
        let snap = Snap {
            grid: 10.0,
            angle_degrees: 15.0,
        };
        let moved = drag.update(ro, rd, Some(snap), false).unwrap();
        assert_eq!(moved.position.y, 10.0);
    }

    #[test]
    fn dragging_along_the_view_direction_is_refused() {
        // Looking straight down the Z arrow there is no sensible drag.
        let (ro, rd) = down_at(0.0, 0.0);
        assert!(Drag::start(Handle::Axis(2), &cube(), ro, rd).is_none());
    }

    #[test]
    fn rotating_a_quarter_turn_about_z() {
        let o = cube();
        let (ro, rd) = down_at(10.0, 0.0);
        let drag = Drag::start(Handle::Ring(2), &o, ro, rd).unwrap();
        let (ro, rd) = down_at(0.0, 10.0);
        let turned = drag.update(ro, rd, None, false).unwrap();
        let x = turned.rotation * Vec3::X;
        assert!((x - Vec3::Y).length() < 1e-4, "{x}");
    }

    #[test]
    fn rotation_snaps_to_whole_steps() {
        let o = cube();
        let (ro, rd) = down_at(10.0, 0.0);
        let drag = Drag::start(Handle::Ring(2), &o, ro, rd).unwrap();
        // About 20 degrees, which snaps to 15.
        let a = 20f32.to_radians();
        let (ro, rd) = down_at(10.0 * a.cos(), 10.0 * a.sin());
        let snap = Snap {
            grid: 10.0,
            angle_degrees: 15.0,
        };
        let turned = drag.update(ro, rd, Some(snap), false).unwrap();
        let (axis, angle) = turned.rotation.to_axis_angle();
        assert!((angle.to_degrees() - 15.0).abs() < 1e-3 && axis.z > 0.99);
    }

    #[test]
    fn stretching_one_side_keeps_the_other_face_fixed() {
        let o = cube();
        let before = face_point(&o, 0, false);
        let (ro, rd) = down_at(25.0, 0.0);
        let drag = Drag::start(
            Handle::Face {
                axis: 0,
                positive: true,
            },
            &o,
            ro,
            rd,
        )
        .unwrap();
        let (ro, rd) = down_at(35.0, 0.0);
        let stretched = drag.update(ro, rd, None, false).unwrap();
        assert!((stretched.size.x - 60.0).abs() < 1e-3);
        assert!((face_point(&stretched, 0, false) - before).length() < 1e-3);
    }

    #[test]
    fn stretching_both_sides_keeps_the_centre() {
        let o = cube();
        let (ro, rd) = down_at(25.0, 0.0);
        let drag = Drag::start(
            Handle::Face {
                axis: 0,
                positive: true,
            },
            &o,
            ro,
            rd,
        )
        .unwrap();
        let (ro, rd) = down_at(35.0, 0.0);
        let stretched = drag.update(ro, rd, None, true).unwrap();
        assert!((stretched.size.x - 70.0).abs() < 1e-3);
        assert_eq!(stretched.position, o.position);
    }

    #[test]
    fn stretching_never_inverts_the_prim() {
        let o = cube();
        let (ro, rd) = down_at(25.0, 0.0);
        let drag = Drag::start(
            Handle::Face {
                axis: 0,
                positive: true,
            },
            &o,
            ro,
            rd,
        )
        .unwrap();
        let (ro, rd) = down_at(-500.0, 0.0);
        let stretched = drag.update(ro, rd, None, false).unwrap();
        assert_eq!(stretched.size.x, MIN_SIZE_MM);
    }

    #[test]
    fn hit_test_finds_the_arrow_under_the_cursor() {
        let o = cube();
        let cam = crate::OrbitCamera::default();
        let view_proj = cam.view_projection(1.0);
        let viewport = Vec2::new(800.0, 800.0);
        let hs = handles(Tool::Move, &o, cam.eye());
        let tip = o.position + Vec3::X * gizmo_size(&o, cam.eye()) * 0.6;
        let cursor = to_screen(view_proj, viewport, tip).unwrap() + Vec2::new(2.0, 2.0);
        assert_eq!(
            hit_test(&hs, view_proj, viewport, cursor),
            Some(Handle::Axis(0))
        );
        assert_eq!(
            hit_test(&hs, view_proj, viewport, Vec2::new(5.0, 5.0)),
            None
        );
    }
}
