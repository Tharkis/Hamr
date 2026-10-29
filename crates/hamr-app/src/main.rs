//! Hamr: build 3D models from primitives.

// Hide the console window in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod about;
mod app;
mod arrange_panel;
mod files;
mod library_panel;
mod object_panel;
mod print_check;
mod renderer;

/// Matches the desktop file and Flatpak ID, so Wayland compositors find the icon.
const APP_ID: &str = "io.github.tharkis.Hamr";

fn main() -> eframe::Result {
    env_logger::init();
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Hamr")
        .with_app_id(APP_ID)
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([640.0, 400.0]);
    match eframe::icon_data::from_png_bytes(about::ICON_PNG) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => log::warn!("could not load the window icon: {e}"),
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Hamr",
        options,
        Box::new(|cc| Ok(Box::new(app::HamrApp::new(cc)))),
    )
}
