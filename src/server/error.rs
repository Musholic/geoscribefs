use crate::volume_manager::StorageError;

#[derive(thiserror::Error, Debug)]
pub enum FsError {
    #[error("Volume not found: {0}")]
    NotFound(String),
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
            FsError::NotFound(_) => tonic::Status::not_found(message),
            _ => tonic::Status::internal(message),
        }
    }
}
