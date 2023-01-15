use std::{
    ffi::{OsStr, OsString},
    fs::File,
    hash::Hasher,
    path::Path,
};
use tokio::sync::Mutex;

use super::error::EleanorError;
use super::{
    config::{Source, SourceKind},
    model::{library, library::Column},
    utils::Context,
};
use adler::Adler32;
use lofty::{read_from_path, Accessor, AudioFile};
use miette::{miette, Result};
use paris::{success, warn};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set};
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

pub async fn index_source(
    source: Source,
    mode: IndexMode,
    ctx: &Context,
) -> Result<(), EleanorError> {
    let mut existing: Vec<OsString> = vec![];

    let db = &ctx.db;

    // Force reindex source
    if mode == IndexMode::Purge {
        warn!("Overwriting source {}", source.id);

        library::Entity::delete_many()
            .filter(library::Column::SourceId.eq(source.id))
            .exec(db)
            .await?;
    // Only index new songs
    } else if mode == IndexMode::New {
        existing = library::Entity::find()
            .filter(library::Column::SourceId.eq(source.id))
            .column(library::Column::Filename)
            .all(db)
            .await?
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
                        .map(|v| v.type_() == mime::AUDIO)
                        .unwrap_or(false)
                })
            {
                if mode == IndexMode::New {
                    if existing.contains(&file.file_name().into()) {
                        continue;
                    };
                }

                let audio = read_from_path(file.path(), true)?;

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
                    hash: Set(hash.try_into().map_err(|_| EleanorError::CastError)?),
                    artist: Set(tags.and_then(|t| t.artist()).map(|t| t.to_string())),
                    album_artist: Set(tags
                        .and_then(|t| t.get_string(&lofty::ItemKey::AlbumArtist))
                        .map(|t| t.to_string())),
                    name: Set(tags.and_then(|t| t.title()).map(|t| t.to_string())),
                    album: Set(tags.and_then(|t| t.album()).map(|t| t.to_string())),
                    genres: Set(tags.and_then(|t| t.genre()).map(|t| t.to_string())),
                    track: Set(tags.and_then(|t| t.track()).map(|t| t as i32)),
                    year: Set(tags
                        .and_then(|t| t.year())
                        .map(|t| if t == 0 { None } else { Some(t as i32) })
                        .flatten()),
                    duration: Set(properties
                        .duration()
                        .as_millis()
                        .try_into()
                        .map_err(|_| EleanorError::CastError)?),
                    ..Default::default()
                };

                library::Entity::insert(song)
                    .on_conflict(
                        sea_query::OnConflict::column(Column::Hash)
                            .do_nothing()
                            .to_owned(),
                    )
                    .exec(db)
                    .await?;
            }
        }
        SourceKind::Remote { address } => {
            let (username, password) = &ctx
                .auth
                .get(&source.id)
                .ok_or(miette!("Couldn't get credentials for source {}", source.id))?;

            let client = &ctx.http_client;

            let index = client
                .get(format!("{address}/"))
                .basic_auth(username, Some(password))
                .send()
                .await?;

            // Handle server failures
            let index = if index.status().is_success() {
                index.bytes().await?
            } else {
                return Err(miette!("Response status: {}", index.status())
                    .wrap_err(format!("Indexing for source {} failed", source.id))
                    .into());
            };

            // Deserialize messagepack into a library model
            let parsed: Vec<library::Model> = rmp_serde::from_slice(&index)?;

            // Use all fields except for id and source_id
            let songs: Vec<_> = parsed
                .into_iter()
                .map(|v| {
                    return library::ActiveModel {
                        path: Set(v.path),
                        filename: Set(v.filename),
                        source_id: Set(source.id.into()), // Use local source id, not remote
                        hash: Set(v.hash),
                        artist: Set(v.artist),
                        album_artist: Set(v.album_artist),
                        name: Set(v.name),
                        album: Set(v.album),
                        genres: Set(v.genres),
                        track: Set(v.track),
                        year: Set(v.year),
                        duration: Set(v.duration),
                        ..Default::default()
                    };
                })
                .collect();

            library::Entity::insert_many(songs)
                .on_conflict(
                    sea_query::OnConflict::column(Column::Hash)
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await?;
        }
    }

    success!("Indexed source {} in {:?} mode", source.id, mode);
    Ok(())
}

pub async fn index_initial(ctx: &Mutex<Context>) -> Result<()> {
    let context = &ctx.lock().await;
    let sources = &context.config.sources;

    for source in sources {
        index_source(source.to_owned(), IndexMode::Initial, &context).await?;
    }

    Ok(())
}

pub async fn index_new(ctx: &Mutex<Context>) -> Result<()> {
    let context = &ctx.lock().await;
    let sources = &context.config.sources;

    for source in sources {
        index_source(source.to_owned(), IndexMode::New, &context).await?;
    }

    Ok(())
}

fn hash_file(path: &Path) -> Result<u64, EleanorError> {
    let file = Box::new(File::open(path)?);

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
        )?
        .format;

    let mut adler = Adler32::new();

    while let Ok(packet) = data.next_packet() {
        adler.write(&packet.data);
    }

    Ok(adler.finish())
}
