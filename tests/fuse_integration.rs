mod common;

use geoscribefs::fuse::{Config, GeoScribeFs};
use geoscribefs::server::ServerConfig;
use std::fs;
use std::io::{Read, Write};
use tempfile::tempdir;
use test_log::test;

use crate::common::{get_random_addr, is_fuse_mounted, start_server, wait_for_fuse_mount};

#[test(tokio::test)]
async fn test_mount_volume_and_read() {
    let dir = tempdir().expect("Failed to create temp dir");
    let volume_path = dir.path().to_str().unwrap().to_string();

    // Create hello.txt with content "Hello"
    let hello_path = dir.path().join("hello.txt");
    std::fs::write(&hello_path, "Hello").expect("Failed to write hello.txt");

    let cfg = Config {
        volume: volume_path,
        token: "test_token".to_string(),
        addr: get_random_addr().to_string(),
    };

    let fs = GeoScribeFs::new(cfg).expect("Failed to create GeoScribeFs");

    let _mount = fs.start().expect("Failed to start FUSE mount");

    wait_for_fuse_mount(dir.path()).await;

    // Read hello.txt and verify its content
    let content = std::fs::read_to_string(&hello_path).expect("Failed to read hello.txt");
    assert_eq!(content, "Hello");
}

// Test writing to one file
#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn test_fuse_write() {
    let dir = tempdir().expect("Failed to create temp dir");
    let volume_path = dir.path().to_str().unwrap().to_string();

    start_server(ServerConfig {
        volumes: vec![volume_path.clone()],
        ..Default::default()
    })
    .await;

    let file_path = dir.path().join("hello.txt");
    let content = b"Hello GeoScribe!";

    // Wait for the volume to be fuse mounted
    wait_for_fuse_mount(dir.path()).await;
    assert!(is_fuse_mounted(dir.path()), "Volume should be FUSE mounted");

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
        !is_fuse_mounted(dir.path()),
        "Volume should be unmounted after write"
    );

    // Cleanup
    fs::remove_dir_all(dir.path()).ok();
}

// Spawn two servers, write on the first and read on the second
#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn test_fuse_write_server1_then_read_server2() {}
