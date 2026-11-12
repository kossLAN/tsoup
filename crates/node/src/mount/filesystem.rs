use std::collections::HashMap;
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fuser::{
    BsdFileFlags, Errno, FileAttr, FileHandle, FileType, FopenFlags, Generation, INodeNo,
    InitFlags, KernelConfig, LockOwner, OpenAccMode, OpenFlags, RenameFlags, ReplyAttr,
    ReplyCreate, ReplyData, ReplyDirectoryPlus, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyStatfs,
    ReplyWrite, Request as Header, TimeOrNow, WriteFlags,
};
use tokio::runtime::Handle;
use tokio::sync::watch;
use tsoup_proto::share::{
    Access, Attributes, Changes, Create, Entry, MAX_DATA, Request, Response, Share, Touch, Usage,
};
use tsoup_rpc::{Connection, RpcError};

use super::inodes::{Inodes, join};

const VALIDITY: Duration = Duration::from_secs(1);
const GENERATION: Generation = Generation(0);
const BLOCK_SIZE: u32 = 4096;
const PERMISSIONS: u32 = 0o7777;

pub type Link = watch::Receiver<Option<Connection<Share>>>;

#[derive(Clone, Copy)]
pub struct Owner {
    pub uid: u32,
    pub gid: u32,
}

struct Listing {
    path: String,
    entries: Vec<Entry>,
}

#[derive(Default)]
struct Listings {
    open: HashMap<u64, Arc<Listing>>,
    last: u64,
}

struct Remote {
    link: Link,
    owner: Owner,
    inodes: Mutex<Inodes>,
    listings: Mutex<Listings>,
}

impl Remote {
    async fn call(&self, request: Request) -> Result<Response, Errno> {
        let connection = self.link.borrow().clone().ok_or(Errno::ENOTCONN)?;

        match connection.request(request).await {
            Ok(Response::Failed { errno }) => Err(Errno::from_i32(errno)),
            Ok(response) => Ok(response),
            Err(RpcError::Disconnected) => Err(Errno::ENOTCONN),
            Err(RpcError::Failed(_) | RpcError::UnexpectedResponse) => Err(Errno::EIO),
        }
    }

    async fn attributes(&self, request: Request) -> Result<Attributes, Errno> {
        match self.call(request).await? {
            Response::Attributes(attributes) => Ok(attributes),
            _ => Err(Errno::EIO),
        }
    }

    async fn done(&self, request: Request) -> Result<(), Errno> {
        match self.call(request).await? {
            Response::Done => Ok(()),
            _ => Err(Errno::EIO),
        }
    }

    async fn open(
        &self,
        path: String,
        flags: OpenFlags,
        create: Option<Create>,
    ) -> Result<(FileHandle, Attributes), Errno> {
        let mode = flags.acc_mode();
        let access = Access {
            read: mode != OpenAccMode::O_WRONLY,
            write: mode != OpenAccMode::O_RDONLY,
            append: flags.0 & libc::O_APPEND != 0,
        };

        match self
            .call(Request::Open {
                path,
                access,
                create,
            })
            .await?
        {
            Response::Opened { handle, attributes } => Ok((FileHandle(handle), attributes)),
            _ => Err(Errno::EIO),
        }
    }

    fn path(&self, ino: INodeNo) -> Result<String, Errno> {
        self.inodes.lock().unwrap().path(ino)
    }

    fn child(&self, parent: INodeNo, name: &OsStr) -> Result<String, Errno> {
        self.inodes.lock().unwrap().child(parent, name)
    }

    fn entry(&self, path: &str, attributes: &Attributes) -> FileAttr {
        let ino = self.inodes.lock().unwrap().remember(path);
        self.attr(ino, attributes)
    }

    fn attr(&self, ino: INodeNo, attributes: &Attributes) -> FileAttr {
        FileAttr {
            ino,
            size: attributes.size,
            blocks: attributes.blocks,
            atime: attributes.accessed,
            mtime: attributes.modified,
            ctime: attributes.changed,
            crtime: attributes.changed,
            kind: kind(attributes.mode),
            perm: (attributes.mode & PERMISSIONS) as u16,
            nlink: attributes.links as u32,
            uid: self.owner.uid,
            gid: self.owner.gid,
            rdev: 0,
            blksize: BLOCK_SIZE,
            flags: 0,
        }
    }
}

trait Reply: Send + 'static {
    fn fail(self, errno: Errno);
}

