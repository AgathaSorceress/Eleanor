use backend::{
    create_app_data,
    fetching::{index_initial, index_new},
    prepare_db,
    utils::{config_dir, is_first_run},
};
use miette::{ensure, miette, IntoDiagnostic, Result};
use paris::info;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;

mod backend;
mod gui;

#[tokio::main]
async fn main() -> Result<()> {
    // First, make sure that the app's files exist
    let first_run = is_first_run()?;
    if first_run {
        info!("No previous configuration found; Starting first run process");
        create_app_data()?;
    }

    // Create a database connection
    let db: DatabaseConnection = Database::connect(&format!(
        "sqlite://{}/eleanor.db?mode=rwc",
        config_dir()
            .ok_or(miette!("Configuration directory not found"))?
            .display()
    ))
    .await
    .into_diagnostic()?;

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

    if first_run {
        index_initial(&db).await?;
    } else {
        // Index only new songs
        index_new(&db).await?;
    }

    Ok(())
}
