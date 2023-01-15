pub mod config;
pub mod error;
pub mod fetching;
pub mod http_decoder;
mod migrator;
pub mod model;
pub mod playback;
pub mod utils;

use std::fs::{create_dir_all, File};

use miette::{miette, IntoDiagnostic, Result};
use migrator::Migrator;
use paris::success;
use sea_orm_migration::prelude::*;

use self::{
    config::Config,
    utils::{cache_dir, config_dir},
};

/// Create the necessary files on first run
pub fn create_app_data() -> Result<()> {
    // Eleanor's config directory
    let config_path = config_dir().ok_or(miette!("Configuration directory does not exist"))?;
    // Eleanor's cache directory
    let cache_path = cache_dir().ok_or(miette!("Configuration directory does not exist"))?;
    // Directory for remote music cache
    let music_path = cache_path.join("tracks");

    // Create all required directories
    for path in [&config_path, &cache_path, &music_path] {
        create_dir_all(path).into_diagnostic()?;
    }

    File::create(&config_path.join("eleanor.db")).into_diagnostic()?;
    Config::write_config(&Default::default())?;
    success!("Created configuration file");

    Ok(())
}

/// Run unapplied migrations
pub async fn prepare_db(db: &sea_orm::DatabaseConnection) -> Result<()> {
    Migrator::up(db, None).await.into_diagnostic()?;

    success!("Applied migrations");

    Ok(())
}
