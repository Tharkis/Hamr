//! The Object panel: placement and shape parameters of the selected prim, laid out like the
//! Object tab of the Second Life build tools.

use std::ops::RangeInclusive;

use eframe::egui::{self, DragValue, Ui};
use glam::{Vec2, Vec3};
use hamr_core::{HollowShape, Object, PrimKind, PrimShape, Unit};

/// What the user did in the panel this frame.
pub enum Outcome {
    Unchanged,
    Changed,
    Delete,
}

/// Smallest size an object can be given, in millimetres.
const MIN_SIZE_MM: f32 = 0.1;

pub fn show(ui: &mut Ui, object: &mut Object, unit: Unit) -> Outcome {
    let mut changed = false;
    ui.heading("Object");
    ui.separator();

    egui::Grid::new("placement").num_columns(2).show(ui, |ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut object.name);
        ui.end_row();

        let suffix = format!(" {}", unit.suffix());
        let mut position = object.position / unit.mm_per_unit();
        if vec3_row(ui, "Position", &mut position, &suffix, None) {
            object.position = position * unit.mm_per_unit();
            changed = true;
        }
        let mut size = object.size / unit.mm_per_unit();
        let min = unit.from_mm(MIN_SIZE_MM);
        if vec3_row(ui, "Size", &mut size, &suffix, Some(min..=f32::MAX)) {
            object.size = size * unit.mm_per_unit();
            changed = true;
        }
        let mut rotation = object.rotation_degrees();
        if vec3_row(ui, "Rotation", &mut rotation, "°", None) {
            object.set_rotation_degrees(rotation);
            changed = true;
        }

        ui.label("Color");
        changed |= ui
            .color_edit_button_rgba_unmultiplied(&mut object.color)
            .changed();
        ui.end_row();

        ui.label("Negative");
        changed |= ui
            .checkbox(&mut object.negative, "Cut holes")
            .on_hover_text(
                "A negative prim is subtracted from the prims it overlaps when the model is \
                 merged for export.",
            )
            .changed();
        ui.end_row();
    });

    ui.separator();
    ui.strong("Shape");
    changed |= shape_editor(ui, &mut object.shape);

    ui.separator();
    if ui.button("Delete").clicked() {
        return Outcome::Delete;
    }
    if changed {
        object.shape = object.shape.clamped();
        Outcome::Changed
    } else {
        Outcome::Unchanged
    }
}

fn shape_editor(ui: &mut Ui, shape: &mut PrimShape) -> bool {
    let mut changed = false;
    let kind = shape.kind;
    egui::Grid::new("shape").num_columns(2).show(ui, |ui| {
        ui.label("Building block type");
        let mut new_kind = kind;
        egui::ComboBox::from_id_salt("kind")
            .selected_text(kind.name())
            .show_ui(ui, |ui| {
                for k in PrimKind::ALL {
                    ui.selectable_value(&mut new_kind, k, k.name());
                }
            });
        ui.end_row();
        if new_kind != kind {
            *shape = PrimShape::new(new_kind);
            changed = true;
        }

        changed |= cut_row(ui, "Path cut", &mut shape.path_cut);

        ui.label("Hollow");
        let mut hollow = shape.hollow * 100.0;
        if ui
            .add(
                DragValue::new(&mut hollow)
                    .range(0.0..=95.0)
                    .speed(0.5)
                    .suffix(" %"),
            )
            .changed()
        {
            shape.hollow = hollow / 100.0;
            changed = true;
        }
        ui.end_row();

        ui.label("Hollow shape");
        egui::ComboBox::from_id_salt("hollow_shape")
            .selected_text(shape.hollow_shape.name())
            .show_ui(ui, |ui| {
                for h in HollowShape::ALL {
                    changed |= ui
                        .selectable_value(&mut shape.hollow_shape, h, h.name())
                        .changed();
                }
            });
        ui.end_row();

        let twist_limit = if kind.is_linear() { 180.0 } else { 360.0 };
        changed |= pair_row(
            ui,
            "Twist",
            &mut shape.twist,
            ["B", "E"],
            -twist_limit..=twist_limit,
            1.0,
            "°",
        );

        if kind != PrimKind::Sphere {
            changed |= vec2_row(ui, "Taper", &mut shape.taper, -1.0..=1.0);
            changed |= vec2_row(ui, "Top shear", &mut shape.top_shear, -0.5..=0.5);
        }
        match kind {
            PrimKind::Box | PrimKind::Cylinder | PrimKind::Prism => {
                changed |= cut_row(ui, "Slice", &mut shape.slice);
            }
            PrimKind::Sphere => {
                changed |= cut_row(ui, "Dimple", &mut shape.dimple);
            }
            PrimKind::Torus | PrimKind::Tube | PrimKind::Ring => {
                changed |= cut_row(ui, "Profile cut", &mut shape.profile_cut);
                let mut hole = [shape.hole_size.x, shape.hole_size.y];
                ui.label("Hole size");
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(fraction(&mut hole[0], 0.05..=1.0).prefix("X "))
                        .changed();
                    changed |= ui
                        .add(fraction(&mut hole[1], 0.05..=0.5).prefix("Y "))
                        .changed();
                });
                ui.end_row();
                shape.hole_size = Vec2::from(hole);
                changed |= scalar_row(ui, "Radius offset", &mut shape.radius_offset, -1.0..=1.0);
                changed |= scalar_row(ui, "Revolutions", &mut shape.revolutions, 1.0..=4.0);
                changed |= scalar_row(ui, "Skew", &mut shape.skew, -0.95..=0.95);
            }
        }
    });
    changed
}

