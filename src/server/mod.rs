use std::{collections::BTreeMap, net::SocketAddr, sync::RwLock};

use crate::{
    fuse::{self},
    proto::{
        AskWriteRequest, AskWriteResponse, ListSnapshotsRequest, ListSnapshotsResponse,
        RestoreRequest, RestoreResponse, SnapshotRequest, SnapshotResponse, StatusRequest,
        StatusResponse, WriteRequest, WriteResponse,
        geo_scribe_fs_service_server::{GeoScribeFsService, GeoScribeFsServiceServer},
    },
    server::{snapshot::SnapshotHandler, status::StatusHandler, write::WriteHandler},
    volume_manager::{self, VolumeManager},
};
use tonic::{Request, Response, Status, service::Interceptor, transport::Server};
use tracing::debug;

mod snapshot;
mod status;
mod write;

#[derive(Default)]
pub struct MyGeoScribeFsService {
    self_addr: String,
    peers: Vec<String>,
    token: String,
    volume_managers: BTreeMap<String, Box<dyn VolumeManager>>,
    volumes: RwLock<BTreeMap<String, String>>,
}

impl MyGeoScribeFsService {
    pub async fn new(
        self_addr: String,
        peers: Vec<String>,
        token: String,
        volume_managers: BTreeMap<String, Box<dyn VolumeManager>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            self_addr,
            peers,
            token,
            volume_managers,
            volumes: RwLock::new(BTreeMap::new()),
        })
    }
}

#[tonic::async_trait]
impl GeoScribeFsService for MyGeoScribeFsService {
    async fn status(&self, _: Request<StatusRequest>) -> Result<Response<StatusResponse>, Status> {
        self.handle_status().await
    }

    async fn write(
        &self,
        request: Request<WriteRequest>,
    ) -> Result<Response<WriteResponse>, Status> {
        let volume_name = request.into_inner().volume_name;
        self.handle_write(&volume_name).await
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

        self.handle_ask_write(&volume_name, &remote_addr).await
    }

    async fn snapshot(
        &self,
        request: Request<SnapshotRequest>,
    ) -> Result<Response<SnapshotResponse>, Status> {
        let volume_name = request.into_inner().volume_name;
        self.handle_snapshot(&volume_name).await
    }

    async fn restore(
        &self,
        request: Request<RestoreRequest>,
    ) -> Result<Response<RestoreResponse>, Status> {
        let request = request.into_inner();
        let volume_name = request.volume_name;
        let date = request.date;
        self.handle_restore(&volume_name, &date).await
    }

    async fn list_snapshots(
        &self,
        request: Request<ListSnapshotsRequest>,
    ) -> Result<Response<ListSnapshotsResponse>, Status> {
        let request = request.into_inner();
        let volume_name = request.volume_name;
        self.handle_list_snapshots(&volume_name).await
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
    pub base_volumes_path: String,
    pub volume_names: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:50051".to_string(),
            token: "default-token".to_string(),
            peers: vec![],
            base_volumes_path: "/var/lib/geoscribefs".to_string(),
            volume_names: vec![],
        }
    }
}

pub async fn run_server(config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut volume_managers = BTreeMap::new();
    for volume_name in &config.volume_names {
        let volume = config.base_volumes_path.clone() + "/" + volume_name;
        let addr = config.addr.clone();
        let token = config.token.clone();

        let cfg = fuse::Config {
            volume,
            token: token.clone(),
            addr: addr.clone(),
        };
        let volume_manager = volume_manager::mount_and_get(cfg)?;
        volume_managers.insert(volume_name.clone(), volume_manager);
    }

    let socket_addr: SocketAddr = config.addr.parse()?;
    debug!("Starting gRPC daemon on {}", socket_addr);

    let service = MyGeoScribeFsService::new(
        config.addr.clone(),
        config.peers.clone(),
        config.token.clone(),
        volume_managers,
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
