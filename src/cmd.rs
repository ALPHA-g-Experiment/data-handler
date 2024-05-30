use anyhow::{ensure, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::Arc;
use tokio::fs;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CoreCmd {
    pub bin: CoreBin,
    pub run_number: u32,
    pub data_dir: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum CoreBin {
    ChronoboxTimestamps,
    InitialOdb,
    FinalOdb,
    Sequencer,
    TrgScalers,
    Vertices,
}

async fn midas_files(dir: impl AsRef<Path>, run_number: u32) -> Result<Vec<PathBuf>> {
    let prefix = format!("run{run_number:05}sub");

    let mut files = Vec::new();
    let mut entries = fs::read_dir(&dir)
        .await
        .with_context(|| format!("failed to read `{}`", dir.as_ref().display()))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .with_context(|| format!("failed to iterate over `{}`", dir.as_ref().display()))?
    {
        if let Some(filename) = entry.file_name().to_str() {
            if filename.starts_with(&prefix) {
                files.push(entry.path());
            }
        }
    }
    ensure!(
        !files.is_empty(),
        "no MIDAS files found for run number `{}` in `{}`",
        run_number,
        dir.as_ref().display()
    );

    Ok(files)
}

impl CoreCmd {
    fn output(&self) -> PathBuf {
        let filename = match self.bin {
            CoreBin::ChronoboxTimestamps => {
                format!("R{}_chronobox_timestamps.csv", self.run_number)
            }
            CoreBin::InitialOdb => format!("R{}_initial_odb.json", self.run_number),
            CoreBin::FinalOdb => format!("R{}_final_odb.json", self.run_number),
            CoreBin::Sequencer => format!("R{}_sequencer.csv", self.run_number),
            CoreBin::TrgScalers => format!("R{}_trg_scalers.csv", self.run_number),
            CoreBin::Vertices => format!("R{}_vertices.csv", self.run_number),
        };

        self.output_dir.join(filename)
    }

    async fn to_command(&self) -> Result<Command> {
        let mut cmd = match self.bin {
            CoreBin::ChronoboxTimestamps => Command::new("alpha-g-chronobox-timestamps"),
            CoreBin::InitialOdb => Command::new("alpha-g-odb"),
            CoreBin::FinalOdb => {
                let mut cmd = Command::new("alpha-g-odb");
                cmd.arg("--final");
                cmd
            }
            CoreBin::Sequencer => Command::new("alpha-g-sequencer"),
            CoreBin::TrgScalers => Command::new("alpha-g-trg-scalers"),
            CoreBin::Vertices => Command::new("alpha-g-vertices"),
        };

        let mut midas_files = midas_files(&self.data_dir, self.run_number)
            .await
            .context("failed to get MIDAS files")?;
        midas_files.sort_unstable();
        match self.bin {
            CoreBin::InitialOdb => cmd.arg(midas_files.first().unwrap()),
            CoreBin::FinalOdb => cmd.arg(midas_files.last().unwrap()),
            _ => cmd.args(midas_files),
        }
        .arg("--output")
        .arg(self.output());

        Ok(cmd)
    }
}

struct CmdActor {
    rx: mpsc::UnboundedReceiver<CmdActorMessage>,
    child: Child,
}

enum CmdActorMessage {
    Wait {
        tx: oneshot::Sender<Result<ExitStatus>>,
    },
}

impl CmdActor {
    async fn new(rx: mpsc::UnboundedReceiver<CmdActorMessage>, cmd: &CoreCmd) -> Result<Self> {
        let mut command = cmd.to_command().await.context("failed to create command")?;

        fs::create_dir_all(&cmd.output_dir)
            .await
            .with_context(|| format!("failed to create `{}`", cmd.output_dir.display()))?;
        let child = command.spawn().context("failed to spawn command")?;
        Ok(Self { rx, child })
    }

    async fn handle_message(&mut self, msg: CmdActorMessage) {
        match msg {
            CmdActorMessage::Wait { tx } => {
                let status = self
                    .child
                    .wait()
                    .await
                    .context("failed to wait child process");
                let _ = tx.send(status);
            }
        }
    }
}

async fn run_cmd_actor(mut actor: CmdActor) {
    while let Some(msg) = actor.rx.recv().await {
        actor.handle_message(msg).await;
    }
}

#[derive(Clone)]
struct CmdActorHandle {
    tx: mpsc::UnboundedSender<CmdActorMessage>,
}

impl CmdActorHandle {
    async fn new(cmd: &CoreCmd) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let actor = CmdActor::new(rx, cmd)
            .await
            .context("failed to create command actor")?;
        tokio::spawn(run_cmd_actor(actor));

        Ok(Self { tx })
    }

    async fn wait(&self) -> Result<ExitStatus> {
        let (tx, rx) = oneshot::channel();
        let msg = CmdActorMessage::Wait { tx };
        let _ = self.tx.send(msg);
        rx.await.context("failed to receive actor response")?
    }
}

#[derive(Default)]
pub struct AppState {
    processes: tokio::sync::Mutex<HashMap<CoreCmd, CmdActorHandle>>,
}

pub async fn spawn_core_command(cmd: CoreCmd, app_state: Arc<AppState>) -> Result<()> {
    let mut processes = app_state.processes.lock().await;

    if cmd.output().is_file() {
        processes.remove(&cmd);
    } else if !processes.contains_key(&cmd) {
        let handle = CmdActorHandle::new(&cmd)
            .await
            .context("failed to create command handle")?;
        processes.insert(cmd, handle);
    }

    Ok(())
}

pub async fn wait_core_command(cmd: &CoreCmd, app_state: Arc<AppState>) -> Result<PathBuf> {
    let mut processes = app_state.processes.lock().await;

    let Some(handle) = processes.get(&cmd).cloned() else {
        ensure!(
            cmd.output().is_file(),
            "`{}` does not exist and no child process is producing it",
            cmd.output().display()
        );
        return Ok(cmd.output());
    };
    std::mem::drop(processes);
    // Do not return immediately. Even if the command failed, we still want to
    // remove it from the "currently running" list.
    let status = handle.wait().await.context("failed to wait core command");
    let mut processes = app_state.processes.lock().await;
    processes.remove(&cmd);
    let status = status?;
    ensure!(status.success(), "core command failed with `{status}`",);

    ensure!(
        cmd.output().is_file(),
        "`{}` does not exist after successful core command",
        cmd.output().display()
    );
    Ok(cmd.output())
}
