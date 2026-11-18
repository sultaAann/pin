use std::time::{Duration, Instant, SystemTime};

use eframe::egui;
use pin_clipboard::{ChangeCount, Privacy};
use pin_core::{
    EntryId, Format, FormatKind, MasterPassword, RecoveryKey, SecretId, SecretLabel, VaultStatus,
};
use zeroize::Zeroizing;

use super::App;

pub const PREFIX: char = '/';
const CLEAR_AFTER: Duration = Duration::from_secs(90);
const FIELD_CAPACITY: usize = 256;
// Instant pauses while the Mac sleeps, so wake-ups are capped and wall-clock deadlines rechecked.
const MAX_WAIT: Duration = Duration::from_secs(30);
const QUIET_AFTER_KDF: Duration = Duration::from_millis(400);
// Option also types characters such as @ or [ on many layouts, so a quick press must not reveal.
const ALT_REVEAL_AFTER: Duration = Duration::from_millis(250);
// A value shown with a click hides itself, so it isn't left on screen.
const SHOWN_FOR: Duration = Duration::from_secs(30);

type Field = Zeroizing<String>;
type ActionResult = Result<(), Box<dyn std::error::Error>>;

pub enum Sheet {
    Create {
        password: Field,
        confirm: Field,
        then: Option<EntryId>,
    },
    Unlock {
        password: Field,
        then: Option<EntryId>,
    },
    Recover {
        key: Field,
        password: Field,
        confirm: Field,
        then: Option<EntryId>,
    },
    RecoveryKey {
        key: Field,
        then: Option<EntryId>,
        /// The user ticked that the key is stored; the sheet can't be left before that.
        saved: bool,
    },
    Label {
        target: LabelTarget,
        name: Field,
        note: Field,
    },
    ConfirmDelete {
        id: SecretId,
    },
}

