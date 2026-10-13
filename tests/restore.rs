mod common;

use crate::common::{
    get_random_addr, start_server,
    temp_volume::{self},
};
use geoscribefs::{client::GeoScribeClient, server::ServerConfig, volume_manager::FsBackend};
use rstest::rstest;
use test_log::test;

#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
#[rstest]
async fn test_restore(
    #[values(FsBackend::Btrfs, FsBackend::Zfs)] backend: FsBackend,
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_vol = temp_volume::create_temp_volume(backend);
    let volume_path = temp_vol.volume.path().to_string_lossy().into_owned();

    let addr = get_random_addr();

    start_server(ServerConfig {
        addr: addr.clone(),
        volume_paths: vec![volume_path.clone()],
        ..Default::default()
    })
    .await;

    let file_path = temp_vol.volume.path().join("hello.txt");
    std::fs::write(&file_path, "Hello v1").unwrap();

    // Snapshot the volume
    let mut client =
        GeoScribeClient::connect(addr.to_string(), ServerConfig::default().token).await?;
    let (_, date) = client.snapshot(volume_path.clone()).await?;

    // Update the file content
    std::fs::write(&file_path, "Hello v2").unwrap();

    // Restore the volume
    client.restore(volume_path.clone(), date).await?;

    // Verify the file content
    assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "Hello v1");

    Ok(())
}
