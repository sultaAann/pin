#![forbid(unsafe_code)]

mod app;
mod autostart;
mod events;
mod hotkeys;
mod platform;
mod view;

use eframe::egui;
use tracing_subscriber::filter::LevelFilter;

fn main() -> anyhow::Result<()> {
    init_logging();
    let database = app::open_database()?;
    let background = std::env::args().any(|arg| arg == autostart::BACKGROUND_ARG);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pin")
            .with_visible(!background)
            .with_inner_size(view::WINDOW_SIZE)
            .with_decorations(false)
            .with_transparent(true)
            .with_has_shadow(true)
            .with_resizable(false)
            .with_always_on_top(),
        centered: true,
        event_loop_builder: platform::event_loop_builder(),
        ..Default::default()
    };
    eframe::run_native(
        "Pin",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, database, background)))),
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
