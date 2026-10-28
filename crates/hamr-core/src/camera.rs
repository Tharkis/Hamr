//! An orbit camera in the spirit of Second Life's camera controls: it always looks at a focus point
//! and can orbit around it, pan it, or zoom towards it.

use glam::{camera, Mat4, Vec2, Vec3};

/// Pitch is kept just short of straight up or down so the view never flips.
const MAX_PITCH: f32 = std::f32::consts::FRAC_PI_2 - 0.01;
const MIN_DISTANCE_MM: f32 = 1.0;
const MAX_DISTANCE_MM: f32 = 100_000.0;

#[derive(Clone, Debug, PartialEq)]
pub struct OrbitCamera {
    /// The point the camera looks at, in millimetres.
    pub focus: Vec3,
    /// Rotation around the world Z axis, in radians.
    pub yaw: f32,
    /// Angle above the XY plane, in radians.
    pub pitch: f32,
    /// Distance from the focus point, in millimetres.
    pub distance: f32,
    /// Vertical field of view, in radians.
    pub fov_y: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            yaw: -std::f32::consts::FRAC_PI_4 * 3.0,
            pitch: 0.5,
            distance: 300.0,
            fov_y: 45f32.to_radians(),
        }
    }
}

impl OrbitCamera {
    /// Position of the camera in world space.
    pub fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.focus + Vec3::new(cp * cy, cp * sy, sp) * self.distance
    }

    /// Unit vectors pointing right and up on screen, in world space.
    fn screen_axes(&self) -> (Vec3, Vec3) {
        let forward = (self.focus - self.eye()).normalize();
        let right = forward.cross(Vec3::Z).normalize();
        let up = right.cross(forward);
        (right, up)
    }

    /// Rotates around the focus point. `delta` is in radians.
    pub fn orbit(&mut self, delta: Vec2) {
        self.yaw -= delta.x;
        self.pitch = (self.pitch + delta.y).clamp(-MAX_PITCH, MAX_PITCH);
    }

    /// Moves the focus point parallel to the screen. `delta_px` is a mouse movement in pixels and
    /// `viewport_height_px` the viewport height, so the scene follows the cursor at the focus depth.
    pub fn pan(&mut self, delta_px: Vec2, viewport_height_px: f32) {
        let mm_per_px =
            2.0 * self.distance * (self.fov_y * 0.5).tan() / viewport_height_px.max(1.0);
        let (right, up) = self.screen_axes();
        self.focus += (-right * delta_px.x + up * delta_px.y) * mm_per_px;
    }

    /// Multiplies the distance to the focus point. Values below 1 move closer.
    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE_MM, MAX_DISTANCE_MM);
    }

    /// Makes `point` the new focus without moving the camera, like Alt-clicking in Second Life.
    pub fn focus_on(&mut self, point: Vec3) {
        let eye = self.eye();
        let offset = eye - point;
        let distance = offset.length();
        if distance < MIN_DISTANCE_MM {
            self.focus = point;
            return;
        }
        self.focus = point;
        self.distance = distance.min(MAX_DISTANCE_MM);
        self.pitch = (offset.z / distance).asin().clamp(-MAX_PITCH, MAX_PITCH);
        self.yaw = offset.y.atan2(offset.x);
    }

    pub fn view(&self) -> Mat4 {
        camera::rh::view::look_at_mat4(self.eye(), self.focus, Vec3::Z)
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        // Near and far planes scale with distance so both tiny parts and large scenes stay sharp.
        let near = (self.distance * 0.001).max(0.01);
        let far = self.distance * 1000.0 + 10_000.0;
        // wgpu uses the DirectX convention: depth from 0 to 1 with Y up.
        camera::rh::proj::directx::perspective(self.fov_y, aspect.max(0.01), near, far)
    }

    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }

    /// Returns a ray (origin, unit direction) through a point on screen, given in normalized device
    /// coordinates where (-1, -1) is the bottom-left corner.
    pub fn ray(&self, ndc: Vec2, aspect: f32) -> (Vec3, Vec3) {
        let inv = self.view_projection(aspect).inverse();
        let near = inv.project_point3(ndc.extend(0.0));
        let far = inv.project_point3(ndc.extend(1.0));
        (near, (far - near).normalize())
    }
}

/// Where a ray crosses the ground plane (Z = 0), if it does so in front of its origin.
pub fn ray_ground(origin: Vec3, dir: Vec3) -> Option<Vec3> {
    if dir.z.abs() < 1e-6 {
        return None;
    }
    let t = -origin.z / dir.z;
    (t > 0.0).then(|| origin + dir * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-2
    }

    #[test]
    fn eye_is_distance_from_focus() {
        let cam = OrbitCamera::default();
        assert!((cam.eye().distance(cam.focus) - cam.distance).abs() < 1e-3);
    }

    #[test]
    fn pitch_is_clamped() {
        let mut cam = OrbitCamera::default();
        cam.orbit(Vec2::new(0.0, 10.0));
        assert!(cam.pitch < std::f32::consts::FRAC_PI_2);
        assert!(cam.view().is_finite());
    }

    #[test]
    fn focus_on_keeps_the_eye_in_place() {
        let mut cam = OrbitCamera::default();
        let eye = cam.eye();
        cam.focus_on(Vec3::new(40.0, -20.0, 10.0));
        assert!(close(cam.eye(), eye));
        assert!(close(cam.focus, Vec3::new(40.0, -20.0, 10.0)));
    }

    #[test]
    fn pan_moves_focus_and_eye_together() {
        let mut cam = OrbitCamera::default();
        let offset = cam.eye() - cam.focus;
        cam.pan(Vec2::new(30.0, -12.0), 600.0);
        assert!(cam.focus != Vec3::ZERO);
        assert!(close(cam.eye() - cam.focus, offset));
    }

    #[test]
    fn ray_from_above_hits_the_ground() {
        let hit = ray_ground(Vec3::new(3.0, 4.0, 10.0), Vec3::NEG_Z).unwrap();
        assert_eq!(hit, Vec3::new(3.0, 4.0, 0.0));
        assert_eq!(ray_ground(Vec3::new(0.0, 0.0, 10.0), Vec3::Z), None);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut cam = OrbitCamera::default();
        cam.zoom(0.0);
        assert_eq!(cam.distance, MIN_DISTANCE_MM);
    }

    #[test]
    fn centre_ray_points_at_focus() {
        let cam = OrbitCamera::default();
        let (origin, dir) = cam.ray(Vec2::ZERO, 1.5);
        let to_focus = (cam.focus - origin).normalize();
        assert!(dir.dot(to_focus) > 0.9999);
    }
}
