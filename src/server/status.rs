use tonic::{Response, Status};

use crate::{proto::StatusResponse, server::MyGeoScribeFsService};

#[tonic::async_trait]
pub trait StatusHandler {
    async fn handle_status(&self) -> Result<Response<StatusResponse>, Status>;
}

#[tonic::async_trait]
impl StatusHandler for MyGeoScribeFsService {
    async fn handle_status(&self) -> Result<Response<StatusResponse>, Status> {
        let mut status = format!("Addr: {}\n", self.self_addr);
        status.push_str("Connected to:\n");
        for peer in &self.peers {
            status.push_str(&format!("- {}\n", peer));
        }
        status.push_str("Volumes:\n");
        let volumes = self
            .volumes
            .read()
            .map_err(|e| Status::internal(e.to_string()))?;
        for (k, v) in volumes.iter() {
            status.push_str(&format!("- {}: {}\n", k, v));
        }
        Ok(Response::new(StatusResponse { status }))
    }
}
