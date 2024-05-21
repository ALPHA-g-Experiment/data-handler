use crate::AppState;
use anyhow::{ensure, Context, Result};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::Arc;
use tokio::fs;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

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

    Ok(files)
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct CoreCmd {
    pub run_number: u32,
    pub bin: CoreBin,
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

impl CoreCmd {
    pub fn output(self) -> String {
        match self.bin {
            CoreBin::ChronoboxTimestamps => {
                format!("R{}_chronobox_timestamps.csv", self.run_number)
            }
            CoreBin::InitialOdb => format!("R{}_initial_odb.json", self.run_number),
            CoreBin::FinalOdb => format!("R{}_final_odb.json", self.run_number),
            CoreBin::Sequencer => format!("R{}_sequencer.csv", self.run_number),
            CoreBin::TrgScalers => format!("R{}_trg_scalers.csv", self.run_number),
            CoreBin::Vertices => format!("R{}_vertices.csv", self.run_number),
        }
    }

    async fn into_command(self, data_dir: impl AsRef<Path>) -> Result<Command> {
        let mut midas_files = midas_files(data_dir, self.run_number)
            .await
            .context("failed to get MIDAS files")?;
        ensure!(!midas_files.is_empty(), "missing MIDAS files");
        midas_files.sort_unstable();

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
    async fn new<P>(
        rx: mpsc::UnboundedReceiver<CmdActorMessage>,
        cmd: CoreCmd,
        data_dir: P,
        output_dir: P,
    ) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let child = cmd
            .into_command(data_dir)
            .await
            .context("failed to create command")?
            .current_dir(output_dir)
            .spawn()
            .context("failed to spawn command")?;
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
pub struct CmdActorHandle {
    tx: mpsc::UnboundedSender<CmdActorMessage>,
}

impl CmdActorHandle {
    async fn new<P>(cmd: CoreCmd, data_dir: P, output_dir: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let (tx, rx) = mpsc::unbounded_channel();
        let actor = CmdActor::new(rx, cmd, data_dir, output_dir)
            .await
            .context("failed to create command actor")?;
        tokio::spawn(run_cmd_actor(actor));

        Ok(Self { tx })
    }

    pub async fn wait(&self) -> Result<ExitStatus> {
        let (tx, rx) = oneshot::channel();
        let msg = CmdActorMessage::Wait { tx };
        let _ = self.tx.send(msg);
        rx.await.context("failed to receive actor response")?
    }
}

pub async fn spawn_core_command<P>(
    cmd: CoreCmd,
    data_dir: P,
    output_dir: P,
    app_state: Arc<AppState>,
) -> Result<Option<CmdActorHandle>>
where
    P: AsRef<Path>,
{
    let mut processes = app_state.processes.lock().await;
    if output_dir.as_ref().join(cmd.output()).is_file() {
        return Ok(None);
    }
    match processes.get(&cmd) {
        Some(handle) => Ok(Some(handle.clone())),
        None => {
            let handle = CmdActorHandle::new(cmd, data_dir, output_dir)
                .await
                .context("failed to create command handle")?;
            processes.insert(cmd, handle.clone());
            Ok(Some(handle))
        }
    }
}
