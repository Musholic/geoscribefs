use std::path::{Path, PathBuf};

use crate::{
    fuse,
    volume_manager::{StorageError, VolumeManager},
};

pub struct BtrfsVolumeManager {
    pub mount: fuse::MountHandle,
    pub snapshots_path: PathBuf,
}

impl BtrfsVolumeManager {
    pub fn new(mount: fuse::MountHandle) -> Result<Self, StorageError> {
        let volume_path = PathBuf::from(&mount.mountpoint);
        let volume_name = volume_path
            .file_name()
            .ok_or_else(|| StorageError::NotFound("Could not get volume name".to_string()))?
            .to_string_lossy()
            .into_owned();
        let base_volumes_path = volume_path.parent().ok_or(StorageError::NotFound(
            "Parent volume path not found".to_string(),
        ))?;
        let snapshots_path = base_volumes_path.join(".snapshots").join(volume_name);

        // Ensure the snapshots directory exists
        std::fs::create_dir_all(&snapshots_path)?;

        Ok(Self {
            mount,
            snapshots_path,
        })
    }
}

fn get_snapshot_path(snapshots_path: &Path, date: &str) -> std::io::Result<String> {
    Ok(snapshots_path.join(date).to_string_lossy().into_owned())
}

#[tonic::async_trait]
impl VolumeManager for BtrfsVolumeManager {
    async fn create_snapshot(&self, date: &str) -> Result<(), StorageError> {
        let snapshot_path = get_snapshot_path(&self.snapshots_path, date)?;

        tracing::trace!(
            "Snapshotting volume: {} to {}",
            self.mount.mountpoint,
            snapshot_path
        );
        let status = std::process::Command::new("btrfs")
            .args([
                "subvolume",
                "snapshot",
                "-r",
                &self.mount.mountpoint,
                &snapshot_path,
            ])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(StorageError::CommandFailed(
                "btrfs subvolume snapshot command failed".to_string(),
            ))
        }
    }

    async fn restore_snapshot(&self, date: &str) -> Result<(), StorageError> {
        let snapshot_path = get_snapshot_path(&self.snapshots_path, date)?;
        tracing::debug!(
            "Restoring volume: {} from {}",
            self.mount.mountpoint,
            snapshot_path
        );

        // To restore, we delete the existing volume and snapshot the backup back
        let delete_status = std::process::Command::new("btrfs")
            .args(["subvolume", "delete", &self.mount.mountpoint])
            .status()?;

        if !delete_status.success() {
            return Err(StorageError::CommandFailed(
                "btrfs subvolume delete command failed".to_string(),
            ));
        }

        let restore_status = std::process::Command::new("btrfs")
            .args([
                "subvolume",
                "snapshot",
                &snapshot_path,
                &self.mount.mountpoint,
            ])
            .status()?;

        if restore_status.success() {
            Ok(())
        } else {
            Err(StorageError::CommandFailed(
                "btrfs subvolume snapshot command failed".to_string(),
            ))
        }
    }

    async fn list_snapshots(&self) -> Result<Vec<String>, StorageError> {
        let snapshots = std::fs::read_dir(&self.snapshots_path)
            .map(|entries| {
                entries
                    .flatten()
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(snapshots)
    }
}
