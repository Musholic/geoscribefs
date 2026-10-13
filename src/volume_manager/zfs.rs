use crate::{
    fuse,
    volume_manager::{StorageError, VolumeManager},
};

pub struct ZfsVolumeManager {
    pub mount: fuse::MountHandle,
    pub dataset_name: String,
}

/// Resolve the mountpoint to a ZFS dataset name
fn get_dataset_name(mountpoint: &str) -> Result<String, StorageError> {
    let output = std::process::Command::new("zfs")
        .args(["list", "-H", "-o", "name", mountpoint])
        .output()
        .map_err(|e| StorageError::CommandFailed(format!("Failed to execute zfs list: {}", e)))?;

    if !output.status.success() {
        return Err(StorageError::CommandFailed(
            "Could not resolve mountpoint to ZFS dataset".to_string(),
        ));
    }

    let dataset_name = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if dataset_name.is_empty() {
        return Err(StorageError::NotFound(
            "ZFS dataset name not found".to_string(),
        ));
    }

    Ok(dataset_name)
}

impl ZfsVolumeManager {
    pub fn new(mount: fuse::MountHandle) -> Result<Self, StorageError> {
        let mountpoint = &mount.mountpoint;

        let dataset_name = get_dataset_name(mountpoint)?;

        Ok(Self {
            mount,
            dataset_name,
        })
    }
}

#[tonic::async_trait]
impl VolumeManager for ZfsVolumeManager {
    async fn create_snapshot(&self, date: &str) -> Result<(), StorageError> {
        let snapshot_name = format!("{}@{}", self.dataset_name, date);

        tracing::trace!(
            "Snapshotting ZFS dataset: {} as {}",
            self.dataset_name,
            snapshot_name
        );

        let status = std::process::Command::new("zfs")
            .args(["snapshot", &snapshot_name])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(StorageError::CommandFailed(
                "zfs snapshot command failed".to_string(),
            ))
        }
    }

    async fn restore_snapshot(&self, date: &str) -> Result<(), StorageError> {
        let snapshot_name = format!("{}@{}", self.dataset_name, date);
        tracing::debug!(
            "Restoring ZFS dataset: {} from snapshot {}",
            self.dataset_name,
            snapshot_name
        );

        // -r recursively rolls back snapshots of dependent datasets
        let status = std::process::Command::new("zfs")
            .args(["rollback", "-r", &snapshot_name])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(StorageError::CommandFailed(
                "zfs rollback command failed".to_string(),
            ))
        }
    }

    async fn list_snapshots(&self) -> Result<Vec<String>, StorageError> {
        let output = std::process::Command::new("zfs")
            .args([
                "list",
                "-H",
                "-t",
                "snapshot",
                "-o",
                "name",
                &self.dataset_name,
            ])
            .output()?;

        if !output.status.success() {
            return Err(StorageError::CommandFailed(
                "zfs list snapshots command failed".to_string(),
            ));
        }

        let snapshots = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                // ZFS returns "pool/dataset@snapshot", we only want the "snapshot" part
                line.split('@').next_back().map(|s| s.to_string())
            })
            .collect();

        Ok(snapshots)
    }
}
