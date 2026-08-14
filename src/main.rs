#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod models;
mod steam;
mod storage;
mod vdf;

use app::LaunchVaultApp;
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LaunchVault")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "LaunchVault",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(LaunchVaultApp::new(cc)))
        }),
    )
}