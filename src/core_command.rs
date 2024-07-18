use crate::PROJECT_HOME;
use anyhow::{ensure, Context, Result};
use std::collections::{hash_map::Entry, HashMap};
use std::path::PathBuf;
use std::process::ExitStatus;
use std::sync::{Arc, OnceLock};
use tokio::fs;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

// Install the latest (compatible) version of the core binaries to
// `PROJECT_HOME/rust/bin`.
//
// It is better to handle our own installation of the binaries instead of
// asking the user to do it and have them in $PATH. This way there is no risk of
// the user having a version not compatible with our UI. Furthermore, this
// allows users to have (and play around) with the latest (or different) CLI
// versions of the analysis without interfering with the data handler.
pub(super) fn install_core_binaries() -> Result<()> {
    // Use `cargo install` because it is currently the only way that the
    // `alpha-g-analysis` package instructs users to install the binaries.
    let cargo_home = PROJECT_HOME.get().unwrap().join("rust");

    let status = std::process::Command::new("cargo")
        .arg("install")
        .arg("--quiet")
        .arg("--locked")
        .arg("--root")
        .arg(cargo_home)
        // Only install the newest version that is semver compatible with
        // whatever was used for development.
        .arg("alpha-g-analysis@^0.5.4")
        .status()
        .context("failed to execute `cargo install`")?;
    ensure!(status.success(), "`cargo install` failed with `{status}`");

    Ok(())
}

// This is set (only once) at the beginning of the program based on the CLI
// arguments.
// Throughout the module it should be assumed to be set.
pub(super) static MIDAS_DATA_PATH: OnceLock<PathBuf> = OnceLock::new();

// Get all the MIDAS files for a given run number.
// If the returned value is OK, the vector is guaranteed to be non-empty.
async fn midas_files(run_number: u32) -> Result<Vec<PathBuf>> {
    let dir = MIDAS_DATA_PATH.get().unwrap();
    let prefix = format!("run{run_number:05}sub");

    let mut files = Vec::new();
    let mut entries = fs::read_dir(dir)
        .await
        .with_context(|| format!("failed to read `{}`", dir.display()))?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .with_context(|| format!("failed to iterate over `{}`", dir.display()))?
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
        dir.display()
    );

    Ok(files)
}
// Core commands are basically those installed by the `alpha-g-analysis`
// package.
//
// Some properties of these core commands:
// 1. Take MIDAS file(s) as input and produce a single output file.
// 2. They all have an `--output` flag to specify the output file.
// 3. The output file is versioned (first few comment lines).
// 4. We want to cache its output.
// 5. We want to keep track of whether it is currently running.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct CoreCmd {
    pub bin: CoreBin,
    pub run_number: u32,
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
    fn output_dir(self) -> PathBuf {
        directories::ProjectDirs::from("com", "ALPHA", "ALPHA-g-Data-Handler")
            // If this ever panics, then I should probably define a default
            // cache directory (configurable via CLI). But it doesn't make sense
            // to ever change this to be a fallible operation.
            .unwrap()
            .cache_dir()
            .join(self.run_number.to_string())
    }
    // We want to force the output filename to not rely on the default behavior
    // of the Command.
    fn filename(self) -> String {
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
    // Full path to the output file.
    fn output(self) -> PathBuf {
        self.output_dir().join(self.filename())
    }

    async fn to_command(self) -> Result<Command> {
        let mut cmd = Command::new(PROJECT_HOME.get().unwrap().join("rust").join("bin").join(
            match self.bin {
                CoreBin::ChronoboxTimestamps => "alpha-g-chronobox-timestamps",
                CoreBin::InitialOdb => "alpha-g-odb",
                CoreBin::FinalOdb => "alpha-g-odb",
                CoreBin::Sequencer => "alpha-g-sequencer",
                CoreBin::TrgScalers => "alpha-g-trg-scalers",
                CoreBin::Vertices => "alpha-g-vertices",
            },
        ));
        if let CoreBin::FinalOdb = self.bin {
            cmd.arg("--final");
        }

        let mut midas_files = midas_files(self.run_number)
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
// Actors with Tokio: https://ryhl.io/blog/actors-with-tokio/
//
// The motivation for the actor pattern is that these commands are long-running
// and we want to be able to spawn them and wait for them asynchronously.
// The actor_handle (which can be cloned out of the AppState) allows us to wait
// for a command without keeping the AppState mutex locked.
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
    async fn new(rx: mpsc::UnboundedReceiver<CmdActorMessage>, cmd: CoreCmd) -> Result<Self> {
        let mut command = cmd.to_command().await.context("failed to create Command")?;
        // Create the output directory after the Command to avoid making
        // unnecessary directories when a command is not even going to run.
        fs::create_dir_all(cmd.output_dir())
            .await
            .with_context(|| format!("failed to create `{}`", cmd.output_dir().display()))?;
        let child = command.spawn().context("failed to spawn Command")?;
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
    async fn new(cmd: CoreCmd) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let actor = CmdActor::new(rx, cmd)
            .await
            .context("failed to create command actor")?;
        tokio::spawn(run_cmd_actor(actor));

        Ok(Self { tx })
    }
    // This message will block the actor from processing further messages until
    // it is done with this one. This is fine if all the messages to this actor
    // are `Wait` because once this finishes, all other messages will be
    // processed very quickly. This should be avoided if there are other type of
    // messages that need to be processed concurrently by this actor.
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
    } else if let Entry::Vacant(entry) = processes.entry(cmd) {
        let handle = CmdActorHandle::new(cmd)
            .await
            .context("failed to create command handle")?;
        entry.insert(handle);
    }

    Ok(())
}

pub async fn wait_core_command(cmd: CoreCmd, app_state: Arc<AppState>) -> Result<PathBuf> {
    let processes = app_state.processes.lock().await;
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
