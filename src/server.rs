use std::{collections::BTreeMap, net::SocketAddr, sync::RwLock};

use crate::{
    fuse::{self, GeoScribeFs},
    proto::{
        AskWriteRequest, AskWriteResponse, StatusRequest, StatusResponse, WriteRequest,
        WriteResponse,
        geo_scribe_fs_service_client::GeoScribeFsServiceClient,
        geo_scribe_fs_service_server::{GeoScribeFsService, GeoScribeFsServiceServer},
    },
};
use tonic::{Request, Response, Status, service::Interceptor, transport::Server};
use tracing::debug;

#[derive(Default)]
pub struct MyGeoScribeFsService {
    self_addr: String,
    peers: Vec<String>,
    token: String,
    // The mounts will be cleaned up when the service is dropped
    _mounts: Vec<fuse::MountHandle>,
    volumes: RwLock<BTreeMap<String, String>>,
}

impl MyGeoScribeFsService {
    pub async fn new(
        self_addr: String,
        peers: Vec<String>,
        token: String,
        mounts: Vec<fuse::MountHandle>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            self_addr,
            peers,
            token,
            _mounts: mounts,
            volumes: RwLock::new(BTreeMap::new()),
        })
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
        let volumes = self
            .volumes
            .read()
            .map_err(|e| Status::internal(e.to_string()))?;
        for (k, v) in volumes.iter() {
            status.push_str(&format!("- {}: {}\n", k, v));
        }

        Ok(Response::new(StatusResponse { status }))
    }

    async fn write(
        &self,
        request: Request<WriteRequest>,
    ) -> Result<Response<WriteResponse>, Status> {
        let volume_name = request.into_inner().volume_name;

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
                volume_name: volume_name.clone(),
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
            .insert(volume_name, self.self_addr.clone());

        Ok(Response::new(WriteResponse { success: true }))
    }

    async fn ask_write(
        &self,
        request: Request<AskWriteRequest>,
    ) -> Result<Response<AskWriteResponse>, Status> {
        let remote_addr = request
            .metadata()
            .get("address")
            .ok_or_else(|| Status::internal("No remote address"))?
            .to_str()
            .map_err(|_| Status::internal("Invalid remote address"))?
            .to_string();

        let volume_name = request.into_inner().volume_name;

        self.volumes
            .write()
            .map_err(|e| Status::internal(e.to_string()))?
            .insert(volume_name, remote_addr);

        Ok(Response::new(AskWriteResponse { can_write: true }))
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

pub struct ServerConfig {
    pub addr: String,
    pub token: String,
    pub peers: Vec<String>,
    pub volumes: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:50051".to_string(),
            token: "default-token".to_string(),
            peers: vec![],
            volumes: vec![],
        }
    }
}

pub async fn run_server(config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut mounts = Vec::new();
    for volume in &config.volumes {
        let volume = volume.clone();
        let addr = config.addr.clone();
        let token = config.token.clone();
        debug!(
            "Mounting volume {} at address {} with token {}",
            volume, addr, token
        );

        let cfg = fuse::Config {
            volume: volume.clone(),
            token: token.clone(),
            addr: addr.clone(),
        };

        mounts.push(GeoScribeFs::new(cfg)?.start()?);
    }

    let socket_addr: SocketAddr = config.addr.parse()?;
    debug!("Starting gRPC daemon on {}", socket_addr);

    let service = MyGeoScribeFsService::new(
        config.addr.clone(),
        config.peers.clone(),
        config.token.clone(),
        mounts,
    )
    .await?;
    let interceptor = AuthInterceptor {
        token: config.token.clone(),
    };

    Server::builder()
        .add_service(GeoScribeFsServiceServer::with_interceptor(
            service,
            interceptor,
        ))
        .serve(socket_addr)
        .await?;
    Ok(())
}
