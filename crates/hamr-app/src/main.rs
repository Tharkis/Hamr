//! Hamr: build 3D models from primitives.

// Hide the console window in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod arrange_panel;
mod files;
mod library_panel;
mod object_panel;
mod print_check;
mod renderer;

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Hamr")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Hamr",
        options,
        Box::new(|cc| Ok(Box::new(app::HamrApp::new(cc)))),
    )
}
