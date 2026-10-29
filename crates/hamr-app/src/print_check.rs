//! The printability check window.

use eframe::egui::{self, DragValue};
use hamr_core::merge;
use hamr_core::printability::{self, Issue, Printer};
use hamr_core::{Scene, Unit};

#[derive(Default)]
pub struct PrintCheck {
    pub open: bool,
    printer: Printer,
    /// The last result, cleared when settings change so it is never stale.
    report: Option<Result<Vec<Issue>, String>>,
}

impl PrintCheck {
    /// Opens the window with a fresh check of `scene`.
    pub fn open(&mut self, scene: &Scene) {
        self.open = true;
        self.run(scene);
    }

    fn run(&mut self, scene: &Scene) {
        self.report = Some(
            merge::merge(&scene.objects)
                .map(|model| printability::check(&model, &self.printer))
                .map_err(|e| e.to_string()),
        );
    }

    pub fn show(&mut self, ctx: &egui::Context, scene: &Scene, unit: Unit) {
        let mut open = self.open;
        let mut recheck = false;
        egui::Window::new("Printability check")
            .open(&mut open)
            .resizable(false)
            .default_width(360.0)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                match &self.report {
                    None => {}
                    Some(Err(e)) => {
                        ui.colored_label(ui.visuals().error_fg_color, format!("Can't check: {e}."));
                    }
                    Some(Ok(issues)) if issues.is_empty() => {
                        ui.colored_label(
                            egui::Color32::from_rgb(90, 200, 120),
                            "No problems found. The merged model is watertight and should print \
                             without support.",
                        );
                    }
                    Some(Ok(issues)) => {
                        ui.label("The merged model is watertight. Before printing, note:");
                        for issue in issues {
                            ui.colored_label(ui.visuals().warn_fg_color, format!("• {issue}"));
                        }
                    }
                }
                ui.separator();
                ui.label("Printer");
                let suffix = format!(" {}", unit.suffix());
                let mut changed = false;
                egui::Grid::new("printer").num_columns(2).show(ui, |ui| {
                    ui.label("Build volume");
                    ui.horizontal(|ui| {
                        for i in 0..3 {
                            let mut v = unit.from_mm(self.printer.build_volume[i]);
                            let drag = DragValue::new(&mut v)
                                .range(unit.from_mm(1.0)..=unit.from_mm(5000.0))
                                .suffix(&suffix);
                            if ui.add(drag).changed() {
                                self.printer.build_volume[i] = unit.to_mm(v);
                                changed = true;
                            }
                        }
                    });
                    ui.end_row();
                    ui.label("Steepest overhang");
                    changed |= ui
                        .add(
                            DragValue::new(&mut self.printer.max_overhang_degrees)
                                .range(0.0..=89.0)
                                .suffix("°"),
                        )
                        .on_hover_text("Measured from vertical. Most printers manage 45°.")
                        .changed();
                    ui.end_row();
                    ui.label("Smallest piece");
                    let mut v = unit.from_mm(self.printer.min_feature);
                    if ui
                        .add(
                            DragValue::new(&mut v)
                                .speed(0.05)
                                .range(0.0..=unit.from_mm(100.0))
                                .suffix(&suffix),
                        )
                        .changed()
                    {
                        self.printer.min_feature = unit.to_mm(v);
                        changed = true;
                    }
                    ui.end_row();
                });
                recheck = changed || ui.button("Check again").clicked();
            });
        self.open = open;
        if recheck {
            self.run(scene);
        }
    }
}
