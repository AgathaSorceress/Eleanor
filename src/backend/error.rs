use std::{fmt::Debug, sync::PoisonError};

use miette::Diagnostic;
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Error, Debug, Diagnostic)]
pub enum EleanorError {
    #[error("Couldn't unlock mutex")]
    LockFailed,
    #[error("Failed to convert types")]
    CastError,
    #[error("Failed to create probe: {0}")]
    SymponiaError(#[from] symphonia::core::errors::Error),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sea_orm::DbErr),
    #[error("Failed to read song metadata: {0}")]
    LoftyError(#[from] lofty::LoftyError),
    #[error("An IO error occured: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Reqwest Middleware error: {0}")]
    MiddlewareError(#[from] reqwest_middleware::Error),
    #[error("MessagePack decoding error: {0}")]
    DecodeError(#[from] rmp_serde::decode::Error),
    #[error("MessagePack encoding error: {0}")]
    EncodeError(#[from] rmp_serde::encode::Error),
    #[error("Couldn't join thread: {0}")]
    JoinError(#[from] JoinError),
    #[error("{0}")]
    MietteError(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl<T> From<PoisonError<T>> for EleanorError {
    fn from(_err: PoisonError<T>) -> Self {
        Self::LockFailed
    }
}

impl From<miette::Error> for EleanorError {
    fn from(err: miette::Error) -> Self {
        let error: Box<dyn std::error::Error + Send + Sync + 'static> = err.into();

        Self::MietteError(error)
    }
}
