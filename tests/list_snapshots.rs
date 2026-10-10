mod common;

use crate::common::{TempSubvolume, get_random_addr, start_server};
use geoscribefs::{client::GeoScribeClient, server::ServerConfig};
use test_log::test;

#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn test_list_snapshots() -> Result<(), Box<dyn std::error::Error>> {
    let temp_vol = TempSubvolume::create();

    let addr = get_random_addr();

    start_server(ServerConfig {
        addr: addr.clone(),
        base_volumes_path: temp_vol.base_volume_path.to_string_lossy().into_owned(),
        volume_names: vec![temp_vol.volume_name.clone()],
        ..Default::default()
    })
    .await;

    let file_path = temp_vol.volume_path.join("hello.txt");
    std::fs::write(&file_path, "Hello v1").unwrap();

    // Snapshot the volume once
    let mut client =
        GeoScribeClient::connect(addr.to_string(), ServerConfig::default().token).await?;
    let (_, date) = client.snapshot(temp_vol.volume_name.clone()).await?;

    // Do another change
    std::fs::write(&file_path, "Hello v2").unwrap();

    // Snapshot the volume again
    let (_, date2) = client.snapshot(temp_vol.volume_name.clone()).await?;

    // List snapshots
    let snapshots = client.list_snapshots(temp_vol.volume_name.clone()).await?;
    assert_eq!(vec![date, date2], snapshots);

    Ok(())
}
