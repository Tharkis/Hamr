//! The Mirror and Array section of the Build panel.

use eframe::egui::{self, DragValue, Ui};
use glam::Vec3;
use hamr_core::Unit;

/// Settings kept between uses of the panel.
pub struct Options {
    /// Keep the original and mirror a copy.
    pub mirror_copy: bool,
    /// Mirror across planes through the world origin instead of through the selection's centre.
    pub mirror_at_origin: bool,
    pub circular: bool,
    /// Total number of items in the array, the original included.
    pub count: u32,
    /// Linear arrays: distance from one item to the next, in millimetres.
    pub offset: Vec3,
    /// Circular arrays: the world axis to turn around (0 = X, 1 = Y, 2 = Z).
    pub axis: usize,
    /// Circular arrays: a point on the axis, in millimetres.
    pub center: Vec3,
    /// Circular arrays: the angle the items are spread over, in degrees.
    pub angle: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mirror_copy: true,
            mirror_at_origin: true,
            circular: false,
            count: 3,
            offset: Vec3::new(60.0, 0.0, 0.0),
            axis: 2,
            center: Vec3::ZERO,
            angle: 360.0,
        }
    }
}

pub enum Action {
    Mirror {
        axis: usize,
        copy: bool,
        at_origin: bool,
    },
    Array,
}

pub fn show(ui: &mut Ui, options: &mut Options, unit: Unit, enabled: bool) -> Option<Action> {
    let mut action = None;
    ui.label("Mirror");
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal(|ui| {
            for (axis, name) in ["X", "Y", "Z"].into_iter().enumerate() {
                if ui
                    .button(format!("Across {name}"))
                    .on_hover_text(format!("Flip the selection along {name}."))
                    .clicked()
                {
                    action = Some(Action::Mirror {
                        axis,
                        copy: options.mirror_copy,
                        at_origin: options.mirror_at_origin,
                    });
                }
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label("Plane through");
        ui.selectable_value(&mut options.mirror_at_origin, true, "Origin");
        ui.selectable_value(&mut options.mirror_at_origin, false, "Selection");
    });
    ui.checkbox(&mut options.mirror_copy, "Keep the original");

    ui.add_space(4.0);
    ui.label("Array");
    ui.horizontal(|ui| {
        ui.selectable_value(&mut options.circular, false, "Linear");
        ui.selectable_value(&mut options.circular, true, "Circular");
    });
    let suffix = format!(" {}", unit.suffix());
    egui::Grid::new("array").num_columns(2).show(ui, |ui| {
        ui.label("Count");
        ui.add(DragValue::new(&mut options.count).range(2..=500));
        ui.end_row();
        if options.circular {
            ui.label("Axis");
            ui.horizontal(|ui| {
                for (axis, name) in ["X", "Y", "Z"].into_iter().enumerate() {
                    ui.selectable_value(&mut options.axis, axis, name);
                }
            });
            ui.end_row();
            ui.label("Centre");
            length_row(ui, &mut options.center, unit, &suffix);
            ui.end_row();
            ui.label("Angle");
            ui.add(
                DragValue::new(&mut options.angle)
                    .range(-360.0..=360.0)
                    .suffix("°"),
            );
            ui.end_row();
        } else {
            ui.label("Offset");
            length_row(ui, &mut options.offset, unit, &suffix);
            ui.end_row();
        }
    });
    if ui
        .add_enabled(enabled, egui::Button::new("Make array"))
        .on_hover_text("Add copies of the selection. The original counts as the first item.")
        .clicked()
    {
        action = Some(Action::Array);
    }
    action
}

/// Three length fields for X, Y and Z, shown in `unit` and stored in millimetres.
fn length_row(ui: &mut Ui, value: &mut Vec3, unit: Unit, suffix: &str) {
    ui.horizontal(|ui| {
        for (i, name) in ["X ", "Y ", "Z "].into_iter().enumerate() {
            let mut v = unit.from_mm(value[i]);
            let drag = DragValue::new(&mut v)
                .speed(0.5)
                .prefix(name)
                .suffix(suffix);
            if ui.add(drag).changed() {
                value[i] = unit.to_mm(v);
            }
        }
    });
}
