#![forbid(unsafe_code)]

use eframe::egui;
use tracing_subscriber::filter::LevelFilter;

fn main() -> eframe::Result {
    init_logging();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([420.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native("Pin", options, Box::new(|_cc| Ok(Box::<App>::default())))
}

#[derive(Default)]
struct App;

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Pin");
            ui.label(format!("v{}", pin_core::version()));
        });
    }
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
