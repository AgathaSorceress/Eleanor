use std::{
    fs::File,
    io::{Error, ErrorKind, Read, Seek, SeekFrom, Write},
    sync::{atomic::AtomicU64, Arc},
};

use miette::miette;
use parking_lot::RwLock;
use reqwest::{
    header::{HeaderValue, CONTENT_LENGTH},
    StatusCode, Url,
};
use reqwest_middleware::ClientWithMiddleware;

use super::{error::EleanorError, utils::cache_dir};

#[derive(Debug)]
pub struct HttpReader {
    client: ClientWithMiddleware,
    auth: (String, String),
    url: Url,
    chunk_size: u64,
    start: AtomicU64,
    fetched_start: AtomicU64,
    end: AtomicU64,
    buffer: Arc<RwLock<Vec<u8>>>,
}

impl HttpReader {
    pub async fn new(
        url: Url,
        client: ClientWithMiddleware,
        chunk_size: u64,
        auth: (String, String),
    ) -> Result<HttpReader, EleanorError> {
        let (username, password) = &auth;

        let response = client
            .head(url.clone())
            .basic_auth(username, Some(password))
            .send()
            .await?;

        let length = response
            .headers()
            .get(CONTENT_LENGTH)
            .ok_or(miette!("No Content-Length header in response"))?
            .to_str()
            .map_err(|_| EleanorError::CastError)
            .and_then(|v| v.parse::<u64>().map_err(|_| EleanorError::CastError))?;

        let reader = HttpReader {
            url,
            client,
            chunk_size,
            auth,
            start: AtomicU64::new(0),
            fetched_start: AtomicU64::new(0),
            end: AtomicU64::new(length - 1),
            buffer: Arc::new(RwLock::new(vec![])),
        };

        Ok(reader)
    }

    pub async fn start(&mut self) {
        tokio::spawn(async {
            loop {
                fetch_song_chunks(
                    self.auth.clone(),
                    &self.client,
                    self.chunk_size,
                    self.url.clone(),
                    &mut self.fetched_start,
                    &mut self.end,
                    self.buffer.clone(),
                );
            }
        });
    }
}

async fn fetch_song_chunks(
    auth: (String, String),
    client: &ClientWithMiddleware,
    chunk_size: u64,
    url: Url,
    fetched_start: &mut AtomicU64,
    end: &mut AtomicU64,
    buffer: Arc<RwLock<Vec<u8>>>,
) -> Result<(), EleanorError> {
    let mut fetched_start = *fetched_start.get_mut();
    let end = *end.get_mut();

    if fetched_start > end {
        // TODO: store buffer to a local file
        Ok(())
    } else {
        let prev = fetched_start;
        fetched_start = std::cmp::min(chunk_size, end - fetched_start + 1);

        let range = reqwest::header::HeaderValue::from_str(&format!(
            "bytes={}-{}",
            prev,
            fetched_start - 1
        ))
        .map_err(|e| miette!("Invalid header: {}", e))?;

        let (bytes, status) = get_chunk(auth, client, &url, range).await?;

        if status == reqwest::StatusCode::OK || status == reqwest::StatusCode::PARTIAL_CONTENT {
            let mut buffer = buffer.write();
            (*buffer).extend(bytes);

            Ok(())
        } else {
            Err(miette!(
                "Failed to fetch song chunk: {prev}-{} for track {url}",
                (fetched_start - 1)
            )
            .into())
        }
    }
}

/// Returns a chunk of bytes and the status code of the response
async fn get_chunk(
    (username, password): (String, String),
    client: &ClientWithMiddleware,
    url: &Url,
    range: HeaderValue,
) -> Result<(Vec<u8>, StatusCode), EleanorError> {
    let res = client
        .get(url.clone())
        .header(reqwest::header::RANGE, range)
        .basic_auth(username, Some(password))
        .send()
        .await?;

    let status = res.status();
    let bytes = &res.bytes().await?;

    Ok((bytes.to_vec(), status))
}
