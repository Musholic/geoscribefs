use geoscribefs::proto::{Empty, geo_scribe_fs_client::GeoScribeFsClient};
use tonic::Request;

pub async fn run_client(
    addr: &str,
    cmd: &str,
    token: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", addr))?
        .connect()
        .await?;

    let mut client = GeoScribeFsClient::with_interceptor(channel, move |mut req: Request<()>| {
        req.metadata_mut()
            .insert("authorization", token.parse().unwrap());
        Ok(req)
    });

    let request = tonic::Request::new(Empty {});

    let response = match cmd {
        "status" => client.get_status(request).await?,
        _ => return Err("Unknown command".into()),
    };

    println!("{}", response.into_inner().message);
    Ok(())
}
