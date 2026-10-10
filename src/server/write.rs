use tonic::{Request, Response, Status};

use crate::{
    proto::{
        AskWriteRequest, AskWriteResponse, WriteResponse,
        geo_scribe_fs_service_client::GeoScribeFsServiceClient,
    },
    server::MyGeoScribeFsService,
};

#[tonic::async_trait]
pub trait WriteHandler {
    async fn handle_write(&self, volume_name: &str) -> Result<Response<WriteResponse>, Status>;
    async fn handle_ask_write(
        &self,
        volume_name: &str,
        remote_addr: &str,
    ) -> Result<Response<AskWriteResponse>, Status>;
}

#[tonic::async_trait]
impl WriteHandler for MyGeoScribeFsService {
    async fn handle_write(&self, volume_name: &str) -> Result<Response<WriteResponse>, Status> {
        tracing::debug!("Writing to volume {}", volume_name);

        for peer in &self.peers {
            let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", peer))
                .map_err(|e| Status::internal(e.to_string()))?
                .connect()
                .await
                .map_err(|e| {
                    Status::internal(format!("Failed to connect to peer {}: {}", peer, e))
                })?;

            let token = self.token.clone();

            #[allow(clippy::result_large_err)]
            let mut client =
                GeoScribeFsServiceClient::with_interceptor(channel, move |mut req: Request<()>| {
                    let token_val = token
                        .parse()
                        .map_err(|_| Status::internal("Failed to parse authorization token"))?;
                    let addr_val = self
                        .self_addr
                        .parse()
                        .map_err(|_| Status::internal("Failed to parse self address"))?;

                    req.metadata_mut().insert("authorization", token_val);
                    req.metadata_mut().insert("address", addr_val);
                    Ok(req)
                });

            let ask_request = tonic::Request::new(AskWriteRequest {
                volume_name: volume_name.to_string(),
            });

            let response = client
                .ask_write(ask_request)
                .await
                .map_err(|e| Status::internal(format!("Peer {} error: {}", peer, e)))?;

            if !response.into_inner().can_write {
                return Err(Status::already_exists(format!(
                    "Peer {} denied write for volume {}",
                    peer, volume_name
                )));
            }
        }

        tracing::debug!("Volume {} can be written to", volume_name);
        self.volumes
            .write()
            .map_err(|e| Status::internal(e.to_string()))?
            .insert(volume_name.to_string(), self.self_addr.clone());

        Ok(Response::new(WriteResponse { success: true }))
    }

    async fn handle_ask_write(
        &self,
        volume_name: &str,
        remote_addr: &str,
    ) -> Result<Response<AskWriteResponse>, Status> {
        self.volumes
            .write()
            .map_err(|e| Status::internal(e.to_string()))?
            .insert(volume_name.to_string(), remote_addr.to_string());

        Ok(Response::new(AskWriteResponse { can_write: true }))
    }
}
