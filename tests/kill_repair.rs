//! Tests of the repair after a kill. The next command for a run ends the
//! workers that stay, removes `lock` and writes Stopped. They start the real
//! command with a simulated `claude`. No worker, no network.
//!
//! The last four tests use `ps` and `pkill`. A sandbox blocks them.
//! There, each of these tests prints one line and ends good.

use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use std::{env, fs, thread};

static COUNT: AtomicU32 = AtomicU32::new(0);

const REPAIRED: &str = "Repaired: the command of this run was dead.";

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
    let name = format!("cu-repair-{}-{n}", std::process::id());
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

fn lacks(o: &Output, needle: &str) {
    assert!(!out(o).contains(needle), "{}", out(o));
}

const CLAUDE: &str = r#"echo called > "$PWD/bin/claude.$$"
echo '{"type":"result","is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

/// A simulated `claude` that writes its number and then waits 30 seconds.
const CLAUDE_HANG: &str = r#"echo $$ > "$PWD/bin/worker.pid"
exec sleep 30"#;

/// A simulated `ps` that gives an error and no text.
const PS_BAD: &str = "echo 'ps: operation not permitted' >&2\nexit 1";

/// A simulated `ps` that gives a start time that no file has.
const PS_OTHER: &str =
    "case $* in\n*lstart*) echo Mon Jan 1 00:00:00 2001;;\n*) echo S sleep;;\nesac";

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

fn save(dir: &Path, id: &str, value: &Value) {
    let text = serde_json::to_vec_pretty(value).unwrap();
    fs::write(run_dir(dir, id).join("state.json"), text).unwrap();
}

fn set_status(dir: &Path, id: &str, status: &str) {
    let mut value = load(dir, id);
    value["status"] = Value::String(status.to_owned());
    save(dir, id, &value);
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

/// The names of the files `worker.<number>` in a folder.
fn worker_files(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name();
        let name = name.to_string_lossy().into_owned();
        if name.starts_with("worker.") {
            found.push(name);
        }
    }
    found
}

/// The names of the files in a folder, with no sizes.
fn names(dir: &Path) -> Vec<String> {
    listing(dir).into_iter().map(|(name, _)| name).collect()
}

/// A run that waits at the question. Gives the id.
fn waiting(dir: &Path) -> String {
    let run = cu(dir, &["run", "a task"]);
    says(&run, "Status:   Waiting");
    id(&run)
}

/// A run with this status and this text in `lock`. Gives the id and `at`.
fn with_lock(dir: &Path, status: &str, lock: &str) -> (String, String) {
    let id = waiting(dir);
    set_status(dir, &id, status);
    fs::write(run_dir(dir, &id).join("lock"), lock).unwrap();
    let at = text(&load(dir, &id), "at");
    assert_ne!(at, "");
    (id, at)
}

