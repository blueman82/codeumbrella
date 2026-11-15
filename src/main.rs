//! The `codeumbrella` command.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use codeumbrella::claude::Claude;
use codeumbrella::codex::Codex;
use codeumbrella::engine::{Run, Worker};
use codeumbrella::graph::Graph;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fs, path::PathBuf};

/// The folder that holds all runs of a project.
const RUNS: &str = ".codeumbrella/runs";

/// A software factory: give it a task, get good code.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// The instruction sheet.
    #[arg(long, default_value = "graph.yaml")]
    sheet: PathBuf,
    /// The folder with the rules pages.
    #[arg(long, default_value = "roles")]
    roles: PathBuf,
    /// The maximum number of minutes for one step.
    #[arg(long, default_value_t = 30)]
    step_minutes: u64,
    /// The tool that does the worker steps.
    #[arg(long, value_enum, default_value = "claude")]
    tool: Tool,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum Tool {
    Claude,
    Codex,
}

#[derive(Subcommand)]
enum Cmd {
    /// Start a new run for a task.
    Run { task: String },
    /// Show where a run is. Without an id: the newest run.
    Status { run: Option<String> },
    /// Answer the open question of a run, then continue.
    Answer {
        run: String,
        answer: YesNo,
        #[arg(default_value = "")]
        text: String,
    },
    /// Try the stopped step of a run again.
    Resume { run: String },
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum YesNo {
    Yes,
    No,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let runs = PathBuf::from(RUNS);
    let mut run = match &cli.command {
        Cmd::Run { task } => {
            let graph = Graph::load(&cli.sheet, &cli.roles)?;
            let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            Run::start(runs.join(format!("run-{seconds}")), task, &graph)?
        }
        Cmd::Status { run: Some(id) } | Cmd::Answer { run: id, .. } | Cmd::Resume { run: id } => {
            Run::open(runs.join(id))?
        }
        Cmd::Status { run: None } => {
            let newest = fs::read_dir(&runs).context("there are no runs in this folder")?;
            let newest = newest.filter_map(|e| e.ok().map(|e| e.path())).max();
            Run::open(newest.context("there are no runs in this folder")?)?
        }
    };
    if let Cmd::Answer { answer, text, .. } = &cli.command {
        run.answer(*answer == YesNo::Yes, text)?;
    }
    if matches!(cli.command, Cmd::Resume { .. }) {
        run.resume();
    }
    if !matches!(cli.command, Cmd::Status { .. }) {
        let graph = Graph::load(&cli.sheet, &cli.roles)?;
        let time_limit = Duration::from_secs(cli.step_minutes * 60);
        let worker: &dyn Worker = match cli.tool {
            Tool::Claude => &Claude { time_limit },
            Tool::Codex => &Codex { time_limit },
        };
        run.go(&graph, &cli.roles, worker)?;
    }
    let id = run
        .dir
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    println!(
        "Run: {id}\nStatus: {:?}\n{}",
        run.state.status, run.state.note
    );
    Ok(())
}
