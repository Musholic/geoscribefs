use crate::{
    fuse::{self},
    proto::{
        AskWriteRequest, AskWriteResponse, ListSnapshotsRequest, ListSnapshotsResponse,
        ReceiveSnapshotRequest, ReceiveSnapshotResponse, RestoreRequest, RestoreResponse,
        SendSnapshotRequest, SendSnapshotResponse, SnapshotRequest, SnapshotResponse,
        StatusRequest, StatusResponse, WriteRequest, WriteResponse,
        geo_scribe_fs_service_server::{GeoScribeFsService, GeoScribeFsServiceServer},
    },
    server::{
        error::FsError, snapshot::SnapshotHandler, status::StatusHandler, write::WriteHandler,
    },
    volume_manager::{self, VolumeManager},
};
use bytes::Bytes;
use std::time::Duration;
use std::{
    collections::BTreeMap,
    io,
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, RwLock},
};
use tokio::sync::oneshot::Receiver;
use tokio_stream::{Stream, StreamExt};
use tokio_util::io::ReaderStream;
use tonic::{Request, Response, Status, Streaming, service::Interceptor, transport::Server};
use tracing::debug;

pub mod error;
pub mod replication;
mod snapshot;
mod status;
pub mod sync;
pub mod write;

struct MyGeoScribeFsService {
    manager: Arc<GeoScribeFsManager>,
}

#[derive(Clone)]
struct Volume {
    owner: String,
    manager: Option<Arc<dyn VolumeManager>>,
}

pub struct GeoScribeFsManager {
    self_addr: String,
    peers: Vec<String>,
    token: String,
    volumes: RwLock<BTreeMap<String, Volume>>,
}

