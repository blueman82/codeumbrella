//! Tests of the command `codeumbrella stop <run id>`. They start the real
//! command with a simulated `claude`. No worker, no network.
//!
//! The last three tests use `ps` and `pkill`. A sandbox of a worker blocks them.
//! There, each of these tests prints one line and ends good.

use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use std::{env, fs, thread};

static COUNT: AtomicU32 = AtomicU32::new(0);

fn git(dir: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.args(args).current_dir(dir);
    command.env("GIT_AUTHOR_NAME", "t");
    command.env("GIT_AUTHOR_EMAIL", "t@t");
    command.env("GIT_COMMITTER_NAME", "t");
    command.env("GIT_COMMITTER_EMAIL", "t@t");
    assert!(command.status().unwrap().success());
}

/// A new project with the factory files and a folder for simulated tools.
/// The path is the full one: on a Mac, `/private/var/...` and not `/var/...`.
fn project() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-stop-{}-{n}", std::process::id());
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
    let dir = fs::canonicalize(&dir).unwrap();
    assert!(cu(&dir, &["init"]).status.success());
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "--quiet", "-m", "x"]);
    dir
}

fn tool(dir: &Path, name: &str, script: &str) {
    let bin = dir.join("bin");
    let path = bin.join(name);
    let script = script.replace("$PWD/bin", &bin.display().to_string());
    let body = format!("#!/bin/sh\n{script}\n");
    fs::write(&path, body).unwrap();
    let mode = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&path, mode).unwrap();
}

/// The real command, with the simulated tools first on the PATH.
fn command(dir: &Path, args: &[&str]) -> Command {
    let bin = dir.join("bin").display().to_string();
    let old = env::var("PATH").unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(args).current_dir(dir);
    command.env("PATH", format!("{bin}:{old}"));
    command
}

fn cu(dir: &Path, args: &[&str]) -> Output {
    command(dir, args).output().expect("start codeumbrella")
}

fn out(o: &Output) -> String {
    let stdout = String::from_utf8_lossy(&o.stdout);
    let stderr = String::from_utf8_lossy(&o.stderr);
    format!("{stdout}{stderr}")
}

fn id(o: &Output) -> String {
    let all = out(o);
    let line = all.lines().find(|l| l.starts_with("Run:      "));
    let line = line.expect("a run");
    line["Run:      ".len()..].to_owned()
}

fn says(o: &Output, needle: &str) {
    assert!(out(o).contains(needle), "{}", out(o));
}

const CLAUDE: &str = r#"echo called > "$PWD/bin/claude.$$"
echo '{"is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

/// A simulated `claude` that writes its number and then waits 30 seconds.
const CLAUDE_HANG: &str = r#"echo $$ > "$PWD/bin/worker.pid"
exec sleep 30"#;

/// A simulated `ps` that gives an error and no text.
const PS_BAD: &str = "echo 'ps: operation not permitted' >&2\nexit 1";

/// A sheet: copy, a worker, a question.
const SHEET: &str = r"start: copy
steps:
  copy:
    type: command
    run:
      - [git, clone, --quiet, --no-hardlinks, ., '.codeumbrella/runs/{run}/work']
    next: one
  one:
    role: planner
    writes: [spec]
    next: ask
  ask:
    type: person
";

fn setup() -> PathBuf {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    let sheet = dir.join(".codeumbrella/graph.yaml");
    fs::write(sheet, SHEET).unwrap();
    dir
}

fn run_dir(dir: &Path, id: &str) -> PathBuf {
    dir.join(".codeumbrella/runs").join(id)
}

fn bytes(dir: &Path, id: &str) -> Vec<u8> {
    fs::read(run_dir(dir, id).join("state.json")).unwrap()
}

fn load(dir: &Path, id: &str) -> Value {
    serde_json::from_slice(&bytes(dir, id)).unwrap()
}

fn set_status(dir: &Path, id: &str, status: &str) {
    let mut value = load(dir, id);
    value["status"] = Value::String(status.to_owned());
    let text = serde_json::to_vec_pretty(&value).unwrap();
    let file = run_dir(dir, id).join("state.json");
    fs::write(file, text).unwrap();
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_owned()
}

/// The names of all files in a folder, with their sizes.
fn listing(dir: &Path) -> Vec<(String, u64)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let size = entry.metadata().unwrap().len();
        let name = entry.file_name().to_string_lossy().into_owned();
        found.push((name, size));
    }
    found.sort();
    found
}

/// A run that waits at the question. Gives the id.
fn waiting(dir: &Path) -> String {
    let run = cu(dir, &["run", "a task"]);
    says(&run, "Status:   Waiting");
    id(&run)
}

