use crate::{
    client::GeoScribeClient,
    fuse::GeoScribeFs,
    server::{GeoScribeFsManager, error::FsError, snapshot::SnapshotHandler},
};
use bytes::Bytes;
use std::io;
use tokio_stream::StreamExt;

#[tonic::async_trait]
pub trait SyncHandler {
    async fn sync(&self, fs: &GeoScribeFs) -> Result<(), FsError>;
}

/// Pulls the latest snapshot from the volume's owner and save it in the snapshots directory of this volume
async fn download_latest_remote_snapshot(
    manager: &GeoScribeFsManager,
    volume_name: &str,
    mut client: GeoScribeClient,
) -> Result<(), FsError> {
    // Unwrap the stream to a generic bytes stream for the generic handle_receive_snapshot method
    let chunks = client
        .send_snapshot(volume_name.to_string())
        .await?
        .map(|chunk| {
            chunk
                .map(|msg| Bytes::from(msg.data))
                .map_err(io::Error::other)
        });

    manager.handle_receive_snapshot(volume_name, chunks).await
}

#[tonic::async_trait]
impl SyncHandler for GeoScribeFsManager {
    async fn sync(&self, fs: &GeoScribeFs) -> Result<(), FsError> {
        let volume = self.get_volume(&fs.volume_name)?;
        let owner = volume.owner;
        if owner == self.self_addr {
            // We are the owner, no need to sync
            return Ok(());
        }

        tracing::trace!(volume_name = %fs.volume_name, "Start sync");
        let manager = self.get_volume_manager(&fs.volume_name)?;
        let client = GeoScribeClient::connect(owner, self.token.clone()).await?;

        // Detach the mount so that we don't keep any opened file that could prevent restore
        fs.park().await?;
        fs.detach().await?;
        download_latest_remote_snapshot(self, &fs.volume_name, client).await?;
        // Get the latest snapshot (the one we just downloaded) and restore it
        let snapshots = manager.list_snapshots().await?;
        let latest_snapshot = snapshots
            .last()
            .ok_or_else(|| FsError::NoSnapshotFound(fs.volume_name.to_string()))?;

        manager.restore_snapshot(latest_snapshot).await?;
        // Attach back after restore since we detached before
        fs.attach().await?;
        fs.unpark().await?;

        Ok(())
    }
}
