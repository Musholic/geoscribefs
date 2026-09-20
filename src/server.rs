use std::{collections::BTreeMap, sync::RwLock};

use crate::proto::{
    AskWriteRequest, AskWriteResponse, StatusRequest, StatusResponse, WriteRequest, WriteResponse, geo_scribe_fs_service_client::GeoScribeFsServiceClient, geo_scribe_fs_service_server::{GeoScribeFsService, GeoScribeFsServiceServer},
};
use tonic::{Request, Response, Status, service::Interceptor, transport::Server};

#[derive(Default)]
pub struct MyGeoScribeFsService {
    self_addr: String,
    peers: Vec<String>,
    token: String,
    volumes: RwLock<BTreeMap<String, String>>,
}

impl MyGeoScribeFsService {
    pub async fn new(self_addr: String, peers: Vec<String>, token: String) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self { self_addr, peers, token, volumes: RwLock::new(BTreeMap::new()) })
    }
}

#[tonic::async_trait]
impl GeoScribeFsService for MyGeoScribeFsService {
    async fn status(&self, _: Request<StatusRequest>) -> Result<Response<StatusResponse>, Status> {
        let mut status = format!("Addr: {}\n", self.self_addr);
        status.push_str("Connected to:\n");
        for peer in &self.peers {
            status.push_str(&format!("- {}\n", peer));
        }
        status.push_str("Volumes:\n");
        let volumes = self.volumes.read().map_err(|e| Status::internal(e.to_string()))?;
        for (k, v) in volumes.iter() {
            status.push_str(&format!("- {}: {}\n", k, v));
        }

        Ok(Response::new(StatusResponse {
            status: status.into(),
        }))
    }

    async fn write(&self, request: Request<WriteRequest>) -> Result<Response<WriteResponse>, Status> {
        let volume_name = request.into_inner().volume_name;

        for peer in &self.peers {
            let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", peer))
                            .map_err(|e| Status::internal(e.to_string()))?
                            .connect()
                            .await
                            .map_err(|e| Status::internal(format!("Failed to connect to peer {}: {}", peer, e)))?;

            let token = self.token.clone();
            let mut client = GeoScribeFsServiceClient::with_interceptor(channel, move |mut req: Request<()>| {
                if let Ok(val) = token.parse() {
                    req.metadata_mut().insert("authorization", val);
                } else {
                    eprintln!("Failed to parse authorization token");
                }
                if let Ok(val) = self.self_addr.parse() {
                    req.metadata_mut().insert("address", val);
                } else {
                    eprintln!("Failed to parse self address");
                }
                Ok(req)
            });

            let ask_request = tonic::Request::new(AskWriteRequest {
                volume_name: volume_name.clone(),
            });

            let response = client.ask_write(ask_request).await
                .map_err(|e| Status::internal(format!("Peer {} error: {}", peer, e)))?;

            if !response.into_inner().can_write {
                return Err(Status::already_exists(format!("Peer {} denied write for volume {}", peer, volume_name)));
            }
        }
        self.volumes.write()
            .map_err(|e| Status::internal(e.to_string()))?
            .insert(volume_name, self.self_addr.clone());

        Ok(Response::new(WriteResponse {
            success: true,
        }))
    }

    async fn ask_write(&self, request: Request<AskWriteRequest>) -> Result<Response<AskWriteResponse>, Status> {
        let remote_addr = request.metadata().get("address")
            .ok_or_else(|| Status::internal("No remote address"))?
            .to_str()
            .map_err(|_| Status::internal("Invalid remote address"))?
            .to_string();

        let volume_name = request.into_inner().volume_name;

        self.volumes.write()
            .map_err(|e| Status::internal(e.to_string()))?
            .insert(volume_name, remote_addr);

        Ok(Response::new(AskWriteResponse {
            can_write: true,
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

pub async fn run_server(addr: &str, token: String, peers: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let service = MyGeoScribeFsService::new(addr.to_string(), peers, token.clone()).await?;
    let interceptor = AuthInterceptor { token };

    println!("Starting gRPC daemon on {}...", addr);
    Server::builder()
        .add_service(GeoScribeFsServiceServer::with_interceptor(service, interceptor))
        .serve(addr.parse()?)
        .await?;
    Ok(())
}
