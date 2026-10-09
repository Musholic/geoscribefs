mod common;

use crate::common::{TempSubvolume, get_random_addr, start_server};
use geoscribefs::{client::GeoScribeClient, server::ServerConfig};
use test_log::test;

#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn test_restore() -> Result<(), Box<dyn std::error::Error>> {
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

    // Snapshot the volume
    let mut client =
        GeoScribeClient::connect(addr.to_string(), ServerConfig::default().token).await?;
    let (_, date) = client.snapshot(temp_vol.volume_name.clone()).await?;

    // Update the file content
    std::fs::write(&file_path, "Hello v2").unwrap();

    // Restore the volume
    client.restore(temp_vol.volume_name.clone(), date).await?;

    // Verify the file content
    assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "Hello v1");

    Ok(())
}
