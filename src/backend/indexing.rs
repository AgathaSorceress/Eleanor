use std::{ffi::OsStr, fs::File, hash::Hasher, path::Path};

use adler::Adler32;
use lofty::{AudioFile, TaggedFileExt};
use miette::{miette, IntoDiagnostic, Result};
use rayon::prelude::*;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect, Set};
use symphonia::{
    core::{
        errors::Error as SymphoniaError,
        formats::{FormatOptions, FormatReader, Packet},
        io::{MediaSourceStream, MediaSourceStreamOptions},
        meta::{Limit, MetadataOptions},
        probe::Hint,
    },
    default::get_probe,
};
use time::OffsetDateTime;
use tracing::debug;
use walkdir::{DirEntry, WalkDir};

use crate::backend::replaygain::{ReplayGain, ReplayGainResult};

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

struct FormatReaderIter {
    inner: Box<dyn FormatReader>,
    error: Option<SymphoniaError>,
    hash: Adler32,
    rg: ReplayGainState,
}

enum ReplayGainState {
    Finished(ReplayGainResult),
    Computing(ReplayGain),
    Failed(EleanorError),
}

impl ReplayGainState {
    fn handle_packet(&mut self, packet: &Packet) {
        if let ReplayGainState::Computing(rg) = self {
            let result = rg.handle_packet(packet);

            if let Err(e) = result {
                *self = ReplayGainState::Failed(e);
            }
        }
    }
}

impl FormatReaderIter {
    /// Initialize a new iterator over `FormatReader`.
    /// Track ReplayGain values will be computed if `rg` is None.
    fn new(
        inner: Box<dyn FormatReader>,
        rg: Option<ReplayGainResult>,
    ) -> Result<Self, EleanorError> {
        let track = inner
            .default_track()
            .ok_or_else(|| miette!("No default track was found"))?;
        let params = &track.codec_params;

        let (sample_rate, channels) = (params.sample_rate, params.channels);

        // Only stereo is supported.
        if !channels.is_some_and(|x| x.count() == 2) {
            return Err(miette!("Unsupported channel configuration: {:?}", channels).into());
        }

        let Some(sample_rate) = sample_rate else {
            return Err(miette!("Sample rate must be known").into());
        };

        let rg = match rg {
            Some(rg_res) => ReplayGainState::Finished(rg_res),
            None => match ReplayGain::init(sample_rate as usize, track)
                .map(ReplayGainState::Computing)
            {
                Ok(rg) => rg,
                Err(e) => ReplayGainState::Failed(e),
            },
        };

        Ok(Self {
            inner,
            error: None,
            hash: Adler32::new(),
            rg,
        })
    }

    fn process(mut self) -> (u64, Result<ReplayGainResult, EleanorError>) {
        // loop over all packets
        while let Some(packet) = (&mut self).next() {
            // hash the packet
            self.hash.write(&packet.data);
            // copy into replaygain
            self.rg.handle_packet(&packet);
        }

        let hash = self.hash.finish();

        // check for error during iteration
        if let Some(error) = self.error {
            return (hash, Err(error.into()));
        }

        let rg = match self.rg {
            ReplayGainState::Finished(rg_res) => Ok(rg_res),
            ReplayGainState::Computing(rg) => rg.finish(),
            ReplayGainState::Failed(e) => Err(e),
        };

        (hash, rg)
    }
}

impl Iterator for &mut FormatReaderIter {
    type Item = Packet;

    fn next(&mut self) -> Option<Self::Item> {
        let res_packet = self.inner.next_packet();
        match res_packet {
            Err(symphonia::core::errors::Error::IoError(ref packet_error))
                if packet_error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                None
            }
            Err(e) => {
                self.error = Some(e);
                None
            }
            Ok(packet) => Some(packet),
        }
    }
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

    // Hash audio packets and calculate replaygain
    let (hash, rg) = FormatReaderIter::new(
        get_packets(file.path())?,
        ReplayGainResult::try_from(tags).ok(),
    )?
    .process();

    let rg = rg?;

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
        rg_track_gain: Set(Some(rg.track_gain.into())),
        rg_track_peak: Set(Some(rg.track_peak.into())),
        rg_album_gain: Set(rg.album_gain.map(f64::from)),
        rg_album_peak: Set(rg.album_peak.map(f64::from)),
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
