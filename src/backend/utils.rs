use miette::{miette, IntoDiagnostic, Result};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use sea_orm::DatabaseConnection;
use std::{collections::HashMap, fs::File, io::Write, path::PathBuf, time::Duration};

use super::{
    config::{Config, SourceKind},
    error::EleanorError,
};

#[derive(Debug)]
pub struct Context {
    pub db: DatabaseConnection,
    pub config: Config,
    pub http_client: ClientWithMiddleware,
    /// Pairs of sources and the corresponding credentials
    pub auth: HashMap<u8, (String, String)>,
}

impl Context {
    pub fn new(db: DatabaseConnection) -> Result<Self> {
        // Retry failed HTTP requests for 30 seconds
        let retry = RetryTransientMiddleware::new_with_policy(
            ExponentialBackoff::builder().build_with_total_retry_duration(Duration::from_secs(30)),
        );

        let http_client = ClientBuilder::new(reqwest::Client::new())
            .with(retry)
            .build();

        let config = Config::read_config()?;

        // Read and store credentials for all defined sources
        let mut auth = HashMap::new();
        for source in &config.sources {
            // Only applies to remote sources
            if matches!(source.source, SourceKind::Remote { .. }) {
                auth.insert(source.id, get_auth_source(source.id)?);
            }
        }

        Ok(Self {
            db,
            config,
            http_client,
            auth,
        })
    }
}

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
pub fn store_auth_source(
    username: String,
    password: String,
    source: u8,
) -> Result<(), EleanorError> {
    let contents = rmp_serde::to_vec(&(username, password))?;

    let path = cache_dir()
        .ok_or(miette!("Cache directory does not exist"))?
        .join(format!("{source}.auth"));

    File::create(path)
        .and_then(|mut v| v.write_all(&contents))
        .map_err(EleanorError::from)
}

/// Returns the stored credentials for a remote source
pub fn get_auth_source(source: u8) -> Result<(String, String), EleanorError> {
    use miette::Context;

    let path = cache_dir()
        .ok_or(miette!("Cache directory does not exist"))?
        .join(format!("{source}.auth"));

    let file = std::fs::read(path)
        .into_diagnostic()
        .wrap_err(format!("Couldn't read credentials for source {source}"))?;

    let contents = rmp_serde::from_slice(&file)?;

    Ok(contents)
}