fn note(at: &str) -> String {
    format!("A person stopped the run at the step {at}.")
}

/// The six lines of a good `stop`.
fn six_lines(dir: &Path, id: &str, at: &str) -> String {
    let log = run_dir(dir, id).join("log.jsonl");
    let rule = "Resume only after the person says yes. An AI must not give this approval.";
    let lines = [
        format!("Run:      {id}"),
        "Status:   Stopped".to_owned(),
        format!("Note:     {}", note(at)),
        format!("To continue: codeumbrella resume {id}"),
        rule.to_owned(),
        format!("More:     {}", log.display()),
    ];
    format!("{}\n", lines.join("\n"))
}

/// The output of `stop` is the six lines.
fn shows(stop: &Output, dir: &Path, id: &str, at: &str) {
    let shown = String::from_utf8_lossy(&stop.stdout).into_owned();
    // The lines of the numbers of the workers come from `tests/run_total.rs`.
    let numbers = ["Workers:", "Time:", "Cost:", "No cost:"];
    let fact = |l: &&str| !numbers.iter().any(|n| l.starts_with(n));
    let shown: Vec<&str> = shown.lines().filter(fact).collect();
    assert_eq!(format!("{}\n", shown.join("\n")), six_lines(dir, id, at));
}

/// The notebook says Stopped with the note of the person, and `at` is the same.
fn assert_stopped(dir: &Path, id: &str, at: &str) {
    let value = load(dir, id);
    assert_eq!(text(&value, "status"), "Stopped", "{value}");
    assert_eq!(text(&value, "note"), note(at), "{value}");
    assert_eq!(text(&value, "at"), at, "{value}");
    let lock = run_dir(dir, id).join("lock");
    assert!(!lock.exists(), "the lock is there");
}

/// A run that says Running, with this text in `lock`. Gives the id and `at`.
fn running(dir: &Path, lock: Option<&str>) -> (String, String) {
    let id = waiting(dir);
    set_status(dir, &id, "Running");
    if let Some(lock) = lock {
        fs::write(run_dir(dir, &id).join("lock"), lock).unwrap();
    }
    let at = text(&load(dir, &id), "at");
    assert_ne!(at, "");
    (id, at)
}

/// The number of a command that is dead and collected.
fn dead_number() -> String {
    let mut gone = Command::new("true").spawn().unwrap();
    gone.wait().unwrap();
    gone.id().to_string()
}

fn alive(pid: u32) -> bool {
    let mut kill = Command::new("kill");
    kill.args(["-0", &pid.to_string()]).stderr(Stdio::null());
    kill.status().unwrap().success()
}

/// Waits some seconds until a command is dead.
fn dies(pid: u32) -> bool {
    let end = Instant::now() + Duration::from_secs(5);
    while alive(pid) {
        if Instant::now() > end {
            return false;
        }
        thread::sleep(Duration::from_millis(50));
    }
    true
}

/// Ends its commands when the test ends, also after a failure.
#[derive(Default)]
struct Cleanup {
    children: Vec<Child>,
    pids: Vec<u32>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        for pid in &self.pids {
            let mut kill = Command::new("kill");
            kill.args(["-9", &pid.to_string()]).stderr(Stdio::null());
            let _ = kill.status();
        }
    }
}

/// A live `sleep`. Its number is the one that a `lock` can hold.
fn sleeper(cleanup: &mut Cleanup) -> u32 {
    let child = Command::new("sleep").arg("30").spawn().unwrap();
    let pid = child.id();
    cleanup.children.push(child);
    pid
}

/// Waits for a child. After `seconds`, it kills the child and fails.
fn finish(mut child: Child, seconds: u64) -> Output {
    let end = Instant::now() + Duration::from_secs(seconds);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > end {
            child.kill().unwrap();
            panic!("the command did not end in {seconds} seconds");
        }
        thread::sleep(Duration::from_millis(50));
    }
    child.wait_with_output().unwrap()
}

fn spawn(dir: &Path, args: &[&str]) -> Child {
    let mut c = command(dir, args);
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    c.spawn().unwrap()
}

/// True if this computer does not let the test use `ps` and `pgrep`.
fn skip() -> bool {
    let me = std::process::id().to_string();
    let mut ps = Command::new("ps");
    ps.args(["-o", "comm=", "-p", &me]);
    let ps = ps.output();
    let ps_ok = ps.is_ok_and(|o| o.status.success() && !o.stdout.is_empty());
    let mut pgrep = Command::new("pgrep");
    pgrep.args(["-P", &me]);
    let pgrep = pgrep.output();
    let pgrep_ok = pgrep.is_ok_and(|o| o.status.code().is_some_and(|c| c <= 1));
    let skipped = !(ps_ok && pgrep_ok);
    if skipped {
        eprintln!("skipped: ps or pgrep cannot run here");
    }
    skipped
}

