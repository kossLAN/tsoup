use std::io;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tsoup_proto::{Exit, MAX_OUTPUT};

pub async fn exec(command: &str, dir: &Path, timeout: Duration) -> io::Result<Option<Exit>> {
    let output = shell(command, dir).kill_on_drop(true).output();

    let Ok(output) = tokio::time::timeout(timeout, output).await else {
        return Ok(None);
    };
    let output = output?;

    Ok(Some(Exit {
        code: output.status.code(),
        stdout: tail(&output.stdout),
        stderr: tail(&output.stderr),
    }))
}

pub fn spawn(command: &str, dir: &Path) -> io::Result<()> {
    shell(command, dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    Ok(())
}

fn shell(command: &str, dir: &Path) -> Command {
    let mut shell = Command::new("sh");
    shell
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .stdin(Stdio::null());
    shell
}

fn tail(output: &[u8]) -> String {
    let output = String::from_utf8_lossy(output);
    let start = output.len().saturating_sub(MAX_OUTPUT);
    let start = (start..output.len())
        .find(|&index| output.is_char_boundary(index))
        .unwrap_or(output.len());

    output[start..].to_owned()
}
