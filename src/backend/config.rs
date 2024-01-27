use std::{fmt::Display, fs::File, io::Write};

use super::{
    kdl_utils::{KdlDocumentExt, KdlNodeExt},
    utils::config_dir,
};
use kdl::{KdlDocument, KdlNode};
use miette::{miette, IntoDiagnostic, Result};

#[derive(Debug)]
pub struct Source {
    pub id: u32,
    pub name: String,
    pub path: String,
}

#[derive(Debug)]
pub struct Config {
    pub crossfade: bool,
    pub crossfade_duration: u32,
    pub song_change_notification: bool,
    pub volume: f64,
    pub sources: Vec<Source>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            crossfade: false,
            crossfade_duration: 5,
            song_change_notification: false,
            volume: 0.5,
            sources: vec![],
        }
    }
}

impl Config {
    pub fn read_config() -> Result<Self> {
        let file = config_dir()
            .map(|v| v.join("settings.kdl"))
            .ok_or(miette!("Configuration file not found"))?;

        let contents = std::fs::read_to_string(file).into_diagnostic()?;
        let kdl_doc: KdlDocument = contents.parse()?;

        let playback = kdl_doc.get_children_or("playback", KdlDocument::new());

        // Fallback for values added in future versions
        let default = Self::default();

        let crossfade = playback.get_bool_or("crossfade", default.crossfade);

        let crossfade_duration =
            playback.get_u32_or("crossfade-duration", default.crossfade_duration);

        let volume = playback.get_f64_or("volume", default.volume);

        let song_change_notification =
            kdl_doc.get_bool_or("song-change-notification", default.song_change_notification);

        let sources = kdl_doc
            .get("sources")
            .and_then(KdlNode::children)
            .map(KdlDocument::nodes)
            .unwrap_or_default();

        let sources = if sources.is_empty() {
            default.sources
        } else {
            sources
                .iter()
                .map(Source::try_from_node)
                .collect::<Result<_>>()?
        };

        Ok(Self {
            crossfade,
            crossfade_duration,
            song_change_notification,
            volume,
            sources,
        })
    }

    pub fn write_config(config: &Config) -> Result<()> {
        let contents = config.to_string();

        let path = config_dir()
            .map(|v| v.join("settings.kdl"))
            .ok_or(miette!("Configuration file not found"))?;

        File::create(path)
            .and_then(|mut v| v.write_all(contents.as_bytes()))
            .into_diagnostic()
    }
}

impl Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut kdl_doc = KdlDocument::new();

        let song_change_notification =
            KdlNode::with_arg("song-change-notification", self.song_change_notification);

        let playback = KdlNode::new("playback")
            .add_child(KdlNode::with_arg("crossfade", self.crossfade))
            .add_child(
                KdlNode::new("crossfade-duration")
                    .add_arg(i64::from(self.crossfade_duration), Some("sec"))
                    .clone(),
            )
            .add_child(KdlNode::with_arg("volume", self.volume))
            .clone();

        let mut sources = KdlNode::new("sources");
        for source in &self.sources {
            sources.add_child(
                KdlNode::new(source.name.clone())
                    .set_param("id", i64::from(source.id))
                    .set_param("path", source.path.clone())
                    .clone(),
            );
        }

        kdl_doc
            .add_child(song_change_notification)
            .add_child(playback)
            .add_child(sources);

        f.write_str(&kdl_doc.to_string())
    }
}

impl Source {
    fn try_from_node(node: &KdlNode) -> Result<Self> {
        let name = node.name().value().to_owned();

        let id = node
            .get("id")
            .ok_or(miette!(format!(
                "Source {name} is missing an `id` parameter"
            )))?
            .value()
            .as_i64()
            .ok_or(miette!("Source id must be a number"))?
            .try_into()
            .into_diagnostic()?;

        let path = node
            .get("path")
            .ok_or(miette!(format!(
                "Source {name} is missing a `path` parameter"
            )))?
            .value()
            .as_string()
            .ok_or(miette!("Source path must be a string"))?
            .to_owned();

        Ok(Self { id, name, path })
    }
}
