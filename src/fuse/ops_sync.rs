use fuse_backend_rs::abi::fuse_abi::FsOptions;
use fuse_backend_rs::api::filesystem::{
    Context, DirEntry, Entry, FileLock, FileSystem, GetxattrReply, IoctlData, ListxattrReply,
    OpenOptions, ZeroCopyReader, ZeroCopyWriter,
};
use fuse_backend_rs::passthrough::PassthroughFs;
use std::ffi::CStr;
use std::time::Duration;

use crate::fuse::GeoScribeFs;

impl FileSystem for GeoScribeFs {
    type Inode = <PassthroughFs as FileSystem>::Inode;
    type Handle = <PassthroughFs as FileSystem>::Handle;

    forward_to_inner! {
        fn read(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, w: &mut dyn ZeroCopyWriter, size: u32, offset: u64, lock_owner: Option<u64>, flags: u32) -> std::io::Result<usize>;
        fn write(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, r: &mut dyn ZeroCopyReader, size: u32, offset: u64, lock_owner: Option<u64>, delayed_write: bool, flags: u32, fuse_flags: u32) -> std::io::Result<usize>;
        fn readdir(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, size: u32, offset: u64, add_entry: &mut dyn FnMut(DirEntry) -> std::io::Result<usize>) -> std::io::Result<()>;
        fn readdirplus(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, size: u32, offset: u64, add_entry: &mut dyn FnMut( DirEntry, Entry, ) -> std::io::Result<usize>) -> std::io::Result<()>;
        fn getlk(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, owner: u64, lock: FileLock, flags: u32) -> std::io::Result<FileLock>;
        fn setlk(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, owner: u64, lock: FileLock, flags: u32) -> std::io::Result<()>;
        fn setlkw(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, owner: u64, lock: FileLock, flags: u32) -> std::io::Result<()>;
        fn ioctl(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, flags: u32, cmd: u32, data: IoctlData, out_size: u32) -> std::io::Result<IoctlData<'_>>;
    }

    forward_to_inner_full_trace! {
        fn init(&self, capable: FsOptions) -> std::io::Result<FsOptions>;
        fn destroy(&self);
        fn lookup(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> std::io::Result<Entry>;
        fn forget(&self, ctx: &Context, inode: Self::Inode, count: u64);
        fn batch_forget(&self, ctx: &Context, requests: Vec<(Self::Inode, u64)>);
        fn getattr( &self, ctx: &Context, inode: Self::Inode, handle: Option<Self::Handle>) -> std::io::Result<(libc::stat64, Duration)>;
        fn readlink(&self, ctx: &Context, inode: Self::Inode) -> std::io::Result<Vec<u8>>;
        fn symlink(&self, ctx: &Context, linkname: &CStr, parent: Self::Inode, name: &CStr) -> std::io::Result<Entry>;
        fn mknod(&self, ctx: &Context, inode: Self::Inode, name: &CStr, mode: u32, rdev: u32, umask: u32) -> std::io::Result<Entry>;
        fn mkdir(&self, ctx: &Context, parent: Self::Inode, name: &CStr, mode: u32, umask: u32) -> std::io::Result<Entry>;
        fn unlink(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> std::io::Result<()>;
        fn rmdir(&self, ctx: &Context, parent: Self::Inode, name: &CStr) -> std::io::Result<()>;
        fn rename(&self, ctx: &Context, olddir: Self::Inode, oldname: &CStr, newdir: Self::Inode, newname: &CStr, flags: u32) -> std::io::Result<()>;
        fn link(&self, ctx: &Context, inode: Self::Inode, newparent: Self::Inode, newname: &CStr) -> std::io::Result<Entry>;
        fn flush(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, lock_owner: u64) -> std::io::Result<()>;
        fn fsync(&self, ctx: &Context, inode: Self::Inode, datasync: bool, handle: Self::Handle) -> std::io::Result<()>;
        fn fallocate(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, mode: u32, offset: u64, length: u64) -> std::io::Result<()>;
        fn release(&self, ctx: &Context, inode: Self::Inode, flags: u32, handle: Self::Handle, flush: bool, flock_release: bool, lock_owner: Option<u64>) -> std::io::Result<()>;
        fn statfs(&self, ctx: &Context, inode: Self::Inode) -> std::io::Result<libc::statvfs64>;
        fn setxattr(&self, ctx: &Context, inode: Self::Inode, name: &CStr, value: &[u8], flags: u32) -> std::io::Result<()>;
        fn getxattr(&self, ctx: &Context, inode: Self::Inode, name: &CStr, size: u32) -> std::io::Result<GetxattrReply>;
        fn listxattr(&self, ctx: &Context, inode: Self::Inode, size: u32) -> std::io::Result<ListxattrReply>;
        fn removexattr(&self, ctx: &Context, inode: Self::Inode, name: &CStr) -> std::io::Result<()>;
        fn opendir(&self, ctx: &Context, inode: Self::Inode, flags: u32) -> std::io::Result<(Option<Self::Handle>, OpenOptions)>;
        fn fsyncdir(&self, ctx: &Context, inode: Self::Inode, datasync: bool, handle: Self::Handle) -> std::io::Result<()>;
        fn releasedir(&self, ctx: &Context, inode: Self::Inode, flags: u32, handle: Self::Handle) -> std::io::Result<()>;
        fn access(&self, ctx: &Context, inode: Self::Inode, mask: u32) -> std::io::Result<()>;
        fn lseek(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, offset: u64, whence: u32) -> std::io::Result<u64>;
        fn bmap(&self, ctx: &Context, inode: Self::Inode, block: u64, blocksize: u32) -> std::io::Result<u64>;
        fn poll(&self, ctx: &Context, inode: Self::Inode, handle: Self::Handle, khandle: Self::Handle, flags: u32, events: u32) -> std::io::Result<u32>;
        fn notify_reply(&self) -> std::io::Result<()>;
        fn id_remap(&self, ctx: &mut Context) -> std::io::Result<()>;
    }
}
