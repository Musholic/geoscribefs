use crate::proto::{
    ListSnapshotsRequest, RestoreRequest, SnapshotRequest, StatusRequest, WriteRequest,
    geo_scribe_fs_service_client::GeoScribeFsServiceClient,
};
use std::time::Duration;
use tonic::{Request, transport::Channel};

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
    pub async fn connect(addr: String, token: String) -> Result<Self, Box<dyn std::error::Error>> {
        let channel =
            tonic::transport::Endpoint::from_shared(format!("http://{}", addr))?.connect_lazy();

        let auth_token = token.parse()?;

        let interceptor = AuthInterceptor { token: auth_token };
        let inner = GeoScribeFsServiceClient::with_interceptor(channel, interceptor);

        Ok(Self { inner })
    }

    pub async fn status(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let res = with_retries!(self.inner.status(StatusRequest {}))?;
        Ok(res.into_inner().status)
    }

    pub async fn write(&mut self, volume_name: String) -> Result<bool, Box<dyn std::error::Error>> {
        let res = with_retries!(self.inner.write(WriteRequest {
            volume_name: volume_name.clone()
        }))?;
        Ok(res.into_inner().success)
    }

    pub async fn snapshot(
        &mut self,
        volume_name: String,
    ) -> Result<(bool, String), Box<dyn std::error::Error>> {
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
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let res = with_retries!(self.inner.restore(RestoreRequest {
            volume_name: volume_name.clone(),
            date: date.clone()
        }))?;
        Ok(res.into_inner().success)
    }

    pub async fn list_snapshots(
        &mut self,
        volume_name: String,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let res = with_retries!(self.inner.list_snapshots(ListSnapshotsRequest {
            volume_name: volume_name.clone()
        }))?;
        Ok(res.into_inner().snapshots)
    }
}
