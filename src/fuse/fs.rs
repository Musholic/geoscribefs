use std::{
    any::Any,
    io::{self, Error, Result},
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use fuse_backend_rs::{
    api::{BackendFileSystem, Vfs, VfsOptions, filesystem::Entry, server::Server},
    async_runtime::Runtime,
    passthrough::{self, PassthroughFs},
    transport::{FuseDevTask, FuseSession},
};
use tokio::sync::Mutex;

use crate::fuse::{Config, GeoScribeFs, MountHandle, detach_mount};

impl GeoScribeFs {
    pub fn new(cfg: Config) -> Result<Self> {
        let volume_path = Path::new(&cfg.volume);
        let volume_name = volume_path
            .file_name()
            .ok_or_else(|| Error::new(std::io::ErrorKind::InvalidInput, "Invalid volume path"))?
            .to_string_lossy()
            .to_string();

        let pt_cfg = passthrough::Config {
            root_dir: cfg.volume.clone(),
            do_import: false,
            ..Default::default()
        };
        let inner = PassthroughFs::<()>::new(pt_cfg).map_err(Error::other)?;
        inner.import().map_err(Error::other)?;

        Ok(Self {
            inner,
            mountpoint: cfg.volume,
            volume_name,
            token: cfg.token,
            addr: cfg.addr,
            detached: Mutex::new(false),
        })
    }

    /// Mounts the filesystem and blocks the current thread to process FUSE requests.
    fn run_blocking(self) -> Result<()> {
        let volume_path_str = self.mountpoint.clone();
        let volume_path = Path::new(&volume_path_str);

        let vfs = Vfs::new(VfsOptions::default());
        vfs.mount(Box::new(self), "/").map_err(Error::other)?;

        let vfs = Arc::new(vfs);
        let server = Arc::new(Server::new(vfs));

        let mut se =
            FuseSession::new(volume_path, "geoscribefs", "", false).map_err(Error::other)?;
        se.mount().map_err(Error::other)?;
        let mut guard = SessionGuard(Some(se));
        let se = guard.0.as_mut().unwrap();

        let state = Arc::new(AtomicBool::new(false));
        let buf_size = (fuse_backend_rs::api::server::MAX_BUFFER_SIZE + 0x1000) as usize;

        let fuse_file = se.clone_fuse_file().map_err(Error::other)?;
        let mut task = FuseDevTask::new(buf_size, fuse_file, server, state);

        let rt = Runtime::new();
        rt.block_on(async move {
            let poller = Runtime::spawn(async move { task.poll_handler().await });
            poller.await.expect("the async fuse task panicked");
        });

        Ok(())
    }

    /// Spawns a background thread to run the filesystem and returns a MountHandle.
    pub fn start(self) -> Result<MountHandle> {
        let mountpoint = self.mountpoint.clone();

        let thread_handle = std::thread::spawn(move || self.run_blocking());

        Ok(MountHandle {
            mountpoint,
            thread_handle: Some(thread_handle),
        })
    }

    pub async fn check_write_auth_and_detach(&self) -> Result<()> {
        tracing::trace!("Waiting for lock before asking for write authorization");

        // Ensure we only detach once, even if multiple threads call this concurrently
        let mut detached = self.detached.lock().await;
        if *detached {
            return Ok(());
        }

        tracing::trace!("Asking for write authorization");

        let timeout_duration = Duration::from_secs(10);

        let auth_future = crate::client::run_client(
            self.addr.clone(),
            crate::client::ClientCommand::Write,
            self.token.clone(),
            Some(self.volume_name.clone()),
        );

        let auth_result = tokio::time::timeout(timeout_duration, auth_future).await?;

        if auth_result.is_err() {
            return Err(Error::from_raw_os_error(libc::EACCES));
        }

        // Authorization was granted, detach the FUSE mount point
        tracing::trace!("Authorization granted, detaching FUSE mount point");
        if let Err(e) = detach_mount(&self.mountpoint) {
            return Err(Error::other(e));
        }

        tracing::trace!("FUSE mount point detached");
        *detached = true;
        Ok(())
    }
}

/// Umount the fuse session when dropped, so the mount doesn't leak even if the process exits.
struct SessionGuard(Option<FuseSession>);

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if let Some(se) = self.0.as_mut() {
            let _ = se.umount();
        }
    }
}

impl BackendFileSystem for GeoScribeFs {
    fn mount(&self) -> io::Result<(Entry, u64)> {
        self.inner.mount()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
