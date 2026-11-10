use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::settings::{SettingsStore, VmSettings};

const ENVIRONMENT_FLAKE: &str = include_str!("flake.nix");
const TSOUP_FLAKE: &str = match option_env!("TSOUP_FLAKE") {
    Some(flake) => flake,
    None => concat!(env!("CARGO_MANIFEST_DIR"), "/../.."),
};
const EMPTY_MODULE: &str = "{ }\n";
const ENVIRONMENT_DIR: &str = "environment";
const HOST_FILE: &str = "host.json";
pub(super) const RUNNER_LINK: &str = "runner";
const ERROR_LINES: usize = 60;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("preparing the VM environment: {0}")]
    Io(#[from] io::Error),
    #[error("encoding the host settings: {0}")]
    Json(#[from] serde_json::Error),
    #[error("nix build failed:\n{0}")]
    Failed(String),
    #[error("listing the VM modules failed:\n{0}")]
    Describe(String),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Host {
    uid: u32,
    #[serde(default)]
    modules: Vec<String>,
    runtime: PathBuf,
    #[serde(flatten, default)]
    vm: VmSettings,
}

pub struct Builder {
    settings: Arc<SettingsStore>,
    uid: u32,
}

impl Builder {
    pub fn new(uid: u32, settings: Arc<SettingsStore>) -> Self {
        Self { settings, uid }
    }

    pub async fn build(
        &self,
        dir: &Path,
        runtime: &Path,
        module: Option<String>,
        modules: Option<Vec<String>>,
    ) -> Result<PathBuf, BuildError> {
        let environment = dir.join(ENVIRONMENT_DIR);
        tokio::fs::create_dir_all(&environment).await?;
        tokio::fs::write(environment.join("flake.nix"), ENVIRONMENT_FLAKE).await?;

        let host = environment.join(HOST_FILE);
        let modules = match modules {
            Some(modules) => modules,
            None => last_modules(&host).await,
        };
        tokio::fs::write(
            &host,
            serde_json::to_vec(&Host {
                uid: self.uid,
                modules,
                runtime: runtime.to_owned(),
                vm: self.settings.get().await.vm,
            })?,
        )
        .await?;

        let module_path = environment.join("module.nix");

        match module {
            Some(module) => tokio::fs::write(&module_path, module).await?,
            None if !tokio::fs::try_exists(&module_path).await? => {
                tokio::fs::write(&module_path, EMPTY_MODULE).await?;
            },
            None => {},
        }

        let runner = dir.join(RUNNER_LINK);
        run(
            nix()
                .args([
                    "build",
                    "--no-write-lock-file",
                    "--override-input",
                    "tsoup",
                    TSOUP_FLAKE,
                    "--out-link",
                ])
                .arg(&runner)
                .arg(format!("path:{}#default", environment.display())),
            BuildError::Failed,
        )
        .await?;

        Ok(runner)
    }
}

pub async fn describe() -> Result<String, BuildError> {
    run(
        nix()
            .args(["eval", "--raw", "--impure", "--apply"])
            .arg("describe: describe builtins.currentSystem")
            .arg(format!("{TSOUP_FLAKE}#lib.describe")),
        BuildError::Describe,
    )
    .await
}

async fn last_modules(host: &Path) -> Vec<String> {
    tokio::fs::read(host)
        .await
        .ok()
        .and_then(|host| serde_json::from_slice::<Host>(&host).ok())
        .map(|host| host.modules)
        .unwrap_or_default()
}

fn nix() -> Command {
    let mut nix = Command::new("nix");
    nix.args(["--extra-experimental-features", "nix-command flakes"])
        .stdin(Stdio::null())
        .kill_on_drop(true);
    nix
}

async fn run(command: &mut Command, error: fn(String) -> BuildError) -> Result<String, BuildError> {
    let output = command.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<_> = stderr.lines().collect();
        let tail = &lines[lines.len().saturating_sub(ERROR_LINES)..];
        return Err(error(tail.join("\n")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
