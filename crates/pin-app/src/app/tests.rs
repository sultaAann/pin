use std::sync::mpsc;

use eframe::egui::{self, Event, Modifiers, RawInput, Rect, pos2, vec2};
use pin_core::{Format, FormatKind, KdfParams, Key, MasterPassword, NewSecret, Options, Store};

use super::*;

fn app(dir: &std::path::Path) -> App {
    let mut options = Options::default();
    options.vault.kdf = KdfParams {
        memory_kib: 64,
        iterations: 1,
        parallelism: 1,
    };
    let mut store = Store::open(
        dir.join("pin.db"),
        &Key::from_bytes(&mut [7; Key::LEN]),
        options,
    )
    .unwrap();
    let _recovery = store
        .vault()
        .create(&MasterPassword::new("pw".into()).unwrap())
        .unwrap();
    store
        .vault()
        .add(NewSecret {
            name: "url".into(),
            note: String::new(),
            formats: vec![Format::new(FormatKind::Text, "hunter2")],
        })
        .unwrap();
    let (_tx, events) = mpsc::channel();
    App {
        store,
        clipboard: None,
        events,
        query: PREFIX.to_string(),
        filter: Filter::All,
        entries: Vec::new(),
        preview: None,
        actions: None,
        selected: 0,
        focus: Focus::Search,
        error: None,
        scroll_to_selected: false,
        secrets: Vec::new(),
        vault_status: VaultStatus::Locked,
        sheet: None,
        sheet_error: None,
        revealed: None,
        alt_hold: vault::AltHold::Up,
        shown_until: None,
        lock_at: None,
        pending_clear: None,
        quiet_until: None,
        forget_text_state: false,
        thumbs: thumbs::Thumbnails::default(),
        window: Window::Shown { had_focus: true },
        reopen: Reopen::Hotkey,
        launch_at_login: None,
        quitting: false,
        _hotkeys: None,
        watch: None,
    }
}

fn frame(ctx: &egui::Context, app: &mut App, events: Vec<Event>) {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(780.0, 480.0))),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        app.tick(ui.ctx());
        app.show_ui(ui);
    });
    out.textures_delta.clear();
}

#[test]
fn holding_alt_reveals_the_selected_secret() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    app.refresh();
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, Vec::new());
    assert!(app.revealed.is_none());
    let alt = Modifiers {
        alt: true,
        ..Modifiers::NONE
    };
    frame(&ctx, &mut app, vec![Event::ModifiersChanged(alt)]);
    std::thread::sleep(std::time::Duration::from_millis(300));
    frame(&ctx, &mut app, Vec::new());
    let shown = app
        .revealed
        .as_ref()
        .and_then(|r| r.text.as_deref().cloned());
    assert_eq!(shown.as_deref(), Some("hunter2"), "error: {:?}", app.error);
    frame(
        &ctx,
        &mut app,
        vec![Event::ModifiersChanged(Modifiers::NONE)],
    );
    assert!(app.revealed.is_none());
}

#[test]
fn show_button_reveals_until_clicked_again() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    app.refresh();
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, Vec::new());
    let click = |app: &mut App, pos| {
        for pressed in [true, false] {
            let event = Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            frame(&ctx, app, vec![Event::PointerMoved(pos), event]);
        }
        frame(&ctx, app, Vec::new());
    };
    // The Show button sits at the right of the value well in the preview.
    let button = pos2(720.0, 197.0);
    click(&mut app, button);
    let shown = app
        .revealed
        .as_ref()
        .and_then(|r| r.text.as_deref().cloned());
    assert_eq!(shown.as_deref(), Some("hunter2"), "error: {:?}", app.error);
    click(&mut app, button);
    assert!(app.revealed.is_none());
}

fn backspace() -> Event {
    Event::Key {
        key: egui::Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn backspace_leaves_secrets_even_from_a_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    app.refresh();
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, Vec::new());
    let id = app.secrets[0].id;
    app.start_delete(id);
    frame(&ctx, &mut app, Vec::new());
    frame(&ctx, &mut app, vec![backspace()]);
    assert!(!app.secrets_mode());
    assert!(app.sheet.is_none());
}

#[test]
fn unsaved_recovery_key_keeps_secrets_open() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app(dir.path());
    app.refresh();
    app.sheet = Some(Sheet::RecoveryKey {
        key: zeroize::Zeroizing::new("AAAA-BBBB".to_owned()),
        then: None,
        saved: false,
    });
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, Vec::new());
    frame(&ctx, &mut app, vec![backspace()]);
    assert!(app.secrets_mode());
    assert!(app.sheet.is_some());
}
