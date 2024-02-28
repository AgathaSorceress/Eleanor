use std::{ffi::OsStr, fs::File, hash::Hasher, path::Path};

use adler::Adler32;
use lofty::{AudioFile, ItemKey, TaggedFileExt};
use miette::{miette, IntoDiagnostic, Result};
use rayon::prelude::*;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set};
use symphonia::{
    core::{
        formats::{FormatOptions, FormatReader},
        io::{MediaSourceStream, MediaSourceStreamOptions},
        meta::{Limit, MetadataOptions},
        probe::Hint,
    },
    default::get_probe,
};
use time::OffsetDateTime;
use tracing::debug;
use walkdir::{DirEntry, WalkDir};

use crate::backend::replaygain::{format_gain, ReplayGain};

use super::{
    config::Source,
    error::EleanorError,
    model::{library, library::Column, sources},
};

/// Get audio packets, ignoring metadata
fn get_packets(path: &Path) -> Result<Box<dyn FormatReader>> {
    let file = Box::new(File::open(path).into_diagnostic()?);

    let probe = get_probe();

    let ext = path.extension().and_then(OsStr::to_str).unwrap_or("");

    let source = MediaSourceStream::new(file, MediaSourceStreamOptions::default());
    probe
        .format(
            Hint::new().with_extension(ext),
            source,
            &FormatOptions::default(),
            &MetadataOptions {
                limit_metadata_bytes: Limit::Maximum(0),
                limit_visual_bytes: Limit::Maximum(0),
            },
        )
        .into_diagnostic()
        .map(|v| v.format)
}

fn hash_packets(data: &mut Box<dyn FormatReader>) -> u64 {
    let mut adler = Adler32::new();

    while let Ok(packet) = data.next_packet() {
        adler.write(&packet.data);
    }

    adler.finish()
}

fn index_song(
    file: &DirEntry,
    source: &Source,
    force: bool,
    indexed_ts: OffsetDateTime,
) -> Result<Option<library::ActiveModel>, EleanorError> {
    // Re-index previously indexed files
    if !force {
        let modified = file
            .metadata()
            .into_diagnostic()?
            .modified()?
            .duration_since(indexed_ts.into())
            .is_ok();

        if modified {
            debug!("Skipping file {}", file.path().display());
            return Ok(None);
        }
    }

    debug!("Indexing file {}", file.path().display());

    let tagged_file = lofty::read_from_path(file.path())?;

    let tags = tagged_file.primary_tag().or(tagged_file.first_tag());
    let properties = tagged_file.properties();

    // Hash audio packets
    let mut packets = get_packets(file.path())?;
    let hash = hash_packets(&mut packets);

    let rg_track_gain = tags
        .and_then(|t| t.get_string(&ItemKey::ReplayGainTrackGain))
        .and_then(|v| format_gain(v).ok());
    let rg_track_peak = tags
        .and_then(|t| t.get_string(&ItemKey::ReplayGainTrackPeak))
        .and_then(|v| format_gain(v).ok());
    let rg_album_gain = tags
        .and_then(|t| t.get_string(&ItemKey::ReplayGainAlbumGain))
        .and_then(|v| format_gain(v).ok());
    let rg_album_peak = tags
        .and_then(|t| t.get_string(&ItemKey::ReplayGainAlbumPeak))
        .and_then(|v| format_gain(v).ok());

    // Check for existing ReplayGain tags.
    let mut rg = if let (Some(track_gain), Some(track_peak)) = (rg_track_gain, rg_track_peak) {
        Ok(ReplayGain {
            track_gain,
            track_peak,
            album_gain: None,
            album_peak: None,
        })
    } else {
        // Calculate replaygain values for the audio track
        let mut packets = get_packets(file.path())?;
        ReplayGain::try_calculate(&mut packets)
    };

    // Set album gain and peak, if present in metadata.
    if let Ok(rg) = &mut rg {
        rg.album_gain = rg_album_gain;
        rg.album_peak = rg_album_peak;
    }

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
        source_id: Set(source.id),
        hash: Set(hash.try_into()?),
        artist: Set(tags
            .and_then(lofty::Accessor::artist)
            .map(|t| t.to_string())),
        album_artist: Set(tags
            .and_then(|t| t.get_string(&lofty::ItemKey::AlbumArtist))
            .map(|t| t.to_string())),
        name: Set(tags.and_then(lofty::Accessor::title).map(|t| t.to_string())),
        album: Set(tags.and_then(lofty::Accessor::album).map(|t| t.to_string())),
        genres: Set(tags.and_then(lofty::Accessor::genre).map(|t| t.to_string())),
        track: Set(tags.and_then(lofty::Accessor::track).map(|t| t as i32)),
        disc: Set(tags.and_then(lofty::Accessor::disk).map(|t| t as i32)),
        year: Set(tags.and_then(lofty::Accessor::year).map(|t| t as i32)),
        duration: Set(properties.duration().as_millis().try_into()?),
        rg_track_gain: Set(rg.as_ref().map(|v| f64::from(v.track_gain)).ok()),
        rg_track_peak: Set(rg.as_ref().map(|v| f64::from(v.track_peak)).ok()),
        rg_album_gain: Set(rg
            .as_ref()
            .map(|v| v.album_gain.map(f64::from))
            .ok()
            .flatten()),
        rg_album_peak: Set(rg
            .as_ref()
            .map(|v| v.album_peak.map(f64::from))
            .ok()
            .flatten()),
        ..Default::default()
    };

    Ok(Some(song))
}

