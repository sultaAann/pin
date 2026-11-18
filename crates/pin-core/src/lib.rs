//! Core of pin: clipboard history model, encrypted storage, search, pins and secrets.

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
