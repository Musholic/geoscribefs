mod common;

use crate::common::{TestResult, get_random_addr, start_server, temp_volume};
use geoscribefs::{client::GeoScribeClient, server::ServerConfig, volume_manager::FsBackend};
use pretty_assertions::assert_eq;
use test_log::test;

#[test(tokio::test)]
async fn test_status() -> TestResult {
    let addr = get_random_addr();
    let addr2 = get_random_addr();
    let addr3 = get_random_addr();
    let temp_vol_a = temp_volume::create_temp_volume_named(FsBackend::Btrfs, "vol_a");
    let volume_path_a = temp_vol_a.volume.path().to_string_lossy().into_owned();
    let temp_vol_b = temp_volume::create_temp_volume_named(FsBackend::Btrfs, "vol_b");
    let volume_path_b = temp_vol_b.volume.path().to_string_lossy().into_owned();

    start_server(ServerConfig {
        addr: addr.to_string(),
        peers: vec![addr2.to_string(), addr3.to_string()],
        volume_paths: vec![volume_path_a.clone()],
        ..Default::default()
    })
    .await;
    start_server(ServerConfig {
        addr: addr2.to_string(),
        peers: vec![addr.to_string(), addr3.to_string()],
        volume_paths: vec![volume_path_b.clone()],
        ..Default::default()
    })
    .await;

    start_server(ServerConfig {
        addr: addr3.to_string(),
        peers: vec![addr.to_string(), addr2.to_string()],
        ..Default::default()
    })
    .await;

    let mut client =
        GeoScribeClient::connect(addr.to_string(), ServerConfig::default().token).await?;
    client.write("vol_a".to_string()).await?;

    let mut client2 =
        GeoScribeClient::connect(addr2.to_string(), ServerConfig::default().token).await?;
    client2.write("vol_b".to_string()).await?;

    let result = client.status().await?;

    let expected = format!(
        "Addr: {addr}\n\
Connected to:\n\
- {addr2}\n\
- {addr3}\n\
Volumes:
- vol_a: {addr}\n\
- vol_b: {addr2}\n"
    );
    assert_eq!(result, expected);

    Ok(())
}