macro_rules! replies {
    ($($reply:ty),*) => {
        $(impl Reply for $reply {
            fn fail(self, errno: Errno) {
                self.error(errno);
            }
        })*
    };
}

replies!(
    ReplyAttr,
    ReplyCreate,
    ReplyData,
    ReplyEmpty,
    ReplyEntry,
    ReplyOpen,
    ReplyStatfs,
    ReplyWrite
);

pub struct Filesystem {
    runtime: Handle,
    remote: Arc<Remote>,
}

impl Filesystem {
    pub fn new(runtime: Handle, link: Link, owner: Owner) -> Self {
        Self {
            runtime,
            remote: Arc::new(Remote {
                link,
                owner,
                inodes: Mutex::new(Inodes::new()),
                listings: Mutex::default(),
            }),
        }
    }

    fn run<R, T, F, O>(&self, reply: R, operation: O, finish: impl FnOnce(R, T) + Send + 'static)
    where
        R: Reply,
        O: FnOnce(Arc<Remote>) -> F,
        F: Future<Output = Result<T, Errno>> + Send + 'static,
    {
        let operation = operation(Arc::clone(&self.remote));

        self.runtime.spawn(async move {
            match operation.await {
                Ok(outcome) => finish(reply, outcome),
                Err(errno) => reply.fail(errno),
            }
        });
    }
}

impl fuser::Filesystem for Filesystem {
    fn init(&mut self, _req: &Header, config: &mut KernelConfig) -> io::Result<()> {
        config
            .add_capabilities(InitFlags::FUSE_DO_READDIRPLUS | InitFlags::FUSE_AUTO_INVAL_DATA)
            .map_err(|missing| {
                io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!("the kernel's FUSE lacks {missing:?}"),
                )
            })?;

        let _ = config.set_max_write(MAX_DATA);
        let _ = config.set_max_readahead(MAX_DATA);
        Ok(())
    }

    fn lookup(&self, _req: &Header, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let name = name.to_owned();

        self.run(
            reply,
            |remote| async move {
                let path = remote.child(parent, &name)?;
                let request = Request::Stat {
                    path: path.clone(),
                    handle: None,
                };
                let attributes = remote.attributes(request).await?;
                Ok(remote.entry(&path, &attributes))
            },
            |reply, attr| reply.entry(&VALIDITY, &attr, GENERATION),
        );
    }

    fn forget(&self, _req: &Header, ino: INodeNo, nlookup: u64) {
        self.remote.inodes.lock().unwrap().forget(ino, nlookup);
    }

    fn getattr(&self, _req: &Header, ino: INodeNo, fh: Option<FileHandle>, reply: ReplyAttr) {
        self.run(
            reply,
            |remote| async move {
                let request = Request::Stat {
                    path: remote.path(ino)?,
                    handle: fh.map(u64::from),
                };
                let attributes = remote.attributes(request).await?;
                Ok(remote.attr(ino, &attributes))
            },
            |reply, attr| reply.attr(&VALIDITY, &attr),
        );
    }

    fn setattr(
        &self,
        _req: &Header,
        ino: INodeNo,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        fh: Option<FileHandle>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        self.run(
            reply,
            |remote| async move {
                let Owner {
                    uid: owner,
                    gid: group,
                } = remote.owner;

                if uid.is_some_and(|uid| uid != owner) || gid.is_some_and(|gid| gid != group) {
                    return Err(Errno::EPERM);
                }

                let request = Request::Change {
                    path: remote.path(ino)?,
                    handle: fh.map(u64::from),
                    changes: Changes {
                        size,
                        mode: mode.map(|mode| mode & PERMISSIONS),
                        accessed: atime.map(touch).transpose()?,
                        modified: mtime.map(touch).transpose()?,
                    },
                };
                let attributes = remote.attributes(request).await?;
                Ok(remote.attr(ino, &attributes))
            },
            |reply, attr| reply.attr(&VALIDITY, &attr),
        );
    }

    fn readlink(&self, _req: &Header, ino: INodeNo, reply: ReplyData) {
        self.run(
            reply,
            |remote| async move {
                let path = remote.path(ino)?;

                match remote.call(Request::ReadLink { path }).await? {
                    Response::Target(target) => Ok(target),
                    _ => Err(Errno::EIO),
                }
            },
            |reply, target| reply.data(target.as_bytes()),
        );
    }

