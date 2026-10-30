use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

const ENVIRONMENT_FLAKE: &str = include_str!("flake.nix");
const TSOUP_FLAKE: &str = match option_env!("TSOUP_FLAKE") {
    Some(flake) => flake,
    None => concat!(env!("CARGO_MANIFEST_DIR"), "/../.."),
};
const TSOUP_EXTRA_MODULES: &str = match option_env!("TSOUP_EXTRA_MODULES") {
    Some(modules) => modules,
    None => "[]",
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
    #[error("reading the extra modules: {0}")]
    ExtraModules(serde_json::Error),
    #[error("hashing an extra module failed:\n{0}")]
    Hash(String),
    #[error("nix build failed:\n{0}")]
    Failed(String),
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VmKind {
    Headless,
    #[default]
    Graphical,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Host {
    uid: u32,
    kind: VmKind,
    runtime: PathBuf,
    #[serde(default)]
    extra_modules: Vec<ExtraModule>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtraModule {
    path: PathBuf,
    nar_hash: String,
}

pub struct Builder {
    uid: u32,
}

impl Builder {
    pub fn new(uid: u32) -> Self {
        Self { uid }
    }

    pub async fn build(
        &self,
        dir: &Path,
        runtime: &Path,
        module: Option<String>,
        kind: Option<VmKind>,
    ) -> Result<PathBuf, BuildError> {
        let environment = dir.join(ENVIRONMENT_DIR);
        tokio::fs::create_dir_all(&environment).await?;
        tokio::fs::write(environment.join("flake.nix"), ENVIRONMENT_FLAKE).await?;

        let host = environment.join(HOST_FILE);
        let kind = match kind {
            Some(kind) => kind,
            None => last_kind(&host).await,
        };
        tokio::fs::write(
            &host,
            serde_json::to_vec(&Host {
                uid: self.uid,
                kind,
                runtime: runtime.to_owned(),
                extra_modules: extra_modules().await?,
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

async fn last_kind(host: &Path) -> VmKind {
    tokio::fs::read(host)
        .await
        .ok()
        .and_then(|host| serde_json::from_slice::<Host>(&host).ok())
        .map(|host| host.kind)
        .unwrap_or_default()
}

async fn extra_modules() -> Result<Vec<ExtraModule>, BuildError> {
    let paths: Vec<PathBuf> =
        serde_json::from_str(TSOUP_EXTRA_MODULES).map_err(BuildError::ExtraModules)?;
    let mut modules = Vec::with_capacity(paths.len());

    for path in paths {
        let nar_hash = run(nix().args(["hash", "path"]).arg(&path), BuildError::Hash).await?;
        modules.push(ExtraModule {
            path,
            nar_hash: nar_hash.trim().to_owned(),
        });
    }

    Ok(modules)
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
