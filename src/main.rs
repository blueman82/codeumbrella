//! The `codeumbrella` command.

use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use codeumbrella::claude::Claude;
use codeumbrella::codex::Codex;
use codeumbrella::engine::{Job, Lock, Outcome, Run, Status, Worker};
use codeumbrella::graph::{Graph, Tool};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs, path::Path, path::PathBuf, process::Command};

/// The folder in a project that holds the sheet, the rules pages and the runs.
const HOME: &str = ".codeumbrella";

/// The default files that `init` writes into the folder of the factory.
macro_rules! defaults {
    ($($role:literal),*) => {
        &[(".gitignore", "*\n"), ("graph.yaml", include_str!("../graph.example.yaml")),
          // For a project that has no ignore file. The path goes up to the project folder.
          ("../.gitignore", include_str!("../default.gitignore")),
          $(role!($role, ".md"), role!($role, ".claude.json"), role!($role, ".codex.txt")),*]
    };
}

/// One role file: its path in the folder of the factory, and its text.
macro_rules! role {
    ($role:literal, $ext:literal) => {
        (
            concat!("roles/", $role, $ext),
            include_str!(concat!("../roles/", $role, $ext)),
        )
    };
}

const DEFAULTS: &[(&str, &str)] = defaults!(
    "planner",
    "attacker",
    "builder",
    "checker",
    "safety",
    "reviewer",
    "security",
    "simplifier"
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
    /// The tool that does the worker steps. A run keeps its tool: `run` sets it
    /// (default claude), and `answer` and `resume` must use it.
    #[arg(long, value_enum)]
    tool: Option<Tool>,
    #[command(subcommand)]
    command: Cmd,
}

/// Gives each worker step to the tool of the step, or to the tool of the run.
struct Mixed {
    /// The tool of the run, for a step that picks none.
    tool: Tool,
    /// The maximum time for one step.
    time_limit: Duration,
}

impl Worker for Mixed {
    fn run(&self, job: &Job) -> Result<Outcome> {
        let time_limit = self.time_limit;
        match job.tool.unwrap_or(self.tool) {
            Tool::Claude => Claude { time_limit }.run(job),
            Tool::Codex => Codex { time_limit }.run(job),
        }
    }
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
    Status {
        run: Option<String>,
        /// Wait until the run is not `Running`, then show it.
        #[arg(long, requires = "run")]
        wait: bool,
    },
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
    let runs = env::current_dir()?.join(HOME).join("runs");
    // An answer and a resume take the lock before they read the notebook.
    let lock = match &cli.command {
        Cmd::Answer { run, .. } | Cmd::Resume { run } => Some(Lock::take(&runs.join(run))?),
        _ => None,
    };
    let graph = match &cli.command {
        Cmd::Init { .. } | Cmd::Status { .. } => None,
        _ => Some(Graph::load(&cli.sheet, &cli.roles)?),
    };
    let mut run = match &cli.command {
        Cmd::Init { update } => return init(*update),
        Cmd::Run { task } => {
            let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            fs::create_dir_all(&runs)?;
            // `create_dir` fails if the id exists, so two runs never share an id.
            let more = (1..).map(|n| format!("run-{seconds}-{n}"));
            let mut ids = [format!("run-{seconds}")].into_iter().chain(more);
            let id = ids.find(|i| fs::create_dir(runs.join(i)).is_ok());
            let dir = runs.join(id.unwrap_or_default());
            let mut run = Run::start(dir, task, graph.as_ref().context("no sheet")?)?;
            run.state.tool = Some(cli.tool.unwrap_or(Tool::Claude));
            run
        }
        Cmd::Status { run, wait: true } => wait_for(&runs.join(run.as_deref().unwrap_or("-")))?,
        Cmd::Status { run: Some(id), .. }
        | Cmd::Answer { run: id, .. }
        | Cmd::Resume { run: id } => Run::open(runs.join(id))?,
        Cmd::Status { run: None, .. } => {
            let newest = fs::read_dir(&runs).context("there are no runs in this folder")?;
            let newest = newest.filter_map(|e| e.ok().map(|e| e.path())).max();
            Run::open(newest.context("there are no runs in this folder")?)?
        }
    };
    let _lock = match lock {
        None if matches!(cli.command, Cmd::Run { .. }) => Some(Lock::take(&run.dir)?),
        lock => lock,
    };
    // An old notebook has no tool: `--tool` then sets it.
    // `status` ignores `--tool`: it has no sheet.
    match (cli.tool.filter(|_| graph.is_some()), run.state.tool) {
        (Some(new), Some(old)) if new != old => {
            let old = format!("{old:?}").to_lowercase();
            bail!("this run uses the tool {old}. Send the command again with no --tool.");
        }
        (Some(new), None) => run.state.tool = Some(new),
        _ => {}
    }
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
    if let Some(graph) = graph {
        if run.state.tool.is_none() {
            println!("This run has no tool, so the command uses Claude.");
        }
        let time_limit = Duration::from_secs(cli.step_minutes * 60);
        let tool = run.state.tool.unwrap_or(Tool::Claude);
        run.go(&graph, &cli.roles, &Mixed { tool, time_limit })?;
    }
    show(&run);
    Ok(())
}

/// Waits until the run is not `Running`, then gives it. It changes no file.
fn wait_for(dir: &Path) -> Result<Run> {
    loop {
        let run = Run::open(dir.to_path_buf())?;
        // No lock, or no number yet, means that a command starts or ends.
        if run.state.status != Status::Running {
            return Ok(run);
        }
        let pid = fs::read_to_string(dir.join("lock")).unwrap_or_default();
        let kill = Command::new("kill").args(["-0", pid.trim()]).output();
        let alive = pid.is_empty() || kill.is_ok_and(|o| o.status.success());
        ensure!(
            alive,
            "the command of this run is dead. Remove the file {}/lock",
            dir.display()
        );
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Prints the status text of a run.
fn show(run: &Run) {
    // The screen gets a short note. The full text is in the log.
    let note: String = run.state.note.chars().take(300).collect();
    let id = run.dir.file_name().unwrap_or_default().to_string_lossy();
    println!("Run: {id}\nStatus: {:?}\n{note}", run.state.status);
    if run.state.status == Status::Stopped && note.contains("not as its writer left it") {
        println!("A person must put the paper back first.");
    } else if run.state.status == Status::Stopped {
        println!("To continue: codeumbrella resume {id}");
        println!("Resume only after the person says yes. An AI must not give this approval.");
    }
    println!("More: {}", run.dir.join("log.jsonl").display());
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
