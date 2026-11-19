use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pin_core::NewEntry;

use crate::{Capture, ChangeCount, Clipboard, Error, Result};

pub const DEFAULT_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub struct Watcher {
    clipboard: Clipboard,
    last: ChangeCount,
    ignored: Arc<AtomicIsize>,
}

impl Watcher {
    pub fn new(clipboard: Clipboard) -> Self {
        Self::with_ignored(clipboard, Arc::new(AtomicIsize::new(isize::MIN)))
    }

    fn with_ignored(clipboard: Clipboard, ignored: Arc<AtomicIsize>) -> Self {
        let last = clipboard.change_count();
        Self {
            clipboard,
            last,
            ignored,
        }
    }

    /// Skips the copy with this change count, typically pin's own write.
    pub fn ignore(&self, count: ChangeCount) {
        self.ignored.store(count.0, Ordering::Relaxed);
    }

    pub fn poll(&mut self) -> Option<Capture> {
        let now = self.clipboard.change_count();
        if now == self.last {
            return None;
        }
        self.last = now;
        if now.0 == self.ignored.load(Ordering::Relaxed) {
            return None;
        }
        Some(self.clipboard.read())
    }
}

#[derive(Debug)]
#[must_use = "dropping the handle stops the watcher"]
pub struct WatchHandle {
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
    ignored: Arc<AtomicIsize>,
}

impl WatchHandle {
    /// Skips the copy with this change count, typically pin's own write.
    pub fn ignore(&self, count: ChangeCount) {
        self.ignored.store(count.0, Ordering::Relaxed);
    }

    pub fn is_running(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("clipboard watcher panicked");
        }
    }
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Polls the system clipboard on a background thread and passes each new copy to `on_entry`.
///
/// On macOS the source app name comes from `NSWorkspace`, which is only kept current while the
/// process runs its main run loop (as the GUI app does).
pub fn watch<F>(interval: Duration, mut on_entry: F) -> Result<WatchHandle>
where
    F: FnMut(NewEntry) + Send + 'static,
{
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<()>>(1);
    let ignored = Arc::new(AtomicIsize::new(isize::MIN));
    let shared = Arc::clone(&ignored);
    // The pasteboard object is not Send, so it is created on the thread that polls it.
    let thread = thread::Builder::new()
        .name("pin-clipboard".into())
        .spawn(move || {
            let mut watcher = match Clipboard::general() {
                Ok(clipboard) => Watcher::with_ignored(clipboard, shared),
                Err(err) => {
                    _ = ready_tx.send(Err(err));
                    return;
                }
            };
            _ = ready_tx.send(Ok(()));
            while let Err(RecvTimeoutError::Timeout) = stop_rx.recv_timeout(interval) {
                match watcher.poll() {
                    Some(Capture::Entry(entry)) => on_entry(entry),
                    Some(Capture::Skipped(reason)) => {
                        tracing::debug!(?reason, "clipboard copy skipped");
                    }
                    Some(Capture::Empty) | None => {}
                }
            }
        })
        .map_err(Error::Spawn)?;
    let handle = WatchHandle {
        stop: Some(stop_tx),
        thread: Some(thread),
        ignored,
    };
    ready_rx.recv().map_err(|_| Error::WatcherStopped)??;
    Ok(handle)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use pin_core::{Format, FormatKind};

    use super::*;
    use crate::Privacy;

    #[test]
    fn poll_reports_each_change_once() {
        let clipboard = Clipboard::unique();
        let writer = Clipboard::named(&clipboard);
        let mut watcher = Watcher::new(clipboard);
        assert_eq!(watcher.poll(), None);
        writer
            .write(&[Format::new(FormatKind::Text, "one")], Privacy::Normal)
            .unwrap();
        assert!(matches!(watcher.poll(), Some(Capture::Entry(_))));
        assert_eq!(watcher.poll(), None);
    }

    #[test]
    fn ignored_change_is_not_captured() {
        let clipboard = Clipboard::unique();
        let writer = Clipboard::named(&clipboard);
        let mut watcher = Watcher::new(clipboard);
        let own = writer
            .write(&[Format::new(FormatKind::Text, "own")], Privacy::Normal)
            .unwrap();
        watcher.ignore(own);
        assert_eq!(watcher.poll(), None);
        writer
            .write(&[Format::new(FormatKind::Text, "next")], Privacy::Normal)
            .unwrap();
        assert!(matches!(watcher.poll(), Some(Capture::Entry(_))));
    }
}
