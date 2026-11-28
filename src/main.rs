//! The `codeumbrella` command.

use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand, ValueEnum};
use codeumbrella::claude::Claude;
use codeumbrella::codex::Codex;
use codeumbrella::engine::{Job, Lock, Numbers, Outcome, Run, Status, Worker};
use codeumbrella::graph::{Graph, Tool};
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, fs, path::Path, path::PathBuf, process::Command};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// The folder in a project that holds the sheet, the rules pages and the runs.
const HOME: &str = ".codeumbrella";

/// The bytes of 1 GB.
const GB: u64 = 1_000_000_000;

/// The file at the top of a temp folder of a worker. It holds the run id.
const MARK: &str = "codeumbrella-run";

/// The default files that `init` writes into the folder of the factory.
macro_rules! defaults {
    ($($role:literal),*) => {
        &[(".gitignore", "*\n"), ("graph.yaml", include_str!("../graph.example.yaml")),
          // For a project that has no ignore file. The path goes up to the project folder.
          ("../.gitignore", include_str!("../default.gitignore")),
          $(role!($role, ".md"), role!($role, ".claude.json"), role!($role, ".codex.txt")),*,
          page!("python"), page!("go"), page!("javascript"), page!("typescript"), page!("java"),
          page!("general"),
          page!("rust")]
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

/// One language page: its path in the folder of the factory, and its text.
macro_rules! page {
    ($name:literal) => {
        (
            concat!("roles/languages/", $name, ".md"),
            include_str!(concat!("../roles/languages/", $name, ".md")),
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
    "simplifier",
    "user",
    "learner",
    "writer"
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
    /// The free disk space in GB that `run` must find. 0 turns the check off.
    #[arg(long, default_value_t = 10)]
    min_free_gb: u64,
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
        self.run_counted(job).0
    }

    fn run_counted(&self, job: &Job) -> (Result<Outcome>, Numbers) {
        let time_limit = self.time_limit;
        match job.tool.unwrap_or(self.tool) {
            Tool::Claude => Claude { time_limit }.run_counted(job),
            Tool::Codex => Codex { time_limit }.run_counted(job),
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
    /// Start a new run for a task, or for the first task of the queue.
    Run {
        #[arg(required_unless_present = "next", conflicts_with = "next")]
        task: Option<String>,
        /// Start the first task of `.codeumbrella/queue/`.
        #[arg(long)]
        next: bool,
    },
    /// Show the tasks that wait in the queue.
    Queue,
    /// Remove the copies of each Done or Stopped run, or of one such run.
    Clean { run: Option<String> },
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
    /// End an active run. Only after the person says to stop it.
    Stop { run: String },
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum YesNo {
    Yes,
    No,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let runs = env::current_dir()?.join(HOME).join("runs");
    if let Cmd::Queue = &cli.command {
        return queue();
    }
    if let Cmd::Stop { run } = &cli.command {
        return stop(&runs.join(run));
    }
    if let Cmd::Clean { run } = &cli.command {
        return clean(&runs, run.as_deref());
    }
    let target = match &cli.command {
        Cmd::Status { run: None, .. } => Some(newest(&runs)?),
        Cmd::Status { run: Some(id), .. }
        | Cmd::Answer { run: id, .. }
        | Cmd::Resume { run: id } => Some(runs.join(id)),
        _ => None,
    };
    // A kill leaves a lock and workers. The next command for the run repairs them.
    if let Some(dir) = &target {
        repair(dir)?;
    }
    // An answer and a resume take the lock before they read the notebook.
    let lock = match (&cli.command, &target) {
        (Cmd::Answer { .. } | Cmd::Resume { .. }, Some(dir)) => Some(Lock::take(dir)?),
        _ => None,
    };
    let graph = match &cli.command {
        Cmd::Init { .. } | Cmd::Status { .. } => None,
        _ => Some(Graph::load(&cli.sheet, &cli.roles)?),
    };
    let mut run = match &cli.command {
        Cmd::Init { update } => return init(*update),
        Cmd::Run { task, .. } => {
            // Before the run folder and the queue: a refused run changes nothing.
            check_space(cli.min_free_gb)?;
            let (task, queued) = match task {
                Some(task) => (task.clone(), None),
                None => next_task()?,
            };
            let dir = new_run_dir(&runs)?;
            // The move decides which command gets the task. A loser leaves no run folder.
            if let Some(file) = queued {
                let moved = fs::rename(&file, dir.join("task.txt"));
                if let Err(e) = moved {
                    let _ = fs::remove_dir_all(&dir);
                    bail!("cannot move {}: {e}", file.display());
                }
            }
            let mut run = Run::start(dir, &task, graph.as_ref().context("no sheet")?)?;
            // Default is Claude: the factory was built for it, and old notebooks have no tool.
            run.state.tool = Some(cli.tool.unwrap_or(Tool::Claude));
            run
        }
        Cmd::Status { wait: true, .. } => wait_for(target.as_deref().context("no run")?)?,
        Cmd::Status { .. } | Cmd::Answer { .. } | Cmd::Resume { .. } => {
            Run::open(target.context("no run")?)?
        }
        Cmd::Stop { .. } | Cmd::Queue | Cmd::Clean { .. } => {
            bail!("this command must return before this point")
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
        if matches!(cli.command, Cmd::Run { .. }) {
            let id = run.dir.file_name().unwrap_or_default().to_string_lossy();
            println!("{:<10}{id}", "Run:");
        }
        let time_limit = Duration::from_secs(cli.step_minutes * 60);
        let tool = run.state.tool.unwrap_or(Tool::Claude);
        run.go(&graph, &cli.roles, &Mixed { tool, time_limit })?;
    }
    show(&run);
    Ok(())
}

/// Gives an error if the disk of the project has less than `limit` GB free.
fn check_space(limit: u64) -> Result<()> {
    if limit == 0 {
        return Ok(());
    }
    let df = Command::new("df")
        .arg("-Pk")
        .arg(env::current_dir()?)
        .output()
        .context("cannot start `df`, so no run started")?;
    let text = String::from_utf8_lossy(&df.stdout);
    // The name of the disk can have spaces, so count from the field with `%`.
    let blocks = text.lines().skip(1).find_map(|line| {
        let words: Vec<&str> = line.split_whitespace().collect();
        let pair = words.windows(2).find(|w| w[1].ends_with('%'))?;
        pair[0].parse::<u64>().ok()
    });
    let blocks = blocks.filter(|_| df.status.success()).with_context(|| {
        let error = String::from_utf8_lossy(&df.stderr);
        format!(
            "`df` gave no free space, so no run started. {}",
            error.trim()
        )
    })?;
    let free = blocks.saturating_mul(1024);
    ensure!(
        free >= limit.saturating_mul(GB),
        "the disk of the project has {} free, and the limit is {limit} GB. No run started. `codeumbrella clean` frees space.",
        gigabytes(free)
    );
    Ok(())
}

/// Bytes as GB with one digit after the point, rounded down.
fn gigabytes(bytes: u64) -> String {
    let tenths = bytes / (GB / 10);
    format!("{}.{} GB", tenths / 10, tenths % 10)
}

/// Removes the copies and the temp folders of each Done or Stopped run, or
/// only of the run `only`. Then it prints the freed space.
fn clean(runs: &Path, only: Option<&str>) -> Result<()> {
    let dirs = if let Some(id) = only {
        // One folder name only, so no path gets to a different folder.
        ensure!(
            !matches!(id, "" | "." | "..") && !id.contains('/'),
            "{id} is not a run id"
        );
        let dir = runs.join(id);
        fs::symlink_metadata(&dir).with_context(|| format!("there is no run {id}"))?;
        if let Ok(run) = Run::open(dir.clone()) {
            let status = run.state.status;
            ensure!(
                ended(status),
                "the run {id} has the status {status:?}, so `clean` keeps its copies"
            );
        }
        vec![dir]
    } else {
        match fs::read_dir(runs) {
            Ok(found) => {
                let mut dirs: Vec<PathBuf> =
                    found.filter_map(|e| e.ok().map(|e| e.path())).collect();
                dirs.sort();
                dirs
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
            Err(e) => bail!("cannot read {}: {e}", runs.display()),
        }
    };
    let temp = temp_folders();
    let (mut freed, mut complete) = (0, true);
    for dir in dirs {
        let real = fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir());
        let done = || Run::open(dir.clone()).is_ok_and(|r| ended(r.state.status));
        if !real || !done() {
            continue;
        }
        let id = dir.file_name().unwrap_or_default().to_string_lossy();
        let _lock = match Lock::take(&dir) {
            Ok(lock) => lock,
            Err(e) => {
                println!("Not cleaned: {id}: {e:#}");
                continue;
            }
        };
        // A `resume` can change the status before the lock, so read it again.
        if !done() {
            continue;
        }
        let marked = temp.iter().filter(|(_, run)| **run == *id);
        let copies = [dir.join("work"), dir.join("clean")];
        for path in copies.iter().chain(marked.map(|(path, _)| path)) {
            if fs::symlink_metadata(path).is_err() {
                continue;
            }
            let before = size(path);
            // `remove_dir_all` removes a link itself and does not follow it.
            if let Err(e) = fs::remove_dir_all(path) {
                println!("Not cleaned: {id}: {}: {e}", path.display());
                complete = false;
            }
            freed += before.saturating_sub(size(path));
        }
    }
    let freed = if freed < GB {
        format!("{} MB", freed / 1_000_000)
    } else {
        gigabytes(freed)
    };
    println!("{:<10}{freed}", "Freed:");
    ensure!(complete, "some folders stay. See the lines `Not cleaned:`.");
    Ok(())
}

/// Tells if a run with this status can lose its copies.
fn ended(status: Status) -> bool {
    matches!(status, Status::Done | Status::Stopped)
}

/// The temp folders of workers, each with the run id of its marker. The full
/// path is the key, so a folder of two bases counts one time.
fn temp_folders() -> BTreeMap<PathBuf, String> {
    let mut bases = vec![env::temp_dir(), PathBuf::from("/tmp")];
    bases.extend(env::var_os("CLAUDE_CODE_TMPDIR").map(PathBuf::from));
    // A base can be a link. A `claude` folder or a temp folder cannot.
    let folders = |dir: &Path| {
        let found = fs::read_dir(dir).into_iter().flatten().flatten();
        found
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
            .collect::<Vec<_>>()
    };
    let mut found = BTreeMap::new();
    for base in bases {
        let mut all = folders(&base);
        let inner: Vec<_> = all
            .iter()
            .filter(|(name, _)| name.starts_with("claude"))
            .flat_map(|(_, path)| folders(path))
            .collect();
        all.extend(inner);
        for (name, path) in all {
            let mark = path.join(MARK);
            let ours = name.starts_with("scratch.") || name.starts_with("lesson.");
            if !ours || !fs::symlink_metadata(&mark).is_ok_and(|m| m.is_file()) {
                continue;
            }
            if let (Ok(id), Ok(full)) = (fs::read_to_string(mark), fs::canonicalize(path)) {
                found.insert(full, id.trim().to_owned());
            }
        }
    }
    found
}

/// The sum of the file lengths in `path`. It follows no link.
fn size(path: &Path) -> u64 {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => {
            let found = fs::read_dir(path).into_iter().flatten().flatten();
            found.map(|e| size(&e.path())).sum()
        }
        Ok(m) if m.is_file() => m.len(),
        _ => 0,
    }
}

/// Makes the folder of a new run with a new id.
fn new_run_dir(runs: &Path) -> Result<PathBuf> {
    // Four digits from the clock and the command number make a same id in two folders rare.
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let digits = (u64::from(now.subsec_nanos()) + u64::from(std::process::id())) % 10_000;
    let stamp = format!("{}-{digits:04}", now.as_secs());
    fs::create_dir_all(runs)
        .with_context(|| format!("cannot make the folder {}", runs.display()))?;
    // `create_dir` fails if the id exists, so two runs never share an id.
    // Only "the folder is there already" makes it try the next id.
    let mut id = format!("run-{stamp}");
    for n in 1.. {
        match fs::create_dir(runs.join(&id)) {
            Ok(()) => break,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                id = format!("run-{stamp}-{n}");
            }
            Err(e) => bail!("cannot make the folder {}/{id}: {e}", runs.display()),
        }
    }

    Ok(runs.join(id))
}

/// Waits until the run is not `Running`, then gives it. It repairs a run
/// with a dead command.
fn wait_for(dir: &Path) -> Result<Run> {
    let mut no_lock = None;
    loop {
        repair(dir)?;
        let run = Run::open(dir.to_path_buf())?;
        if run.state.status != Status::Running {
            return Ok(run);
        }
        // No lock for 2 seconds in a row: no command works on the run.
        let since = *no_lock.get_or_insert_with(Instant::now);
        if dir.join("lock").exists() {
            no_lock = None;
        } else if since.elapsed() >= Duration::from_secs(2) {
            let id = dir.file_name().unwrap_or_default().to_string_lossy();
            bail!(
                "the run says Running and no command works on it. Run `codeumbrella resume {id}` only after the person says yes."
            );
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// The newest run folder.
fn newest(runs: &Path) -> Result<PathBuf> {
    let found = fs::read_dir(runs).context("there are no runs in this folder")?;
    let found = found.filter_map(|e| e.ok().map(|e| e.path())).max();
    found.context("there are no runs in this folder")
}

/// Reads a text as a number larger than 0.
fn number(text: &str) -> Option<u32> {
    text.trim().parse().ok().filter(|n| *n > 0)
}

/// Tells if a command with this number is alive.
fn alive(pid: u32) -> bool {
    let kill = Command::new("kill").args(["-0", &pid.to_string()]).output();
    kill.is_ok_and(|o| o.status.success())
}

/// Removes a file. It is no error if the file is not there.
fn remove(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != ErrorKind::NotFound => {
            bail!("cannot remove {}: {e}", path.display())
        }
        _ => Ok(()),
    }
}

/// Ends the workers of the files `worker.<number>`, then removes the files.
/// A number is a worker only if its start time is the one in the file.
/// It first looks at each file and changes nothing. An error changes no file.
fn end_workers(dir: &Path) -> Result<()> {
    let files = fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))?;
    let mut found = Vec::new();
    for path in files.filter_map(|e| e.ok().map(|e| e.path())) {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let Some(pid) = name.strip_prefix("worker.").and_then(number) else {
            continue;
        };
        let mut ours = false;
        if alive(pid) {
            let start = Command::new("ps")
                .args(["-o", "lstart=", "-p", &pid.to_string()])
                .output()
                .ok()
                .filter(|o| o.status.success());
            let start = start.with_context(|| {
                format!("cannot see the worker {pid} with `ps`. Run the command with no sandbox.")
            })?;
            let start = String::from_utf8_lossy(&start.stdout);
            let held = fs::read_to_string(&path).unwrap_or_default();
            ours = start.trim() == held.trim();
        }
        found.push((path, pid, ours));
    }
    for (path, pid, ours) in found {
        if ours {
            let _ = Command::new("pkill")
                .args(["-TERM", "-P", &pid.to_string()])
                .output();
            let _ = Command::new("kill").arg(pid.to_string()).output();
        }
        remove(&path)?;
    }
    Ok(())
}

/// Repairs a run after a kill of its command: ends the workers that stay,
/// removes `lock` and writes Stopped. Gives `false` if it did nothing.
fn repair(dir: &Path) -> Result<bool> {
    let lock = dir.join("lock");
    let first = fs::read_to_string(&lock).unwrap_or_default();
    let Some(pid) = number(&first) else {
        return Ok(false);
    };
    if owner(pid)? == Some(true) {
        return Ok(false);
    }
    let mut run = Run::open(dir.to_path_buf())?;
    // A different text means that a new command got the lock.
    if fs::read_to_string(&lock).unwrap_or_default() != first {
        return Ok(false);
    }
    end_workers(dir)?;
    remove(&lock)?;
    if run.state.status == Status::Running {
        let at = run.state.at.clone().unwrap_or_default();
        run.state.status = Status::Stopped;
        run.state.note = format!("The command of the run ended at the step {at} with no result.");
        run.save()?;
    }
    println!("Repaired: the command of this run was dead.");
    Ok(true)
}

/// Looks at the command with this number: `None` if it is dead, else if it is `codeumbrella`.
/// It gives an error if `ps` cannot tell.
fn owner(pid: u32) -> Result<Option<bool>> {
    if !alive(pid) {
        return Ok(None);
    }
    let pid = pid.to_string();
    let ps = Command::new("ps")
        .args(["-o", "stat=,comm=", "-p", &pid])
        .output();
    let text = ps
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    let text = text.filter(|t| !t.trim().is_empty()).with_context(|| {
        format!("cannot see the command {pid} with `ps`. Run `codeumbrella stop` with no sandbox.")
    })?;
    let (stat, name) = text
        .trim()
        .split_once(char::is_whitespace)
        .unwrap_or((&text, ""));
    let ours = Path::new(name.trim())
        .file_name()
        .is_some_and(|n| n == "codeumbrella");
    Ok((!stat.starts_with('Z')).then_some(ours))
}

/// Ends the command of an active run and its workers, removes the lock and writes Stopped.
fn stop(dir: &Path) -> Result<()> {
    let run = Run::open(dir.to_path_buf())?;
    ensure!(
        run.state.status == Status::Running,
        "the run has the status {:?}, not Running, so there is nothing to stop",
        run.state.status
    );
    let lock = dir.join("lock");
    let pid = number(&fs::read_to_string(&lock).unwrap_or_default());
    if let Some(pid) = pid.filter(|p| owner(*p).is_ok_and(|o| o == Some(true))) {
        let _ = Command::new("pkill")
            .args(["-TERM", "-P", &pid.to_string()])
            .output();
        let _ = Command::new("kill").arg(pid.to_string()).output();
        let end = Instant::now() + Duration::from_secs(10);
        while owner(pid)?.is_some() {
            ensure!(
                Instant::now() < end,
                "the command {pid} did not end in 10 seconds"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    } else if let Some(pid) = pid {
        owner(pid)?;
    }
    end_workers(dir)?;
    // The ended command can write the notebook, so read it again.
    let mut run = Run::open(dir.to_path_buf())?;
    remove(&lock)?;
    let at = run.state.at.clone().unwrap_or_default();
    run.state.status = Status::Stopped;
    run.state.note = format!("A person stopped the run at the step {at}.");
    run.save()?;
    show(&run);
    Ok(())
}

/// Prints the status text of a run.
fn show(run: &Run) {
    // The screen gets a short note. The full text is in the log.
    let note: String = run.state.note.chars().take(300).collect();
    let id = run.dir.file_name().unwrap_or_default().to_string_lossy();
    // One fact on each line. Each value starts at character 11.
    let line = |name: &str, value: &str| println!("{:<10}{value}", format!("{name}:"));
    let (state, total) = (&run.state, &run.state.total);
    line("Run", &id);
    line("Status", &format!("{:?}", state.status));
    if !note.is_empty() {
        line("Note", &note);
    }
    if state.status == Status::Running {
        line("Step", state.at.as_deref().unwrap_or_default());
        let since = state.since.as_deref();
        if let Some(Ok(since)) = since.map(|t| OffsetDateTime::parse(t, &Rfc3339)) {
            let minutes = (OffsetDateTime::now_utc() - since).whole_minutes();
            let minutes = u64::try_from(minutes).unwrap_or(0);
            line("Since", &count(minutes, "minute"));
        }
    }
    if total.workers > 0 {
        line("Workers", &total.workers.to_string());
        line(
            "Time",
            &format!("{:.1} worker minutes", total.seconds / 60.0),
        );
    }
    if total.workers > total.no_cost {
        line("Cost", &format!("{:.2} dollars", total.dollars));
    }
    if total.no_cost > 0 && total.no_cost < total.workers {
        line("No cost", &count(total.no_cost, "worker"));
    }
    if run.state.status == Status::Stopped && note.contains("not as its writer left it") {
        println!("A person must put the paper back first.");
    } else if run.state.status == Status::Stopped {
        println!("To continue: codeumbrella resume {id}");
        println!("Resume only after the person says yes. An AI must not give this approval.");
    }
    line("More", &run.dir.join("log.jsonl").display().to_string());
}

/// Gives `n` and the word, with an "s" if `n` is not 1.
fn count(n: u64, word: &str) -> String {
    let s = if n == 1 { "" } else { "s" };
    format!("{n} {word}{s}")
}

/// The files of the queue, in the sequence of their names.
fn queued() -> Result<Vec<PathBuf>> {
    let found = match fs::read_dir(Path::new(HOME).join("queue")) {
        Ok(found) => found,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => bail!("cannot read the queue: {e}"),
    };
    let mut files: Vec<PathBuf> = found.filter_map(|e| e.ok().map(|e| e.path())).collect();
    files.sort();
    Ok(files)
}

/// The text of a task file. A link or a folder is an error: no read out of the queue.
fn task_text(path: &Path) -> Result<String> {
    let plain = fs::symlink_metadata(path).is_ok_and(|m| m.is_file());
    ensure!(plain, "{} is not a plain file", path.display());
    fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))
}

/// Prints for each task that waits: the file name and its `Files:` line. A task
/// with a `Purpose:` line gets that line below.
fn queue() -> Result<()> {
    let files = queued()?;
    if files.is_empty() {
        println!("The queue is empty.");
    }
    for path in files {
        let text = task_text(&path)?;
        let given = text.lines().find(|l| l.starts_with("Files:"));
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        println!("{name}  {}", given.unwrap_or("Files: not given").trim());
        if let Some(purpose) = text.lines().find(|l| l.starts_with("Purpose:")) {
            println!("  {}", purpose.trim());
        }
    }
    Ok(())
}

/// Gives the first task of the queue and its file. It changes nothing. It gives
/// an error for an empty queue. A different active run does not stop it.
fn next_task() -> Result<(String, Option<PathBuf>)> {
    let first = queued()?.into_iter().next().context("the queue is empty")?;
    Ok((task_text(&first)?, Some(first)))
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
    // The script of the person stays, also with `--update`.
    let script = Path::new(HOME).join("set-version");
    if !script.exists() {
        fs::write(&script, include_str!("../default.set-version"))?;
    }
    fs::create_dir_all(Path::new(HOME).join("queue"))?;
    println!("The factory files are in {HOME}/. Change them for this project.");
    Ok(())
}
