use crate::proto::{StatusRequest, WriteRequest, geo_scribe_fs_service_client::GeoScribeFsServiceClient};
use tonic::Request;

pub enum ClientCommand {
    Status,
    Write,
}

pub async fn run_client(
    addr: &str,
    cmd: ClientCommand,
    token: String,
    extra_arg: Option<String>,
) -> Result<String, Box<dyn std::error::Error>> {
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", addr))?
        .connect()
        .await?;

    let mut client = GeoScribeFsServiceClient::with_interceptor(channel, move |mut req: Request<()>| {
        req.metadata_mut()
            .insert("authorization", token.parse().unwrap());
        Ok(req)
    });


    match cmd {
        ClientCommand::Status => {
            let request = tonic::Request::new(StatusRequest {});
            let response = client.status(request).await?;
            Ok(response.into_inner().status)
        },
        ClientCommand::Write => {
            let request = tonic::Request::new(WriteRequest {
                volume_name: extra_arg.unwrap_or_default(),
            });
            let response = client.write(request).await?;
            Ok(response.into_inner().success.to_string())
        },
    }
}
