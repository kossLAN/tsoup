use std::io;
use std::path::Path;

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use similar::TextDiff;
use tsoup_proto::WORKSPACE;

use super::{Outcome, ToolError};
use crate::session::FileDiff;

pub async fn blocking<T, F>(work: F) -> Result<T, ToolError>
where
    F: FnOnce() -> Result<T, ToolError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(io::Error::other)?
}

pub fn open(workspace: &Path) -> io::Result<Dir> {
    Dir::open_ambient_dir(workspace, ambient_authority())
}

pub fn relative(path: &str) -> &Path {
    let path = Path::new(path);
    path.strip_prefix(WORKSPACE).unwrap_or(path)
}

pub fn changed(path: String, before: &str, after: &str) -> Outcome {
    let unified = TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(3)
        .header(&path, &path)
        .to_string();

    Outcome {
        output: format!("wrote {path}"),
        error: false,
        diff: Some(FileDiff {
            path,
            patch: unified,
        }),
    }
}
