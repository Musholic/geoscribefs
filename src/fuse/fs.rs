use crate::{
    fuse::{Config, GeoScribeFs, MountHandle},
    server::{error::FsError, sync::SyncHandler, write::WriteHandler},
};
use arc_swap::ArcSwap;
use fuse_backend_rs::{
    api::{
        BackendFileSystem, Vfs, VfsOptions,
        filesystem::{Entry, FileSystem},
        server::Server,
    },
    async_runtime::Runtime,
    passthrough::{self, PassthroughFs},
    transport::{FuseDevTask, FuseSession},
};
use std::{
    any::Any,
    fs::File,
    io::{self},
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{Arc, OnceLock, atomic::AtomicBool},
    time::Duration,
};
use tokio::sync::{Mutex, MutexGuard};

/// Mounts the filesystem and blocks the current thread to process FUSE requests.
fn run_blocking(vfs: Vfs, fuse_file: File) -> Result<(), FsError> {
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

impl GeoScribeFs {
    fn build_inner(volume: &Path) -> Result<PassthroughFs, FsError> {
        let pt_cfg = passthrough::Config {
            root_dir: volume.to_string_lossy().to_string(),
            do_import: false,
            // Disable caching to ensure that the FUSE filesystem always reads the latest data from the underlying storage even on restore
            entry_timeout: Duration::ZERO,
            attr_timeout: Duration::ZERO,
            cache_policy: passthrough::CachePolicy::Metadata,
            ..Default::default()
        };
        let inner = PassthroughFs::<()>::new(pt_cfg).map_err(io::Error::other)?;
        Ok(inner)
    }

    pub async fn new(cfg: Config) -> Result<Self, FsError> {
        let inner = Self::build_inner(&cfg.volume)?;
        // Park directory for this volume: the FUSE mount is MS_MOVE'd here when the
        // volume is hidden (write authorization handed over) and moved back to
        // `cfg.volume` when it is revealed again. Moving (rather than unmounting)
        // keeps the same /dev/fuse fd, superblock, Vfs and serving thread alive.
        let park_mountpoint = Path::new("/tmp/geoscribefs")
            .join(format!("{:04}", std::process::id()))
            .join(".volumes")
            .join(&cfg.volume_name);

        // Create the whole park path with owner-only permissions; it must exist
        // before FuseSession::new (which canonicalizes the mountpoint) and before
        // any move into it.
        std::fs::create_dir_all(&park_mountpoint)?;
        if let Some(parent) = park_mountpoint.parent() {
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }

        // We need to start the fuse session in the parked location since .umount() will always
        // try to umount at the initial location, and we can always safely park before umount
        let session = FuseSession::new(&park_mountpoint, "geoscribefs", "", false)?;

        let result = Self {
            inner: ArcSwap::from_pointee(inner),
            mountpoint: cfg.volume,
            session: std::sync::Mutex::new(session),
            manager: Arc::downgrade(&cfg.manager),
            parked: Mutex::new(true),
            volume_name: cfg.volume_name,
            fs_opts: OnceLock::new(),
            park_mountpoint,
        };

        result.attach().await?;
        result.session.lock()?.mount()?;

        Ok(result)
    }

    /// Spawns a background thread to run the filesystem and returns a MountHandle.
    pub async fn start(self) -> Result<MountHandle, FsError> {
        let mountpoint = self.mountpoint.clone();
        let volume_name = self.volume_name.clone();
        let fuse_file = self.session.lock()?.clone_fuse_file()?;

        // Keep OPEN/OPENDIR alive: with the default options the Vfs answers them itself with
        // ENOSYS as soon as the kernel offers ZERO_MESSAGE_OPEN. Without a real open reply the
        // kernel never invalidates its page cache on open, so reading a volume that another node
        // has just restored would be served from stale cached pages
        let vfs = Vfs::new(VfsOptions {
            no_open: false,
            no_opendir: false,
            ..Default::default()
        });
        vfs.mount(Box::new(self), "/").map_err(io::Error::other)?;
        let fs = Arc::downgrade(&vfs.get_rootfs("/").unwrap().unwrap().0);

        let thread_handle = std::thread::spawn(move || run_blocking(vfs, fuse_file));

        Ok(MountHandle {
            mountpoint,
            volume_name,
            thread_handle: Some(thread_handle),
            fs,
        })
    }

    pub async fn check_write_auth_and_park(&self) -> Result<(), FsError> {
        tracing::trace!(mountpoint = ?self.mountpoint, "Waiting for lock before asking for write authorization");

        // Ensure we only check and park once, even if multiple threads call this concurrently
        let parked = self.parked.lock().await;
        if *parked {
            return Ok(());
        }

        tracing::trace!(
            "Asking for write authorization for volume {}",
            self.volume_name
        );

        let timeout_duration = Duration::from_secs(10);

        let auth_result = tokio::time::timeout(timeout_duration, async {
            self.manager
                .upgrade()
                .ok_or_else(|| FsError::WeakReferenceLost("Manager"))?
                .handle_write(&self.volume_name)
                .await
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
        self.park_with_lock(parked)?;
        Ok(())
    }

    /// Sync the data before reading.
    pub async fn sync_before_read(&self) -> Result<(), FsError> {
        // Simplest version is to sync regardless if we really need to, so we just trigger a sync
        self.manager
            .upgrade()
            .ok_or_else(|| FsError::WeakReferenceLost("Manager"))?
            .sync(self)
            .await?;
        Ok(())
    }

    /// Unmount and release any opened file handle
    pub async fn detach(&self) -> Result<(), FsError> {
        self.inner
            .store(Arc::new(Self::build_inner(&self.mountpoint)?));
        Ok(())
    }

    pub async fn attach(&self) -> Result<(), FsError> {
        let inner = self.inner.load();
        inner.import()?;
        if let Some(fs_opts) = self.fs_opts.get() {
            inner.init(*fs_opts)?;
        }
        Ok(())
    }

    /// Move the mount from the user-visible volume path to the park directory
    pub async fn park(&self) -> Result<(), FsError> {
        self.park_with_lock(self.parked.lock().await)
    }

    fn park_with_lock(&self, mut parked: MutexGuard<'_, bool>) -> Result<(), FsError> {
        if !*parked {
            tracing::trace!(mountpoint = ?self.mountpoint, park_mountpoint = ?self.park_mountpoint, "Parking volume");
            nix::mount::mount(
                Some(&self.mountpoint),
                &self.park_mountpoint,
                None::<&std::path::Path>,
                nix::mount::MsFlags::MS_MOVE,
                None::<&std::path::Path>,
            )?;
        }
        *parked = true;
        Ok(())
    }

    /// Move the mount from the park directory to the user-visible volume path
    pub async fn unpark(&self) -> Result<(), FsError> {
        let mut parked = self.parked.lock().await;

        if *parked {
            tracing::trace!(mountpoint = ?self.mountpoint, park_mountpoint = ?self.park_mountpoint, "Un-Parking volume");
            nix::mount::mount(
                Some(&self.park_mountpoint),
                &self.mountpoint,
                None::<&std::path::Path>,
                nix::mount::MsFlags::MS_MOVE,
                None::<&std::path::Path>,
            )?;
            *parked = false;
        }
        Ok(())
    }

    /// Called on server exit to clean up the mount
    pub async fn umount_session(&self) -> Result<(), FsError> {
        // We need to park first the fuse mount as the fuse session can only umount his initial parked location
        self.park().await?;
        self.session.lock()?.umount()?;
        Ok(())
    }
}

impl BackendFileSystem for GeoScribeFs {
    fn mount(&self) -> io::Result<(Entry, u64)> {
        self.inner.load().mount()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
