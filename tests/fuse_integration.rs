use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use geoscribefs::server::{ServerConfig, run_server};
use test_log::test;
use tokio::time::timeout;

fn start_server(volume_path: String, token: String) {
    tokio::spawn(async {
        let _ = run_server(ServerConfig {
            addr: "127.0.0.1:50051".to_string(),
            token,
            volumes: vec![volume_path],
            ..Default::default()
        })
        .await;
    });
}

fn is_fuse_mounted(path: &std::path::Path) -> bool {
    if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
        for line in mounts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3
                && parts[1] == path.to_str().unwrap_or("")
                && parts[0] == "geoscribefs"
            {
                return true;
            }
        }
    }
    false
}

async fn wait_for_fuse_mount(path: &std::path::Path) {
    timeout(Duration::from_secs(5), async {
        loop {
            if is_fuse_mounted(path) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("Fuse mount should have been created")
}

// Test writing to one file
#[test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn test_fuse_write() {
    let token = "test_token_123";
    let vol_name = format!(
        "geoscribe_test_vol_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    let vol_path = PathBuf::from(format!("/tmp/{}", vol_name));
    fs::create_dir_all(&vol_path).expect("Failed to create volume dir");

    start_server(vol_path.to_str().unwrap().to_string(), token.to_string());

    let file_path = vol_path.join("hello.txt");
    let content = b"Hello GeoScribe!";

    // Wait for the volume to be fuse mounted
    wait_for_fuse_mount(&vol_path).await;
    assert!(is_fuse_mounted(&vol_path), "Volume should be FUSE mounted");

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
        !is_fuse_mounted(&vol_path),
        "Volume should be unmounted after write"
    );

    // Cleanup
    fs::remove_dir_all(&vol_path).ok();
}
