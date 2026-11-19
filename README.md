# pin

Fast, private clipboard manager for macOS, Linux and Windows.

pin keeps a searchable history of everything you copy, stores it encrypted on disk,
lets you pin items so they never expire, bind them to hotkeys, and keep secrets
behind a master password.

> Status: early development. Works on macOS; Linux support is in progress, Windows later.

## Features (planned)

- Clipboard history for everything the OS clipboard carries: plain text, rich text/HTML, images, files
- Encrypted at rest; the key lives in the OS keychain
- Secrets: a separate vault unlocked with a master password, never indexed or shown in plain history
- Instant full-text search
- Pins: pinned items stay on top, survive history limits, can get their own paste hotkey
- Global hotkey popup, near-zero CPU and memory when idle
- CLI: `pin list`, `pin search`, `pin copy`

## Install (macOS)

1. Download `Pin.zip` from the latest [release](https://github.com/sultaAann/pin/releases/latest) and unzip it.
2. Move `Pin.app` to `/Applications`.
3. Open it. Pin isn't notarized by Apple, so the first time macOS refuses to open it: either
   go to System Settings → Privacy & Security and click Open Anyway, or run
   `xattr -dr com.apple.quarantine /Applications/Pin.app` once.
4. When macOS asks for your password to let Pin use its key in the keychain, choose
   Always Allow.

Press Cmd+Shift+C to open Pin. Turn on Launch at Login in the ⋯ menu at the bottom right:
Pin then starts hidden when you log in, waiting for the hotkey.

To update, replace `Pin.app` with the new version.

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

### Releases

Pushing a tag `v*` runs `.github/workflows/release.yml`: it builds a universal `Pin.app`
(Apple Silicon and Intel) with `scripts/bundle-macos.sh` and attaches `Pin.zip` to a GitHub
release. Build the same locally with `scripts/bundle-macos.sh`, or
`TARGETS=aarch64-apple-darwin scripts/bundle-macos.sh` for this Mac only.

### Release signing

macOS remembers keychain access per app signature. Ad-hoc signed releases get a new signature
every version, so users are asked for their password once after each update. A signing
certificate of your own keeps the signature stable:

1. In Keychain Access, choose Certificate Assistant → Create a Certificate. Pick a name
   (for example `Pin Release`), Identity Type Self-Signed Root, Certificate Type Code Signing.
2. Export the certificate with its private key as a `.p12` file with a password.
3. Add three repository secrets: `MACOS_CERT_P12` (the file as base64:
   `base64 -i cert.p12 | pbcopy`), `MACOS_CERT_PASSWORD` and `MACOS_CERT_NAME` (the name from
   step 1).

The release workflow signs with it whenever these secrets exist and falls back to ad-hoc
signing otherwise.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