// Group 1: tests that work in a sandbox and outside it.

fn refuses_status(status: &str) {
    let dir = setup();
    let id = waiting(&dir);
    set_status(&dir, &id, status);
    let folder = run_dir(&dir, &id);
    let (state, files) = (bytes(&dir, &id), listing(&folder));
    let stop = cu(&dir, &["stop", &id]);
    assert!(!stop.status.success(), "{}", out(&stop));
    says(&stop, status);
    assert_eq!(bytes(&dir, &id), state);
    assert_eq!(listing(&folder), files);
}

#[test]
fn stop_on_a_run_with_status_waiting_gives_an_error() {
    let dir = setup();
    let id = waiting(&dir);
    let folder = run_dir(&dir, &id);
    let (state, files) = (bytes(&dir, &id), listing(&folder));
    let stop = cu(&dir, &["stop", &id]);
    assert!(!stop.status.success(), "{}", out(&stop));
    says(&stop, "Waiting");
    assert_eq!(bytes(&dir, &id), state);
    assert_eq!(listing(&folder), files);
}

#[test]
fn stop_on_a_run_with_status_done_gives_an_error() {
    refuses_status("Done");
}

#[test]
fn stop_on_a_run_with_status_stopped_gives_an_error() {
    refuses_status("Stopped");
}

#[test]
fn stop_on_a_run_id_with_no_notebook_names_the_id() {
    let dir = setup();
    let runs = dir.join(".codeumbrella/runs");
    // A new project has no folder for runs.
    fs::create_dir_all(&runs).unwrap();
    let files = listing(&runs);
    let stop = cu(&dir, &["stop", "run-1234567890"]);
    assert!(!stop.status.success(), "{}", out(&stop));
    says(&stop, "run-1234567890");
    assert_eq!(listing(&runs), files);
    assert!(!runs.join("run-1234567890").exists());
}

#[test]
fn stop_with_a_dead_number_in_the_lock_removes_the_lock_and_stops_the_run() {
    let dir = setup();
    let (id, at) = running(&dir, Some(&dead_number()));
    let stop = cu(&dir, &["stop", &id]);
    assert!(stop.status.success(), "{}", out(&stop));
    assert_stopped(&dir, &id, &at);
    shows(&stop, &dir, &id, &at);
}

#[test]
fn stop_with_text_that_is_not_a_number_in_the_lock_stops_the_run() {
    let dir = setup();
    let (id, at) = running(&dir, Some("not a number\n"));
    let stop = cu(&dir, &["stop", &id]);
    assert!(stop.status.success(), "{}", out(&stop));
    assert_stopped(&dir, &id, &at);
    says(&stop, "Status:   Stopped");
}

#[test]
fn stop_with_no_lock_file_stops_the_run_and_prints_the_resume_line() {
    let dir = setup();
    let (id, at) = running(&dir, None);
    let stop = cu(&dir, &["stop", &id]);
    assert!(stop.status.success(), "{}", out(&stop));
    assert_stopped(&dir, &id, &at);
    let line = format!("To continue: codeumbrella resume {id}\n");
    says(&stop, &line);
}

#[test]
fn stop_needs_no_sheet_and_ignores_the_option_tool() {
    let dir = setup();
    let (id, at) = running(&dir, None);
    let before = load(&dir, &id)["tool"].clone();
    let sheet = dir.join(".codeumbrella/graph.yaml");
    fs::remove_file(sheet).unwrap();
    let stop = cu(&dir, &["--tool", "codex", "stop", &id]);
    assert!(stop.status.success(), "{}", out(&stop));
    assert_stopped(&dir, &id, &at);
    // The notebook of a run keeps its tool: `stop` does not write it.
    assert_eq!(load(&dir, &id)["tool"], before);
}

#[test]
fn stop_with_a_ps_that_fails_gives_an_error_and_changes_nothing() {
    let dir = setup();
    tool(&dir, "ps", PS_BAD);
    let mut cleanup = Cleanup::default();
    let pid = sleeper(&mut cleanup);
    let number = pid.to_string();
    let (id, _) = running(&dir, Some(&number));
    let lock = run_dir(&dir, &id).join("lock");
    let state = bytes(&dir, &id);
    let stop = cu(&dir, &["stop", &id]);
    assert!(!stop.status.success(), "{}", out(&stop));
    let all = out(&stop).to_lowercase();
    assert!(all.contains("sandbox"), "{all}");
    assert_eq!(fs::read_to_string(&lock).unwrap(), number);
    assert_eq!(bytes(&dir, &id), state);
    assert!(alive(pid), "the sleep was ended");
}

