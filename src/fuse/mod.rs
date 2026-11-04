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

use std::panic;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;

use arc_swap::ArcSwap;
use fuse_backend_rs::transport::FuseSession;
use fuse_backend_rs::{abi::fuse_abi::FsOptions, passthrough::PassthroughFs};
use tokio::sync::Mutex;

use crate::server::{GeoScribeFsManager, error::FsError};

/// Automatically unmounts the FUSE mount when dropped.
pub struct MountHandle {
    pub mountpoint: PathBuf,
    pub volume_name: String,
    thread_handle: Option<JoinHandle<Result<(), FsError>>>,
}

impl MountHandle {
    /// Manually detach and wait for the filesystem thread to finish.
    pub fn unmount(mut self) -> Result<(), FsError> {
        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<(), FsError> {
        if let Some(handle) = self.thread_handle.take()
            && let Err(e) = handle.join()
        {
            panic::resume_unwind(e);
        }
        Ok(())
    }
}

impl Drop for MountHandle {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

pub struct GeoScribeFs {
    inner: ArcSwap<PassthroughFs>,
    session: std::sync::Mutex<FuseSession>,
    pub volume_name: String,
    mountpoint: PathBuf,
    manager: Arc<GeoScribeFsManager>,
    detached: Mutex<bool>,
    fs_opts: OnceLock<FsOptions>,
}

pub struct Config {
    pub volume: PathBuf,
    pub volume_name: String,
    pub manager: Arc<GeoScribeFsManager>,
}
