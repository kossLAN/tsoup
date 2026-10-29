use std::env;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;

const SOURCES: [&str; 7] = [
    "src",
    "static",
    "package.json",
    "package-lock.json",
    "svelte.config.js",
    "vite.config.ts",
    "tsconfig.json",
];

fn main() -> io::Result<()> {
    let web = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web");
    let out = env::var_os("OUT_DIR")
        .map(|dir| PathBuf::from(dir).join("web"))
        .ok_or_else(|| io::Error::other("OUT_DIR is not set"))?;

    let mut stdout = io::stdout().lock();

    for source in SOURCES {
        writeln!(
            stdout,
            "cargo::rerun-if-changed={}",
            web.join(source).display()
        )?;
    }

    if !web.join("node_modules").exists() {
        run(Command::new("npm").arg("ci").current_dir(&web))?;
    }

    run(Command::new("npm")
        .args(["run", "build"])
        .current_dir(&web)
        .env("TSOUP_WEB_OUT", &out))
}

fn run(command: &mut Command) -> io::Result<()> {
    let status = command.status()?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "building the web UI failed with {status}"
        )))
    }
}
