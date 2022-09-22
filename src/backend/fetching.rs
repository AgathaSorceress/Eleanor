use std::{
    ffi::{OsStr, OsString},
    fs::File,
    hash::Hasher,
    path::Path,
};

use super::{
    config::{Config, Source, SourceKind},
    model::library,
};
use adler::Adler32;
use lofty::{read_from_path, Accessor, AudioFile};
use miette::{miette, IntoDiagnostic, Result};
use paris::{success, warn};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set};
use symphonia::{
    core::{
        io::MediaSourceStream,
        meta::{Limit, MetadataOptions},
        probe::Hint,
    },
    default::get_probe,
};
use walkdir::WalkDir;

#[derive(PartialEq, Debug)]
pub enum IndexMode {
    Purge,
    New,
    Initial,
}

pub async fn index_source(source: Source, mode: IndexMode, db: &DatabaseConnection) -> Result<()> {
    let mut existing: Vec<OsString> = vec![];

    // Force reindex source
    if mode == IndexMode::Purge {
        warn!("Overwriting source {}", source.id);

        library::Entity::delete_many()
            .filter(library::Column::SourceId.eq(source.id))
            .exec(db)
            .await
            .into_diagnostic()?;
    // Only index new songs
    } else if mode == IndexMode::New {
        existing = library::Entity::find()
            .filter(library::Column::SourceId.eq(source.id))
            .column(library::Column::Filename)
            .all(db)
            .await
            .into_diagnostic()?
            .into_iter()
            .map(|v| v.filename.into())
            .collect();
    }

    match source.source {
        SourceKind::Local { path } => {
            for file in WalkDir::new(path)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| !e.file_type().is_dir())
                .filter(|e| {
                    mime_guess::from_path(e.path())
                        .first()
                        .and_then(|v| Some(v.type_() == mime::AUDIO))
                        .unwrap_or(false)
                })
            {
                if mode == IndexMode::New {
                    if existing.contains(&file.file_name().into()) {
                        continue;
                    };
                }

                let audio = read_from_path(file.path(), true).into_diagnostic()?;

                let tags = audio.primary_tag().or(audio.first_tag());

                let properties = audio.properties();

                let hash = hash_file(file.path())?;

                let song: library::ActiveModel = library::ActiveModel {
                    path: Set(file
                        .path()
                        .parent()
                        .and_then(Path::to_str)
                        .ok_or(miette!("Couldn't get path for file {:?}", file))?
                        .to_string()),
                    filename: Set(file
                        .file_name()
                        .to_str()
                        .ok_or(miette!("Couldn't get filename for file {:?}", file))?
                        .to_string()),
                    source_id: Set(source.id.into()),
                    hash: Set(hash.try_into().into_diagnostic()?),
                    artist: Set(tags
                        .and_then(|t| t.artist())
                        .and_then(|t| Some(t.to_string()))),
                    name: Set(tags
                        .and_then(|t| t.title())
                        .and_then(|t| Some(t.to_string()))),
                    album: Set(tags
                        .and_then(|t| t.album())
                        .and_then(|t| Some(t.to_string()))),
                    genres: Set(tags
                        .and_then(|t| t.genre())
                        .and_then(|t| Some(t.to_string()))),
                    track: Set(tags.and_then(|t| t.track()).and_then(|t| Some(t as i32))),
                    year: Set(tags.and_then(|t| t.year()).and_then(|t| Some(t as i32))),
                    duration: Set(properties
                        .duration()
                        .as_millis()
                        .try_into()
                        .into_diagnostic()?),
                    ..Default::default()
                };

                library::Entity::insert(song)
                    .on_conflict(
                        sea_query::OnConflict::column(library::Column::Filename)
                            .do_nothing()
                            .to_owned(),
                    )
                    .exec(db)
                    .await
                    .into_diagnostic()?;
            }
        }
        SourceKind::Remote { address } => {
            unimplemented!();
        }
    }

    success!("Indexed source {} in {:?} mode", source.id, mode);
    Ok(())
}

pub async fn index_initial(db: &DatabaseConnection) -> Result<()> {
    let sources = Config::read_config()?.sources;

    for source in sources {
        index_source(source, IndexMode::Initial, db).await?;
    }

    Ok(())
}

pub async fn index_new(db: &DatabaseConnection) -> Result<()> {
    let sources = Config::read_config()?.sources;

    for source in sources {
        index_source(source, IndexMode::New, db).await?;
    }

    Ok(())
}

fn hash_file(path: &Path) -> Result<u64> {
    let file = Box::new(File::open(path).into_diagnostic()?);

    let probe = get_probe();

    let ext = path.extension().and_then(OsStr::to_str).unwrap_or("");

    let source = MediaSourceStream::new(file, Default::default());
    let mut data = probe
        .format(
            &Hint::new().with_extension(ext),
            source,
            &Default::default(),
            &MetadataOptions {
                limit_metadata_bytes: Limit::Maximum(0),
                limit_visual_bytes: Limit::Maximum(0),
            },
        )
        .into_diagnostic()?
        .format;

    let mut adler = Adler32::new();

    while let Ok(packet) = data.next_packet() {
        adler.write(&packet.data);
    }

    Ok(adler.finish())
}
