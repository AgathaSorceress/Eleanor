pub mod config;
pub mod fetching;
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
    let config_path = config_dir().ok_or(miette!("Configuration directory does not exist"))?;

    // Create Eleanor's config directory
    create_dir_all(&config_path).into_diagnostic()?;

    let cache_path = cache_dir().ok_or(miette!("Configuration directory does not exist"))?;

    // Create Eleanor's cache directory
    create_dir_all(&cache_path).into_diagnostic()?;

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
