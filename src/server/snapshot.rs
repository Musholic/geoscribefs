use tonic::{Response, Status};

use crate::{
    proto::{ListSnapshotsResponse, RestoreResponse, SnapshotResponse},
    server::MyGeoScribeFsService,
};

#[tonic::async_trait]
pub trait SnapshotHandler {
    async fn handle_snapshot(
        &self,
        volume_name: &str,
    ) -> Result<Response<SnapshotResponse>, Status>;
    async fn handle_restore(
        &self,
        volume_name: &str,
        date: &str,
    ) -> Result<Response<RestoreResponse>, Status>;
    async fn handle_list_snapshots(
        &self,
        volume_name: &str,
    ) -> Result<Response<ListSnapshotsResponse>, Status>;
}

fn get_volume_path(base_volumes_path: &str, volume_name: &str) -> String {
    std::path::Path::new(base_volumes_path)
        .join(volume_name)
        .to_string_lossy()
        .into_owned()
}

fn get_snapshots_dir(
    base_volumes_path: &str,
    volume_name: &str,
) -> std::io::Result<std::path::PathBuf> {
    let snapshots_dir = std::path::Path::new(base_volumes_path)
        .join(".snapshots")
        .join(volume_name);

    std::fs::create_dir_all(&snapshots_dir)?;

    Ok(snapshots_dir)
}

fn get_snapshot_path(
    base_volumes_path: &str,
    volume_name: &str,
    date: &str,
) -> std::io::Result<String> {
    let snapshots_dir = get_snapshots_dir(base_volumes_path, volume_name)?;

    Ok(snapshots_dir.join(date).to_string_lossy().into_owned())
}

#[tonic::async_trait]
impl SnapshotHandler for MyGeoScribeFsService {
    async fn handle_snapshot(
        &self,
        volume_name: &str,
    ) -> Result<Response<SnapshotResponse>, Status> {
        let now = chrono::Utc::now()
            .format("%Y-%m-%d_%H-%M-%S%.3f")
            .to_string();
        let volume_path = get_volume_path(&self.base_volumes_path, volume_name);
        let snapshot_path = get_snapshot_path(&self.base_volumes_path, volume_name, &now)?;

        tracing::trace!("Snapshotting volume: {} to {}", volume_path, snapshot_path);
        let status = std::process::Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r", &volume_path, &snapshot_path])
            .status()
            .map_err(|e| Status::internal(format!("Failed to execute btrfs command: {}", e)))?;

        if status.success() {
            Ok(Response::new(SnapshotResponse {
                success: true,
                date: now,
            }))
        } else {
            Err(Status::internal("btrfs snapshot command failed"))
        }
    }

    async fn handle_restore(
        &self,
        volume_name: &str,
        date: &str,
    ) -> Result<Response<RestoreResponse>, Status> {
        let volume_path = get_volume_path(&self.base_volumes_path, volume_name);
        let snapshot_path = get_snapshot_path(&self.base_volumes_path, volume_name, date)?;

        tracing::debug!("Restoring volume: {} from {}", volume_path, snapshot_path);

        // To restore, we delete the existing volume and snapshot the backup back
        let delete_status = std::process::Command::new("btrfs")
            .args(["subvolume", "delete", &volume_path])
            .status()
            .map_err(|e| Status::internal(format!("Failed to execute btrfs delete: {}", e)))?;

        if !delete_status.success() {
            return Err(Status::internal(
                "Failed to delete existing volume for restore",
            ));
        }

        let restore_status = std::process::Command::new("btrfs")
            .args(["subvolume", "snapshot", &snapshot_path, &volume_path])
            .status()
            .map_err(|e| Status::internal(format!("Failed to execute btrfs snapshot: {}", e)))?;

        if restore_status.success() {
            Ok(Response::new(RestoreResponse { success: true }))
        } else {
            Err(Status::internal("btrfs restore snapshot command failed"))
        }
    }

    async fn handle_list_snapshots(
        &self,
        volume_name: &str,
    ) -> Result<Response<ListSnapshotsResponse>, Status> {
        let snapshots_dir = get_snapshots_dir(&self.base_volumes_path, volume_name)?;

        let snapshots = std::fs::read_dir(&snapshots_dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(Response::new(ListSnapshotsResponse { snapshots }))
    }
}
