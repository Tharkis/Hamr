//! Length units. Internally every length is stored as `f32` millimetres.

/// A unit the user can choose for display and input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Unit {
    #[default]
    Millimetre,
    Centimetre,
    Metre,
    Inch,
}

impl Unit {
    pub const ALL: [Unit; 4] = [Unit::Millimetre, Unit::Centimetre, Unit::Metre, Unit::Inch];

    /// How many millimetres one of this unit is.
    pub fn mm_per_unit(self) -> f32 {
        match self {
            Unit::Millimetre => 1.0,
            Unit::Centimetre => 10.0,
            Unit::Metre => 1000.0,
            Unit::Inch => 25.4,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Unit::Millimetre => "mm",
            Unit::Centimetre => "cm",
            Unit::Metre => "m",
            Unit::Inch => "in",
        }
    }

    pub fn from_mm(self, mm: f32) -> f32 {
        mm / self.mm_per_unit()
    }

    pub fn to_mm(self, value: f32) -> f32 {
        value * self.mm_per_unit()
    }

    /// Formats a millimetre length in this unit, e.g. `"12.70 mm"` or `"0.50 in"`.
    pub fn format(self, mm: f32) -> String {
        format!("{:.2} {}", self.from_mm(mm), self.suffix())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_every_unit() {
        for unit in Unit::ALL {
            let mm = 123.4;
            assert!((unit.to_mm(unit.from_mm(mm)) - mm).abs() < 1e-3, "{unit:?}");
        }
    }

    #[test]
    fn formats_inches() {
        assert_eq!(Unit::Inch.format(25.4), "1.00 in");
    }
}
