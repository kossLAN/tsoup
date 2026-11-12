use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cap_fs_ext::{DirExt as _, FollowSymlinks, OpenOptionsFollowExt as _, SystemTimeSpec};
use cap_std::ambient_authority;
use cap_std::fs::{
    Dir, DirBuilder, DirBuilderExt as _, File, FileExt as _, Metadata, MetadataExt as _,
    OpenOptions, OpenOptionsExt as _, Permissions, PermissionsExt as _,
};
use cap_std::time::SystemTime as CapTime;
use rustix::io::Errno;
use tsoup_proto::share::{
    Access, Attributes, Changes, Create, Entry, MAX_DATA, Request, Response, Touch, Usage,
};

use super::roots::{Root, Roots};

const ROOT_MODE: u32 = 0o040_555;

enum Place {
    Root,
    Within(Dir, PathBuf),
}

#[derive(Default)]
struct Files {
    open: HashMap<u64, Arc<File>>,
    last: u64,
}

pub struct Tree {
    roots: Roots,
    files: Mutex<Files>,
}

impl Tree {
    pub fn new(roots: Roots) -> Self {
        Self {
            roots,
            files: Mutex::default(),
        }
    }

    pub fn answer(&self, request: Request) -> Response {
        self.handle(request)
            .unwrap_or_else(|error| Response::Failed {
                errno: errno(&error),
            })
    }

    fn handle(&self, request: Request) -> io::Result<Response> {
        match request {
            Request::Stat { path, handle } => Ok(Response::Attributes(match handle {
                Some(handle) => attributes(&self.file(handle)?.metadata()?),
                None => match self.locate(&path)? {
                    Place::Root => self.root()?,
                    Place::Within(dir, path) => attributes(&dir.symlink_metadata(path)?),
                },
            })),
            Request::List { path } => Ok(Response::Entries(match self.locate(&path)? {
                Place::Root => self.workspaces(),
                Place::Within(dir, path) => entries(&dir, &path)?,
            })),
            Request::ReadLink { path } => {
                let (dir, path) = self.within(&path)?;
                let target = dir.read_link_contents(path)?;
                Ok(Response::Target(
                    target
                        .into_os_string()
                        .into_string()
                        .map_err(|_| Errno::ILSEQ)?,
                ))
            },
            Request::Open {
                path,
                access,
                create,
            } => {
                let (dir, path) = match create {
                    Some(_) => self.entry(&path)?,
                    None => self.within(&path)?,
                };
                let file = open(&dir, &path, access, create)?;
                let attributes = attributes(&file.metadata()?);
                Ok(Response::Opened {
                    handle: self.keep(file),
                    attributes,
                })
            },
            Request::Read {
                handle,
                offset,
                length,
            } => {
                let file = self.file(handle)?;
                let mut data = vec![0; length.min(MAX_DATA) as usize];
                let mut filled = 0;

                while filled < data.len() {
                    match file.read_at(&mut data[filled..], offset + filled as u64)? {
                        0 => break,
                        read => filled += read,
                    }
                }

                data.truncate(filled);
                Ok(Response::Data(data))
            },
            Request::Write {
                handle,
                offset,
                data,
            } => {
                self.file(handle)?.write_all_at(&data, offset)?;
                Ok(Response::Done)
            },
            Request::Sync { handle } => {
                self.file(handle)?.sync_all()?;
                Ok(Response::Done)
            },
            Request::Close { handle } => {
                self.files.lock().unwrap().open.remove(&handle);
                Ok(Response::Done)
            },
            Request::Change {
                path,
                handle,
                changes:
                    Changes {
                        size,
                        mode,
                        accessed,
                        modified,
                    },
            } => {
                let (dir, path) = self.within(&path)?;
                let file = handle.map(|handle| self.file(handle)).transpose()?;

                if let Some(size) = size {
                    match &file {
                        Some(file) => file.set_len(size)?,
                        None => dir
                            .open_with(&path, OpenOptions::new().write(true))?
                            .set_len(size)?,
                    }
                }

                if let Some(mode) = mode {
                    dir.set_permissions(&path, Permissions::from_mode(mode))?;
                }

                if accessed.is_some() || modified.is_some() {
                    dir.set_symlink_times(&path, accessed.map(spec), modified.map(spec))?;
                }

                Ok(Response::Attributes(attributes(&match &file {
                    Some(file) => file.metadata()?,
                    None => dir.symlink_metadata(&path)?,
                })))
            },
            Request::MakeDir { path, mode } => {
                let (dir, path) = self.entry(&path)?;
                dir.create_dir_with(&path, DirBuilder::new().mode(mode))?;
                Ok(Response::Attributes(attributes(
                    &dir.symlink_metadata(path)?,
                )))
            },
            Request::Remove { path } => {
                let (dir, path) = self.entry(&path)?;
                dir.remove_file(path)?;
                Ok(Response::Done)
            },
            Request::RemoveDir { path } => {
                let (dir, path) = self.entry(&path)?;
                dir.remove_dir(path)?;
                Ok(Response::Done)
            },
            Request::Rename { from, to } => {
                let (from_dir, from) = self.entry(&from)?;
                let (to_dir, to) = self.entry(&to)?;
                from_dir.rename(from, &to_dir, to)?;
                Ok(Response::Done)
            },
            Request::Symlink { path, target } => {
                let (dir, path) = self.entry(&path)?;
                dir.symlink_contents(target, &path)?;
                Ok(Response::Attributes(attributes(
                    &dir.symlink_metadata(path)?,
                )))
            },
            Request::Link { from, to } => {
                let (from_dir, from) = self.entry(&from)?;
                let (to_dir, to) = self.entry(&to)?;
                from_dir.hard_link(from, &to_dir, &to)?;
                Ok(Response::Attributes(attributes(
                    &to_dir.symlink_metadata(to)?,
                )))
            },
            Request::Usage => {
                let usage = rustix::fs::statvfs(self.roots.base())?;
                Ok(Response::Usage(Usage {
                    block_size: usage.f_frsize,
                    blocks: usage.f_blocks,
                    free: usage.f_bfree,
                    available: usage.f_bavail,
                    files: usage.f_files,
                    files_free: usage.f_ffree,
                    name_length: usage.f_namemax,
                }))
            },
        }
    }

