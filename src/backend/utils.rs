use miette::{miette, Result};
use std::path::PathBuf;

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|v| v.join("eleanor"))
}

#[allow(dead_code)]
pub fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|v| v.join("eleanor"))
}

/// If no files have been created in the config directory, the app is running for the first time
pub fn is_first_run() -> Result<bool> {
    let path = config_dir().ok_or(miette!("Configuration directory not found"))?;

    Ok(!path.exists())
}
