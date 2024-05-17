use std::process::ExitStatus;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub(crate) enum CoreCommand {
    ChronoboxTimestamps,
    InitialOdb,
    FinalOdb,
    Sequencer,
    TrgScalers,
    Vertices,
}

impl CoreCommand {
    pub(crate) fn into_command(self) -> Command {
        match self {
            CoreCommand::ChronoboxTimestamps => Command::new("alpha-g-chronobox-timestamps"),
            CoreCommand::InitialOdb => Command::new("alpha-g-odb"),
            CoreCommand::FinalOdb => {
                let mut cmd = Command::new("alpha-g-odb");
                cmd.arg("--final");
                cmd
            }
            CoreCommand::Sequencer => Command::new("alpha-g-sequencer"),
            CoreCommand::TrgScalers => Command::new("alpha-g-trg-scalers"),
            CoreCommand::Vertices => Command::new("alpha-g-vertices"),
        }
    }

    pub(crate) fn output(self, run_number: u32) -> String {
        match self {
            CoreCommand::ChronoboxTimestamps => format!("R{run_number}_chronobox_timestamps.csv"),
            CoreCommand::InitialOdb => format!("R{run_number}_initial_odb.json"),
            CoreCommand::FinalOdb => format!("R{run_number}_final_odb.json"),
            CoreCommand::Sequencer => format!("R{run_number}_sequencer.csv"),
            CoreCommand::TrgScalers => format!("R{run_number}_trg_scalers.csv"),
            CoreCommand::Vertices => format!("R{run_number}_vertices.csv"),
        }
    }
}

struct CmdActor {
    rx: mpsc::UnboundedReceiver<CmdActorMessage>,
    child: Child,
}

enum CmdActorMessage {
    Wait {
        tx: oneshot::Sender<Result<ExitStatus, std::io::Error>>,
    },
}

impl CmdActor {
    fn new(
        rx: mpsc::UnboundedReceiver<CmdActorMessage>,
        mut cmd: Command,
    ) -> Result<Self, std::io::Error> {
        let child = cmd.spawn()?;
        Ok(Self { rx, child })
    }

    async fn handle_message(&mut self, msg: CmdActorMessage) {
        match msg {
            CmdActorMessage::Wait { tx } => {
                let status = self.child.wait().await;
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
pub(crate) struct CmdActorHandle {
    tx: mpsc::UnboundedSender<CmdActorMessage>,
}

impl CmdActorHandle {
    pub(crate) fn new(cmd: Command) -> Result<Self, std::io::Error> {
        let (tx, rx) = mpsc::unbounded_channel();
        let actor = CmdActor::new(rx, cmd)?;
        tokio::spawn(run_cmd_actor(actor));

        Ok(Self { tx })
    }

    pub(crate) async fn wait(&self) -> Result<ExitStatus, std::io::Error> {
        let (tx, rx) = oneshot::channel();
        let msg = CmdActorMessage::Wait { tx };
        let _ = self.tx.send(msg);
        rx.await.unwrap()
    }
}