impl Sheet {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Create { .. } => "create",
            Self::Unlock { .. } => "unlock",
            Self::Recover { .. } => "recover",
            Self::RecoveryKey { .. } => "recovery-key",
            Self::Label { .. } => "label",
            Self::ConfirmDelete { .. } => "delete",
        }
    }

    fn opened_by_prefix(&self) -> bool {
        matches!(
            self,
            Self::Unlock { then: None, .. }
                | Self::Create { then: None, .. }
                | Self::Recover { then: None, .. }
        )
    }

    fn shows_secret_data(&self) -> bool {
        matches!(
            self,
            Self::Label {
                target: LabelTarget::Edit(_),
                ..
            } | Self::ConfirmDelete { .. }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelTarget {
    Move(EntryId),
    Edit(SecretId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetAction {
    Submit,
    Cancel,
    Forgot,
    Back,
    CopyRecoveryKey,
}

/// A secret's value shown while the user holds the reveal control; `text` is `None` for images.
pub struct Revealed {
    pub id: SecretId,
    pub text: Option<Field>,
}

/// How the Alt key is being held, for revealing with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AltHold {
    Up,
    Since(Instant),
    /// Another key was pressed during this hold, so it is typing, not a reveal.
    Typing,
}

pub(super) struct PendingClear {
    count: ChangeCount,
    at: Instant,
    wall: SystemTime,
}

impl PendingClear {
    fn due(&self, now: Instant) -> bool {
        now >= self.at || SystemTime::now() >= self.wall
    }
}

fn field() -> Field {
    Zeroizing::new(String::with_capacity(FIELD_CAPACITY))
}

#[derive(Debug, thiserror::Error)]
enum Input {
    #[error("passwords don't match")]
    Mismatch,
}

impl App {
    pub(super) fn refresh_secrets(&mut self) -> pin_core::Result<()> {
        let filter: Vec<char> = self
            .secret_query()
            .trim()
            .chars()
            .flat_map(char::to_lowercase)
            .collect();
        let mut vault = self.store.vault();
        self.vault_status = vault.status()?;
        if self.vault_status != VaultStatus::Unlocked {
            self.hide_value();
            self.secrets.clear();
            if self.sheet.as_ref().is_none_or(Sheet::shows_secret_data) {
                self.open_sheet(entry_sheet(self.vault_status, None));
            }
            return Ok(());
        }
        let mut secrets = vault.secrets()?;
        if !filter.is_empty() {
            secrets
                .retain(|s| contains_folded(&s.name, &filter) || contains_folded(&s.note, &filter));
        }
        self.secrets = secrets;
        Ok(())
    }

    // Unlock and setup prompts that only exist because "/" was typed go away with it.
    pub(super) fn leave_secrets(&mut self) {
        self.hide_value();
        self.secrets.clear();
        let stale = |s: &Sheet| s.opened_by_prefix() || s.shows_secret_data();
        if self.sheet.as_ref().is_some_and(stale) {
            self.set_sheet(None);
        }
    }

    pub(super) fn start_delete(&mut self, id: SecretId) {
        self.open_sheet(Sheet::ConfirmDelete { id });
    }

    pub(super) fn start_move(&mut self, entry: EntryId) -> ActionResult {
        let status = self.store.vault().status()?;
        let sheet = if status == VaultStatus::Unlocked {
            Self::move_sheet(entry)
        } else {
            entry_sheet(status, Some(entry))
        };
        self.open_sheet(sheet);
        Ok(())
    }

    pub(super) fn start_edit(&mut self, id: SecretId) {
        if let Some(secret) = self.secrets.iter().find(|s| s.id == id) {
            let sheet = Sheet::Label {
                target: LabelTarget::Edit(id),
                name: Zeroizing::new(secret.name.clone()),
                note: Zeroizing::new(secret.note.clone()),
            };
            self.open_sheet(sheet);
        }
    }

    pub(super) fn copy_secret(&mut self, id: SecretId) -> ActionResult {
        let formats = self.store.vault().formats(id)?;
        self.copy_concealed(&formats)
    }

    pub(super) fn sheet_action(&mut self, action: SheetAction) {
        let result = match action {
            SheetAction::Submit => self.submit(),
            SheetAction::Cancel => {
                self.cancel_sheet();
                Ok(())
            }
            SheetAction::Forgot => {
                let then = match &self.sheet {
                    Some(Sheet::Unlock { then, .. }) => *then,
                    _ => None,
                };
                self.open_sheet(Sheet::Recover {
                    key: field(),
                    password: field(),
                    confirm: field(),
                    then,
                });
                Ok(())
            }
            SheetAction::Back => {
                let then = match &self.sheet {
                    Some(Sheet::Recover { then, .. }) => *then,
                    _ => None,
                };
                self.open_sheet(Sheet::Unlock {
                    password: field(),
                    then,
                });
                Ok(())
            }
            SheetAction::CopyRecoveryKey => self.copy_recovery_key(),
        };
        if let Err(err) = result {
            tracing::debug!(%err, "vault action failed");
            self.sheet_error = Some(match err.downcast_ref::<pin_core::Error>() {
                Some(pin_core::Error::WrongPassword) => {
                    "Wrong password. Try again or use your recovery key.".to_owned()
                }
                _ => capitalize(&err.to_string()),
            });
        }
    }

    pub(super) fn tick(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if let Some(pending) = &self.pending_clear {
            if pending.due(now) {
                self.clear_pending_secret();
            } else {
                ctx.request_repaint_after((pending.at - now).min(MAX_WAIT));
            }
        }
        if self.store.vault().lock_if_idle() {
            self.on_locked();
        }
        self.lock_at = self.store.vault().lock_deadline();
        if let Some(deadline) = self.lock_at {
            ctx.request_repaint_after(deadline.saturating_duration_since(now).min(MAX_WAIT));
        }
    }

    pub(super) fn lock_now(&mut self) {
        self.store.vault().lock();
        self.lock_at = None;
        self.on_locked();
    }

    /// Time left before the vault locks itself if it stays unused.
    pub(crate) fn locks_in(&self) -> Option<Duration> {
        self.lock_at
            .map(|at| at.saturating_duration_since(Instant::now()))
    }

    /// Time left before a copied secret is cleared from the clipboard, and that as a share of
    /// the whole wait.
    pub(crate) fn clears_in(&self) -> Option<(Duration, f32)> {
        let pending = self.pending_clear.as_ref()?;
        let left = pending.at.checked_duration_since(Instant::now())?;
        Some((left, left.as_secs_f32() / CLEAR_AFTER.as_secs_f32()))
    }

    /// The search text typed after the secrets prefix.
    pub(crate) fn secret_query(&self) -> &str {
        self.query.strip_prefix(PREFIX).unwrap_or_default()
    }

    /// Whether Alt, with `others` telling if any other key or modifier is down, has been held
    /// alone long enough to reveal; otherwise how long until it would be.
    pub(crate) fn alt_reveals(&mut self, alt: bool, others: bool) -> (bool, Option<Duration>) {
        let now = Instant::now();
        self.alt_hold = match (alt, others, self.alt_hold) {
            (false, _, _) => AltHold::Up,
            (true, true, _) | (true, _, AltHold::Typing) => AltHold::Typing,
            (true, false, AltHold::Up) => AltHold::Since(now),
            (true, false, since) => since,
        };
        match self.alt_hold {
            AltHold::Since(since) => {
                let held = now.saturating_duration_since(since);
                (
                    held >= ALT_REVEAL_AFTER,
                    ALT_REVEAL_AFTER.checked_sub(held).filter(|d| !d.is_zero()),
                )
            }
            _ => (false, None),
        }
    }

    /// Shows the selected secret's value for a while, or hides it if it is shown.
    pub(crate) fn toggle_reveal(&mut self) {
        let selected = self.secrets.get(self.selected).map(|s| s.id);
        self.shown_until = match (self.revealed.as_ref().map(|r| r.id), selected) {
            (Some(shown), Some(id)) if shown == id => None,
            (_, Some(id)) => Some((id, Instant::now() + SHOWN_FOR)),
            (_, None) => None,
        };
    }

    /// Time left before a value shown with a click hides itself.
    pub(crate) fn shown_for(&self) -> Option<Duration> {
        self.shown_until
            .and_then(|(_, until)| until.checked_duration_since(Instant::now()))
    }

    /// Drops the shown value and any pending reveal.
    pub(crate) fn hide_value(&mut self) {
        self.shown_until = None;
        self.revealed = None;
    }

    /// Decrypts the selected secret's value while Alt is `held` or after a click on Show, and
    /// drops it as soon as neither applies.
    pub(crate) fn hold_reveal(&mut self, held: bool) {
        let selected = self.secrets.get(self.selected).map(|s| s.id);
        let clicked = self
            .shown_until
            .is_some_and(|(id, until)| Some(id) == selected && Instant::now() < until);
        if !clicked {
            self.shown_until = None;
        }
        let id = match selected {
            Some(id) if (held || clicked) && self.is_shown() => id,
            _ => {
                self.revealed = None;
                return;
            }
        };
        if self.revealed.as_ref().is_some_and(|r| r.id == id) {
            return;
        }
        match self.store.vault().reveal(id) {
            Ok(text) => self.revealed = Some(Revealed { id, text }),
            Err(err) => {
                // Remembered as revealed-without-text so repaints don't retry until release.
                self.revealed = Some(Revealed { id, text: None });
                self.report(&err);
            }
        }
    }

    fn on_locked(&mut self) {
        self.hide_value();
        self.secrets.clear();
        if self.sheet.as_ref().is_some_and(Sheet::shows_secret_data) {
            self.set_sheet(None);
        }
        self.refresh_if_shown();
    }

    pub(crate) fn quiet_for(&self) -> Option<Duration> {
        self.quiet_until
            .and_then(|until| until.checked_duration_since(Instant::now()))
    }

    pub(crate) fn input_quiet(&self) -> bool {
        self.quiet_until.is_some_and(|until| Instant::now() < until)
    }

    pub(crate) fn recovery_key_open(&self) -> bool {
        matches!(self.sheet, Some(Sheet::RecoveryKey { .. }))
    }

    fn set_sheet(&mut self, sheet: Option<Sheet>) {
        self.sheet = sheet;
        self.forget_text_state = true;
    }

    pub(super) fn clear_pending_secret(&mut self) {
        if let (Some(pending), Some(clipboard)) = (self.pending_clear.take(), &self.clipboard)
            && clipboard.clear_if_unchanged(pending.count)
        {
            tracing::debug!("cleared copied secret from clipboard");
        }
    }

    fn open_sheet(&mut self, sheet: Sheet) {
        self.set_sheet(Some(sheet));
        self.sheet_error = None;
        self.focus = super::Focus::Sheet;
    }

    fn cancel_sheet(&mut self) {
        match self.sheet {
            Some(Sheet::RecoveryKey { saved: true, .. }) => {
                self.sheet_action(SheetAction::Submit);
                return;
            }
            Some(Sheet::RecoveryKey { .. }) => return,
            _ => {}
        }
        self.set_sheet(None);
        self.sheet_error = None;
        if self.secrets_mode() && self.vault_status != VaultStatus::Unlocked {
            self.query.clear();
        }
        self.focus = super::Focus::Search;
        self.refresh();
    }

    fn submit(&mut self) -> ActionResult {
        let Some(sheet) = self.sheet.take() else {
            return Ok(());
        };
        let (next, result) = self.advance(sheet);
        match next {
            Some(sheet) if result.is_ok() => self.open_sheet(sheet),
            Some(sheet) => {
                self.sheet = Some(sheet);
                self.focus = super::Focus::Sheet;
                self.forget_text_state = true;
            }
            None => self.set_sheet(None),
        }
        if result.is_ok() {
            self.sheet_error = None;
            if self.sheet.is_none() {
                self.focus = super::Focus::Search;
            }
            self.refresh();
        }
        result
    }

    fn advance(&mut self, sheet: Sheet) -> (Option<Sheet>, ActionResult) {
        match sheet {
            Sheet::Create {
                password,
                confirm,
                then,
            } => match self.quietly(|app| app.create(&password, &confirm)) {
                Ok(key) => (
                    Some(Sheet::RecoveryKey {
                        key,
                        then,
                        saved: false,
                    }),
                    Ok(()),
                ),
                Err(err) => (
                    Some(Sheet::Create {
                        password,
                        confirm,
                        then,
                    }),
                    Err(err),
                ),
            },
            Sheet::Unlock { mut password, then } => {
                let unlocked = self
                    .quietly(|app| master(&password).and_then(|pw| app.store.vault().unlock(&pw)));
                match unlocked {
                    Ok(()) => (then.map(Self::move_sheet), Ok(())),
                    Err(err) => {
                        password.clear();
                        (Some(Sheet::Unlock { password, then }), Err(err.into()))
                    }
                }
            }
            Sheet::Recover {
                key,
                password,
                confirm,
                then,
            } => match self.quietly(|app| app.recover(&key, &password, &confirm)) {
                Ok(fresh) => (
                    Some(Sheet::RecoveryKey {
                        key: fresh,
                        then,
                        saved: false,
                    }),
                    Ok(()),
                ),
                Err(err) => (
                    Some(Sheet::Recover {
                        key,
                        password,
                        confirm,
                        then,
                    }),
                    Err(err),
                ),
            },
            Sheet::ConfirmDelete { id } => match self.store.vault().delete(id) {
                Ok(()) => (None, Ok(())),
                Err(err) => (Some(Sheet::ConfirmDelete { id }), Err(err.into())),
            },
            Sheet::RecoveryKey {
                then, saved: true, ..
            } => (then.map(Self::move_sheet), Ok(())),
            sheet @ Sheet::RecoveryKey { saved: false, .. } => (Some(sheet), Ok(())),
            Sheet::Label { target, name, note } => {
                let label = SecretLabel {
                    name: name.trim().to_owned(),
                    note: note.trim().to_owned(),
                };
                let saved = match target {
                    LabelTarget::Move(entry) => self
                        .store
                        .vault()
                        .move_from_history(entry, label)
                        .map(|_| ()),
                    LabelTarget::Edit(id) => self.store.vault().update(id, label),
                };
                match saved {
                    Ok(()) => (None, Ok(())),
                    Err(err) => (Some(Sheet::Label { target, name, note }), Err(err.into())),
                }
            }
        }
    }

    fn create(
        &mut self,
        password: &str,
        confirm: &str,
    ) -> Result<Field, Box<dyn std::error::Error>> {
        if password != confirm {
            return Err(Input::Mismatch.into());
        }
        let key = self.store.vault().create(&master(password)?)?;
        Ok(key.to_display())
    }

    fn recover(
        &mut self,
        key: &str,
        password: &str,
        confirm: &str,
    ) -> Result<Field, Box<dyn std::error::Error>> {
        if password != confirm {
            return Err(Input::Mismatch.into());
        }
        let key = RecoveryKey::parse(key.trim())?;
        let fresh = self.store.vault().recover(&key, &master(password)?)?;
        Ok(fresh.to_display())
    }

    fn copy_recovery_key(&mut self) -> ActionResult {
        let Some(Sheet::RecoveryKey { key, .. }) = &self.sheet else {
            return Ok(());
        };
        let formats = Zeroizing::new(vec![Format::new(FormatKind::Text, key.as_bytes())]);
        self.copy_concealed(&formats)
    }

    fn copy_concealed(&mut self, formats: &[Format]) -> ActionResult {
        let written = self.write_clipboard(formats, Privacy::Concealed)?;
        self.pending_clear = Some(PendingClear {
            count: written,
            at: Instant::now() + CLEAR_AFTER,
            wall: SystemTime::now() + CLEAR_AFTER,
        });
        Ok(())
    }

    // Argon2 blocks the UI thread; keys pressed meanwhile arrive afterwards and must not act on the new screen.
    fn quietly<T>(&mut self, work: impl FnOnce(&mut Self) -> T) -> T {
        let result = work(self);
        self.quiet_until = Some(Instant::now() + QUIET_AFTER_KDF);
        result
    }

    // The name stays empty: prefilling it from the content would print the secret in the list.
    fn move_sheet(entry: EntryId) -> Sheet {
        Sheet::Label {
            target: LabelTarget::Move(entry),
            name: field(),
            note: field(),
        }
    }
}

fn contains_folded(haystack: &str, needle: &[char]) -> bool {
    haystack.char_indices().any(|(start, _)| {
        let mut folded = haystack[start..].chars().flat_map(char::to_lowercase);
        needle.iter().all(|c| folded.next() == Some(*c))
    })
}

fn entry_sheet(status: VaultStatus, then: Option<EntryId>) -> Sheet {
    if status == VaultStatus::Missing {
        Sheet::Create {
            password: field(),
            confirm: field(),
            then,
        }
    } else {
        Sheet::Unlock {
            password: field(),
            then,
        }
    }
}

fn master(password: &str) -> pin_core::Result<MasterPassword> {
    MasterPassword::new(password.to_owned())
}

fn capitalize(message: &str) -> String {
    let mut chars = message.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folded(s: &str) -> Vec<char> {
        s.chars().flat_map(char::to_lowercase).collect()
    }

    #[test]
    fn folded_search_ignores_case_anywhere() {
        assert!(contains_folded("GitHub Token", &folded("hub t")));
        assert!(contains_folded("ΣΟΦΙΑ", &folded("σοφ")));
        assert!(!contains_folded("GitHub", &folded("lab")));
        assert!(contains_folded("x", &[]));
    }
}
