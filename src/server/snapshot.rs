use crate::{
    proto::{ListSnapshotsResponse, RestoreResponse, SnapshotResponse},
    server::{GeoScribeFsManager, error::FsError},
};
use tokio::process::ChildStdout;

#[tonic::async_trait]
pub trait SnapshotHandler {
    async fn handle_snapshot(&self, volume_name: &str) -> Result<SnapshotResponse, FsError>;
    async fn handle_restore(
        &self,
        volume_name: &str,
        date: &str,
    ) -> Result<RestoreResponse, FsError>;
    async fn handle_list_snapshots(
        &self,
        volume_name: &str,
    ) -> Result<ListSnapshotsResponse, FsError>;
    async fn handle_send_snapshot(&self, volume_name: &str) -> Result<ChildStdout, FsError>;
}

#[tonic::async_trait]
impl SnapshotHandler for GeoScribeFsManager {
    async fn handle_snapshot(&self, volume_name: &str) -> Result<SnapshotResponse, FsError> {
        let now = chrono::Utc::now()
            .format("%Y-%m-%d_%H-%M-%S%.3f")
            .to_string();

        self.get_volume_manager(volume_name)?
            .create_snapshot(&now)
            .await?;

        Ok(SnapshotResponse {
            success: true,
            date: now,
        })
    }

    async fn handle_restore(
        &self,
        volume_name: &str,
        date: &str,
    ) -> Result<RestoreResponse, FsError> {
        self.get_volume_manager(volume_name)?
            .restore_snapshot(date)
            .await?;

        Ok(RestoreResponse { success: true })
    }

    async fn handle_list_snapshots(
        &self,
        volume_name: &str,
    ) -> Result<ListSnapshotsResponse, FsError> {
        let snapshots = self
            .get_volume_manager(volume_name)?
            .list_snapshots()
            .await?;

        Ok(ListSnapshotsResponse { snapshots })
    }

    async fn handle_send_snapshot(&self, volume_name: &str) -> Result<ChildStdout, FsError> {
        let snapshot_date = self.handle_snapshot(volume_name).await?.date;

        let manager = self.get_volume_manager(volume_name)?;
        let mut send_proc = manager.send_snapshot_process(&snapshot_date)?;

        let stdout = send_proc
            .stdout
            .take()
            .ok_or_else(|| FsError::IoOther("Failed to read send process stdout".to_string()))?;

        Ok(stdout)
    }
}