#[test]
fn the_chat_command_tells_to_stop_a_run_only_when_the_person_says_so() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file = root.join("skills/codeumbrella/SKILL.md");
    let page = fs::read_to_string(file).unwrap();
    let line = page
        .lines()
        .find(|l| l.contains("codeumbrella stop <run id>"));
    let line = line.expect("a line with `codeumbrella stop <run id>`");
    let line = line.to_lowercase();
    assert!(line.contains("person"), "{line}");
    assert!(line.contains("sandbox"), "{line}");
}

// Group 2: tests that use the `ps` and `pkill` of the computer.

fn has_lock(entry: &fs::DirEntry) -> bool {
    entry.path().join("lock").exists()
}

/// Starts a run whose worker hangs. Gives the id of the run, the number of the
/// command of the run and the number of the worker.
fn active(dir: &Path, cleanup: &mut Cleanup) -> (String, u32, u32) {
    tool(dir, "claude", CLAUDE_HANG);
    let runner = spawn(dir, &["run", "a task"]);
    let runner_pid = runner.id();
    cleanup.children.push(runner);
    let runs = dir.join(".codeumbrella/runs");
    let pid_file = dir.join("bin/worker.pid");
    let end = Instant::now() + Duration::from_secs(15);
    while Instant::now() < end {
        let first = fs::read_dir(&runs).ok().and_then(|mut d| d.next());
        let first = first.and_then(Result::ok);
        let pid = fs::read_to_string(&pid_file).unwrap_or_default();
        let worker = pid.trim().parse::<u32>();
        let locked = first.as_ref().is_some_and(has_lock);
        if let (Some(entry), Ok(worker), true) = (first, worker, locked) {
            cleanup.pids.push(worker);
            let id = entry.file_name().to_string_lossy().into_owned();
            return (id, runner_pid, worker);
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("the worker did not start");
}

/// How many times the simulated `claude` ran to the end.
fn calls(dir: &Path) -> usize {
    let entries = fs::read_dir(dir.join("bin")).unwrap();
    let names = entries.filter_map(Result::ok).map(|e| e.file_name());
    let prefix = "claude.";
    let found = names.filter(|n| n.to_string_lossy().starts_with(prefix));
    found.count()
}

#[test]
fn stop_on_an_active_run_ends_the_command_and_its_worker() {
    if skip() {
        return;
    }
    let dir = setup();
    let mut cleanup = Cleanup::default();
    let (id, runner, worker) = active(&dir, &mut cleanup);
    let file = run_dir(&dir, &id).join("lock");
    let lock = fs::read_to_string(file).unwrap();
    assert_eq!(lock.trim(), runner.to_string());
    let at = text(&load(&dir, &id), "at");
    let stop = finish(spawn(&dir, &["stop", &id]), 30);
    assert!(stop.status.success(), "{}", out(&stop));
    // The test is the parent of the command, so it collects the dead command.
    let end = Instant::now() + Duration::from_secs(5);
    while cleanup.children[0].try_wait().unwrap().is_none() {
        assert!(Instant::now() < end, "the command of the run is alive");
        thread::sleep(Duration::from_millis(50));
    }
    assert!(dies(worker), "the worker is alive");
    assert_stopped(&dir, &id, &at);
    shows(&stop, &dir, &id, &at);
}

#[test]
fn resume_after_stop_starts_the_same_step_again() {
    if skip() {
        return;
    }
    let dir = setup();
    let mut cleanup = Cleanup::default();
    let (id, _, _) = active(&dir, &mut cleanup);
    let at = text(&load(&dir, &id), "at");
    let stop = finish(spawn(&dir, &["stop", &id]), 30);
    assert!(stop.status.success(), "{}", out(&stop));
    assert_stopped(&dir, &id, &at);
    tool(&dir, "claude", CLAUDE);
    let resume = finish(spawn(&dir, &["resume", &id]), 60);
    assert!(resume.status.success(), "{}", out(&resume));
    says(&resume, "Status:   Waiting");
    assert_eq!(calls(&dir), 1, "the worker of the step did not start again");
}

#[test]
fn stop_does_not_end_a_live_command_that_is_not_codeumbrella() {
    if skip() {
        return;
    }
    let dir = setup();
    let mut cleanup = Cleanup::default();
    let pid = sleeper(&mut cleanup);
    let (id, at) = running(&dir, Some(&pid.to_string()));
    let stop = finish(spawn(&dir, &["stop", &id]), 30);
    assert!(stop.status.success(), "{}", out(&stop));
    assert!(alive(pid), "the sleep was ended");
    assert_stopped(&dir, &id, &at);
}
