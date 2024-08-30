use crate::PROJECT_HOME;
use anyhow::{ensure, Context, Result};
use rand::distributions::{Alphanumeric, DistString};
use semver::{Version, VersionReq};
use serde::Deserialize;
use std::{fmt, path::PathBuf};
use tokio::{fs, process::Command};

fn python3() -> PathBuf {
    PROJECT_HOME
        .get()
        .unwrap()
        .join("analysis-scripts")
        .join(".venv")
        .join("bin")
        .join("python3")
}

fn analysis_scripts_dir() -> PathBuf {
    PROJECT_HOME
        .get()
        .unwrap()
        .join("analysis-scripts")
        .join("bin")
}
// Instead of asking users to manually install dependencies and all scripts in a
// specific folder, we can handle this ourselves. This way we also
// avoid potential conflicts with non-supported versions of the scripts, etc.
// Additionally, doing this instead of embedding the scripts in the binary
// allows us to update the scripts without having to recompile the program.
pub(super) fn setup_analysis_scripts() -> Result<()> {
    // Only install the latest version that is semver compatible with whatever
    // was tested during development.
    let req = VersionReq::parse("^0.1.1").unwrap();
    // The analysis-scripts repo doesn't provide (so far) pre-built interpreters,
    // etc. Hence just follow the instructions in the README.
    //
    // I don't want to manage a git repo through std::Command invocations. It is
    // easier to just delete and re-clone every time. This is a temporary
    // solution until there is a proper way of installing a pre-built
    // interpreter and the scripts.
    let _ = std::fs::remove_dir_all(PROJECT_HOME.get().unwrap().join("analysis-scripts"));

    let output = std::process::Command::new("git")
        .arg("clone")
        .arg("https://github.com/ALPHA-g-Experiment/analysis-scripts.git")
        .current_dir(PROJECT_HOME.get().unwrap())
        .output()
        .context("failed to execute `git clone`")?;
    ensure!(output.status.success(), "`git clone` failed");

    let output = std::process::Command::new("git")
        .args(["tag", "--list", "--sort=-v:refname", "v*"])
        .current_dir(PROJECT_HOME.get().unwrap().join("analysis-scripts"))
        .output()
        .context("failed to execute `git tag --list --sort=-v:refname v*`")?;
    ensure!(output.status.success(), "`git tag --list` failed");
    let tag = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|tag| {
            Version::parse(&tag[1..])
                .map(|version| req.matches(&version))
                .unwrap_or(false)
        })
        .unwrap()
        .to_string();

    let output = std::process::Command::new("git")
        .args(["checkout", &tag])
        .current_dir(PROJECT_HOME.get().unwrap().join("analysis-scripts"))
        .output()
        .context("failed to execute `git checkout`")?;
    ensure!(output.status.success(), "`git checkout` failed");

    let output = std::process::Command::new("python3")
        .args(["-m", "venv", ".venv"])
        .current_dir(PROJECT_HOME.get().unwrap().join("analysis-scripts"))
        .output()
        .context("failed to execute `python3 -m venv .venv`")?;
    ensure!(output.status.success(), "`python3 -m venv .venv` failed");

    let output = std::process::Command::new(python3())
        .args(["-m", "pip", "install", "--upgrade", "pip"])
        .output()
        .context("failed to execute `python3 -m pip install --upgrade pip`")?;
    ensure!(output.status.success(), "`pip upgrade` failed");

    let output = std::process::Command::new(python3())
        .args(["-m", "pip", "install", "-r", "requirements.txt"])
        .current_dir(PROJECT_HOME.get().unwrap().join("analysis-scripts"))
        .output()
        .context("failed to execute `python3 -m pip install -r requirements.txt`")?;
    ensure!(output.status.success(), "`install requirements.txt` failed");

    Ok(())
}

pub trait SecondaryScript: fmt::Display {
    // Secondary scripts can be wildly different. We can even start adding
    // different scripts in different languages, etc. All we care about is that
    // they can be spawned and waited for.
    // Returns the path to the output file.
    async fn spawn_and_wait(&self, output: &str) -> Result<PathBuf>;
}

