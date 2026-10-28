//! Parametric primitives modelled on the Second Life build tools.
//!
//! Every prim is a 2D *profile* swept along a *path*. Box, cylinder and prism sweep their profile
//! straight up (a linear path); sphere, torus, tube and ring sweep it around the Z axis (a circular
//! path). The parameters below follow the names and ranges of the SL build tools' Object tab, so
//! anyone who knows SL can find their way around. The geometry is generated here from those
//! documented behaviours; no SL viewer code is used.
//!
//! Meshes are generated in a unit box centred on the origin (−0.5..0.5 on each axis) and scaled to
//! the object's size by its transform.

mod profile;
mod sweep;

use glam::Vec2;

use crate::{Mesh, Solid};

/// Normals across edges sharper than this stay hard; smoother edges are shaded smooth.
const CREASE_DEGREES: f32 = 40.0;

/// The seven SL building block types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimKind {
    Box,
    Cylinder,
    Prism,
    Sphere,
    Torus,
    Tube,
    Ring,
}

impl PrimKind {
    pub const ALL: [PrimKind; 7] = [
        PrimKind::Box,
        PrimKind::Cylinder,
        PrimKind::Prism,
        PrimKind::Sphere,
        PrimKind::Torus,
        PrimKind::Tube,
        PrimKind::Ring,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PrimKind::Box => "Box",
            PrimKind::Cylinder => "Cylinder",
            PrimKind::Prism => "Prism",
            PrimKind::Sphere => "Sphere",
            PrimKind::Torus => "Torus",
            PrimKind::Tube => "Tube",
            PrimKind::Ring => "Ring",
        }
    }

    /// Box, cylinder and prism sweep their profile in a straight line.
    pub fn is_linear(self) -> bool {
        matches!(self, PrimKind::Box | PrimKind::Cylinder | PrimKind::Prism)
    }

    /// Torus, tube and ring have the full set of circular-path parameters.
    pub fn is_ring_like(self) -> bool {
        matches!(self, PrimKind::Torus | PrimKind::Tube | PrimKind::Ring)
    }

    /// The outline of the profile that gets swept.
    fn outline(self) -> HollowShape {
        match self {
            PrimKind::Box | PrimKind::Tube => HollowShape::Square,
            PrimKind::Prism | PrimKind::Ring => HollowShape::Triangle,
            PrimKind::Cylinder | PrimKind::Sphere | PrimKind::Torus => HollowShape::Circle,
        }
    }
}

/// The shape of the hole left by a non-zero hollow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum HollowShape {
    /// The same shape as the prim's own profile.
    #[default]
    Same,
    Circle,
    Square,
    Triangle,
}

impl HollowShape {
    pub const ALL: [HollowShape; 4] = [
        HollowShape::Same,
        HollowShape::Circle,
        HollowShape::Square,
        HollowShape::Triangle,
    ];

    pub fn name(self) -> &'static str {
        match self {
            HollowShape::Same => "Default",
            HollowShape::Circle => "Circle",
            HollowShape::Square => "Square",
            HollowShape::Triangle => "Triangle",
        }
    }
}

/// Smallest allowed gap between the begin and end of any cut, as in SL.
pub const MIN_CUT_SPAN: f32 = 0.02;
/// Largest hollow, as a fraction of the profile size.
pub const MAX_HOLLOW: f32 = 0.95;

/// All shape parameters of a prim. Which ones apply depends on [`PrimKind`]; the rest are ignored.
#[derive(Clone, Debug, PartialEq)]
pub struct PrimShape {
    pub kind: PrimKind,
    /// Begin and end of the path cut, 0..1. On box, cylinder and prism this cuts the profile like a
    /// pie; on the circular types it cuts the sweep around the Z axis.
    pub path_cut: [f32; 2],
    /// Fraction of the profile removed from the middle, 0..0.95.
    pub hollow: f32,
    pub hollow_shape: HollowShape,
    /// Rotation of the profile at the begin and end of the path, in degrees. Linear types allow
    /// −180..180, circular types −360..360.
    pub twist: [f32; 2],
    /// −1..1. Positive values shrink the end of the path, negative values the beginning.
    pub taper: Vec2,
    /// Offset of the end of the path relative to the beginning, −0.5..0.5.
    pub top_shear: Vec2,
    /// Box, cylinder, prism: begin and end along the height, 0..1.
    pub slice: [f32; 2],
    /// Sphere: begin and end of the cut from the bottom pole to the top pole, 0..1.
    pub dimple: [f32; 2],
    /// Torus, tube, ring: begin and end of the profile cut, 0..1.
    pub profile_cut: [f32; 2],
    /// Torus, tube, ring: X is the profile's height (0.05..1), Y its thickness (0.05..0.5).
    pub hole_size: Vec2,
    /// Torus, tube, ring: how far the path drifts outwards (or inwards) by the end, −1..1.
    pub radius_offset: f32,
    /// Torus, tube, ring: number of turns, 1..4. More than one makes a coil.
    pub revolutions: f32,
    /// Torus, tube, ring: sideways drift of the path along X by the end, −0.95..0.95.
    pub skew: f32,
}

