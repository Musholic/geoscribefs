use std::time::Duration;

use crate::proto::{
    StatusRequest, WriteRequest, geo_scribe_fs_service_client::GeoScribeFsServiceClient,
};
use tonic::Request;

pub enum ClientCommand {
    Status,
    Write,
}

pub async fn run_client(
    addr: String,
    cmd: ClientCommand,
    token: String,
    extra_arg: Option<String>,
) -> Result<String, Box<dyn std::error::Error>> {
    tracing::info!("Running client...");

    let channel =
        tonic::transport::Endpoint::from_shared(format!("http://{}", addr))?.connect_lazy();

    #[allow(clippy::result_large_err)]
    let mut client =
        GeoScribeFsServiceClient::with_interceptor(channel, move |mut req: Request<()>| {
            req.metadata_mut()
                .insert("authorization", token.parse().unwrap());
            Ok(req)
        });

    for attempt in 1..=5 {
        let result = match &cmd {
            ClientCommand::Status => client
                .status(StatusRequest {})
                .await
                .map(|res| res.into_inner().status),
            ClientCommand::Write => client
                .write(WriteRequest {
                    volume_name: extra_arg.clone().unwrap_or_default(),
                })
                .await
                .map(|res| res.into_inner().success.to_string()),
        };

        if let Ok(val) = result {
            return Ok(val);
        }

        if attempt < 5 {
            tracing::warn!("Request failed, retrying... (attempt {})", attempt);
            tokio::time::sleep(Duration::from_secs(2)).await;
        } else {
            return result.map_err(|e| e.into());
        }
    }

    unreachable!()
}
