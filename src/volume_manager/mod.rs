pub mod btrfs;
pub mod zfs;

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use nix::sys::statfs::{FsType, statfs};
use thiserror::Error;
use tokio::process::Child;

use crate::{
    fuse::{self, GeoScribeFs},
    server::error::FsError,
    volume_manager::{btrfs::BtrfsVolumeManager, zfs::ZfsVolumeManager},
};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Failed to execute storage command: {0}")]
    CommandFailed(String),
    #[error("Volume or snapshot not found: {0}")]
    NotFound(String),
    #[error("The filesystem type is not supported or detected for path: {0}")]
    FsNotSupported(String),
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("FS error: {0}")]
    FsError(#[from] Box<FsError>),
}

impl From<FsError> for StorageError {
    fn from(err: FsError) -> Self {
        StorageError::FsError(Box::new(err))
    }
}

#[tonic::async_trait]
pub trait VolumeManager: Send + Sync {
    /// Creates a read-only snapshot.
    async fn create_snapshot(&self, date: &str) -> Result<(), StorageError>;

    /// Restores to a specific snapshot.
    async fn restore_snapshot(&self, date: &str) -> Result<(), StorageError>;

    /// Lists all snapshots for the volume.
    async fn list_snapshots(&self) -> Result<Vec<String>, StorageError>;

    fn receive_snapshot_process(&self) -> io::Result<Child>;

    fn send_snapshot_process(&self, snapshot_date: &str) -> io::Result<Child>;

    fn volume_path(&self) -> PathBuf;

    fn volume_name(&self) -> String;
}

#[derive(Clone, Copy)]
pub enum FsBackend {
    Btrfs,
    Zfs,
}

fn get_fs_backend(volume: &Path) -> Option<FsBackend> {
    let stat = statfs(volume).ok()?;

    // ZFS magic number is 0x2fc12fc1
    const ZFS_SUPER_MAGIC: FsType = FsType(0x2fc12fc1);

    match stat.filesystem_type() {
        nix::sys::statfs::BTRFS_SUPER_MAGIC => Some(FsBackend::Btrfs),
        ZFS_SUPER_MAGIC => Some(FsBackend::Zfs),
        _ => None,
    }
}

pub fn mount_and_get(cfg: fuse::Config) -> Result<Arc<dyn VolumeManager>, StorageError> {
    tracing::debug!(volume_path = ?cfg.volume, "Mounting volume");
    let volume = cfg.volume.clone();

    // We must check what is the fs backend before overmounting our fuse fs
    let backend = get_fs_backend(&volume)
        .ok_or_else(|| StorageError::FsNotSupported(volume.to_string_lossy().to_string()))?;

    let mount = GeoScribeFs::new(cfg)?.start()?;

    match backend {
        FsBackend::Btrfs => Ok(Arc::new(BtrfsVolumeManager::new(mount)?)),
        FsBackend::Zfs => Ok(Arc::new(ZfsVolumeManager::new(mount)?)),
    }
}
