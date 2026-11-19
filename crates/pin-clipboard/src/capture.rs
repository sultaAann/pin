use pin_core::NewEntry;

// Checked per format after the OS has handed the data over: macOS cannot report a pasteboard payload's size before transferring it.
pub const MAX_CAPTURE_BYTES: usize = 100 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChangeCount(pub(crate) isize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Privacy {
    #[default]
    Normal,
    Concealed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    Private,
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capture {
    Entry(NewEntry),
    Skipped(Skip),
    Empty,
}
