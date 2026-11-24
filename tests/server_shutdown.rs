mod common;

use crate::common::process::start_server_process;
use crate::common::{
    TestResult, get_random_addr, is_fuse_mounted, is_fuse_mounted_by_volume_name, temp_volume,
};
use geoscribefs::server::ServerConfig;
use geoscribefs::volume_manager::FsBackend;
use rstest::rstest;
use std::{fs, io::Write};
use test_log::test;

#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
#[rstest]
async fn test_umount_on_shutdown(
    #[values(FsBackend::Btrfs, FsBackend::Zfs)] backend: FsBackend,
) -> TestResult {
    let temp_vol = temp_volume::create_temp_volume(backend);
    let volume_path = temp_vol.volume.path().to_string_lossy().into_owned();

    let addr = get_random_addr();
    let mut process = start_server_process(ServerConfig {
        addr,
        volume_paths: vec![volume_path.clone()],
        ..Default::default()
    })
    .await;

    assert!(
        is_fuse_mounted(temp_vol.volume.path()),
        "Volume should be FUSE mounted"
    );

    // Kill the process gracefully then check that the volume is no longer fuse mounted
    process.shutdown().await;

    assert!(
        !is_fuse_mounted_by_volume_name(temp_vol.volume.volume_name()),
        "Volume should no longer be FUSE mounted after shutdown"
    );

    Ok(())
}

#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
#[rstest]
async fn test_umount_on_shutdown_after_park(
    #[values(FsBackend::Btrfs, FsBackend::Zfs)] backend: FsBackend,
) -> TestResult {
    let temp_vol = temp_volume::create_temp_volume(backend);
    let volume_path = temp_vol.volume.path().to_string_lossy().into_owned();

    let addr = get_random_addr();
    let mut process = start_server_process(ServerConfig {
        addr,
        volume_paths: vec![volume_path.clone()],
        ..Default::default()
    })
    .await;
    let pid = process.pid().expect("server should have pid");

    // Trigger a write to park the volume
    let file_path = temp_vol.volume.path().join("hello.txt");
    {
        let mut file = fs::File::create(&file_path).expect("Failed to create file");
        file.write_all(b"Hello, World!")
            .expect("Failed to write to file");
    }

    let park_path = &temp_vol.volume.park_path(pid);
    assert!(
        is_fuse_mounted(park_path),
        "Volume should be FUSE mounted at park path {park_path:?}"
    );

    // Kill the process gracefully then check that the volume is no longer fuse mounted
    process.shutdown().await;

    assert!(
        !is_fuse_mounted_by_volume_name(temp_vol.volume.volume_name()),
        "Volume should no longer be FUSE mounted after shutdown"
    );

    Ok(())
}
