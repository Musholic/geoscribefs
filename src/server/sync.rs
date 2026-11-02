use crate::{
    client::GeoScribeClient,
    fuse::GeoScribeFs,
    server::{GeoScribeFsManager, error::FsError},
    volume_manager::VolumeManager,
};
use tokio::io::AsyncWriteExt;

#[tonic::async_trait]
pub trait SyncHandler {
    async fn sync(&self, fs: &GeoScribeFs) -> Result<bool, FsError>;
}

async fn download_latest_remote_snapshot(
    manager: &dyn VolumeManager,
    mut client: GeoScribeClient,
) -> Result<(), FsError> {
    let mut child = manager.receive_snapshot_process()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| FsError::IoOther("stdin not available".to_string()))?;

    let mut stream = client
        .send_snapshot(manager.volume_name().to_string())
        .await?;

    while let Some(chunk) = stream.message().await? {
        stdin.write_all(&chunk.data).await?;
    }

    drop(stdin);
    let status = child.wait().await?;
    if status.success() {
        Ok(())
    } else {
        Err(FsError::IoOther("btrfs receive failed".to_string()))
    }
}

#[tonic::async_trait]
impl SyncHandler for GeoScribeFsManager {
    async fn sync(&self, fs: &GeoScribeFs) -> Result<bool, FsError> {
        let volume = self.get_volume(&fs.volume_name)?;
        let owner = volume.owner;
        if owner == self.self_addr {
            // We are the owner, no need to sync
            return Ok(false);
        }

        let manager = self.get_volume_manager(&fs.volume_name)?;
        let client = GeoScribeClient::connect(owner, self.token.clone()).await?;
        download_latest_remote_snapshot(manager.as_ref(), client).await?;
        // Get the latest snapshot (the one we just downloaded) and restore it
        let snapshots = manager.list_snapshots().await?;
        let latest_snapshot = snapshots
            .last()
            .ok_or_else(|| FsError::NoSnapshotFound(fs.volume_name.to_string()))?;

        // First unmount the fuse fs to be able to restore
        fs.umount()?;

        manager.restore_snapshot(latest_snapshot).await?;

        Ok(true)
    }
}
