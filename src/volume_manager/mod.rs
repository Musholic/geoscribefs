pub mod btrfs;

use thiserror::Error;

use crate::fuse::{self, GeoScribeFs};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Failed to execute storage command: {0}")]
    CommandFailed(String),
    #[error("Volume or snapshot not found: {0}")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[tonic::async_trait]
pub trait VolumeManager: Send + Sync {
    /// Creates a read-only snapshot.
    async fn create_snapshot(&self, date: &str) -> Result<(), StorageError>;

    /// Restores to a specific snapshot.
    async fn restore_snapshot(&self, date: &str) -> Result<(), StorageError>;

    /// Lists all snapshots for the volume.
    async fn list_snapshots(&self) -> Result<Vec<String>, StorageError>;
}

pub fn mount_and_get(cfg: fuse::Config) -> Result<Box<dyn VolumeManager>, StorageError> {
    tracing::debug!(
        "Mounting volume {} at address {} with token {}",
        cfg.volume,
        cfg.addr,
        cfg.token
    );
    let mount = GeoScribeFs::new(cfg)?.start()?;
    let manager = btrfs::BtrfsVolumeManager::new(mount)?;
    Ok(Box::new(manager))
}
