//! The `codeumbrella` command.

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use codeumbrella::claude::Claude;
use codeumbrella::codex::Codex;
use codeumbrella::engine::{Lock, Run, Worker};
use codeumbrella::graph::Graph;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fs, path::Path, path::PathBuf};

/// The folder in a project that holds the sheet, the rules pages and the runs.
const HOME: &str = ".codeumbrella";

/// The default files that `init` writes into the folder of the factory.
macro_rules! defaults {
    ($($name:literal),*) => {
        &[(".gitignore", "*\n"), ("graph.yaml", include_str!("../graph.example.yaml")),
          // For a project that has no ignore file. The path goes up to the project folder.
          ("../.gitignore", include_str!("../default.gitignore")),
          $((concat!("roles/", $name), include_str!(concat!("../roles/", $name)))),*]
    };
}

const DEFAULTS: &[(&str, &str)] = defaults!(
    "planner.md",
    "planner.claude.json",
    "planner.codex.txt",
    "builder.md",
    "builder.claude.json",
    "builder.codex.txt",
    "checker.md",
    "checker.claude.json",
    "checker.codex.txt"
);

/// A software factory: it puts code into main after the tests and your approval.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// The instruction sheet.
    #[arg(long, default_value = ".codeumbrella/graph.yaml")]
    sheet: PathBuf,
    /// The folder with the rules pages.
    #[arg(long, default_value = ".codeumbrella/roles")]
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
    /// Put the default sheet and rules pages into this project.
    Init {
        /// Replace changed files with the defaults. The old file is kept as `.before`.
        #[arg(long)]
        update: bool,
    },
    /// Start a new run for a task.
    Run { task: String },
    /// Show where a run is. Without an id: the newest run.
    Status { run: Option<String> },
    /// Answer the open question of a run, then continue.
    Answer {
        run: String,
        /// The step that has the question: `look_at_tests` or `approval`.
        step: String,
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
    let runs = Path::new(HOME).join("runs");
    // An answer and a resume take the lock before they read the notebook.
    let lock = match &cli.command {
        Cmd::Answer { run, .. } | Cmd::Resume { run } => Some(Lock::take(&runs.join(run))?),
        _ => None,
    };
    let mut run = match &cli.command {
        Cmd::Init { update } => return init(*update),
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
    let _lock = match lock {
        None if matches!(cli.command, Cmd::Run { .. }) => Some(Lock::take(&run.dir)?),
        lock => lock,
    };
    if let Cmd::Answer {
        step, answer, text, ..
    } = &cli.command
    {
        // An answer is for one question only. A second send does not answer the next one.
        let at = run.state.at.as_deref().unwrap_or("no step");
        ensure!(at == step, "the run is at {at}, not at {step}");
        run.answer(*answer == YesNo::Yes, text)?;
    }
    if let (Cmd::Resume { .. }, Some(at)) = (&cli.command, &run.state.at) {
        run.state.tries.remove(at);
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
    // The screen gets a short note. The full text is in the log.
    let note: String = run.state.note.chars().take(300).collect();
    let id = run.dir.file_name().unwrap_or_default().to_string_lossy();
    println!("Run: {id}\nStatus: {:?}\n{note}", run.state.status);
    println!("More: {}", run.dir.join("log.jsonl").display());
    Ok(())
}

/// Writes each default file that the project does not have. With `update`, it
/// also replaces a changed file and keeps the old one as `<name>.before`.
fn init(update: bool) -> Result<()> {
    for (name, text) in DEFAULTS
        .iter()
        .filter(|(n, _)| !update || !n.starts_with(".."))
    {
        let path = Path::new(HOME).join(name);
        let old = fs::read_to_string(&path).ok();
        if old.as_deref() == Some(text) || (!update && path.exists()) {
            continue;
        }
        fs::create_dir_all(path.parent().context("the path has no folder")?)?;
        if old.is_some() {
            fs::copy(&path, format!("{}.before", path.display()))
                .with_context(|| format!("cannot keep {} as .before", path.display()))?;
        }
        fs::write(&path, text).with_context(|| format!("cannot write {}", path.display()))?;
        if update {
            println!("Updated {}", path.display());
        }
    }
    println!("The factory files are in {HOME}/. Change them for this project.");
    Ok(())
}
