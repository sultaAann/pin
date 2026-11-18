use anyhow::Context as _;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use pin_core::PinSlot;

use crate::events::{Event, Events};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Toggle,
    Slot(PinSlot),
}

#[cfg(target_os = "macos")]
const OPEN: Modifiers = Modifiers::META.union(Modifiers::SHIFT);
#[cfg(target_os = "macos")]
const SLOT: Modifiers = Modifiers::META.union(Modifiers::CONTROL);
#[cfg(target_os = "macos")]
pub const SLOT_LABEL: &str = "Cmd+Ctrl+";

#[cfg(not(target_os = "macos"))]
const OPEN: Modifiers = Modifiers::CONTROL.union(Modifiers::SHIFT);
#[cfg(not(target_os = "macos"))]
const SLOT: Modifiers = Modifiers::CONTROL.union(Modifiers::ALT);
#[cfg(not(target_os = "macos"))]
pub const SLOT_LABEL: &str = "Ctrl+Alt+";

const DIGITS: [Code; 9] = [
    Code::Digit1,
    Code::Digit2,
    Code::Digit3,
    Code::Digit4,
    Code::Digit5,
    Code::Digit6,
    Code::Digit7,
    Code::Digit8,
    Code::Digit9,
];

pub struct Hotkeys {
    _manager: GlobalHotKeyManager,
    has_toggle: bool,
}

impl Hotkeys {
    pub fn register(events: Events) -> anyhow::Result<Self> {
        let manager = GlobalHotKeyManager::new().context("start global hotkeys")?;
        let mut bindings = Vec::with_capacity(1 + DIGITS.len());
        let slots = PinSlot::all()
            .zip(DIGITS)
            .map(|(slot, code)| (HotKey::new(Some(SLOT), code), Action::Slot(slot)));
        for (hotkey, action) in
            std::iter::once((HotKey::new(Some(OPEN), Code::KeyC), Action::Toggle)).chain(slots)
        {
            match manager.register(hotkey) {
                Ok(()) => bindings.push((hotkey.id(), action)),
                Err(err) => tracing::warn!(?action, %err, "hotkey not registered"),
            }
        }
        let has_toggle = bindings.iter().any(|&(_, action)| action == Action::Toggle);
        // global-hotkey keeps the first handler for the life of the process, so this is registered once.
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state() != HotKeyState::Pressed {
                return;
            }
            if let Some(&(_, action)) = bindings.iter().find(|(id, _)| *id == event.id()) {
                events.send(Event::Hotkey(action));
            }
        }));
        Ok(Self {
            _manager: manager,
            has_toggle,
        })
    }

    pub fn has_toggle(&self) -> bool {
        self.has_toggle
    }
}
