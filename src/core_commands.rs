use tokio::process::Command;

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
    pub(crate) fn command(self) -> Command {
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