/// A run that says Running, with a dead number in `lock`.
fn dead_run(dir: &Path) -> (String, String) {
    with_lock(dir, "Running", &dead_number())
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

/// The notebook says Stopped with the note of the repair, and `at` is the same.
fn assert_repaired(dir: &Path, id: &str, at: &str) {
    let value = load(dir, id);
    let note = format!("The command of the run ended at the step {at} with no result.");
    assert_eq!(text(&value, "status"), "Stopped", "{value}");
    assert_eq!(text(&value, "note"), note, "{value}");
    assert_eq!(text(&value, "at"), at, "{value}");
    assert!(!run_dir(dir, id).join("lock").exists(), "the lock is there");
}

/// The output of a command that repaired a run.
fn shows_repair(o: &Output, id: &str) {
    assert!(o.status.success(), "{}", out(o));
    says(o, REPAIRED);
    says(o, "Status:   Stopped");
    says(o, &format!("To continue: codeumbrella resume {id}"));
}

// Group 1: tests that work in a sandbox and outside it.

#[test]
fn status_with_an_id_repairs_a_run_with_a_dead_command() {
    let dir = setup();
    let (id, at) = dead_run(&dir);
    let status = cu(&dir, &["status", &id]);
    shows_repair(&status, &id);
    assert_repaired(&dir, &id, &at);
}

#[test]
fn status_with_no_id_repairs_the_newest_run() {
    let dir = setup();
    let (id, at) = dead_run(&dir);
    let status = cu(&dir, &["status"]);
    shows_repair(&status, &id);
    assert_repaired(&dir, &id, &at);
}

#[test]
fn status_wait_repairs_a_run_with_a_dead_command_and_shows_it() {
    let dir = setup();
    let (id, at) = dead_run(&dir);
    let wait = finish(spawn(&dir, &["status", "--wait", &id]), 20);
    shows_repair(&wait, &id);
    assert_repaired(&dir, &id, &at);
    lacks(&wait, "is dead");
}

#[test]
fn a_second_status_after_the_repair_prints_no_repair_line() {
    let dir = setup();
    let (id, at) = dead_run(&dir);
    assert!(cu(&dir, &["status", &id]).status.success());
    let state = bytes(&dir, &id);
    let again = cu(&dir, &["status", &id]);
    assert!(again.status.success(), "{}", out(&again));
    lacks(&again, REPAIRED);
    says(&again, "Status:   Stopped");
    assert_eq!(bytes(&dir, &id), state);
    assert_repaired(&dir, &id, &at);
}

#[test]
fn resume_repairs_first_and_then_starts_the_same_step_again() {
    let dir = setup();
    let (id, _) = dead_run(&dir);
    let mut value = load(&dir, &id);
    value["at"] = Value::String("one".to_owned());
    save(&dir, &id, &value);
    let before = fs::read_dir(dir.join("bin")).unwrap().count();
    let resume = cu(&dir, &["resume", &id]);
    assert!(resume.status.success(), "{}", out(&resume));
    says(&resume, REPAIRED);
    says(&resume, "Status:   Waiting");
    lacks(&resume, "different command");
    let after = fs::read_dir(dir.join("bin")).unwrap().count();
    assert_eq!(after, before + 1, "the worker of the step did not start");
    assert!(!run_dir(&dir, &id).join("lock").exists());
}

#[test]
fn answer_on_a_waiting_run_with_a_dead_command_removes_the_lock_and_goes_on() {
    let dir = setup();
    let (id, at) = with_lock(&dir, "Waiting", &dead_number());
    let answer = cu(&dir, &["answer", &id, &at, "yes"]);
    assert!(answer.status.success(), "{}", out(&answer));
    let value = load(&dir, &id);
    assert_ne!(text(&value, "status"), "Stopped", "{value}");
    assert!(
        !text(&value, "note").contains("ended at the step"),
        "{value}"
    );
    assert!(!run_dir(&dir, &id).join("lock").exists());
}

#[test]
fn answer_on_a_running_run_repairs_it_and_then_has_no_open_question() {
    let dir = setup();
    let (id, at) = dead_run(&dir);
    let answer = cu(&dir, &["answer", &id, &at, "yes"]);
    assert!(!answer.status.success(), "{}", out(&answer));
    says(&answer, "no open question");
    assert_repaired(&dir, &id, &at);
}

#[test]
fn a_dead_lock_on_a_done_run_is_removed_and_the_notebook_stays() {
    let dir = setup();
    let (id, _) = with_lock(&dir, "Done", &dead_number());
    let state = bytes(&dir, &id);
    let status = cu(&dir, &["status", &id]);
    assert!(status.status.success(), "{}", out(&status));
    assert!(!run_dir(&dir, &id).join("lock").exists());
    assert_eq!(bytes(&dir, &id), state);
}

#[test]
fn status_with_an_empty_lock_changes_no_file() {
    let dir = setup();
    let (id, _) = with_lock(&dir, "Running", "");
    let folder = run_dir(&dir, &id);
    let (state, files) = (bytes(&dir, &id), listing(&folder));
    let status = cu(&dir, &["status", &id]);
    assert!(status.status.success(), "{}", out(&status));
    lacks(&status, REPAIRED);
    assert_eq!(bytes(&dir, &id), state);
    assert_eq!(listing(&folder), files);
    assert!(folder.join("lock").exists());
}

fn ps_fails_and_nothing_changes(args: &[&str]) {
    let dir = setup();
    let mut cleanup = Cleanup::default();
    let pid = sleeper(&mut cleanup);
    let (id, _) = with_lock(&dir, "Running", &pid.to_string());
    tool(&dir, "ps", PS_BAD);
    let folder = run_dir(&dir, &id);
    let (state, files) = (bytes(&dir, &id), listing(&folder));
    let mut words = args.to_vec();
    words.push(&id);
    let o = cu(&dir, &words);
    assert!(!o.status.success(), "{}", out(&o));
    assert!(out(&o).to_lowercase().contains("sandbox"), "{}", out(&o));
    assert_eq!(bytes(&dir, &id), state);
    assert_eq!(listing(&folder), files);
    assert!(alive(pid), "the sleep was ended");
}

#[test]
fn status_with_a_live_number_and_a_ps_that_fails_changes_no_file() {
    ps_fails_and_nothing_changes(&["status"]);
}

#[test]
fn resume_with_a_live_number_and_a_ps_that_fails_changes_no_file() {
    ps_fails_and_nothing_changes(&["resume"]);
}

#[test]
fn the_repair_removes_a_worker_file_with_a_dead_number() {
    let dir = setup();
    let (id, at) = dead_run(&dir);
    let file = format!("worker.{}", dead_number());
    let path = run_dir(&dir, &id).join(&file);
    fs::write(&path, "Mon Jan  1 00:00:00 2001\n").unwrap();
    let status = cu(&dir, &["status", &id]);
    shows_repair(&status, &id);
    assert!(!path.exists(), "the worker file is there");
    assert_repaired(&dir, &id, &at);
}

#[test]
fn the_repair_keeps_a_live_command_with_a_different_start_time() {
    let dir = setup();
    let (id, at) = dead_run(&dir);
    let mut cleanup = Cleanup::default();
    let pid = sleeper(&mut cleanup);
    tool(&dir, "ps", PS_OTHER);
    let path = run_dir(&dir, &id).join(format!("worker.{pid}"));
    fs::write(&path, "Thu Jan  1 00:00:00 1970\n").unwrap();
    let status = cu(&dir, &["status", &id]);
    shows_repair(&status, &id);
    assert!(!path.exists(), "the worker file is there");
    assert!(alive(pid), "the sleep was ended");
    assert_repaired(&dir, &id, &at);
}

#[test]
fn agents_md_names_the_limit_of_2000_lines() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let page = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    assert!(page.contains("`src/lib.rs`): 2000 lines of code."));
    assert!(!page.contains("750 lines of code"));
    assert!(page.contains("Each tool part: 200 lines of code."));
}

