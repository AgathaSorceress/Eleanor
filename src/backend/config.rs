use std::{fs::File, io::Write};

use super::utils::config_dir;
use miette::{miette, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};

/// Determines if the files will be loaded from a local path or remotely
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum SourceKind {
    /// Path to a directory
    Local { path: String },
    /// Remote server address
    Remote { address: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Source {
    pub id: u8,
    pub name: String,
    #[serde(flatten)]
    pub source: SourceKind,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub cache_expire_days: usize,
    pub crossfade: bool,
    pub crossfade_duration: u8,
    pub song_change_notification: bool,
    pub volume: f32,
    pub sources: Vec<Source>,
}
impl Config {
    pub fn read_config() -> Result<Self> {
        let file = config_dir()
            .and_then(|v| Some(v.join("settings.toml")))
            .ok_or(miette!("Configuration file not found"))?;

        let contents = std::fs::read_to_string(file).into_diagnostic()?;

        toml::from_str(&contents).into_diagnostic()
    }

    pub fn write_config(config: &Config) -> Result<()> {
        let contents = toml::to_string(config).into_diagnostic()?;

        let path = config_dir()
            .and_then(|v| Some(v.join("settings.toml")))
            .ok_or(miette!("Configuration file not found"))?;

        File::create(path)
            .and_then(|mut v| v.write_all(contents.as_bytes()))
            .into_diagnostic()
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            cache_expire_days: 30,
            crossfade: false,
            crossfade_duration: 5,
            song_change_notification: false,
            volume: 0.5,
            sources: vec![Source {
                id: 0,
                name: "Music".into(),
                source: SourceKind::Local {
                    path: "/home/agatha/Music/local".into(),
                },
            }],
        }
    }
}
