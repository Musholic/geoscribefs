use geoscribefs::proto::{
    Empty, Response as ProtoResponse,
    geo_scribe_fs_server::{GeoScribeFs, GeoScribeFsServer},
};
use tonic::{Request, Response, Status, service::Interceptor, transport::Server};

#[derive(Default)]
pub struct MyGeoScribeFs {}

#[tonic::async_trait]
impl GeoScribeFs for MyGeoScribeFs {
    async fn get_status(&self, _: Request<Empty>) -> Result<Response<ProtoResponse>, Status> {
        Ok(Response::new(ProtoResponse {
            message: "Daemon is running smoothly via gRPC!".into(),
        }))
    }
}

#[derive(Clone)]
pub struct AuthInterceptor {
    pub token: String,
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        match request.metadata().get("authorization") {
            Some(t) if t == self.token.as_str() => Ok(request),
            _ => Err(Status::unauthenticated("Invalid or missing auth token")),
        }
    }
}

pub async fn run_server(addr: &str, token: String) -> Result<(), Box<dyn std::error::Error>> {
    let service = MyGeoScribeFs::default();
    let interceptor = AuthInterceptor { token };

    println!("Starting gRPC daemon on {}...", addr);
    Server::builder()
        .add_service(GeoScribeFsServer::with_interceptor(service, interceptor))
        .serve(addr.parse()?)
        .await?;
    Ok(())
}
