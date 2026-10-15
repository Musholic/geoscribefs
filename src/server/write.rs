use tonic::{Request, Status};

use crate::{
    proto::{
        AskWriteRequest, AskWriteResponse, WriteResponse,
        geo_scribe_fs_service_client::GeoScribeFsServiceClient,
    },
    server::{GeoScribeFsManager, error::FsError},
};

#[tonic::async_trait]
pub trait WriteHandler {
    async fn handle_write(&self, volume_name: &str) -> Result<WriteResponse, FsError>;
    async fn handle_ask_write(
        &self,
        volume_name: &str,
        remote_addr: &str,
    ) -> Result<AskWriteResponse, FsError>;
}

#[tonic::async_trait]
impl WriteHandler for GeoScribeFsManager {
    async fn handle_write(&self, volume_name: &str) -> Result<WriteResponse, FsError> {
        tracing::debug!("Writing to volume {}", volume_name);

        for peer in &self.peers {
            let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", peer))?
                .connect()
                .await?;

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

            let response = client.ask_write(ask_request).await?.into_inner();

            if !response.can_write {
                return Err(FsError::DeniedWrite(peer.clone(), volume_name.to_string()));
            }
        }

        tracing::debug!("Volume {} can be written to", volume_name);
        self.volumes
            .write()
            .map_err(|e| Status::internal(e.to_string()))?
            .insert(volume_name.to_string(), self.self_addr.clone());

        Ok(WriteResponse { success: true })
    }

    async fn handle_ask_write(
        &self,
        volume_name: &str,
        remote_addr: &str,
    ) -> Result<AskWriteResponse, FsError> {
        self.volumes
            .write()
            .map_err(|e| FsError::LockPoisoned(e.to_string()))?
            .insert(volume_name.to_string(), remote_addr.to_string());

        Ok(AskWriteResponse { can_write: true })
    }
}