    fn mkdir(
        &self,
        _req: &Header,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let name = name.to_owned();

        self.run(
            reply,
            |remote| async move {
                let path = remote.child(parent, &name)?;
                let request = Request::MakeDir {
                    path: path.clone(),
                    mode: mode & PERMISSIONS,
                };
                let attributes = remote.attributes(request).await?;
                Ok(remote.entry(&path, &attributes))
            },
            |reply, attr| reply.entry(&VALIDITY, &attr, GENERATION),
        );
    }

    fn unlink(&self, _req: &Header, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let name = name.to_owned();

        self.run(
            reply,
            |remote| async move {
                let path = remote.child(parent, &name)?;
                remote.done(Request::Remove { path: path.clone() }).await?;
                remote.inodes.lock().unwrap().unlink(&path);
                Ok(())
            },
            |reply, ()| reply.ok(),
        );
    }

    fn rmdir(&self, _req: &Header, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let name = name.to_owned();

        self.run(
            reply,
            |remote| async move {
                let path = remote.child(parent, &name)?;
                remote
                    .done(Request::RemoveDir { path: path.clone() })
                    .await?;
                remote.inodes.lock().unwrap().unlink(&path);
                Ok(())
            },
            |reply, ()| reply.ok(),
        );
    }

    fn symlink(
        &self,
        _req: &Header,
        parent: INodeNo,
        link_name: &OsStr,
        target: &Path,
        reply: ReplyEntry,
    ) {
        let name = link_name.to_owned();
        let target = target.to_str().map(str::to_owned);

        self.run(
            reply,
            |remote| async move {
                let path = remote.child(parent, &name)?;
                let request = Request::Symlink {
                    path: path.clone(),
                    target: target.ok_or(Errno::EINVAL)?,
                };
                let attributes = remote.attributes(request).await?;
                Ok(remote.entry(&path, &attributes))
            },
            |reply, attr| reply.entry(&VALIDITY, &attr, GENERATION),
        );
    }

    fn rename(
        &self,
        _req: &Header,
        parent: INodeNo,
        name: &OsStr,
        newparent: INodeNo,
        newname: &OsStr,
        flags: RenameFlags,
        reply: ReplyEmpty,
    ) {
        let name = name.to_owned();
        let newname = newname.to_owned();

        self.run(
            reply,
            |remote| async move {
                if !flags.is_empty() {
                    return Err(Errno::EINVAL);
                }

                let from = remote.child(parent, &name)?;
                let to = remote.child(newparent, &newname)?;
                let request = Request::Rename {
                    from: from.clone(),
                    to: to.clone(),
                };
                remote.done(request).await?;
                remote.inodes.lock().unwrap().rename(&from, &to);
                Ok(())
            },
            |reply, ()| reply.ok(),
        );
    }

    fn link(
        &self,
        _req: &Header,
        ino: INodeNo,
        newparent: INodeNo,
        newname: &OsStr,
        reply: ReplyEntry,
    ) {
        let newname = newname.to_owned();

        self.run(
            reply,
            |remote| async move {
                let to = remote.child(newparent, &newname)?;
                let request = Request::Link {
                    from: remote.path(ino)?,
                    to: to.clone(),
                };
                let attributes = remote.attributes(request).await?;
                Ok(remote.entry(&to, &attributes))
            },
            |reply, attr| reply.entry(&VALIDITY, &attr, GENERATION),
        );
    }

    fn open(&self, _req: &Header, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        self.run(
            reply,
            |remote| async move {
                let (handle, _) = remote.open(remote.path(ino)?, flags, None).await?;
                Ok(handle)
            },
            |reply, handle| reply.opened(handle, FopenFlags::empty()),
        );
    }

    fn read(
        &self,
        _req: &Header,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyData,
    ) {
        self.run(
            reply,
            |remote| async move {
                let request = Request::Read {
                    handle: fh.0,
                    offset,
                    length: size,
                };

                match remote.call(request).await? {
                    Response::Data(data) => Ok(data),
                    _ => Err(Errno::EIO),
                }
            },
            |reply, data| reply.data(&data),
        );
    }

    fn write(
        &self,
        _req: &Header,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: WriteFlags,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyWrite,
    ) {
        let written = data.len() as u32;
        let request = Request::Write {
            handle: fh.0,
            offset,
            data: data.to_vec(),
        };

        self.run(
            reply,
            |remote| async move { remote.done(request).await },
            move |reply, ()| reply.written(written),
        );
    }

    fn release(
        &self,
        _req: &Header,
        _ino: INodeNo,
        fh: FileHandle,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        self.run(
            reply,
            |remote| async move { remote.done(Request::Close { handle: fh.0 }).await },
            |reply, ()| reply.ok(),
        );
    }

