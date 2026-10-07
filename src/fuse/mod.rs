//! # GeoScribeFs — Dynamic Write-Permission Overmount FUSE Filesystem
//!
//! ### Purpose & Lifecycle:
//! 1. **Overmounting**:
//!    Mounts directly over an existing directory (`volume_path`). The daemon captures
//!    a file descriptor to the directory upon startup, preserving direct access to the
//!    underlying storage while shadowing the path for userspace.
//!
//! 2. **Proxied Read Requests**:
//!    Read operations (`lookup`, `getattr`, `readdir`, read-only `open`) are passed
//!    directly through to the underlying filesystem with minimal overhead.
//!
//! 3. **Gatekept Write Requests**:
//!    Any write or modification attempt (writing `open`, file creation, or truncation)
//!    triggers an authorization check with the remote server:
//!    - **Denied**: returns `EACCES`.
//!    - **Approved**: lazily unmounts (`MNT_DETACH`) this FUSE mount from `volume_path`.
//!
//! 4. **Self-Detaching for Native Speed**:
//!    Once detached, `volume_path` immediately exposes the native filesystem again:
//!    - **New file operations** bypass FUSE entirely for 100% native performance.
//!    - **Already opened file handles** safely complete through FUSE until closed.
mod fs;
#[macro_use]
mod macros;
mod ops_async;
mod ops_sync;

use std::io::Result;
use std::path::Path;
use std::process::Command;
use std::thread::JoinHandle;

use fuse_backend_rs::passthrough::PassthroughFs;
use nix::mount::{MntFlags, umount2};
use tokio::sync::Mutex;

pub struct MountHandle {
    mountpoint: String,
    thread_handle: Option<JoinHandle<Result<()>>>,
}

impl MountHandle {
    /// Manually detach and wait for the filesystem thread to finish.
    pub fn unmount(mut self) -> Result<()> {
        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<()> {
        let mut res = Ok(());
        if !self.mountpoint.is_empty() {
            res = detach_mount(&self.mountpoint);
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
        res
    }
}

impl Drop for MountHandle {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

pub struct GeoScribeFs {
    inner: PassthroughFs,
    mountpoint: String,
    volume_name: String,
    token: String,
    addr: String,
    detached: Mutex<bool>,
}

fn detach_mount(mountpoint: &str) -> std::io::Result<()> {
    // Fast path: Direct kernel syscall (works if root / CAP_SYS_ADMIN)
    if umount2(Path::new(mountpoint), MntFlags::MNT_DETACH).is_ok() {
        tracing::trace!("Detached mountpoint via umount2");
        return Ok(());
    }

    // Unprivileged path: Delegate to setuid helper binary
    for bin in ["fusermount3", "fusermount"] {
        if Command::new(bin)
            .args(["-u", "-z", "-q", mountpoint])
            .status()
            .is_ok_and(|s| s.success())
        {
            tracing::trace!("Detached mountpoint via {bin}");
            return Ok(());
        }
    }

    Err(std::io::Error::last_os_error())
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use super::*;
    use tempfile::tempdir;
    use test_log::test;
    use tokio::time::timeout;

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

    #[test(tokio::test)]
    async fn test_mount_volume_and_read_does_not_fail() {
        let dir = tempdir().expect("Failed to create temp dir");
        let volume_path = dir.path().to_str().unwrap().to_string();

        // Create hello.txt with content "Hello"
        let hello_path = dir.path().join("hello.txt");
        std::fs::write(&hello_path, "Hello").expect("Failed to write hello.txt");

        let cfg = Config {
            volume: volume_path,
            token: "test_token".to_string(),
            addr: "127.0.0.1:50051".to_string(),
        };

        let fs = GeoScribeFs::new(cfg).expect("Failed to create GeoScribeFs");

        let _mount = fs.start().expect("Failed to start FUSE mount");

        wait_for_fuse_mount(&dir.path()).await;

        // Read hello.txt and verify its content
        let content = std::fs::read_to_string(&hello_path).expect("Failed to read hello.txt");
        assert_eq!(content, "Hello");
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub volume: String,
    pub token: String,
    pub addr: String,
}
