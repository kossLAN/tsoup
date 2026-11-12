use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::binary;
use crate::rpc::Protocol;

pub const PROTOCOL: &str = "tsoup-share";
pub const MAX_DATA: u32 = 1024 * 1024;

pub struct Share;

impl Protocol for Share {
    type Request = Request;
    type Response = Response;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request {
    Stat {
        path: String,
        handle: Option<u64>,
    },
    List {
        path: String,
    },
    ReadLink {
        path: String,
    },
    Open {
        path: String,
        access: Access,
        create: Option<Create>,
    },
    Read {
        handle: u64,
        offset: u64,
        length: u32,
    },
    Write {
        handle: u64,
        offset: u64,
        #[serde(with = "binary")]
        data: Vec<u8>,
    },
    Sync {
        handle: u64,
    },
    Close {
        handle: u64,
    },
    Change {
        path: String,
        handle: Option<u64>,
        changes: Changes,
    },
    MakeDir {
        path: String,
        mode: u32,
    },
    Remove {
        path: String,
    },
    RemoveDir {
        path: String,
    },
    Rename {
        from: String,
        to: String,
    },
    Symlink {
        path: String,
        target: String,
    },
    Link {
        from: String,
        to: String,
    },
    Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    Attributes(Attributes),
    Entries(Vec<Entry>),
    Target(String),
    Opened { handle: u64, attributes: Attributes },
    Data(#[serde(with = "binary")] Vec<u8>),
    Usage(Usage),
    Done,
    Failed { errno: i32 },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Access {
    pub read: bool,
    pub write: bool,
    pub append: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Create {
    pub mode: u32,
    pub exclusive: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Changes {
    pub size: Option<u64>,
    pub mode: Option<u32>,
    pub accessed: Option<Touch>,
    pub modified: Option<Touch>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Touch {
    Now,
    At(SystemTime),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Attributes {
    pub mode: u32,
    pub size: u64,
    pub blocks: u64,
    pub links: u64,
    pub accessed: SystemTime,
    pub modified: SystemTime,
    pub changed: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub attributes: Attributes,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Usage {
    pub block_size: u64,
    pub blocks: u64,
    pub free: u64,
    pub available: u64,
    pub files: u64,
    pub files_free: u64,
    pub name_length: u64,
}
