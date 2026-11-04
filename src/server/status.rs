use crate::{
    proto::StatusResponse,
    server::{GeoScribeFsManager, error::FsError},
};

#[tonic::async_trait]
pub trait StatusHandler {
    async fn handle_status(&self) -> Result<StatusResponse, FsError>;
}

#[tonic::async_trait]
impl StatusHandler for GeoScribeFsManager {
    async fn handle_status(&self) -> Result<StatusResponse, FsError> {
        let mut status = format!("Addr: {}\n", self.self_addr);
        status.push_str("Connected to:\n");
        for peer in &self.peers {
            status.push_str(&format!("- {}\n", peer));
        }
        status.push_str("Volumes:\n");
        let volumes = self
            .volumes
            .read()
            .map_err(|e| FsError::LockPoisoned(e.to_string()))?;

        for (k, v) in volumes.iter() {
            status.push_str(&format!("- {}: {}\n", k, v.owner));
        }
        Ok(StatusResponse { status })
    }
}
