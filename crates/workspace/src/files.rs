use std::io;
use std::path::Path;

const MAX_FILE: u64 = 8 * 1024 * 1024;

pub async fn read(path: &Path) -> io::Result<Option<String>> {
    let size = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    if size > MAX_FILE {
        return Err(io::Error::other(format!(
            "the file is {size} bytes, larger than the {MAX_FILE} bytes that can be read"
        )));
    }

    tokio::fs::read_to_string(path).await.map(Some)
}

pub async fn write(path: &Path, content: String) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(path, content).await
}
