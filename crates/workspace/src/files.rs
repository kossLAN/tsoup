use std::io;
use std::path::Path;

use cap_std::ambient_authority;
use cap_std::fs::Dir;

const MAX_FILE: u64 = 8 * 1024 * 1024;

pub async fn read(path: &Path) -> io::Result<Option<String>> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) => readable(metadata.len())?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }

    tokio::fs::read_to_string(path).await.map(Some)
}

pub async fn write(path: &Path, content: String) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(path, content).await
}

pub async fn read_within(dir: &Path, path: &Path) -> io::Result<Option<String>> {
    let (dir, path) = (dir.to_owned(), path.to_owned());

    blocking(move || {
        let dir = Dir::open_ambient_dir(dir, ambient_authority())?;

        match dir.metadata(&path) {
            Ok(metadata) => readable(metadata.len())?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }

        dir.read_to_string(path).map(Some)
    })
    .await
}

pub async fn write_within(dir: &Path, path: &Path, content: String) -> io::Result<()> {
    let (dir, path) = (dir.to_owned(), path.to_owned());

    blocking(move || {
        let dir = Dir::open_ambient_dir(dir, ambient_authority())?;

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            dir.create_dir_all(parent)?;
        }

        dir.write(path, content)
    })
    .await
}

fn readable(size: u64) -> io::Result<()> {
    if size > MAX_FILE {
        return Err(io::Error::other(format!(
            "the file is {size} bytes, larger than the {MAX_FILE} bytes that can be read"
        )));
    }

    Ok(())
}

async fn blocking<T, F>(work: F) -> io::Result<T>
where
    F: FnOnce() -> io::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(io::Error::other)?
}
