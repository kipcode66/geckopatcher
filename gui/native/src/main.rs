#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod app;
pub mod progress;
pub use app::PatcherApp;
use eframe::Theme;
use egui::Vec2;

const ICON: &[u8; 0x47D11] = include_bytes!("../assets/icon.png");

pub(crate) fn load_icon() -> eframe::IconData {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(ICON)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };

    eframe::IconData {
        rgba: icon_rgba,
        width: icon_width,
        height: icon_height,
    }
}

// When compiling natively:
fn main() -> eframe::Result<()> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    // let native_options = eframe::NativeOptions::default();
    let native_options = eframe::NativeOptions {
        icon_data: Some(load_icon()),
        drag_and_drop_support: true,
        centered: true,
        follow_system_theme: true,
        initial_window_size: Some(Vec2::new(300., 200.)),
        min_window_size: Some(Vec2::new(280., 220.)),
        default_theme: Theme::Dark,
        run_and_return: false,
        ..Default::default()
    };
    eframe::run_native(
        "Romhack Patcher",
        native_options,
        Box::new(|cc| Box::new(app::PatcherApp::new(cc))),
    )
}
