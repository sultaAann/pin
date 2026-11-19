<div align="center">

<img src="assets/logo.svg" width="112" height="112" alt="pin">

# pin

**A fast, private clipboard manager for the Mac.**

Everything you copy, one keystroke away — encrypted, searchable, yours.

[Download for macOS](https://github.com/sultaAann/pin/releases/latest) ·
[Install](#install) ·
[Shortcuts](#shortcuts) ·
[Build from source](#build-from-source)

</div>

---

## What it does

- **Remembers what you copy.** Text, links, rich text, images and files land in a history
  you open with <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>C</kbd>.
- **Finds it again.** Type to search; filter by text, links, images, files or pinned items.
- **Keeps the important bits.** Pin an entry to keep it on top and give it its own hotkey,
  <kbd>⌘</kbd> <kbd>⌃</kbd> <kbd>1</kbd>–<kbd>9</kbd>, that copies it without opening the window.
- **Locks away secrets.** Passwords and tokens go to a separate vault behind a master
  password. They never show up in history or search.
- **Stays private.** History is encrypted on disk with a key kept in the macOS keychain.
  Nothing leaves your Mac.
- **Stays out of the way.** No Dock icon, near-zero CPU when idle, starts at login if you
  want it to.

## Install

1. Download **Pin.zip** from the [latest release](https://github.com/sultaAann/pin/releases/latest),
   unzip it and move **Pin.app** to **Applications**.
2. Open Pin. It isn't notarized by Apple yet, so macOS blocks it the first time. Click
   **Open Anyway** in **System Settings → Privacy & Security**, or run once:

   ```sh
   xattr -dr com.apple.quarantine /Applications/Pin.app
   ```

3. When macOS asks to let Pin use its key in the keychain, choose **Always Allow**.

Press <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>C</kbd> to open it. To start Pin with your Mac, turn on
**Launch at Login** in the **⋯** menu.

To update, replace Pin.app with the new version.

## Shortcuts

| Keys | Action |
|---|---|
| <kbd>⌘</kbd> <kbd>⇧</kbd> <kbd>C</kbd> | Open or close Pin |
| <kbd>↑</kbd> <kbd>↓</kbd>, <kbd>↩</kbd> | Choose an entry, copy it |
| <kbd>⌘</kbd> <kbd>1</kbd>–<kbd>9</kbd> | Copy one of the first nine entries |
| <kbd>Tab</kbd> | Next filter |
| <kbd>⌘</kbd> <kbd>↩</kbd> | All actions for the entry |
| <kbd>⌘</kbd> <kbd>P</kbd> | Pin or unpin |
| <kbd>⌘</kbd> <kbd>S</kbd> | Move to secrets |
| <kbd>⌘</kbd> <kbd>⌫</kbd> | Delete |
| <kbd>/</kbd> | Open secrets; <kbd>⌫</kbd> goes back |
| <kbd>⌘</kbd> <kbd>⌃</kbd> <kbd>1</kbd>–<kbd>9</kbd> | Copy a pinned slot from anywhere |
| <kbd>Esc</kbd> | Clear the search, then close |

## Secrets

Open secrets with <kbd>/</kbd> or the **Secrets** button. The first time, pick a master
password; Pin then shows a **recovery key** once. Keep it somewhere safe: it is the only way
back in if you forget the password.

Move a copied password into the vault with <kbd>⌘</kbd> <kbd>S</kbd>. Copying a secret
hides it from other clipboard managers and clears the clipboard after 90 seconds. The vault
locks itself after 5 minutes without use.

## Platforms

| | Status |
|---|---|
| macOS 12+ (Apple Silicon and Intel) | Works |
| Linux | In progress |
| Windows | Planned |

## Build from source

You need [Rust](https://rustup.rs); the version is pinned in `rust-toolchain.toml`.

```sh
cargo run -p pin-app       # run the app
cargo test --workspace     # run the tests
scripts/bundle-macos.sh    # build dist/Pin.app and dist/Pin.zip
```

On Linux, the UI also needs `libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev`.

The code is split into `pin-core` (storage, search, secrets), `pin-clipboard` (system
clipboard), `pin-app` (the window) and `pin-cli`. Releases are described in
[docs/releasing.md](docs/releasing.md).

## License

[MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
