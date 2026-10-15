use std::{
    collections::BTreeMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use crate::{
    fuse::{self},
    proto::{
        AskWriteRequest, AskWriteResponse, ListSnapshotsRequest, ListSnapshotsResponse,
        RestoreRequest, RestoreResponse, SnapshotRequest, SnapshotResponse, StatusRequest,
        StatusResponse, WriteRequest, WriteResponse,
        geo_scribe_fs_service_server::{GeoScribeFsService, GeoScribeFsServiceServer},
    },
    server::{
        error::FsError, snapshot::SnapshotHandler, status::StatusHandler, write::WriteHandler,
    },
    volume_manager::{self, VolumeManager},
};
use tonic::{Request, Response, Status, service::Interceptor, transport::Server};
use tracing::debug;

pub mod error;
mod snapshot;
mod status;
pub mod write;

struct MyGeoScribeFsService {
    manager: Arc<GeoScribeFsManager>,
}

pub struct GeoScribeFsManager {
    self_addr: String,
    peers: Vec<String>,
    token: String,
    volume_managers: RwLock<BTreeMap<String, Arc<dyn VolumeManager>>>,
    volumes: RwLock<BTreeMap<String, String>>,
}

impl GeoScribeFsManager {
    pub fn new(self_addr: String, peers: Vec<String>, token: String) -> Self {
        Self {
            self_addr,
            peers,
            token,
            volume_managers: RwLock::new(BTreeMap::new()),
            volumes: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn register_volume(&self, volume_manager: Arc<dyn VolumeManager>) {
        let mut volume_managers = self.volume_managers.write().unwrap();
        volume_managers.insert(volume_manager.volume_path().to_string(), volume_manager);
    }

    fn get_volume_manager(&self, volume_name: &str) -> Result<Arc<dyn VolumeManager>, FsError> {
        let volume_managers = self
            .volume_managers
            .read()
            .map_err(|e| FsError::LockPoisoned(e.to_string()))?;
        volume_managers
            .get(volume_name)
            .cloned()
            .ok_or_else(|| FsError::NotFound(volume_name.to_string()))
    }
}

#[tonic::async_trait]
impl GeoScribeFsService for MyGeoScribeFsService {
    async fn status(&self, _: Request<StatusRequest>) -> Result<Response<StatusResponse>, Status> {
        Ok(Response::new(self.manager.handle_status().await?))
    }

    async fn write(
        &self,
        request: Request<WriteRequest>,
    ) -> Result<Response<WriteResponse>, Status> {
        let volume_name = request.into_inner().volume_name;
        Ok(Response::new(
            self.manager.handle_write(&volume_name).await?,
        ))
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

        Ok(Response::new(
            self.manager
                .handle_ask_write(&volume_name, &remote_addr)
                .await?,
        ))
    }

    async fn snapshot(
        &self,
        request: Request<SnapshotRequest>,
    ) -> Result<Response<SnapshotResponse>, Status> {
        let volume_name = request.into_inner().volume_name;
        Ok(Response::new(
            self.manager.handle_snapshot(&volume_name).await?,
        ))
    }

    async fn restore(
        &self,
        request: Request<RestoreRequest>,
    ) -> Result<Response<RestoreResponse>, Status> {
        let request = request.into_inner();
        let volume_name = request.volume_name;
        let date = request.date;
        Ok(Response::new(
            self.manager.handle_restore(&volume_name, &date).await?,
        ))
    }

    async fn list_snapshots(
        &self,
        request: Request<ListSnapshotsRequest>,
    ) -> Result<Response<ListSnapshotsResponse>, Status> {
        let request = request.into_inner();
        let volume_name = request.volume_name;
        Ok(Response::new(
            self.manager.handle_list_snapshots(&volume_name).await?,
        ))
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
    pub volume_paths: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:50051".to_string(),
            token: "default-token".to_string(),
            peers: vec![],
            volume_paths: vec![],
        }
    }
}

pub async fn run_server(config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let manager = Arc::new(GeoScribeFsManager::new(
        config.addr.clone(),
        config.peers.clone(),
        config.token.clone(),
    ));
    for volume_path in &config.volume_paths {
        let cfg = fuse::Config {
            volume: volume_path.clone(),
            manager: manager.clone(),
        };
        let volume_manager = volume_manager::mount_and_get(cfg)?;
        manager.register_volume(volume_manager);
    }

    let socket_addr: SocketAddr = config.addr.parse()?;
    debug!("Starting gRPC daemon on {}", socket_addr);

    let service = MyGeoScribeFsService {
        manager: manager.clone(),
    };

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
