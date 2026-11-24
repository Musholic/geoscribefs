mod common;

use crate::common::{
    TestResult, get_random_addr, start_server,
    temp_volume::{self},
};
use geoscribefs::{
    client::GeoScribeClient,
    server::{ReplicaConfig, ServerConfig},
    volume_manager::FsBackend,
};
use std::time::Duration;
use test_log::test;

/// The owner server must push a snapshot to the configured warm replica on every `replication_interval` tick.
#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn test_warm_replica_receives_periodic_snapshot() -> TestResult {
    let backend = FsBackend::Btrfs;

    let owner_vol = temp_volume::create_temp_volume(backend);
    let volume_name = owner_vol.volume.volume_name().to_string();
    let replica_vol = temp_volume::create_temp_volume_named(backend, &volume_name);

    let owner_addr = get_random_addr();
    let replica_addr = get_random_addr();
    let token = ServerConfig::default().token;

    let _replica = start_server(ServerConfig {
        addr: replica_addr.clone(),
        volume_paths: vec![replica_vol.volume.path().to_string_lossy().into_owned()],
        ..Default::default()
    })
    .await;

    let _owner = start_server(ServerConfig {
        addr: owner_addr.clone(),
        volume_paths: vec![owner_vol.volume.path().to_string_lossy().into_owned()],
        replicas: vec![ReplicaConfig {
            volume_name: volume_name.clone(),
            addr: replica_addr.clone(),
        }],
        replication_interval: Duration::from_secs(1),
        ..Default::default()
    })
    .await;

    let file_path = owner_vol.volume.path().join("hello.txt");
    std::fs::write(&file_path, "Hello replica").unwrap();

    let mut replica_client = GeoScribeClient::connect(replica_addr.clone(), token).await?;

    // Wait for the scheduler to push at least one snapshot.
    let mut snapshots = Vec::new();
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        snapshots = replica_client.list_snapshots(volume_name.clone()).await?;
        if !snapshots.is_empty() {
            break;
        }
    }

    assert!(
        !snapshots.is_empty(),
        "replica should have received at least one snapshot, got: {snapshots:?}"
    );

    Ok(())
}
