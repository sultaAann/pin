#[cfg(target_os = "macos")]
mod imp {
    use eframe::EventLoopBuilderHook;
    use eframe::egui::{Pos2, Vec2, pos2};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSScreen};
    use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

    // Set before launch so no Dock icon or menu bar flashes up; winit would otherwise apply Regular.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "same signature as platforms without a hook"
    )]
    pub fn event_loop_builder() -> Option<EventLoopBuilderHook> {
        Some(Box::new(|builder| {
            builder.with_activation_policy(ActivationPolicy::Accessory);
        }))
    }

    // Without the open hotkey a hidden accessory app could never be brought back, so keep a Dock icon.
    pub fn show_in_dock() {
        if let Some(mtm) = MainThreadMarker::new() {
            _ = NSApplication::sharedApplication(mtm)
                .setActivationPolicy(NSApplicationActivationPolicy::Regular);
        }
    }

    // Hiding only the window would leave pin active, so Cmd+V would not reach the app the user came from.
    pub fn yield_focus() {
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).hide(None);
        }
    }

    pub fn reveal() {
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).unhide(None);
        }
    }

    /// Top-left point that puts a window of `size` in the upper part of the screen the user is working on.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "screen coordinates fit in f32"
    )]
    pub fn popup_origin(size: Vec2) -> Option<Pos2> {
        let mtm = MainThreadMarker::new()?;
        let screen = NSScreen::mainScreen(mtm)?;
        let primary = NSScreen::screens(mtm).to_vec().into_iter().next()?;
        let area = screen.visibleFrame();
        // AppKit measures from the bottom of the primary screen; winit from its top.
        let top = primary.frame().size.height - (area.origin.y + area.size.height);
        let x = area.origin.x as f32 + (area.size.width as f32 - size.x) / 2.0;
        let y = top as f32 + (area.size.height as f32 - size.y) / 4.0;
        Some(pos2(x.max(area.origin.x as f32), y.max(top as f32)))
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use eframe::EventLoopBuilderHook;
    use eframe::egui::{Pos2, Vec2};

    pub fn event_loop_builder() -> Option<EventLoopBuilderHook> {
        None
    }

    pub fn show_in_dock() {}

    pub fn yield_focus() {}

    pub fn reveal() {}

    pub fn popup_origin(_size: Vec2) -> Option<Pos2> {
        None
    }
}

pub use imp::{event_loop_builder, popup_origin, reveal, show_in_dock, yield_focus};
