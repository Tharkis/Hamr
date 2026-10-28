//! The ground grid drawn on the XY plane.

use glam::Vec3;

/// One line segment with a colour, ready to upload to the GPU.
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    /// Distance between minor lines, in millimetres.
    pub spacing: f32,
    /// Every `major_every`-th line is drawn brighter.
    pub major_every: u32,
    /// Number of minor cells from the origin to each edge.
    pub half_cells: u32,
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            spacing: 10.0,
            major_every: 10,
            half_cells: 50,
        }
    }
}

const MINOR: [f32; 4] = [0.35, 0.35, 0.38, 0.5];
const MAJOR: [f32; 4] = [0.55, 0.55, 0.6, 0.8];
const X_AXIS: [f32; 4] = [0.9, 0.25, 0.25, 1.0];
const Y_AXIS: [f32; 4] = [0.3, 0.8, 0.3, 1.0];
const Z_AXIS: [f32; 4] = [0.3, 0.45, 0.95, 1.0];

impl Grid {
    /// Builds line-list vertices (two per segment) for the grid plus the X, Y and Z axes.
    pub fn lines(&self) -> Vec<LineVertex> {
        let n = self.half_cells as i32;
        let extent = n as f32 * self.spacing;
        let mut out = Vec::new();
        // Long lines are split into one segment per cell. Short segments clip more reliably on
        // some drivers and let the distance fade work per segment.
        let mut push = |a: Vec3, b: Vec3, color: [f32; 4]| {
            let pieces = ((b - a).length() / self.spacing).round().max(1.0) as u32;
            for k in 0..pieces {
                let p0 = a.lerp(b, k as f32 / pieces as f32);
                let p1 = a.lerp(b, (k + 1) as f32 / pieces as f32);
                out.push(LineVertex {
                    position: p0.into(),
                    color,
                });
                out.push(LineVertex {
                    position: p1.into(),
                    color,
                });
            }
        };
        for i in -n..=n {
            let t = i as f32 * self.spacing;
            if i == 0 {
                // The axes replace the centre lines on the positive side.
                push(Vec3::new(-extent, 0.0, 0.0), Vec3::ZERO, MAJOR);
                push(Vec3::new(0.0, -extent, 0.0), Vec3::ZERO, MAJOR);
                continue;
            }
            let color = if i.unsigned_abs() % self.major_every.max(1) == 0 {
                MAJOR
            } else {
                MINOR
            };
            push(Vec3::new(t, -extent, 0.0), Vec3::new(t, extent, 0.0), color);
            push(Vec3::new(-extent, t, 0.0), Vec3::new(extent, t, 0.0), color);
        }
        push(Vec3::ZERO, Vec3::new(extent, 0.0, 0.0), X_AXIS);
        push(Vec3::ZERO, Vec3::new(0.0, extent, 0.0), Y_AXIS);
        push(Vec3::ZERO, Vec3::new(0.0, 0.0, extent * 0.5), Z_AXIS);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_come_in_pairs() {
        let lines = Grid::default().lines();
        assert_eq!(lines.len() % 2, 0);
        assert!(!lines.is_empty());
    }
}
