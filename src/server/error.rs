use std::backtrace::Backtrace;

use crate::volume_manager::StorageError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FsError {
    #[error("Volume not found: {volume}")]
    NotFound {
        volume: String,
        bt: Option<Backtrace>,
    },
    #[error("Volume not mounted: {0}")]
    NotMounted(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Peer {0} denied write for volume {1}")]
    DeniedWrite(String, String),
    #[error("Tonic error: {0}")]
    Tonic(#[source] Box<tonic::transport::Error>),
    #[error("Peer error: {0}")]
    PeerError(#[source] Box<tonic::Status>),
    #[error("Internal concurrency error on lock: {0}")]
    LockPoisoned(String),
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("Unexpected error: {0}")]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("Unexpected error: {0}")]
    IoOther(String),
    #[error("Invalid volume path: {0}")]
    InvalidVolumePath(String),
}

impl From<tonic::transport::Error> for FsError {
    fn from(err: tonic::transport::Error) -> Self {
        Self::Tonic(Box::new(err))
    }
}

impl From<tonic::Status> for FsError {
    fn from(err: tonic::Status) -> Self {
        Self::PeerError(Box::new(err))
    }
}

impl From<FsError> for tonic::Status {
    fn from(err: FsError) -> Self {
        let message = err.to_string();
        match err {
            FsError::NotFound { .. } => tonic::Status::not_found(message),
            _ => tonic::Status::internal(message),
        }
    }
}

impl FsError {
    pub fn backtrace(&self) -> Option<&Backtrace> {
        match self {
            Self::NotFound { bt, .. } => bt.as_ref(),
            _ => None,
        }
    }

    pub fn not_found(volume: &str) -> Self {
        let backtrace = Backtrace::capture();
        tracing::trace!(volume = %volume, backtrace = %backtrace, "Volume not found");
        Self::NotFound {
            volume: volume.to_string(),
            bt: Some(backtrace),
        }
    }
}
