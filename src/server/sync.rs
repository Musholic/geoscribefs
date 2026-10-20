use tokio::{io::AsyncWriteExt, process::Command};

use crate::{
    client::GeoScribeClient,
    server::{GeoScribeFsManager, error::FsError},
};

#[tonic::async_trait]
pub trait SyncHandler {
    async fn sync(&self, volume: &str) -> Result<(), FsError>;
}

#[tonic::async_trait]
impl SyncHandler for GeoScribeFsManager {
    async fn sync(&self, volume_name: &str) -> Result<(), FsError> {
        let volume = self.get_volume(volume_name)?;
        let owner = volume.owner;
        let manager = self.get_volume_manager(volume_name)?;
        let volume_path = manager.volume_path();
        // Hardcode btrfs for now
        let mut client = GeoScribeClient::connect(owner, self.token.clone()).await?;
        let mut child = Command::new("btrfs")
            .args(["receive", &volume_path.to_string_lossy()])
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| FsError::IoOther("stdin not available".to_string()))?;

        let mut stream = client.send_snapshot(volume_name.to_string()).await?;

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
}
