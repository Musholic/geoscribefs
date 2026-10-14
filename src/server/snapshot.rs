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

#[tonic::async_trait]
impl SnapshotHandler for MyGeoScribeFsService {
    async fn handle_snapshot(
        &self,
        volume_name: &str,
    ) -> Result<Response<SnapshotResponse>, Status> {
        let now = chrono::Utc::now()
            .format("%Y-%m-%d_%H-%M-%S%.3f")
            .to_string();
        let volume_manager = self
            .volume_managers
            .get(volume_name)
            .ok_or(Status::not_found("Volume not found"))?;
        volume_manager
            .create_snapshot(&now)
            .await
            .map_err(|e| Status::internal(format!("Failed to create snapshot: {}", e)))?;

        Ok(Response::new(SnapshotResponse {
            success: true,
            date: now,
        }))
    }

    async fn handle_restore(
        &self,
        volume_name: &str,
        date: &str,
    ) -> Result<Response<RestoreResponse>, Status> {
        let volume_manager = self
            .volume_managers
            .get(volume_name)
            .ok_or(Status::not_found("Volume not found"))?;
        volume_manager
            .restore_snapshot(date)
            .await
            .map_err(|e| Status::internal(format!("Failed to restore snapshot: {}", e)))?;

        Ok(Response::new(RestoreResponse { success: true }))
    }

    async fn handle_list_snapshots(
        &self,
        volume_name: &str,
    ) -> Result<Response<ListSnapshotsResponse>, Status> {
        let volume_manager = self
            .volume_managers
            .get(volume_name)
            .ok_or(Status::not_found("Volume not found"))?;
        let snapshots = volume_manager
            .list_snapshots()
            .await
            .map_err(|e| Status::internal(format!("Failed to restore snapshot: {}", e)))?;

        Ok(Response::new(ListSnapshotsResponse { snapshots }))
    }
}
