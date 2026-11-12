use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::{env, io};

use tokio::process::Command;

const SYSTEM: [&str; 11] = [
    "/nix",
    "/usr",
    "/bin",
    "/sbin",
    "/lib",
    "/lib64",
    "/etc",
    "/sys",
    "/run/current-system",
    "/run/opengl-driver",
    "/run/systemd/resolve",
];
const HOME_DIR: &str = "home";
const TEMP_DIR: &str = "tmp";

pub struct Sandbox {
    state: PathBuf,
    home: Option<PathBuf>,
}

impl Sandbox {
    pub fn new(state: PathBuf) -> Self {
        Self {
            state,
            home: env::var_os("HOME").map(PathBuf::from),
        }
    }

    pub async fn attached(&self, command: &str, dir: &Path) -> io::Result<Command> {
        let mut bwrap = self.enter(dir).await?;
        bwrap.arg("--die-with-parent");
        Ok(sh(bwrap, command))
    }

    pub async fn detached(&self, command: &str, dir: &Path) -> io::Result<Command> {
        Ok(sh(self.enter(dir).await?, command))
    }

    async fn enter(&self, dir: &Path) -> io::Result<Command> {
        let state = self.state.join(dir.file_name().unwrap_or_default());
        let temp = state.join(TEMP_DIR);
        tokio::fs::create_dir_all(&temp).await?;

        let mut bwrap = Command::new("bwrap");
        bwrap.args(["--new-session", "--unshare-pid", "--unshare-ipc"]);

        for path in SYSTEM {
            bwrap.args(["--ro-bind-try", path, path]);
        }

        bwrap
            .args(["--dev-bind", "/dev", "/dev"])
            .args(["--tmpfs", "/dev/shm"])
            .args(["--proc", "/proc"])
            .arg("--bind")
            .arg(temp)
            .arg("/tmp");

        if let Some(home) = &self.home {
            let kept = state.join(HOME_DIR);
            tokio::fs::create_dir_all(&kept).await?;
            bwrap.arg("--bind").arg(kept).arg(home);
        }

        bwrap
            .arg("--bind")
            .arg(dir)
            .arg(dir)
            .arg("--chdir")
            .arg(dir);
        Ok(bwrap)
    }
}

fn sh(mut bwrap: Command, command: &str) -> Command {
    bwrap.args(["--", "sh", "-c", command]).stdin(Stdio::null());
    bwrap
}
