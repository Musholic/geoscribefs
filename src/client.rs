use crate::proto::{
    ListSnapshotsRequest, ReceiveSnapshotRequest, RestoreRequest, SendSnapshotRequest,
    SendSnapshotResponse, SnapshotRequest, StatusRequest, WriteRequest,
    geo_scribe_fs_service_client::GeoScribeFsServiceClient,
};
use bytes::Bytes;
use std::{
    io,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio_stream::{Stream, StreamExt};
use tonic::{IntoStreamingRequest, Request, Streaming, transport::Channel};

macro_rules! with_retries {
    ($action:expr) => {{
        let mut attempt = 1;
        loop {
            match $action.await {
                Ok(res) => break Ok(res),
                Err(e) if attempt < 5 => {
                    tracing::warn!(
                        "Request failed (attempt {}/5). Retrying in 2s... Error: {}",
                        attempt,
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    attempt += 1;
                }
                Err(e) => break Err(e),
            }
        }
    }};
}

#[derive(Clone)]
pub struct AuthInterceptor {
    token: tonic::metadata::MetadataValue<tonic::metadata::Ascii>,
}

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(&mut self, mut req: Request<()>) -> Result<Request<()>, tonic::Status> {
        req.metadata_mut()
            .insert("authorization", self.token.clone());
        Ok(req)
    }
}

pub struct GeoScribeClient {
    inner: GeoScribeFsServiceClient<
        tonic::service::interceptor::InterceptedService<Channel, AuthInterceptor>,
    >,
}

impl GeoScribeClient {
    pub async fn connect(
        addr: String,
        token: String,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let channel =
            tonic::transport::Endpoint::from_shared(format!("http://{}", addr))?.connect_lazy();

        let auth_token = token.parse()?;

        let interceptor = AuthInterceptor { token: auth_token };
        let inner = GeoScribeFsServiceClient::with_interceptor(channel, interceptor);

        Ok(Self { inner })
    }

    pub async fn status(&mut self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let res = with_retries!(self.inner.status(StatusRequest {}))?;
        Ok(res.into_inner().status)
    }

    pub async fn write(
        &mut self,
        volume_name: String,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let res = with_retries!(self.inner.write(WriteRequest {
            volume_name: volume_name.clone()
        }))?;
        Ok(res.into_inner().success)
    }

    pub async fn snapshot(
        &mut self,
        volume_name: String,
    ) -> Result<(bool, String), Box<dyn std::error::Error + Send + Sync>> {
        let res = with_retries!(self.inner.snapshot(SnapshotRequest {
            volume_name: volume_name.clone()
        }))?;
        let res = res.into_inner();
        Ok((res.success, res.date))
    }

    pub async fn restore(
        &mut self,
        volume_name: String,
        date: String,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let res = with_retries!(self.inner.restore(RestoreRequest {
            volume_name: volume_name.clone(),
            date: date.clone()
        }))?;
        Ok(res.into_inner().success)
    }

    pub async fn list_snapshots(
        &mut self,
        volume_name: String,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let res = with_retries!(self.inner.list_snapshots(ListSnapshotsRequest {
            volume_name: volume_name.clone()
        }))?;
        Ok(res.into_inner().snapshots)
    }
    pub async fn send_snapshot(
        &mut self,
        volume_name: String,
    ) -> Result<Streaming<SendSnapshotResponse>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self
            .inner
            .send_snapshot(SendSnapshotRequest {
                volume_name: volume_name.clone(),
            })
            .await?
            .into_inner())
    }

    /// Streams snapshot bytes into the remote receive process.
    pub async fn receive_snapshot(
        &mut self,
        volume_name: &str,
        chunks: impl Stream<Item = io::Result<Bytes>> + Send + 'static,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let read_err = Arc::new(OnceLock::new());
        let err_slot = Arc::clone(&read_err);

        // Capture errors during the streaming process
        let messages = chunks.map_while(move |chunk| match chunk {
            Ok(data) => Some(ReceiveSnapshotRequest { data: data.into() }),
            Err(err) => {
                let _ = err_slot.set(err);
                None
            }
        });

        let mut request = messages.into_streaming_request();
        request
            .metadata_mut()
            .insert("volume_name", volume_name.parse()?);

        let res = self.inner.receive_snapshot(request).await;

        if let Some(err) = read_err.get() {
            return Err(err.to_string().into());
        }

        Ok(res?.into_inner().success)
    }
}