    fn root(&self) -> io::Result<Attributes> {
        let metadata =
            Dir::open_ambient_dir(self.roots.base(), ambient_authority())?.dir_metadata()?;
        Ok(Attributes {
            mode: ROOT_MODE,
            ..attributes(&metadata)
        })
    }

    fn workspaces(&self) -> Vec<Entry> {
        self.roots
            .list()
            .into_iter()
            .filter_map(|Root { name, path }| {
                let metadata = Dir::open_ambient_dir(path, ambient_authority())
                    .and_then(|dir| dir.dir_metadata())
                    .ok()?;
                Some(Entry {
                    name,
                    attributes: attributes(&metadata),
                })
            })
            .collect()
    }

    fn locate(&self, path: &str) -> io::Result<Place> {
        if path.is_empty() {
            return Ok(Place::Root);
        }

        let (workspace, rest) = path.split_once('/').unwrap_or((path, "."));
        let dir = Dir::open_ambient_dir(self.roots.find(workspace)?, ambient_authority())?;
        Ok(Place::Within(dir, PathBuf::from(rest)))
    }

    fn within(&self, path: &str) -> io::Result<(Dir, PathBuf)> {
        match self.locate(path)? {
            Place::Root => Err(Errno::ACCESS.into()),
            Place::Within(dir, path) => Ok((dir, path)),
        }
    }

    fn entry(&self, path: &str) -> io::Result<(Dir, PathBuf)> {
        let (workspace, rest) = path.split_once('/').ok_or(Errno::ACCESS)?;
        let dir = Dir::open_ambient_dir(self.roots.find(workspace)?, ambient_authority())?;
        Ok((dir, PathBuf::from(rest)))
    }

    fn keep(&self, file: File) -> u64 {
        let mut files = self.files.lock().unwrap();
        files.last += 1;
        let handle = files.last;
        files.open.insert(handle, Arc::new(file));
        handle
    }

    fn file(&self, handle: u64) -> io::Result<Arc<File>> {
        self.files
            .lock()
            .unwrap()
            .open
            .get(&handle)
            .cloned()
            .ok_or_else(|| Errno::BADF.into())
    }
}

fn open(dir: &Dir, path: &Path, access: Access, create: Option<Create>) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(access.read)
        .write(access.write)
        .append(access.append)
        .follow(FollowSymlinks::No);

    if let Some(Create { mode, exclusive }) = create {
        if access.write || access.append {
            options.create(true).create_new(exclusive).mode(mode);
        } else {
            // OpenOptions refuses to create a file it does not also write.
            let mut creation = OpenOptions::new();
            creation
                .write(true)
                .create(true)
                .create_new(exclusive)
                .mode(mode)
                .follow(FollowSymlinks::No);
            dir.open_with(path, &creation)?;
        }
    }

    dir.open_with(path, &options)
}

fn entries(dir: &Dir, path: &Path) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();

    for entry in dir.read_dir(path)? {
        let entry = entry?;

        if let Ok(name) = entry.file_name().into_string()
            && let Ok(metadata) = entry.metadata()
        {
            entries.push(Entry {
                name,
                attributes: attributes(&metadata),
            });
        }
    }

    Ok(entries)
}

fn attributes(metadata: &Metadata) -> Attributes {
    Attributes {
        mode: metadata.mode(),
        size: metadata.len(),
        blocks: metadata.blocks(),
        links: metadata.nlink(),
        accessed: time(metadata.atime(), metadata.atime_nsec()),
        modified: time(metadata.mtime(), metadata.mtime_nsec()),
        changed: time(metadata.ctime(), metadata.ctime_nsec()),
    }
}

fn time(seconds: i64, nanoseconds: i64) -> SystemTime {
    u64::try_from(seconds).map_or(UNIX_EPOCH, |seconds| {
        UNIX_EPOCH + Duration::new(seconds, nanoseconds as u32)
    })
}

fn spec(touch: Touch) -> SystemTimeSpec {
    match touch {
        Touch::Now => SystemTimeSpec::SymbolicNow,
        Touch::At(time) => SystemTimeSpec::Absolute(CapTime::from_std(time)),
    }
}

fn errno(error: &io::Error) -> i32 {
    let errno = Errno::from_io_error(error).unwrap_or_else(|| match error.kind() {
        io::ErrorKind::PermissionDenied => Errno::ACCESS,
        _ => Errno::IO,
    });
    errno.raw_os_error()
}
