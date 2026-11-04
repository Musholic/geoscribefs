mod common;

use geoscribefs::server::ServerConfig;
use geoscribefs::volume_manager::FsBackend;
use rstest::rstest;
use std::fs;
use std::io::{Read, Write};
use test_log::test;

use crate::common::{
    get_random_addr, is_fuse_mounted, start_server, temp_volume, wait_for_fuse_mount,
};

#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
#[rstest]
async fn test_read(#[values(FsBackend::Btrfs, FsBackend::Zfs)] backend: FsBackend) {
    let temp_vol = temp_volume::create_temp_volume(backend);
    let volume_path = temp_vol.volume.path().to_string_lossy().into_owned();

    // Create hello.txt with content "Hello"
    let hello_path = temp_vol.volume.path().join("hello.txt");
    std::fs::write(&hello_path, "Hello").expect("Failed to write hello.txt");

    let addr = get_random_addr();
    start_server(ServerConfig {
        addr,
        volume_paths: vec![volume_path.clone()],
        ..Default::default()
    })
    .await;

    wait_for_fuse_mount(temp_vol.volume.path()).await;

    // Read hello.txt and verify its content
    let content = std::fs::read_to_string(&hello_path).expect("Failed to read hello.txt");
    assert_eq!(content, "Hello");
}

// Test writing to one file
#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
#[rstest]
async fn test_fuse_write(#[values(FsBackend::Btrfs, FsBackend::Zfs)] backend: FsBackend) {
    let temp_vol = temp_volume::create_temp_volume(backend);
    let volume_path = temp_vol.volume.path().to_string_lossy().into_owned();

    let addr = get_random_addr();
    start_server(ServerConfig {
        addr,
        volume_paths: vec![volume_path.clone()],
        ..Default::default()
    })
    .await;

    let file_path = temp_vol.volume.path().join("hello.txt");
    let content = b"Hello GeoScribe!";

    // Wait for the volume to be fuse mounted
    wait_for_fuse_mount(temp_vol.volume.path()).await;
    assert!(
        is_fuse_mounted(temp_vol.volume.path()),
        "Volume should be FUSE mounted"
    );

    // Attempt to write to the volume.
    // This should trigger: open -> gRPC write request -> MNT_DETACH -> native write
    {
        tracing::debug!("Opening file for write...");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file_path)
            .expect("Failed to open file for writing");

        tracing::debug!("Writing content...");
        file.write_all(content).expect("Failed to write content");
    }

    // Verify content
    tracing::debug!("Verifying content...");
    let mut read_content = Vec::new();
    let mut file = fs::File::open(&file_path).expect("Failed to open file for reading");
    file.read_to_end(&mut read_content)
        .expect("Failed to read content");

    assert_eq!(read_content, content);

    // Check that the volume is no longer fuse mounted
    assert!(
        !is_fuse_mounted(temp_vol.volume.path()),
        "Volume should be unmounted after write"
    );
}

// Spawn two servers, write on the first and read on the second
#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
#[rstest]
async fn test_fuse_write_server1_then_read_server2(
    #[values(FsBackend::Btrfs, FsBackend::Zfs)] backend: FsBackend,
) {
    // Start the two servers, each with their volume of the same name
    let addr = get_random_addr();
    let addr2 = get_random_addr();

    let temp_vol = temp_volume::create_temp_volume(backend);
    let volume_path = temp_vol.volume.path().to_string_lossy().into_owned();
    let temp_vol2 = temp_volume::create_temp_volume_named(backend, temp_vol.volume.volume_name());
    let volume_path2 = temp_vol2.volume.path().to_string_lossy().into_owned();

    start_server(ServerConfig {
        addr: addr.to_string(),
        peers: vec![addr2.to_string()],
        volume_paths: vec![volume_path.clone()],
        ..Default::default()
    })
    .await;
    start_server(ServerConfig {
        addr: addr2.to_string(),
        peers: vec![addr.to_string()],
        volume_paths: vec![volume_path2.clone()],
        ..Default::default()
    })
    .await;

    // Write into the first volume
    let file_path = temp_vol.volume.path().join("hello.txt");
    let mut file = fs::File::create(&file_path).expect("Failed to create file");
    file.write_all(b"Hello, World!")
        .expect("Failed to write to file");

    // Read from the second volume
    let file_path2 = temp_vol2.volume.path().join("hello.txt");
    let mut file2 = fs::File::open(&file_path2).expect("Failed to open file");
    let mut contents = String::new();
    file2
        .read_to_string(&mut contents)
        .expect("Failed to read from file");
    assert_eq!(contents, "Hello, World!");
}
