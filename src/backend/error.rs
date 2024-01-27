use std::{fmt::Debug, num::TryFromIntError, sync::PoisonError};

use kdl::KdlError;
use miette::Diagnostic;
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Error, Debug, Diagnostic)]
pub enum EleanorError {
    #[error("Couldn't unlock mutex")]
    LockFailed,
    #[error("Failed to convert types")]
    CastError,
    #[error("Failed to convert from integer: {0}")]
    TryFromIntError(#[from] TryFromIntError),
    #[error("Failed to create probe: {0}")]
    SymponiaError(#[from] symphonia::core::errors::Error),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sea_orm::DbErr),
    #[error("An IO error occured: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Couldn't join thread: {0}")]
    JoinError(#[from] JoinError),
    #[error("{0}")]
    MietteError(Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Error processing KDL: {0}")]
    KdlError(#[from] KdlError),
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
