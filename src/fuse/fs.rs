use crate::{
    fuse::{Config, GeoScribeFs, MountHandle},
    server::{error::FsError, sync::SyncHandler, write::WriteHandler},
};
use fuse_backend_rs::{
    api::{BackendFileSystem, Vfs, VfsOptions, filesystem::Entry, server::Server},
    async_runtime::Runtime,
    passthrough::{self, PassthroughFs},
    transport::{FuseDevTask, FuseSession},
};
use std::{
    any::Any,
    io::{self},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use tokio::sync::Mutex;

impl GeoScribeFs {
    pub fn new(cfg: Config) -> Result<Self, FsError> {
        let pt_cfg = passthrough::Config {
            root_dir: cfg.volume.to_string_lossy().to_string(),
            do_import: false,
            ..Default::default()
        };
        let inner = PassthroughFs::<()>::new(pt_cfg).map_err(io::Error::other)?;
        inner.import().map_err(io::Error::other)?;

        let mut session = FuseSession::new(&cfg.volume, "geoscribefs", "", false)?;
        session.mount().map_err(io::Error::other)?;

        Ok(Self {
            inner,
            mountpoint: session.mountpoint().to_path_buf(),
            session: std::sync::Mutex::new(session),
            manager: cfg.manager,
            detached: Mutex::new(false),
            volume_name: cfg.volume_name,
        })
    }

    /// Mounts the filesystem and blocks the current thread to process FUSE requests.
    fn run_blocking(self) -> Result<(), FsError> {
        let fuse_file = self.session.lock()?.clone_fuse_file()?;

        let vfs = Vfs::new(VfsOptions::default());
        vfs.mount(Box::new(self), "/").map_err(io::Error::other)?;

        let vfs = Arc::new(vfs);
        let server = Arc::new(Server::new(vfs));

        let state = Arc::new(AtomicBool::new(false));
        let buf_size = (fuse_backend_rs::api::server::MAX_BUFFER_SIZE + 0x1000) as usize;

        let mut task = FuseDevTask::new(buf_size, fuse_file, server, state);

        let rt = Runtime::new();
        rt.block_on(async move {
            let poller = Runtime::spawn(async move { task.poll_handler().await });
            poller.await.expect("the async fuse task panicked");
        });

        Ok(())
    }

    /// Spawns a background thread to run the filesystem and returns a MountHandle.
    pub fn start(self) -> Result<MountHandle, FsError> {
        let mountpoint = self.mountpoint.clone();
        let volume_name = self.volume_name.clone();

        let thread_handle = std::thread::spawn(move || self.run_blocking());

        Ok(MountHandle {
            mountpoint,
            volume_name,
            thread_handle: Some(thread_handle),
        })
    }

    pub async fn check_write_auth_and_detach(&self) -> Result<(), FsError> {
        tracing::trace!(mountpoint = ?self.mountpoint, "Waiting for lock before asking for write authorization");

        // Ensure we only detach once, even if multiple threads call this concurrently
        let mut detached = self.detached.lock().await;
        if *detached {
            return Ok(());
        }

        tracing::trace!(
            "Asking for write authorization for volume {}",
            self.volume_name
        );

        let timeout_duration = Duration::from_secs(10);

        let auth_result = tokio::time::timeout(timeout_duration, async {
            self.manager.handle_write(&self.volume_name).await
        })
        .await?;

        if auth_result.is_err() {
            return Err(FsError::DeniedWrite(
                "self".to_string(),
                self.volume_name.clone(),
            ));
        }

        // Authorization was granted, detach the FUSE mount point
        tracing::trace!("Authorization granted, detaching FUSE mount point");
        self.umount()?;

        tracing::trace!("FUSE mount point detached");
        *detached = true;
        Ok(())
    }

    /// Sync the data before reading.
    pub async fn sync_before_read(&self) -> Result<(), FsError> {
        // Simplest version is to sync regardless if we really need to, so we just trigger a sync
        let synced = self.manager.sync(self).await?;
        if synced {
            self.inner.import()?;
        }
        Ok(())
    }

    pub fn umount(&self) -> Result<(), FsError> {
        tracing::trace!(volume_name = self.volume_name, "Unmounting volume");
        self.session.lock()?.umount().map_err(|e| e.into())
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