impl GeoScribeFsManager {
    pub fn new(self_addr: String, peers: Vec<String>, token: String) -> Self {
        Self {
            self_addr,
            peers,
            token,
            volumes: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn register_volume(&self, volume_manager: Arc<dyn VolumeManager>) -> Result<(), FsError> {
        let mut volumes = self.volumes.write().unwrap();
        let volume_name = volume_manager.volume_name();
        let volume = Volume {
            // TODO: Should call leaders to know if someone else owns the volume
            owner: self.self_addr.clone(),
            manager: Some(volume_manager),
        };
        volumes.insert(volume_name, volume);
        Ok(())
    }

    fn get_volume(&self, volume_name: &str) -> Result<Volume, FsError> {
        let volumes = self.volumes.read()?;
        volumes
            .get(volume_name)
            .cloned()
            .ok_or_else(|| FsError::not_found(volume_name))
    }

    fn get_volume_manager(&self, volume_name: &str) -> Result<Arc<dyn VolumeManager>, FsError> {
        let volume = self.get_volume(volume_name)?;
        let manager = volume
            .manager
            .ok_or_else(|| FsError::NotMounted(volume_name.to_string()))?;
        Ok(manager)
    }
}

// Tonic Status error are large, so we allow clippy to ignore them
#[allow(clippy::result_large_err)]
#[tonic::async_trait]
impl GeoScribeFsService for MyGeoScribeFsService {
    type SendSnapshotStream =
        Pin<Box<dyn Stream<Item = Result<SendSnapshotResponse, Status>> + Send + 'static>>;

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

    async fn send_snapshot(
        &self,
        request: Request<SendSnapshotRequest>,
    ) -> Result<Response<Self::SendSnapshotStream>, Status> {
        let request = request.into_inner();
        let volume_name = request.volume_name;
        let stdout = self.manager.handle_send_snapshot(&volume_name).await?;

        let stream = ReaderStream::new(stdout).map(|chunk| {
            chunk
                .map(|c| SendSnapshotResponse { data: c.to_vec() })
                .map_err(|e| tonic::Status::internal(e.to_string()))
        });

        Ok(Response::new(Box::pin(stream)))
    }

    async fn receive_snapshot(
        &self,
        request: Request<Streaming<ReceiveSnapshotRequest>>,
    ) -> Result<Response<ReceiveSnapshotResponse>, Status> {
        let volume_name = request
            .metadata()
            .get("volume_name")
            .ok_or_else(|| Status::internal("No volume name in metadata"))?
            .to_str()
            .map_err(|_| Status::internal("Invalid volume name in metadata"))?
            .to_string();

        let chunks = request.into_inner().map(|chunk| {
            chunk
                .map(|msg| Bytes::from(msg.data))
                .map_err(io::Error::other)
        });

        self.manager
            .handle_receive_snapshot(&volume_name, chunks)
            .await?;

        Ok(Response::new(ReceiveSnapshotResponse { success: true }))
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

/// Target to which a volume's snapshots are periodically pushed.
#[derive(Debug, Clone)]
pub struct ReplicaConfig {
    /// Name of the volume to replicate.
    pub volume_name: String,
    /// Address of the replica geoscribefs server.
    pub addr: String,
}

#[derive(Debug)]
pub struct ServerConfig {
    pub addr: String,
    pub token: String,
    pub peers: Vec<String>,
    pub volume_paths: Vec<String>,
    /// Volumes configured for periodic snapshot replication to warm replicas.
    pub replicas: Vec<ReplicaConfig>,
    /// How often snapshots are pushed to each configured replica.
    pub replication_interval: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:50051".to_string(),
            token: "default-token".to_string(),
            peers: vec![],
            volume_paths: vec![],
            replicas: vec![],
            replication_interval: Duration::from_secs(300),
        }
    }
}

pub async fn run_server(
    config: ServerConfig,
    shutdown_rx: Option<Receiver<()>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::trace!(config = ?config, "Starting server");
    let manager = Arc::new(GeoScribeFsManager::new(
        config.addr.clone(),
        config.peers.clone(),
        config.token.clone(),
    ));
    for volume_path in &config.volume_paths {
        let volume_path = PathBuf::from(volume_path);

        let volume_name = volume_path
            .file_name()
            .ok_or_else(|| FsError::InvalidVolumePath(volume_path.to_string_lossy().to_string()))?
            .to_string_lossy()
            .to_string();

        let cfg = fuse::Config {
            volume: volume_path,
            volume_name,
            manager: manager.clone(),
        };
        let volume_manager = volume_manager::mount_and_get(cfg).await?;
        manager.register_volume(volume_manager)?;
    }

    if !config.replicas.is_empty() {
        replication::ReplicationScheduler::new(
            manager.clone(),
            config.replicas,
            config.replication_interval,
        )
        .spawn();
    }

    let socket_addr: SocketAddr = config.addr.parse()?;
    debug!("Starting gRPC daemon on {}", socket_addr);

    let service = MyGeoScribeFsService {
        manager: manager.clone(),
    };

    let interceptor = AuthInterceptor {
        token: config.token.clone(),
    };

    let result = Server::builder()
        .add_service(GeoScribeFsServiceServer::with_interceptor(
            service,
            interceptor,
        ))
        .serve_with_shutdown(socket_addr, shutdown_signal(shutdown_rx))
        .await;

    result?;

    tracing::info!("Server exited");
    Ok(())
}

/// Completes when the process is asked to stop.
async fn shutdown_signal(shutdown_rx: Option<Receiver<()>>) {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate =
        signal(SignalKind::terminate()).expect("failed to register the SIGTERM handler");
    let mut interrupt =
        signal(SignalKind::interrupt()).expect("failed to register the SIGINT handler");

    let external_shutdown = async {
        match shutdown_rx {
            Some(rx) => rx.await.ok(),
            None => std::future::pending::<Option<()>>().await,
        }
    };

    tokio::select! {
        _ = terminate.recv() => tracing::debug!("Received SIGTERM, shutting down"),
        _ = interrupt.recv() => tracing::debug!("Received SIGINT, shutting down"),
        _ = external_shutdown => tracing::debug!("Received shutdown signal, shutting down"),
    }
}
