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

#[derive(Clone, Debug)]
pub struct Config {
    pub volume: String,
    pub token: String,
    pub addr: String,
}
