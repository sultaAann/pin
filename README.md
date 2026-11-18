# pin

Fast, private clipboard manager for macOS, Linux and Windows.

pin keeps a searchable history of everything you copy, stores it encrypted on disk,
lets you pin items so they never expire, bind them to hotkeys, and keep secrets
behind a master password.

> Status: early development.

## Features (planned)

- Clipboard history for everything the OS clipboard carries: plain text, rich text/HTML, images, files
- Encrypted at rest; the key lives in the OS keychain
- Secrets: a separate vault unlocked with a master password, never indexed or shown in plain history
- Instant full-text search
- Pins: pinned items stay on top, survive history limits, can get their own paste hotkey
- Global hotkey popup, near-zero CPU and memory when idle
- CLI: `pin list`, `pin search`, `pin copy`

## Layout

| Crate | Purpose |
|---|---|
| `crates/pin-core` | Domain model, encrypted storage, search, pins, secrets |
| `crates/pin-app` | Desktop UI (egui/eframe) |
| `crates/pin-cli` | Command-line interface, binary `pin` |

## Development

Requires stable Rust (see `rust-toolchain.toml`).

```sh
cargo run -p pin-app        # desktop app
cargo run -p pin-cli        # CLI
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Linux build dependencies for the UI: `libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev`.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
