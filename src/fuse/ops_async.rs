use fuse_backend_rs::abi::fuse_abi::CreateIn;
use fuse_backend_rs::api::filesystem::{
    AsyncFileSystem, AsyncZeroCopyReader, AsyncZeroCopyWriter, Context, Entry, OpenOptions,
    SetattrValid,
};
use libc::stat64;
use std::ffi::CStr;
use std::io::{self, Result};
use std::time::Duration;
use tonic::async_trait;

use crate::fuse::GeoScribeFs;

// Delegate all calls to the inner PassthroughFs except for write operations
#[warn(clippy::missing_trait_methods)]
#[async_trait]
impl AsyncFileSystem for GeoScribeFs {
    async fn async_open(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        flags: u32,
        fuse_flags: u32,
    ) -> Result<(Option<Self::Handle>, OpenOptions)> {
        let is_write = (flags as i32 & libc::O_ACCMODE) != libc::O_RDONLY
            || (flags as i32 & libc::O_APPEND) != 0
            || (flags as i32 & libc::O_TRUNC) != 0;
        if is_write {
            self.check_write_auth_and_detach().await?;
        }
        self.inner.async_open(ctx, inode, flags, fuse_flags).await
    }

    async fn async_create(
        &self,
        ctx: &Context,
        parent: Self::Inode,
        name: &CStr,
        args: CreateIn,
    ) -> Result<(Entry, Option<Self::Handle>, OpenOptions)> {
        self.check_write_auth_and_detach().await?;
        self.inner.async_create(ctx, parent, name, args).await
    }

    async fn async_write(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        r: &mut (dyn AsyncZeroCopyReader + Send),
        size: u32,
        offset: u64,
        lock_owner: Option<u64>,
        delayed_write: bool,
        flags: u32,
        fuse_flags: u32,
    ) -> io::Result<usize> {
        self.check_write_auth_and_detach().await?;
        self.inner
            .async_write(
                ctx,
                inode,
                handle,
                r,
                size,
                offset,
                lock_owner,
                delayed_write,
                flags,
                fuse_flags,
            )
            .await
    }

    async fn async_setattr(
        &self,
        _ctx: &Context,
        inode: Self::Inode,
        attr: libc::stat64,
        handle: Option<Self::Handle>,
        valid: SetattrValid,
    ) -> Result<(libc::stat64, Duration)> {
        // FATTR_SIZE triggers during file truncation (e.g. `> file.txt`)
        if valid.contains(SetattrValid::SIZE) {
            self.check_write_auth_and_detach().await?;
        }
        self.inner
            .async_setattr(_ctx, inode, attr, handle, valid)
            .await
    }

    // Delegated methods
    async fn async_lookup(
        &self,
        ctx: &Context,
        parent: Self::Inode,
        name: &CStr,
    ) -> io::Result<Entry> {
        self.inner.async_lookup(ctx, parent, name).await
    }

    async fn async_getattr(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Option<Self::Handle>,
    ) -> io::Result<(stat64, Duration)> {
        self.inner.async_getattr(ctx, inode, handle).await
    }

    async fn async_read(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        w: &mut (dyn AsyncZeroCopyWriter + Send),
        size: u32,
        offset: u64,
        lock_owner: Option<u64>,
        flags: u32,
    ) -> io::Result<usize> {
        self.inner
            .async_read(ctx, inode, handle, w, size, offset, lock_owner, flags)
            .await
    }

    async fn async_fsync(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        datasync: bool,
        handle: Self::Handle,
    ) -> io::Result<()> {
        self.inner.async_fsync(ctx, inode, datasync, handle).await
    }

    async fn async_fallocate(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        handle: Self::Handle,
        mode: u32,
        offset: u64,
        length: u64,
    ) -> io::Result<()> {
        self.inner
            .async_fallocate(ctx, inode, handle, mode, offset, length)
            .await
    }

    async fn async_fsyncdir(
        &self,
        ctx: &Context,
        inode: Self::Inode,
        datasync: bool,
        handle: Self::Handle,
    ) -> io::Result<()> {
        self.inner
            .async_fsyncdir(ctx, inode, datasync, handle)
            .await
    }
}
