#![forbid(unsafe_code)]

mod app;
mod events;
mod hotkeys;
mod platform;
mod view;

use eframe::egui;
use tracing_subscriber::filter::LevelFilter;

fn main() -> anyhow::Result<()> {
    init_logging();
    let store = app::open_store()?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pin")
            .with_inner_size([420.0, 560.0])
            .with_resizable(false)
            .with_always_on_top(),
        event_loop_builder: platform::event_loop_builder(),
        ..Default::default()
    };
    eframe::run_native(
        "Pin",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc, store)))),
    )
    .map_err(|err| anyhow::anyhow!("ui failed: {err}"))
}

fn init_logging() {
    let level = std::env::var("PIN_LOG")
        .ok()
        .and_then(|v| v.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::WARN);
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .init();
}
