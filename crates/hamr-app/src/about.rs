//! The About window.

use eframe::egui;

/// The app icon (Prim Mjǫllnir), also used for the window icon.
pub const ICON_PNG: &[u8] = include_bytes!("../../../assets/icon/hamr-256.png");

#[derive(Default)]
pub struct About {
    pub open: bool,
    icon: Option<egui::TextureHandle>,
}

impl About {
    pub fn show(&mut self, ctx: &egui::Context) {
        let icon = self
            .icon
            .get_or_insert_with(|| {
                let data =
                    eframe::icon_data::from_png_bytes(ICON_PNG).expect("icon is a valid PNG");
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [data.width as usize, data.height as usize],
                    &data.rgba,
                );
                ctx.load_texture("hamr-icon", image, egui::TextureOptions::LINEAR)
            })
            .clone();
        egui::Window::new("About Hamr")
            .open(&mut self.open)
            .resizable(false)
            .collapsible(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Image::new(&icon).fit_to_exact_size(egui::vec2(96.0, 96.0)));
                    ui.vertical(|ui| {
                        ui.heading(format!("Hamr {}", env!("CARGO_PKG_VERSION")));
                        ui.label("Build 3D models from primitives.");
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(
                                "Old Norse hamr: a shape, a skin, the form a shapeshifter takes.\n\
                                 Its cousin hamarr is the hammer.",
                            )
                            .italics(),
                        );
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Licensed under MIT or Apache-2.0.");
                    ui.hyperlink_to("Source code", env!("CARGO_PKG_REPOSITORY"));
                });
            });
    }
}
