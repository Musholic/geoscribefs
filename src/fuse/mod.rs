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
use std::sync::{Arc, OnceLock, Weak};
use std::thread::JoinHandle;

use arc_swap::ArcSwap;
use fuse_backend_rs::api::BackendFileSystem;
use fuse_backend_rs::transport::FuseSession;
use fuse_backend_rs::{abi::fuse_abi::FsOptions, passthrough::PassthroughFs};
use tokio::runtime::Handle;
use tokio::sync::Mutex;
use tokio::task::block_in_place;

use crate::server::{GeoScribeFsManager, error::FsError};

/// Automatically unmounts the FUSE mount when dropped.
pub struct MountHandle {
    pub mountpoint: PathBuf,
    pub volume_name: String,
    fs: Weak<Box<dyn BackendFileSystem<Inode = u64, Handle = u64> + Send + Sync>>,
    thread_handle: Option<JoinHandle<Result<(), FsError>>>,
}

impl MountHandle {
    pub async fn mount(&self) -> Result<(), FsError> {
        if let Some(fs) = self.fs.upgrade() {
            let fs = fs
                .as_any()
                .downcast_ref::<GeoScribeFs>()
                .ok_or_else(|| FsError::Unexpected("Could not cast to GeoScribeFs"))?;
            return fs.unpark().await;
        }
        Ok(())
    }

    /// Manually detach and wait for the filesystem thread to finish.
    pub async fn unmount(mut self) -> Result<(), FsError> {
        self.cleanup().await
    }

    async fn cleanup(&mut self) -> Result<(), FsError> {
        tracing::debug!(mountpoint = ?self.mountpoint, "Cleaning up...");
        if let Some(fs) = self.fs.upgrade() {
            let fs = fs
                .as_any()
                .downcast_ref::<GeoScribeFs>()
                .ok_or_else(|| FsError::Unexpected("Could not cast to GeoScribeFs"))?;
            fs.umount_session().await?;
        }
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
        let cleanup = block_in_place(|| Handle::current().block_on(self.cleanup()));
        if let Err(e) = cleanup {
            tracing::error!(mountpoint = ?self.mountpoint, error = ?e, "Failed to clean up mount on drop");
        }
    }
}

pub struct GeoScribeFs {
    inner: ArcSwap<PassthroughFs>,
    session: std::sync::Mutex<FuseSession>,
    pub volume_name: String,
    mountpoint: PathBuf,
    manager: Weak<GeoScribeFsManager>,
    parked: Mutex<bool>,
    fs_opts: OnceLock<FsOptions>,
    park_mountpoint: PathBuf,
}

pub struct Config {
    pub volume: PathBuf,
    pub volume_name: String,
    pub manager: Arc<GeoScribeFsManager>,
}
