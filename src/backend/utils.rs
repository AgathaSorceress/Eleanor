use miette::{miette, IntoDiagnostic, Result};
use std::{fs::File, io::Write, path::PathBuf};

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|v| v.join("eleanor"))
}

pub fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|v| v.join("eleanor"))
}

/// If no files have been created in the config directory, the app is running for the first time
pub fn is_first_run() -> Result<bool> {
    let path = config_dir().ok_or(miette!("Configuration directory not found"))?;

    Ok(!path.exists())
}

/// Stores credentials for a remote source
pub fn store_auth_source(username: String, password: String, source: u8) -> Result<()> {
    let contents = rmp_serde::to_vec(&(username, password)).into_diagnostic()?;

    let path = cache_dir()
        .ok_or(miette!("Cache directory does not exist"))?
        .join(format!("{source}.auth"));

    File::create(path)
        .and_then(|mut v| v.write_all(&contents))
        .into_diagnostic()
}

/// Returns the stored credentials for a remote source
pub fn get_auth_source(source: u8) -> Result<(String, String)> {
    let path = cache_dir()
        .ok_or(miette!("Cache directory does not exist"))?
        .join(format!("{source}.auth"));

    let file = std::fs::read(path).into_diagnostic()?;

    let contents = rmp_serde::from_slice(&file).into_diagnostic()?;

    Ok(contents)
}