fn fraction(value: &mut f32, range: RangeInclusive<f32>) -> DragValue<'_> {
    DragValue::new(value)
        .range(range)
        .speed(0.005)
        .fixed_decimals(3)
}

fn scalar_row(ui: &mut Ui, label: &str, value: &mut f32, range: RangeInclusive<f32>) -> bool {
    ui.label(label);
    let changed = ui.add(fraction(value, range)).changed();
    ui.end_row();
    changed
}

/// A begin/end pair in 0..1, like SL's cut, slice and dimple fields.
fn cut_row(ui: &mut Ui, label: &str, cut: &mut [f32; 2]) -> bool {
    pair_row(ui, label, cut, ["B", "E"], 0.0..=1.0, 0.005, "")
}

fn vec2_row(ui: &mut Ui, label: &str, value: &mut Vec2, range: RangeInclusive<f32>) -> bool {
    let mut pair = value.to_array();
    let changed = pair_row(ui, label, &mut pair, ["X", "Y"], range, 0.005, "");
    *value = Vec2::from(pair);
    changed
}

fn pair_row(
    ui: &mut Ui,
    label: &str,
    pair: &mut [f32; 2],
    names: [&str; 2],
    range: RangeInclusive<f32>,
    speed: f64,
    suffix: &str,
) -> bool {
    let mut changed = false;
    ui.label(label);
    ui.horizontal(|ui| {
        for (value, name) in pair.iter_mut().zip(names) {
            changed |= ui
                .add(
                    DragValue::new(value)
                        .range(range.clone())
                        .speed(speed)
                        .max_decimals(3)
                        .prefix(format!("{name} "))
                        .suffix(suffix),
                )
                .changed();
        }
    });
    ui.end_row();
    changed
}

fn vec3_row(
    ui: &mut Ui,
    label: &str,
    value: &mut Vec3,
    suffix: &str,
    range: Option<RangeInclusive<f32>>,
) -> bool {
    let mut changed = false;
    ui.label(label);
    ui.horizontal(|ui| {
        for (component, name) in [&mut value.x, &mut value.y, &mut value.z]
            .into_iter()
            .zip(["X", "Y", "Z"])
        {
            let mut drag = DragValue::new(component)
                .speed(0.1)
                .max_decimals(2)
                .prefix(format!("{name} "))
                .suffix(suffix);
            if let Some(range) = &range {
                drag = drag.range(range.clone());
            }
            changed |= ui.add(drag).changed();
        }
    });
    ui.end_row();
    changed
}
