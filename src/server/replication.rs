use std::{sync::Arc, time::Duration};

use tokio::time::interval;
use tokio_util::io::ReaderStream;

use crate::{
    client::GeoScribeClient,
    server::{GeoScribeFsManager, ReplicaConfig, error::FsError, snapshot::SnapshotHandler},
};

/// Pushes an incremental snapshot of each replicated volume to its configured
/// warm replica on every tick of `interval`.
pub struct ReplicationScheduler {
    manager: Arc<GeoScribeFsManager>,
    /// Volumes to replicate, each with the replica it is pushed to.
    replicas: Vec<ReplicaConfig>,
    /// How often snapshots are pushed to each replica.
    interval: Duration,
}

impl ReplicationScheduler {
    pub fn new(
        manager: Arc<GeoScribeFsManager>,
        replicas: Vec<ReplicaConfig>,
        interval: Duration,
    ) -> Self {
        Self {
            manager,
            replicas,
            interval,
        }
    }

    /// Detaches the scheduling loop onto the current runtime.
    ///
    /// Failures are logged and skipped: the next tick retries with a fresh
    /// snapshot. The loop runs until the process exits, so its handle is
    /// deliberately not kept around.
    pub fn spawn(self) {
        tokio::spawn(self.run());
    }

    async fn run(self) {
        let mut ticker = interval(self.interval);
        loop {
            ticker.tick().await;
            for replica in &self.replicas {
                if let Err(err) = self.push_snapshot_to_replica(replica).await {
                    tracing::warn!(
                        ?replica,
                        %err,
                        "Failed to push snapshot to replica"
                    );
                }
            }
        }
    }

    async fn push_snapshot_to_replica(&self, replica: &ReplicaConfig) -> Result<(), FsError> {
        tracing::debug!(?replica, "Pushing snapshot to replica");

        let mut client =
            GeoScribeClient::connect(replica.addr.clone(), self.manager.token.clone()).await?;
        let stdout = self
            .manager
            .handle_send_snapshot(&replica.volume_name)
            .await?;

        client
            .receive_snapshot(&replica.volume_name, ReaderStream::new(stdout))
            .await?;

        Ok(())
    }
}
