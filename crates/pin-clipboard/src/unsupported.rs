use std::convert::Infallible;
use std::marker::PhantomData;

use pin_core::Format;

use crate::{Capture, ChangeCount, Error, Privacy, Result};

#[derive(Debug)]
pub struct Clipboard {
    never: Infallible,
    // Matches the macOS pasteboard handle, which is neither Send nor Sync.
    _not_send: PhantomData<*const ()>,
}

impl Clipboard {
    pub fn general() -> Result<Self> {
        Err(Error::Unsupported)
    }

    pub fn change_count(&self) -> ChangeCount {
        match self.never {}
    }

    pub fn read(&self) -> Capture {
        match self.never {}
    }

    pub fn clear_if_unchanged(&self, _count: ChangeCount) -> bool {
        match self.never {}
    }

    pub fn write(&self, _formats: &[Format], _privacy: Privacy) -> Result<ChangeCount> {
        match self.never {}
    }
}
