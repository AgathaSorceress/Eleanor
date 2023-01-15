use std::{fs::File, io::BufReader};

use miette::{miette, IntoDiagnostic, Result};
use rodio::{Decoder, Source};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tokio::sync::Mutex;

use super::{config::SourceKind, http_decoder::HttpReader, model::library, utils::Context};

/// Returns a Decoder of the requested track
pub async fn decode_track(
    ctx: &Mutex<Context>,
    hash: u32,
) -> Result<Box<dyn Source<Item = f32> + Send + Sync>> {
    let context = &ctx.lock().await;

    let track = library::Entity::find()
        .filter(library::Column::Hash.eq(hash))
        .one(&context.db)
        .await
        .into_diagnostic()?
        .ok_or(miette!("Track {} not found", hash))?;

    let sources = &context.config.sources;

    let source = sources
        .into_iter()
        .find(|source| source.id as i32 == track.source_id)
        .ok_or(miette!("Source {} not found", track.source_id))?;

    match &source.source {
        SourceKind::Local { .. } => {
            let file = BufReader::new(
                File::open(format!("{}/{}", track.path, track.filename)).into_diagnostic()?,
            );

            return Ok(Box::new(
                Decoder::new(file)
                    .map_err(|e| return miette!("Failed to decode track: {}", e.to_string()))?
                    .convert_samples(),
            ));
        }
        SourceKind::Remote { address } => {
            let url = reqwest::Url::parse(&format!("{address}/{hash}")).into_diagnostic()?;
            let client = &context.http_client;

            let chunk_size = &context.config.chunk_size_bytes;

            let auth = context
                .auth
                .get(&source.id)
                .ok_or(miette!("Credentials for source not found"))?;

            Ok(Box::new(
                Decoder::new(HttpReader::new(url, client.clone(), chunk_size, auth.clone()).await?)
                    .map_err(|e| return miette!("Failed to decode track: {}", e.to_string()))?
                    .convert_samples(),
            ))
        }
    }
}
