use std::sync::mpsc::Sender;

use eframe::egui;
use pin_core::NewEntry;

use crate::hotkeys::Action;

pub enum Event {
    Captured(NewEntry),
    Hotkey(Action),
}

#[derive(Clone)]
pub struct Events {
    tx: Sender<Event>,
    ctx: egui::Context,
}

impl Events {
    pub fn new(tx: Sender<Event>, ctx: egui::Context) -> Self {
        Self { tx, ctx }
    }

    pub fn send(&self, event: Event) {
        if self.tx.send(event).is_ok() {
            self.ctx.request_repaint();
        }
    }
}
