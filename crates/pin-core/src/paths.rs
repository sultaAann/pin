use std::path::PathBuf;

use directories::ProjectDirs;

use crate::error::{Error, Result};

const DATABASE_FILE: &str = "pin.db";

pub fn default_database_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "pin").ok_or(Error::NoDataDir)?;
    let dir = dirs.data_dir();
    std::fs::create_dir_all(dir)?;
    Ok(dir.join(DATABASE_FILE))
}
