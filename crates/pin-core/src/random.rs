use crate::error::{Error, Result};

pub(crate) fn fill(bytes: &mut [u8]) -> Result<()> {
    getrandom::fill(bytes).map_err(Error::Random)
}

pub(crate) fn array<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    fill(&mut bytes)?;
    Ok(bytes)
}
