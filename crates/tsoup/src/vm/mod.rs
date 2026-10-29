mod build;

use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use serde::Serialize;
use tokio::net::{UnixListener, UnixStream};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, broadcast, watch};
use tokio_util::codec::Framed;
use tsoup_proto::{CodecError, Frame, HostCodec, Request, Response};

use self::build::RUNNER_LINK;
pub use self::build::{BuildError, Builder};
use crate::session::{Event, WORKSPACE_DIR};

const AGENT_SOCKET: &str = "agent.sock";
const DISPLAY_SOCKET: &str = "vnc.sock";
const CONSOLE_LOG: &str = "console.log";
const BOOT_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum VmStatus {
    Stopped,
    Building,
    Booting,
    Running,
    Failed { message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error(transparent)]
    Build(#[from] BuildError),
    #[error("starting the VM: {0}")]
    Io(#[from] io::Error),
    #[error("the VM exited while booting ({0}), see {CONSOLE_LOG}")]
    Exited(ExitStatus),
    #[error("the VM did not boot within {} seconds, see {CONSOLE_LOG}", BOOT_TIMEOUT.as_secs())]
    BootTimeout,
    #[error("no VM is running, call create_vm or run_vm first")]
    NotRunning,
    #[error("the VM is already running")]
    AlreadyRunning,
    #[error("no VM has been built yet, call create_vm first")]
    NotBuilt,
    #[error("talking to the guest agent: {0}")]
    Guest(#[from] CodecError),
    #[error("the guest agent hung up")]
    Disconnected,
}

pub struct Vm {
    dir: PathBuf,
    runtime: PathBuf,
    builder: Arc<Builder>,
    running: Mutex<Option<Running>>,
    status: watch::Sender<VmStatus>,
    events: broadcast::Sender<Event>,
}

impl Vm {
    pub fn new(
        dir: PathBuf,
        runtime: PathBuf,
        builder: Arc<Builder>,
        events: broadcast::Sender<Event>,
    ) -> Self {
        Self {
            dir,
            runtime,
            builder,
            running: Mutex::new(None),
            status: watch::Sender::new(VmStatus::Stopped),
            events,
        }
    }

    pub fn status(&self) -> VmStatus {
        self.status.borrow().clone()
    }

    pub fn display_socket(&self) -> PathBuf {
        self.runtime.join(DISPLAY_SOCKET)
    }

    pub async fn create(&self, module: Option<String>) -> Result<(), VmError> {
        let mut running = self.running.lock().await;

        self.set_status(VmStatus::Building);

        let runner = match self.builder.build(&self.dir, module).await {
            Ok(runner) => runner,
            Err(error) => {
                self.set_status(if running.is_some() {
                    VmStatus::Running
                } else {
                    VmStatus::Failed {
                        message: error.to_string(),
                    }
                });
                return Err(error.into());
            },
        };

        stop(&mut running).await;
        self.boot(&mut running, &runner).await
    }

    pub async fn run(&self) -> Result<(), VmError> {
        let mut running = self.running.lock().await;

        if running.is_some() {
            return Err(VmError::AlreadyRunning);
        }

        let runner = self.dir.join(RUNNER_LINK);

        if !tokio::fs::try_exists(&runner).await? {
            return Err(VmError::NotBuilt);
        }

        self.boot(&mut running, &runner).await
    }

    async fn boot(&self, running: &mut Option<Running>, runner: &Path) -> Result<(), VmError> {
        self.set_status(VmStatus::Booting);

        match Running::launch(&self.dir, &self.runtime, runner).await {
            Ok(launched) => {
                *running = Some(launched);
                self.set_status(VmStatus::Running);
                Ok(())
            },
            Err(error) => {
                self.set_status(VmStatus::Failed {
                    message: error.to_string(),
                });
                Err(error)
            },
        }
    }

    pub async fn destroy(&self) {
        stop(&mut *self.running.lock().await).await;

        match tokio::fs::remove_dir_all(&self.runtime).await {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                tracing::warn!(%error, "removing a VM runtime directory");
            },
            _ => {},
        }

        self.set_status(VmStatus::Stopped);
    }

    pub async fn reconcile(&self) {
        let running = self.running.lock().await;

        if matches!(
            *self.status.borrow(),
            VmStatus::Building | VmStatus::Booting
        ) {
            self.set_status(if running.is_some() {
                VmStatus::Running
            } else {
                VmStatus::Stopped
            });
        }
    }

    pub async fn request(&self, request: Request) -> Result<Response, VmError> {
        let mut running = self.running.lock().await;
        let vm = running.as_mut().ok_or(VmError::NotRunning)?;

        match vm.guest.request(request).await {
            Ok(response) => Ok(response),
            Err(error) => {
                stop(&mut running).await;
                self.set_status(VmStatus::Failed {
                    message: error.to_string(),
                });
                Err(error)
            },
        }
    }

    fn set_status(&self, status: VmStatus) {
        self.status.send_replace(status.clone());
        let _ = self.events.send(Event::Vm { status });
    }
}

async fn stop(running: &mut Option<Running>) {
    if let Some(mut vm) = running.take()
        && let Err(error) = vm.process.kill().await
    {
        tracing::warn!(%error, "killing a VM");
    }
}

struct Running {
    process: Child,
    guest: Guest,
}

impl Running {
    async fn launch(dir: &Path, runtime: &Path, runner: &Path) -> Result<Self, VmError> {
        tokio::fs::create_dir_all(runtime).await?;

        for stale in [AGENT_SOCKET, DISPLAY_SOCKET, WORKSPACE_DIR] {
            match tokio::fs::remove_file(runtime.join(stale)).await {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
                _ => {},
            }
        }

        tokio::fs::symlink(dir.join(WORKSPACE_DIR), runtime.join(WORKSPACE_DIR)).await?;

        let listener = UnixListener::bind(runtime.join(AGENT_SOCKET))?;
        let console = tokio::fs::File::create(dir.join(CONSOLE_LOG))
            .await?
            .into_std()
            .await;
        let mut process = Command::new(runner.join("bin/microvm-run"))
            .current_dir(runtime)
            .stdin(Stdio::null())
            .stdout(console.try_clone()?)
            .stderr(console)
            .kill_on_drop(true)
            .spawn()?;

        let connect = async {
            let (stream, _) = listener.accept().await?;
            let mut guest = Guest::new(stream);
            guest.request(Request::Ping).await?;
            Ok::<_, VmError>(guest)
        };

        let guest = tokio::select! {
            guest = tokio::time::timeout(BOOT_TIMEOUT, connect) => {
                guest.map_err(|_| VmError::BootTimeout)??
            },
            status = process.wait() => return Err(VmError::Exited(status?)),
        };

        Ok(Self { process, guest })
    }
}

struct Guest {
    framed: Framed<UnixStream, HostCodec>,
    last_id: u64,
}

impl Guest {
    fn new(stream: UnixStream) -> Self {
        Self {
            framed: Framed::new(stream, HostCodec::default()),
            last_id: 0,
        }
    }

    async fn request(&mut self, body: Request) -> Result<Response, VmError> {
        self.last_id += 1;
        let id = self.last_id;
        self.framed.send(Frame { id, body }).await?;

        loop {
            let frame = self.framed.next().await.ok_or(VmError::Disconnected)??;

            if frame.id == id {
                return Ok(frame.body);
            }
        }
    }
}
