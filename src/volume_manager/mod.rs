pub mod btrfs;
pub mod zfs;

use std::path::Path;

use nix::sys::statfs::{FsType, statfs};
use thiserror::Error;

use crate::{
    fuse::{self, GeoScribeFs},
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

#[derive(Clone, Copy)]
pub enum FsBackend {
    Btrfs,
    Zfs,
}

fn get_fs_backend(volume: &str) -> Option<FsBackend> {
    let stat = statfs(Path::new(volume)).ok()?;

    // ZFS magic number is 0x2fc12fc1
    const ZFS_SUPER_MAGIC: FsType = FsType(0x2fc12fc1);

    match stat.filesystem_type() {
        nix::sys::statfs::BTRFS_SUPER_MAGIC => Some(FsBackend::Btrfs),
        ZFS_SUPER_MAGIC => Some(FsBackend::Zfs),
        _ => None,
    }
}

pub fn mount_and_get(cfg: fuse::Config) -> Result<Box<dyn VolumeManager>, StorageError> {
    tracing::debug!("Mounting volume {} at address {}", cfg.volume, cfg.addr);
    let volume = cfg.volume.clone();

    let mount = GeoScribeFs::new(cfg)?.start()?;
    let backend = get_fs_backend(&volume).ok_or(StorageError::FsNotSupported(volume.clone()))?;
    match backend {
        FsBackend::Btrfs => Ok(Box::new(BtrfsVolumeManager::new(mount)?)),
        FsBackend::Zfs => Ok(Box::new(ZfsVolumeManager::new(mount)?)),
    }
}
