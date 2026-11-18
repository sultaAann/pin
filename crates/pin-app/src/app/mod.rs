mod actions;
mod preview;
mod thumbs;
mod vault;

use std::sync::mpsc::{self, Receiver};

use anyhow::Context as _;
use eframe::egui;
use pin_clipboard::{Clipboard, DEFAULT_INTERVAL, Privacy, WatchHandle};
use pin_core::{
    Entry, EntryId, Filter, Keychain, Options, PinSlot, Secret, SecretId, Store, VaultStatus,
};

pub use self::actions::{Actions, EntryAction, Item, Page, entry_items};

use crate::events::{Event, Events};
use crate::hotkeys::{Action, Hotkeys};
use crate::platform;

use self::vault::PendingClear;
pub use self::vault::{LabelTarget, PREFIX, Sheet, SheetAction};

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
    MoveToVault(EntryId),
    CopySecret(SecretId),
    EditSecret(SecretId),
    DeleteSecret(SecretId),
    Sheet(SheetAction),
    Hide,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    None,
    Search,
    Sheet,
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
    pub(crate) filter: Filter,
    pub(crate) entries: Vec<Entry>,
    preview: Option<preview::Preview>,
    pub(crate) actions: Option<Actions>,
    pub(crate) selected: usize,
    pub(crate) focus: Focus,
    pub(crate) error: Option<String>,
    pub(crate) scroll_to_selected: bool,
    pub(crate) secrets: Vec<Secret>,
    pub(crate) vault_status: VaultStatus,
    pub(crate) sheet: Option<Sheet>,
    pub(crate) sheet_error: Option<String>,
    pending_clear: Option<PendingClear>,
    quiet_until: Option<std::time::Instant>,
    pub(crate) forget_text_state: bool,
    thumbs: thumbs::Thumbnails,
    window: Window,
    reopen: Reopen,
    quitting: bool,
    _hotkeys: Option<Hotkeys>,
    watch: Option<WatchHandle>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, store: Store) -> Self {
        crate::view::install(&cc.egui_ctx);
        place_window(&cc.egui_ctx);
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
        let thumbs = thumbs::Thumbnails::spawn(events.clone());
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
            filter: Filter::All,
            entries: Vec::new(),
            preview: None,
            actions: None,
            selected: 0,
            focus: Focus::Search,
            error: None,
            scroll_to_selected: true,
            secrets: Vec::new(),
            vault_status: VaultStatus::Locked,
            sheet: None,
            sheet_error: None,
            pending_clear: None,
            quiet_until: None,
            forget_text_state: false,
            thumbs,
            reopen,
            window: Window::Shown { had_focus: false },
            quitting: false,
            _hotkeys: hotkeys,
            watch,
        };
        app.refresh();
        app
    }

    /// Returns whether `target` should grab keyboard focus this frame.
    pub(crate) fn take_focus(&mut self, target: Focus) -> bool {
        if self.focus == target {
            self.focus = Focus::None;
            true
        } else {
            false
        }
    }

    pub(crate) fn secrets_mode(&self) -> bool {
        self.query.starts_with(PREFIX)
    }

    pub(crate) fn len(&self) -> usize {
        if self.secrets_mode() {
            self.secrets.len()
        } else {
            self.entries.len()
        }
    }

    pub(crate) fn refresh(&mut self) {
        if self.secrets_mode() {
            if let Err(err) = self.refresh_secrets() {
                self.report(&err);
            }
            self.selected = self.selected.min(self.len().saturating_sub(1));
            return;
        }
        self.leave_secrets();
        match self.store.search(&self.query, self.filter, LIST_LIMIT) {
            Ok(entries) => self.entries = entries,
            Err(err) => self.report(&err),
        }
        self.prune_thumbnails();
        self.prune_preview();
        self.prune_actions();
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    pub(crate) fn set_filter(&mut self, filter: Filter) {
        if self.filter != filter {
            self.filter = filter;
            self.selected = 0;
            self.scroll_to_selected = true;
            self.refresh();
        }
    }

    pub(crate) fn run(&mut self, ctx: &egui::Context, command: Command) {
        let result = match command {
            Command::Copy(id) => self.copy(id).map(|()| self.hide(ctx)),
            Command::SetPinned(id, pinned) => self.store.set_pinned(id, pinned).map_err(Into::into),
            Command::SetSlot(id, slot) => self.store.set_slot(id, slot).map_err(Into::into),
            Command::Delete(id) => self.store.delete(id).map_err(Into::into),
            Command::MoveToVault(id) => self.start_move(id),
            Command::CopySecret(id) => self.copy_secret(id).map(|()| self.hide(ctx)),
            Command::EditSecret(id) => {
                self.start_edit(id);
                Ok(())
            }
            Command::DeleteSecret(id) => {
                self.start_delete(id);
                Ok(())
            }
            Command::Sheet(action) => {
                self.sheet_action(action);
                Ok(())
            }
            Command::Hide => {
                self.hide(ctx);
                Ok(())
            }
            Command::Quit if self.recovery_key_open() => Ok(()),
            Command::Quit => {
                self.clear_pending_secret();
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
                // Listing secrets counts as vault use; background copies must not hold it unlocked.
                if !self.secrets_mode() {
                    self.refresh_if_shown();
                }
            }
            Event::Thumbnail(id, thumb) => self.thumbnail_ready(ctx, id, thumb),
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
                if !self.secrets_mode() {
                    self.refresh_if_shown();
                }
            }
        }
    }

    fn copy(&mut self, id: EntryId) -> Result<(), Box<dyn std::error::Error>> {
        let formats = self.store.formats(id)?;
        let written = self.write_clipboard(&formats, Privacy::Normal)?;
        tracing::debug!(%id, kinds = ?formats.iter().map(|f| f.kind.as_str()).collect::<Vec<_>>(), ?written, "copied entry to clipboard");
        self.store.touch(id)?;
        Ok(())
    }

    fn write_clipboard(
        &self,
        formats: &[pin_core::Format],
        privacy: Privacy,
    ) -> Result<pin_clipboard::ChangeCount, pin_clipboard::Error> {
        let clipboard = self
            .clipboard
            .as_ref()
            .ok_or(pin_clipboard::Error::Unsupported)?;
        let written = clipboard.write(formats, privacy)?;
        if let Some(watch) = &self.watch {
            watch.ignore(written);
        }
        Ok(written)
    }

    fn show(&mut self, ctx: &egui::Context) {
        self.window = Window::Shown { had_focus: false };
        self.focus = Focus::Search;
        self.query.clear();
        self.filter = Filter::All;
        self.actions = None;
        self.error = None;
        self.selected = 0;
        self.scroll_to_selected = true;
        self.refresh();
        place_window(ctx);
        platform::reveal();
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    fn hide(&mut self, ctx: &egui::Context) {
        platform::yield_focus();
        self.actions = None;
        self.drop_preview();
        if self.reopen == Reopen::Dock {
            self.window = Window::Shown { had_focus: false };
            return;
        }
        self.window = Window::Hidden;
        self.drop_thumbnails();
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

fn place_window(ctx: &egui::Context) {
    if let Some(origin) = platform::popup_origin(crate::view::WINDOW_SIZE) {
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(origin));
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.events.try_recv() {
            self.handle(ctx, event);
        }
        self.tick(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show_ui(ui);
    }

    fn on_exit(&mut self) {
        self.clear_pending_secret();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        crate::view::transparent()
    }
}