// Create a new random subdirectory in the system's temporary directory.
// Every secondary script will write its output to a new directory to avoid
// overwriting files. This allows to keep the files themselves with
// human-readable names instead of random strings.
async fn temp_dir() -> Result<PathBuf> {
    let dir = loop {
        let mut dir = String::from("alpha-g-data-handler/");
        Alphanumeric.append_string(&mut rand::thread_rng(), &mut dir, 8);
        let path = std::env::temp_dir().join(dir);
        if !path.exists() {
            break path;
        }
    };
    fs::create_dir_all(&dir)
        .await
        .with_context(|| format!("failed to create `{}`", dir.display()))?;

    Ok(dir)
}

pub struct Sequencer {
    pub sequencer_csv: PathBuf,
    pub initial_odb_json: PathBuf,
    pub chronobox_csv: PathBuf,
}

impl fmt::Display for Sequencer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sequencer.py")
    }
}

impl SecondaryScript for Sequencer {
    async fn spawn_and_wait(&self, output: &str) -> Result<PathBuf> {
        let output = temp_dir()
            .await
            .context("failed to create temporary directory")?
            .join(output);

        let status = Command::new(python3())
            .arg(analysis_scripts_dir().join(self.to_string()))
            .arg(&self.sequencer_csv)
            .arg("--odb-json")
            .arg(&self.initial_odb_json)
            .arg("--chronobox-csv")
            .arg(&self.chronobox_csv)
            .arg("--output")
            .arg(&output)
            .status()
            .await
            .with_context(|| format!("failed to run `{self}`"))?;
        ensure!(status.success(), "`{self}` failed with `{status}`");

        Ok(output)
    }
}

pub struct SpillLog {
    pub sequencer_events_csv: PathBuf,
    pub initial_odb_json: PathBuf,
    pub chronobox_csv: PathBuf,
    pub trg_scalers_csv: PathBuf,
}

impl fmt::Display for SpillLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "spill_log.py")
    }
}

impl SecondaryScript for SpillLog {
    async fn spawn_and_wait(&self, output: &str) -> Result<PathBuf> {
        let output = temp_dir()
            .await
            .context("failed to create temporary directory")?
            .join(output);

        let status = Command::new(python3())
            .arg(analysis_scripts_dir().join(self.to_string()))
            .arg(&self.sequencer_events_csv)
            .arg(&self.initial_odb_json)
            .arg(&self.chronobox_csv)
            .arg(&self.trg_scalers_csv)
            .arg("--output")
            .arg(&output)
            .status()
            .await
            .with_context(|| format!("failed to run `{self}`"))?;
        ensure!(status.success(), "`{self}` failed with `{status}`");

        Ok(output)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChronoboxTimestampsArgs {
    board_name: String,
    channel_number: u8,
    t_bins: Option<u32>,
    t_max: Option<f64>,
    t_min: Option<f64>,
}

pub struct ChronoboxTimestamps {
    pub csv: PathBuf,
    pub args: ChronoboxTimestampsArgs,
}

impl fmt::Display for ChronoboxTimestamps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "chronobox_timestamps.py")
    }
}

impl SecondaryScript for ChronoboxTimestamps {
    async fn spawn_and_wait(&self, output: &str) -> Result<PathBuf> {
        let output = temp_dir()
            .await
            .context("failed to create temporary directory")?
            .join(output);

        let mut cmd = Command::new(python3());
        cmd.arg(analysis_scripts_dir().join(self.to_string()))
            .arg(&self.csv)
            .arg(&self.args.board_name)
            .arg(self.args.channel_number.to_string())
            .arg("--output")
            .arg(&output);
        if let Some(t_bins) = self.args.t_bins {
            cmd.args(["--t-bins", &t_bins.to_string()]);
        }
        if let Some(t_max) = self.args.t_max {
            cmd.args(["--t-max", &t_max.to_string()]);
        }
        if let Some(t_min) = self.args.t_min {
            cmd.args(["--t-min", &t_min.to_string()]);
        }

        let status = cmd
            .status()
            .await
            .with_context(|| format!("failed to run `{self}`"))?;
        ensure!(status.success(), "`{self}` failed with `{status}`");

        Ok(output)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TrgScalersArgs {
    t_bins: Option<u32>,
    t_max: Option<f64>,
    t_min: Option<f64>,
    include_drift_veto: bool,
    include_pulser: bool,
    include_scaledown: bool,
    remove_input: bool,
    remove_output: bool,
}

pub struct TrgScalers {
    pub csv: PathBuf,
    pub args: TrgScalersArgs,
}

impl fmt::Display for TrgScalers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "trg_scalers.py")
    }
}

