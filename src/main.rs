//! Warust - WhatsApp Desktop Client
//!
//! A native WhatsApp client built with Rust, egui, and whatsapp-rust.

mod app;
pub mod autostart;
mod backend;
mod commands;
pub mod export;
mod avatars;
pub mod media;
pub mod search;
mod state;
mod storage;
mod theme;
pub mod tray;
mod ui;
mod util;

use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("warust=debug".parse()?)
            .add_directive("whatsapp_rust=info".parse()?))
        .with(tracing_subscriber::fmt::layer()
            .with_target(false)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true))
        .init();

    tracing::info!("Starting Warust v{}", env!("CARGO_PKG_VERSION"));

    let start_in_tray = std::env::args().any(|a| a == "--tray" || a == "--minimized");

    // Run the egui application
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Warust")
            .with_visible(!start_in_tray)
            .with_icon(load_icon()),
        renderer: eframe::Renderer::Wgpu,
        persist_window: true,
        centered: true,
        ..Default::default()
    };

    // Use map_err to convert eframe::Error to anyhow::Error
    eframe::run_native(
        "Warust",
        native_options,
        Box::new(|cc| Ok(Box::new(app::WarustApp::new(cc)?))),
    ).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    Ok(())
}

fn load_icon() -> egui::IconData {
    let icon_bytes = include_bytes!("../assets/icon.png");
    if let Ok(image) = image::load_from_memory(icon_bytes) {
        let image = image.to_rgba8();
        let (width, height) = image.dimensions();
        egui::IconData {
            rgba: image.into_raw(),
            width,
            height,
        }
    } else {
        egui::IconData {
            rgba: vec![0, 0, 0, 0],
            width: 1,
            height: 1,
        }
    }
}