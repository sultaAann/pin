#![forbid(unsafe_code)]
#![expect(clippy::print_stdout)]

use tracing_subscriber::filter::LevelFilter;

fn main() {
    init_logging();
    println!("pin {}", pin_core::version());
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