impl SecondaryScript for TrgScalers {
    async fn spawn_and_wait(&self, output: &str) -> Result<PathBuf> {
        let output = temp_dir()
            .await
            .context("failed to create temporary directory")?
            .join(output);

        let mut cmd = Command::new(python3());
        cmd.arg(analysis_scripts_dir().join(self.to_string()))
            .arg(&self.csv)
            .arg("--output")
            .arg(&output);
        if let Some(t_bins) = self.args.t_bins {
            cmd.args(["--t-bins", &t_bins.to_string()]);
        }
        if let Some(t_max) = self.args.t_max {
            cmd.args(["--t-max", &t_max.to_string()]);
        }
        if let Some(t_min) = self.args.t_min {
            cmd.args(["--t-min", &t_min.to_string()]);
        }
        if self.args.include_drift_veto {
            cmd.arg("--include-drift-veto-counter");
        }
        if self.args.include_pulser {
            cmd.arg("--include-pulser-counter");
        }
        if self.args.include_scaledown {
            cmd.arg("--include-scaledown-counter");
        }
        if self.args.remove_input {
            cmd.arg("--remove-input-counter");
        }
        if self.args.remove_output {
            cmd.arg("--remove-output-counter");
        }

        let status = cmd
            .status()
            .await
            .with_context(|| format!("failed to run `{self}`"))?;
        ensure!(status.success(), "`{self}` failed with `{status}`");

        Ok(output)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct VerticesArgs {
    phi_bins: Option<u32>,
    phi_max: Option<f64>,
    phi_min: Option<f64>,
    r_bins: Option<u32>,
    r_max: Option<f64>,
    r_min: Option<f64>,
    t_bins: Option<u32>,
    t_max: Option<f64>,
    t_min: Option<f64>,
    z_bins: Option<u32>,
    z_max: Option<f64>,
    z_min: Option<f64>,
}

pub struct Vertices {
    pub csv: PathBuf,
    pub args: VerticesArgs,
}

impl fmt::Display for Vertices {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vertices.py")
    }
}

impl SecondaryScript for Vertices {
    async fn spawn_and_wait(&self, output: &str) -> Result<PathBuf> {
        let output = temp_dir()
            .await
            .context("failed to create temporary directory")?
            .join(output);

        let mut cmd = Command::new(python3());
        cmd.arg(analysis_scripts_dir().join(self.to_string()))
            .arg(&self.csv)
            .arg("--output")
            .arg(&output);
        if let Some(phi_bins) = self.args.phi_bins {
            cmd.args(["--phi-bins", &phi_bins.to_string()]);
        }
        if let Some(phi_max) = self.args.phi_max {
            cmd.args(["--phi-max", &phi_max.to_string()]);
        }
        if let Some(phi_min) = self.args.phi_min {
            cmd.args(["--phi-min", &phi_min.to_string()]);
        }
        if let Some(r_bins) = self.args.r_bins {
            cmd.args(["--r-bins", &r_bins.to_string()]);
        }
        if let Some(r_max) = self.args.r_max {
            cmd.args(["--r-max", &r_max.to_string()]);
        }
        if let Some(r_min) = self.args.r_min {
            cmd.args(["--r-min", &r_min.to_string()]);
        }
        if let Some(t_bins) = self.args.t_bins {
            cmd.args(["--t-bins", &t_bins.to_string()]);
        }
        if let Some(t_max) = self.args.t_max {
            cmd.args(["--t-max", &t_max.to_string()]);
        }
        if let Some(t_min) = self.args.t_min {
            cmd.args(["--t-min", &t_min.to_string()]);
        }
        if let Some(z_bins) = self.args.z_bins {
            cmd.args(["--z-bins", &z_bins.to_string()]);
        }
        if let Some(z_max) = self.args.z_max {
            cmd.args(["--z-max", &z_max.to_string()]);
        }
        if let Some(z_min) = self.args.z_min {
            cmd.args(["--z-min", &z_min.to_string()]);
        }

        let status = cmd
            .status()
            .await
            .with_context(|| format!("failed to run `{self}`"))?;
        ensure!(status.success(), "`{self}` failed with `{status}`");

        Ok(output)
    }
}
