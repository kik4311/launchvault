#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod models;
mod steam;
mod storage;
mod theme;
mod vdf;

use app::LaunchVaultApp;
use eframe::egui;
use std::sync::Arc;

fn load_icon() -> Arc<egui::IconData> {
    let png = include_bytes!("launchvault_icon.png");
    let img = image::load_from_memory(png).expect("failed to decode icon").to_rgba8();
    let (width, height) = img.dimensions();
    Arc::new(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default()        .default_filter_or("warn,egui_extras=trace,egui=info"))
        .format_timestamp(None)
        .init();
    #[cfg_attr(not(target_os = "windows"), allow(unused_mut))]
    let mut viewport = egui::ViewportBuilder::default()
        .with_app_id("launchvault")
        .with_title("LaunchVault")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([900.0, 600.0])
        .with_icon(load_icon());
    #[cfg(target_os = "windows")]
    {
        viewport = viewport.with_transparent(true);
    }
    let options = eframe::NativeOptions {
        viewport,
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