    fn fsync(
        &self,
        _req: &Header,
        _ino: INodeNo,
        fh: FileHandle,
        _datasync: bool,
        reply: ReplyEmpty,
    ) {
        self.run(
            reply,
            |remote| async move { remote.done(Request::Sync { handle: fh.0 }).await },
            |reply, ()| reply.ok(),
        );
    }

    fn opendir(&self, _req: &Header, ino: INodeNo, _flags: OpenFlags, reply: ReplyOpen) {
        self.run(
            reply,
            |remote| async move {
                let path = remote.path(ino)?;
                let Response::Entries(entries) =
                    remote.call(Request::List { path: path.clone() }).await?
                else {
                    return Err(Errno::EIO);
                };

                let mut listings = remote.listings.lock().unwrap();
                listings.last += 1;
                let handle = listings.last;
                listings
                    .open
                    .insert(handle, Arc::new(Listing { path, entries }));
                Ok(FileHandle(handle))
            },
            |reply, handle| reply.opened(handle, FopenFlags::empty()),
        );
    }

    fn readdirplus(
        &self,
        _req: &Header,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectoryPlus,
    ) {
        let listing = self
            .remote
            .listings
            .lock()
            .unwrap()
            .open
            .get(&fh.0)
            .cloned();
        let Some(listing) = listing else {
            reply.error(Errno::EBADF);
            return;
        };
        let mut inodes = self.remote.inodes.lock().unwrap();

        for (index, entry) in listing.entries.iter().enumerate().skip(offset as usize) {
            let ino = inodes.remember(&join(&listing.path, &entry.name));
            let attr = self.remote.attr(ino, &entry.attributes);
            let next = index as u64 + 1;

            if reply.add(ino, next, &entry.name, &VALIDITY, &attr, GENERATION) {
                inodes.forget(ino, 1);
                break;
            }
        }

        drop(inodes);
        reply.ok();
    }

    fn releasedir(
        &self,
        _req: &Header,
        _ino: INodeNo,
        fh: FileHandle,
        _flags: OpenFlags,
        reply: ReplyEmpty,
    ) {
        self.remote.listings.lock().unwrap().open.remove(&fh.0);
        reply.ok();
    }

    fn statfs(&self, _req: &Header, _ino: INodeNo, reply: ReplyStatfs) {
        self.run(
            reply,
            |remote| async move {
                match remote.call(Request::Usage).await? {
                    Response::Usage(usage) => Ok(usage),
                    _ => Err(Errno::EIO),
                }
            },
            |reply, usage: Usage| {
                reply.statfs(
                    usage.blocks,
                    usage.free,
                    usage.available,
                    usage.files,
                    usage.files_free,
                    usage.block_size as u32,
                    usage.name_length as u32,
                    usage.block_size as u32,
                );
            },
        );
    }

    fn create(
        &self,
        _req: &Header,
        parent: INodeNo,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        let name = name.to_owned();

        self.run(
            reply,
            |remote| async move {
                let path = remote.child(parent, &name)?;
                let create = Create {
                    mode: mode & PERMISSIONS,
                    exclusive: flags & libc::O_EXCL != 0,
                };
                let (handle, attributes) = remote
                    .open(path.clone(), OpenFlags(flags), Some(create))
                    .await?;
                Ok((remote.entry(&path, &attributes), handle))
            },
            |reply, (attr, handle)| {
                reply.created(&VALIDITY, &attr, GENERATION, handle, FopenFlags::empty());
            },
        );
    }
}

fn kind(mode: u32) -> FileType {
    match mode & libc::S_IFMT {
        libc::S_IFDIR => FileType::Directory,
        libc::S_IFLNK => FileType::Symlink,
        libc::S_IFIFO => FileType::NamedPipe,
        libc::S_IFSOCK => FileType::Socket,
        libc::S_IFCHR => FileType::CharDevice,
        libc::S_IFBLK => FileType::BlockDevice,
        _ => FileType::RegularFile,
    }
}

fn touch(time: TimeOrNow) -> Result<Touch, Errno> {
    match time {
        TimeOrNow::Now => Ok(Touch::Now),
        TimeOrNow::SpecificTime(time) if time >= UNIX_EPOCH => Ok(Touch::At(time)),
        TimeOrNow::SpecificTime(_) => Err(Errno::EINVAL),
    }
}
