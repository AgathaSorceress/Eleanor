use rodio::OutputStream;
use std::sync::Arc;
use tokio::sync::Mutex;

use backend::{
    create_app_data,
    fetching::{index_initial, index_new},
    http_decoder::HttpReader,
    playback::decode_track,
    prepare_db,
    utils::{config_dir, is_first_run, Context},
};
use miette::{ensure, miette, IntoDiagnostic, Result};
use paris::info;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;

mod backend;
mod gui;

#[tokio::main]
async fn main() -> Result<()> {
    // Prepare error handling

    miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .without_cause_chain()
                .build(),
        )
    }))?;

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

    let context = Arc::new(Mutex::new(
        Context::new(db).expect("Failed to initialize Eleanor"),
    ));

    if first_run {
        index_initial(&context).await?;
    } else {
        // Index only new songs
        index_new(&context).await?;
    }

    // let (_stream, stream_handle) = OutputStream::try_default().unwrap();

    // let source = decode_track(&context, 2375409328).await?;

    // stream_handle.play_raw(source).into_diagnostic()?;

    let context = context.lock().await;

    let url =
        reqwest::Url::parse(&format!("http://localhost:8008/2375409328")).into_diagnostic()?;
    let client = &context.http_client;

    let auth = context
        .auth
        .get(&1)
        .ok_or(miette!("Credentials for source not found"))?;

    let reader = HttpReader::new(url, client.clone(), auth.clone()).await;
    println!("hello after return!");

    loop {}

    Ok(())
}
