mod common;

use std::net::SocketAddr;

use clap::Parser;
use geoscribefs::{
    cli::{Cli, run_cli},
    proto::{
        AskWriteRequest, AskWriteResponse, RestoreRequest, RestoreResponse, SnapshotRequest,
        SnapshotResponse, StatusRequest, StatusResponse, WriteRequest, WriteResponse,
        geo_scribe_fs_service_server::{GeoScribeFsService, GeoScribeFsServiceServer},
    },
};
use tonic::{Request, Response, Status, transport::Server};

#[derive(Default)]
struct MockGeoScribeFsServer;

#[tonic::async_trait]
impl GeoScribeFsService for MockGeoScribeFsServer {
    async fn status(&self, _: Request<StatusRequest>) -> Result<Response<StatusResponse>, Status> {
        Ok(Response::new(StatusResponse {
            status: "status".to_string(),
        }))
    }

    async fn write(&self, _: Request<WriteRequest>) -> Result<Response<WriteResponse>, Status> {
        Ok(Response::new(WriteResponse { success: true }))
    }

    async fn ask_write(
        &self,
        _: Request<AskWriteRequest>,
    ) -> Result<Response<AskWriteResponse>, Status> {
        Ok(Response::new(AskWriteResponse { can_write: true }))
    }

    async fn snapshot(
        &self,
        _: Request<SnapshotRequest>,
    ) -> Result<Response<SnapshotResponse>, Status> {
        Ok(Response::new(SnapshotResponse {
            success: true,
            date: "2026-10-10_14-53-11.580".to_string(),
        }))
    }

    async fn restore(
        &self,
        _: Request<RestoreRequest>,
    ) -> Result<Response<RestoreResponse>, Status> {
        Ok(Response::new(RestoreResponse { success: true }))
    }
}

use test_log::test;

use crate::common::get_random_addr;

fn start_mock_server() -> String {
    let addr = get_random_addr();
    let socket_addr: SocketAddr = addr.parse().unwrap();

    tokio::spawn(async move {
        Server::builder()
            .add_service(GeoScribeFsServiceServer::new(MockGeoScribeFsServer))
            .serve(socket_addr)
            .await
            .unwrap();
    });

    addr
}

#[test(tokio::test)]
async fn test_cli_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let addr = start_mock_server();
    let cli = Cli::try_parse_from(vec![
        "geoscribefs",
        "--addr",
        &addr,
        "--token",
        "dummy",
        "snapshot",
        "-v",
        "vol_a",
    ])
    .map_err(|e| e.to_string())?;

    let result = run_cli(cli).await?;
    assert_eq!(
        result,
        "Snapshot created with success for vol_a with date: 2026-10-10_14-53-11.580"
    );
    Ok(())
}

#[test(tokio::test)]
async fn test_cli_status() -> Result<(), Box<dyn std::error::Error>> {
    let addr = start_mock_server();

    let cli = Cli::try_parse_from(vec![
        "geoscribefs",
        "--addr",
        &addr,
        "--token",
        "dummy",
        "status",
    ])?;

    let result = run_cli(cli).await?;
    assert_eq!(result, "status");
    Ok(())
}
