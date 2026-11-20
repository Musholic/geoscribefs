// Test utilities are not meant to be used by all tests
#![allow(dead_code)]

pub mod process;
pub mod temp_volume;

use std::fs;
use std::time::Duration;

use geoscribefs::server::{ServerConfig, run_server};
use tokio::sync::oneshot;
use tokio::{
    runtime::Handle,
    task::{JoinHandle, block_in_place},
    time::{sleep, timeout},
};
use tonic::transport::Endpoint;

pub fn is_fuse_mounted(path: &std::path::Path) -> bool {
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

pub fn is_fuse_mounted_by_volume_name(volume_name: &str) -> bool {
    if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
        for line in mounts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1].contains(volume_name) && parts[0] == "geoscribefs" {
                return true;
            }
        }
    }
    false
}

pub async fn wait_for_fuse_mount(path: &std::path::Path) {
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

#[must_use]
pub struct ServerGuard {
    addr: String,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        tracing::info!(addr = self.addr, "Stopping server...");
        if let Some(shutdown) = self.shutdown.take()
            && let Some(handle) = self.handle.take()
        {
            let _ = shutdown.send(());
            if let Err(error) = block_in_place(|| Handle::current().block_on(handle)) {
                tracing::error!(%error, "Error while stopping server")
            } else {
                tracing::info!(addr = self.addr, "Server stopped");
            }
        }
    }
}

pub async fn start_server(config: ServerConfig) -> ServerGuard {
    tracing::debug!("Starting server at {}", config.addr);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let addr = &config.addr.clone();
    let handle = tokio::spawn(async {
        if let Err(e) = run_server(config, Some(shutdown_rx)).await {
            tracing::error!("Server error: {}", e);
        }
    });

    tracing::debug!("Waiting for server...");
    wait_for_server(addr).await;
    tracing::debug!("Server started successfully");

    ServerGuard {
        addr: addr.to_string(),
        shutdown: Some(shutdown_tx),
        handle: Some(handle),
    }
}

pub async fn wait_for_server(addr: &str) {
    let mut attempts = 0;
    let max_attempts = 10;

    while attempts < max_attempts {
        // Attempt to connect to the address
        if Endpoint::from_shared(format!("http://{}", addr))
            .unwrap()
            .connect()
            .await
            .is_ok()
        {
            return; // Server is up!
        }

        attempts += 1;
        sleep(Duration::from_millis(100)).await;
    }
    panic!("Server at {} failed to start in time", addr);
}

pub fn get_random_addr() -> String {
    // Bind to port 0 to let the OS assign a random available port
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    format!("127.0.0.1:{}", port)
}

pub type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