pub async fn index_source(
    source: Source,
    force: bool,
    db: &DatabaseConnection,
) -> Result<(), EleanorError> {
    // Get timestamp of last successful scan for current source, or fall back to
    // a timestamp that's unlikely to be encountered
    let indexed_ts = sources::Entity::find()
        .filter(sources::Column::Id.eq(source.id))
        .column(sources::Column::LastIndexed)
        .all(db)
        .await?
        .into_iter()
        .next()
        .and_then(|v| v.last_indexed)
        .unwrap_or(String::from("0"))
        .parse::<i64>()?;

    let indexed_ts = OffsetDateTime::from_unix_timestamp(indexed_ts).into_diagnostic()?;

    let songs: Vec<library::ActiveModel> = WalkDir::new(&source.path)
        .into_iter()
        .filter_map(Result::ok)
        .collect::<Vec<_>>()
        .par_iter()
        .filter(|e| !e.file_type().is_dir()) // Exclude directories
        .filter(|e| {
            mime_guess::from_path(e.path())
                .first()
                .is_some_and(|v| v.type_() == mime::AUDIO) // Exclude non-audio files
        })
        .map(|file| index_song(file, &source, force, indexed_ts))
        .collect::<Result<Vec<_>, EleanorError>>()?
        .into_iter()
        .flatten()
        .collect();

    // Write metadata to database
    library::Entity::insert_many(songs)
        .on_conflict(
            sea_query::OnConflict::column(Column::Hash)
                .update_columns([
                    Column::Artist,
                    Column::AlbumArtist,
                    Column::Name,
                    Column::Album,
                    Column::Duration,
                    Column::Genres,
                    Column::Track,
                    Column::Disc,
                    Column::Year,
                    Column::RgTrackGain,
                    Column::RgTrackPeak,
                    Column::RgAlbumGain,
                    Column::RgAlbumPeak,
                ])
                .clone(),
        )
        .on_empty_do_nothing()
        .exec(db)
        .await?;

    // Update last indexed timestamp
    sources::Entity::insert(sources::ActiveModel {
        id: Set(source.id),
        last_indexed: Set(Some(
            time::OffsetDateTime::now_utc().unix_timestamp().to_string(),
        )),
    })
    .on_conflict(
        sea_query::OnConflict::column(sources::Column::Id)
            .update_column(sources::Column::LastIndexed)
            .clone(),
    )
    .exec(db)
    .await?;

    Ok(())
}
