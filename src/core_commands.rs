use tokio::process::Command;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
enum CoreCommand {
    ChronoboxTimestamps,
    InitialOdb,
    FinalOdb,
    Sequencer,
    TrgScalers,
    Vertices,
}

impl CoreCommand {
    fn command(self) -> Command {
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
}
