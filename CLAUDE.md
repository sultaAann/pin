# pin

Cross-platform (macOS, Linux, Windows) clipboard manager in Rust: encrypted history, search, pins with hotkeys, secrets vault. Desktop UI on egui/eframe plus a CLI.

## Workspace

- `crates/pin-core` — owns all domain state: entries, storage, crypto, search, pins, secrets. No UI or platform clipboard code here.
- `crates/pin-app` — egui/eframe desktop app; only renders state and forwards user actions to pin-core.
- `crates/pin-cli` — binary `pin`; thin layer over pin-core.
- Platform clipboard watching/writing goes in its own crate (`crates/pin-clipboard`) when added, with per-OS modules behind `cfg(target_os)`.
- A new feature gets its own module; `lib.rs`/`main.rs` only wire modules together.

## Commands

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p pin-app
cargo run -p pin-cli
```

Before finishing a change: fmt, clippy with `-D warnings`, tests must pass.

## Design decisions

- Content types: everything the OS clipboard carries — plain text, rich text/HTML, images, file lists. Store all representations of one copy event as one entry with several formats.
- Storage: SQLCipher (rusqlite `bundled-sqlcipher`; CommonCrypto on macOS, vendored OpenSSL elsewhere), raw 32-byte key, WAL, `secure_delete = ON`, `temp_store = MEMORY`. Plaintext never touches disk (no temp files, no unencrypted caches or thumbnails). Write paths use IMMEDIATE transactions because pin-app and pin-cli share the database.
- Search: FTS5 trigram over plain text or decoded file paths (first 64 KiB) for queries of 3+ chars; 1–2 char queries scan previews with Unicode case folding.
- Ordering: pinned first, then `use_seq` (monotonic counter bumped on copy/paste), not wall-clock time.
- Key: random data key generated on first run, stored in the OS keychain (`keyring`: Keychain / Secret Service / Credential Manager).
- Secrets: separate vault, encrypted with a key derived from a master password (Argon2id). Secrets are never put into the search index, history list, logs or the CLI output unless explicitly unlocked. Zeroize key material and decrypted secrets after use.
- Pins: pinned entries are exempt from history limits and sorted on top; a pin can have a paste hotkey.
- Dedup: identical content (by hash) bumps the existing entry instead of inserting a new one.

## Performance budget

pin runs all the time, so idle cost matters more than peak speed.

- Idle CPU ~0%: event-driven clipboard watching where the OS supports it (Windows `AddClipboardFormatListener`, Wayland data-control, X11 XFixes); on macOS poll only `NSPasteboard.changeCount` (cheap integer) at a low interval.
- The UI window is created on hotkey and does not repaint when hidden; no continuous repaint.
- Load large payloads (images, files) lazily; the list shows previews/thumbnails, full data is decrypted only on use.
- Search must stay interactive on 10k+ entries; avoid decrypting the whole history per keystroke.
- Release profile: thin LTO, `codegen-units = 1`, stripped.

## Git

- No Linear for this repo: do not use Linear tools; branch names are `type/short-name` without a task key.

## Code rules

- Rust 2024 edition, stable toolchain, workspace lints apply to every crate (`[lints] workspace = true`).
- No `unwrap`/`expect` outside tests; errors via `thiserror` in libraries, `anyhow` only in binaries.
- No comments except a short WHY for a non-obvious constraint or workaround.
- Shared dependency versions go in `[workspace.dependencies]`.
- Never log clipboard contents or secrets, even at debug level.
