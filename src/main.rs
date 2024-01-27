use miette::{ensure, miette, IntoDiagnostic, Result};

use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;

use tracing::info;

use crate::backend::{
    create_app_data, logging, prepare_db,
    utils::{config_dir, is_first_run},
};

mod backend;
mod gui;

#[tokio::main]
async fn main() {
    if let Err(e) = startup().await {
        eprintln!("{:?}", e);
    }
}

// Separate function to avoid the main function error message prefix
async fn startup() -> Result<()> {
    logging::setup();

    // First, make sure that the app's files exist
    let first_run = is_first_run()?;
    if first_run {
        info!("No previous configuration found; Starting first run process");
        create_app_data()?;
    }

    // Create a database connection
    let mut conn = ConnectOptions::new(format!(
        "sqlite://{}/eleanor.db?mode=rwc",
        config_dir()
            .ok_or(miette!("Configuration directory not found"))?
            .display()
    ));
    conn.sqlx_logging_level(tracing::log::LevelFilter::Trace);

    let db: DatabaseConnection = Database::connect(conn).await.into_diagnostic()?;

    // Run migrations
    prepare_db(&db).await?;

    let schema_manager = SchemaManager::new(&db);

    ensure!(
        schema_manager
            .has_table("library")
            .await
            .into_diagnostic()?,
        miette!("Running migrations failed")
    );

    Ok(())
}
