#![allow(dead_code)]

mod app;
mod data;
mod gui;
mod log;
mod pipeline;

use app::NmrApp;

fn main() -> eframe::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    ::log::info!(
        "Starting NMR Spectral Processing GUI v{}",
        env!("CARGO_PKG_VERSION")
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            .with_maximized(true)
            .with_title("NMR Spectral Processing")
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "NMR Spectral Processing GUI",
        options,
        Box::new(|cc| Ok(Box::new(NmrApp::new(cc)))),
    )
}
