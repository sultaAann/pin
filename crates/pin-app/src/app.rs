use std::sync::mpsc::{self, Receiver};

use anyhow::Context as _;
use eframe::egui;
use pin_clipboard::{Clipboard, DEFAULT_INTERVAL, Privacy, WatchHandle};
use pin_core::{Entry, EntryId, Keychain, Options, PinSlot, Store};

use crate::events::{Event, Events};
use crate::hotkeys::{Action, Hotkeys};
use crate::platform;

const LIST_LIMIT: usize = 200;

pub fn open_store() -> anyhow::Result<Store> {
    let path = pin_core::default_database_path().context("locate history database")?;
    let keychain = Keychain::new(Keychain::DATABASE_ACCOUNT).context("open keychain")?;
    Store::open_with_keys(&path, &keychain, Options::default()).context("open history database")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Copy(EntryId),
    SetPinned(EntryId, bool),
    SetSlot(EntryId, Option<PinSlot>),
    Delete(EntryId),
    Hide,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Window {
    Hidden,
    Shown { had_focus: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reopen {
    Hotkey,
    Dock,
}

pub struct App {
    store: Store,
    clipboard: Option<Clipboard>,
    events: Receiver<Event>,
    pub(crate) query: String,
    pub(crate) entries: Vec<Entry>,
    pub(crate) selected: usize,
    pub(crate) focus_search: bool,
    pub(crate) error: Option<String>,
    pub(crate) scroll_to_selected: bool,
    window: Window,
    reopen: Reopen,
    quitting: bool,
    _hotkeys: Option<Hotkeys>,
    watch: Option<WatchHandle>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, store: Store) -> Self {
        let (tx, rx) = mpsc::channel();
        let events = Events::new(tx, cc.egui_ctx.clone());
        let watch = {
            let events = events.clone();
            pin_clipboard::watch(DEFAULT_INTERVAL, move |entry| {
                events.send(Event::Captured(entry));
            })
        };
        let watch = watch
            .inspect_err(|err| tracing::warn!(%err, "clipboard history disabled"))
            .ok();
        let hotkeys = Hotkeys::register(events)
            .inspect_err(|err| tracing::warn!(%err, "global hotkeys disabled"))
            .ok();
        let reopen = if hotkeys.as_ref().is_some_and(Hotkeys::has_toggle) {
            Reopen::Hotkey
        } else {
            platform::show_in_dock();
            Reopen::Dock
        };
        let clipboard = Clipboard::general()
            .inspect_err(|err| tracing::warn!(%err, "copying back disabled"))
            .ok();
        let mut app = Self {
            store,
            clipboard,
            events: rx,
            query: String::new(),
            entries: Vec::new(),
            selected: 0,
            focus_search: true,
            error: None,
            scroll_to_selected: true,
            reopen,
            window: Window::Shown { had_focus: false },
            quitting: false,
            _hotkeys: hotkeys,
            watch,
        };
        app.refresh();
        app
    }

    pub(crate) fn refresh(&mut self) {
        let query = self.query.trim();
        let entries = if query.is_empty() {
            self.store.recent(LIST_LIMIT, 0)
        } else {
            self.store.search(query, LIST_LIMIT)
        };
        match entries {
            Ok(entries) => self.entries = entries,
            Err(err) => self.report(&err),
        }
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    pub(crate) fn run(&mut self, ctx: &egui::Context, command: Command) {
        let result = match command {
            Command::Copy(id) => self.copy(id).map(|()| self.hide(ctx)),
            Command::SetPinned(id, pinned) => self.store.set_pinned(id, pinned).map_err(Into::into),
            Command::SetSlot(id, slot) => self.store.set_slot(id, slot).map_err(Into::into),
            Command::Delete(id) => self.store.delete(id).map_err(Into::into),
            Command::Hide => {
                self.hide(ctx);
                Ok(())
            }
            Command::Quit => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                Ok(())
            }
        };
        match result {
            Ok(()) => self.error = None,
            Err(err) => self.report(&*err),
        }
        self.refresh();
    }

    pub(crate) fn track_focus(&mut self, ctx: &egui::Context) {
        match (self.window, ctx.input(|i| i.viewport().focused)) {
            (Window::Shown { .. }, Some(true)) => self.window = Window::Shown { had_focus: true },
            (Window::Shown { had_focus: true }, Some(false)) => self.hide(ctx),
            _ => {}
        }
    }

    pub(crate) fn intercept_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide(ctx);
        }
    }

    fn handle(&mut self, ctx: &egui::Context, event: Event) {
        match event {
            Event::Captured(entry) => {
                if let Err(err) = self.store.insert(entry) {
                    self.report(&err);
                }
                self.refresh_if_shown();
            }
            Event::Hotkey(Action::Toggle) => match self.window {
                Window::Hidden => self.show(ctx),
                Window::Shown { had_focus: false } => {
                    platform::reveal();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                Window::Shown { had_focus: true } => self.hide(ctx),
            },
            Event::Hotkey(Action::Slot(slot)) => {
                let copied = match self.store.by_slot(slot) {
                    Ok(Some(entry)) => self.copy(entry.id),
                    Ok(None) => Ok(()),
                    Err(err) => Err(err.into()),
                };
                if let Err(err) = copied {
                    self.report(&*err);
                }
                self.refresh_if_shown();
            }
        }
    }

    fn copy(&mut self, id: EntryId) -> Result<(), Box<dyn std::error::Error>> {
        let Some(clipboard) = &self.clipboard else {
            return Err(Box::new(pin_clipboard::Error::Unsupported));
        };
        let formats = self.store.formats(id)?;
        let written = clipboard.write(&formats, Privacy::Normal)?;
        if let Some(watch) = &self.watch {
            watch.ignore(written);
        }
        self.store.touch(id)?;
        Ok(())
    }

    fn show(&mut self, ctx: &egui::Context) {
        self.window = Window::Shown { had_focus: false };
        self.focus_search = true;
        self.query.clear();
        self.error = None;
        self.selected = 0;
        self.scroll_to_selected = true;
        self.refresh();
        platform::reveal();
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn hide(&mut self, ctx: &egui::Context) {
        platform::yield_focus();
        if self.reopen == Reopen::Dock {
            self.window = Window::Shown { had_focus: false };
            return;
        }
        self.window = Window::Hidden;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }

    fn refresh_if_shown(&mut self) {
        if self.window != Window::Hidden {
            self.refresh();
        }
    }

    fn report(&mut self, err: &dyn std::error::Error) {
        tracing::warn!(%err, "action failed");
        self.error = Some(err.to_string());
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.events.try_recv() {
            self.handle(ctx, event);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show_ui(ui);
    }
}
