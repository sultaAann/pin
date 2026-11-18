#[cfg(target_os = "macos")]
mod imp {
    use eframe::EventLoopBuilderHook;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
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
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use eframe::EventLoopBuilderHook;

    pub fn event_loop_builder() -> Option<EventLoopBuilderHook> {
        None
    }

    pub fn show_in_dock() {}

    pub fn yield_focus() {}

    pub fn reveal() {}
}

pub use imp::{event_loop_builder, reveal, show_in_dock, yield_focus};