#[test]
fn the_chat_command_tells_that_status_wait_repairs_a_dead_run() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file = root.join("skills/codeumbrella/SKILL.md");
    let page = fs::read_to_string(file).unwrap();
    let new = "`status --wait` repairs a run with a dead command and shows Stopped";
    assert!(page.contains(new), "the new text is not there");
    assert!(!page.contains("says that the command is not alive"));
}

// Group 2: tests that use the `ps` and `pkill` of the computer.

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
        if let (Some(entry), Ok(worker)) = (first, worker) {
            let folder = entry.path();
            let marked = folder.join(format!("worker.{worker}")).exists();
            if folder.join("lock").exists() && marked {
                cleanup.pids.push(worker);
                let id = entry.file_name().to_string_lossy().into_owned();
                return (id, runner_pid, worker);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("the worker did not start");
}

/// Waits until the test collects the dead command of the run.
fn collect(cleanup: &mut Cleanup) {
    let end = Instant::now() + Duration::from_secs(5);
    while cleanup.children[0].try_wait().unwrap().is_none() {
        assert!(Instant::now() < end, "the command of the run is alive");
        thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn status_ends_the_worker_that_stays_after_a_hard_kill_of_the_run_command() {
    if skip() {
        return;
    }
    let dir = setup();
    let mut cleanup = Cleanup::default();
    let (id, runner, worker) = active(&dir, &mut cleanup);
    let folder = run_dir(&dir, &id);
    let at = text(&load(&dir, &id), "at");
    let mut kill = Command::new("kill");
    kill.args(["-9", &runner.to_string()]);
    assert!(kill.status().unwrap().success());
    collect(&mut cleanup);
    assert!(alive(worker), "the worker ended with its parent");
    assert!(folder.join("lock").exists());
    let status = finish(spawn(&dir, &["status", &id]), 30);
    shows_repair(&status, &id);
    assert!(dies(worker), "the worker is alive");
    assert!(worker_files(&folder).is_empty(), "a worker file is there");
    assert_repaired(&dir, &id, &at);
}

#[test]
fn stop_on_an_active_run_removes_the_worker_files() {
    if skip() {
        return;
    }
    let dir = setup();
    let mut cleanup = Cleanup::default();
    let (id, _, worker) = active(&dir, &mut cleanup);
    let stop = finish(spawn(&dir, &["stop", &id]), 30);
    assert!(stop.status.success(), "{}", out(&stop));
    collect(&mut cleanup);
    assert!(dies(worker), "the worker is alive");
    let folder = run_dir(&dir, &id);
    assert!(worker_files(&folder).is_empty(), "a worker file is there");
    assert!(!folder.join("lock").exists());
}

#[test]
fn a_worker_file_has_the_start_time_of_the_worker() {
    if skip() {
        return;
    }
    let dir = setup();
    let mut cleanup = Cleanup::default();
    let (id, _, worker) = active(&dir, &mut cleanup);
    let file = run_dir(&dir, &id).join(format!("worker.{worker}"));
    let held = fs::read_to_string(file).unwrap();
    let mut ps = Command::new("ps");
    ps.args(["-o", "lstart=", "-p", &worker.to_string()]);
    let shown = String::from_utf8(ps.output().unwrap().stdout).unwrap();
    assert_ne!(shown.trim(), "");
    assert_eq!(held.trim(), shown.trim());
}

#[test]
fn status_changes_no_file_while_a_live_codeumbrella_has_the_lock() {
    if skip() {
        return;
    }
    let dir = setup();
    let mut cleanup = Cleanup::default();
    let (id, runner, worker) = active(&dir, &mut cleanup);
    let folder = run_dir(&dir, &id);
    let (state, files) = (bytes(&dir, &id), names(&folder));
    let status = finish(spawn(&dir, &["status", &id]), 30);
    assert!(status.status.success(), "{}", out(&status));
    lacks(&status, REPAIRED);
    assert_eq!(bytes(&dir, &id), state);
    assert_eq!(names(&folder), files);
    assert!(folder.join("lock").exists());
    assert!(alive(runner), "the command of the run was ended");
    assert!(alive(worker), "the worker was ended");
}