impl PrimShape {
    /// A prim with SL's default parameters for `kind`.
    pub fn new(kind: PrimKind) -> Self {
        Self {
            kind,
            path_cut: [0.0, 1.0],
            hollow: 0.0,
            hollow_shape: HollowShape::Same,
            twist: [0.0, 0.0],
            taper: Vec2::ZERO,
            top_shear: Vec2::ZERO,
            slice: [0.0, 1.0],
            dimple: [0.0, 1.0],
            profile_cut: [0.0, 1.0],
            hole_size: if kind.is_ring_like() {
                Vec2::new(1.0, 0.25)
            } else {
                Vec2::ONE
            },
            radius_offset: 0.0,
            revolutions: 1.0,
            skew: 0.0,
        }
    }

    /// Returns a copy with every parameter forced into its valid range, and parameters the kind
    /// does not have (such as taper on a sphere) reset to their defaults.
    pub fn clamped(&self) -> Self {
        let mut out = self.clamped_ranges();
        let defaults = PrimShape::new(self.kind);
        if self.kind == PrimKind::Sphere {
            out.taper = defaults.taper;
            out.top_shear = defaults.top_shear;
        }
        if !self.kind.is_ring_like() {
            out.hole_size = defaults.hole_size;
            out.radius_offset = defaults.radius_offset;
            out.revolutions = defaults.revolutions;
            out.skew = defaults.skew;
        }
        out
    }

    fn clamped_ranges(&self) -> Self {
        let twist_limit = if self.kind.is_linear() { 180.0 } else { 360.0 };
        let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
        let clamp = |v: f32, lo: f32, hi: f32, fallback: f32| finite(v, fallback).clamp(lo, hi);
        Self {
            kind: self.kind,
            path_cut: clamp_cut(self.path_cut),
            hollow: clamp(self.hollow, 0.0, MAX_HOLLOW, 0.0),
            hollow_shape: self.hollow_shape,
            twist: self.twist.map(|t| clamp(t, -twist_limit, twist_limit, 0.0)),
            taper: Vec2::new(
                clamp(self.taper.x, -1.0, 1.0, 0.0),
                clamp(self.taper.y, -1.0, 1.0, 0.0),
            ),
            top_shear: Vec2::new(
                clamp(self.top_shear.x, -0.5, 0.5, 0.0),
                clamp(self.top_shear.y, -0.5, 0.5, 0.0),
            ),
            slice: clamp_cut(self.slice),
            dimple: clamp_cut(self.dimple),
            profile_cut: clamp_cut(self.profile_cut),
            hole_size: Vec2::new(
                clamp(self.hole_size.x, 0.05, 1.0, 1.0),
                clamp(self.hole_size.y, 0.05, 0.5, 0.25),
            ),
            radius_offset: clamp(self.radius_offset, -1.0, 1.0, 0.0),
            revolutions: clamp(self.revolutions, 1.0, 4.0, 1.0),
            skew: clamp(self.skew, -0.95, 0.95, 0.0),
        }
    }

    /// Builds the prim's closed surface in the unit box.
    pub fn solid(&self) -> Solid {
        sweep::build(&self.clamped())
    }

    /// Builds the prim's surface in the unit box, with vertices split for shading.
    pub fn mesh(&self) -> Mesh {
        self.solid().to_mesh(CREASE_DEGREES)
    }
}

/// Keeps `begin <= end - MIN_CUT_SPAN` inside 0..1, moving `begin` if they collide.
fn clamp_cut([begin, end]: [f32; 2]) -> [f32; 2] {
    let end = if end.is_finite() {
        end.clamp(MIN_CUT_SPAN, 1.0)
    } else {
        1.0
    };
    let begin = if begin.is_finite() { begin } else { 0.0 };
    [begin.clamp(0.0, end - MIN_CUT_SPAN), end]
}

#[cfg(test)]
mod tests;
