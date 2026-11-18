//! Tests of the small repairs of version 0.1.11. They start the real command
//! with a simulated `claude` and `codex`. No worker, no network.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::Graph;
use serde_json::Value;
use std::collections::BTreeSet;
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
fn project() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let dir = env::temp_dir().join(format!("cu-repairs-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
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
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
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
    let line = all.lines().find(|l| l.starts_with("Run: ")).expect("a run");
    line["Run: ".len()..].to_owned()
}

fn says(o: &Output, needle: &str) {
    assert!(out(o).contains(needle), "{}", out(o));
}

fn lacks(o: &Output, needle: &str) {
    assert!(!out(o).contains(needle), "{}", out(o));
}

const CLAUDE: &str = r#"echo called > "$PWD/bin/claude.$$"
echo '{"is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

const CODEX: &str = r#"echo called > "$PWD/bin/codex.$$"
while [ "$1" != "--output-last-message" ]; do shift; done
echo '{"ok":true,"summary":"done"}' > "$2""#;

/// A simulated `claude` that copies the lock file and its parent number.
const CLAUDE_LOCK: &str = r#"cat "$PWD/bin/../.codeumbrella/runs/"*/lock > "$PWD/bin/lockcopy"
echo $PPID > "$PWD/bin/ppid"
echo '{"is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

/// A simulated `claude` that takes 2 seconds.
const CLAUDE_SLOW: &str = r#"sleep 2
echo '{"is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

/// A sheet: copy, a worker, a question, a worker, a question, a worker.
const THREE: &str = r#"start: copy
steps:
  copy:
    type: command
    run:
      - [git, clone, --quiet, --no-hardlinks, ., ".codeumbrella/runs/{run}/work"]
    next: one
  one:
    role: planner
    writes: [spec]
    next: ask1
  ask1:
    type: person
    next: two
  two:
    role: builder
    next: ask2
  ask2:
    type: person
    next: three
  three:
    role: builder
"#;

fn setup() -> PathBuf {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    tool(&dir, "codex", CODEX);
    write_sheet(&dir, THREE);
    dir
}

fn write_sheet(dir: &Path, text: &str) {
    fs::write(dir.join(".codeumbrella/graph.yaml"), text).unwrap();
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

fn saved_tool(dir: &Path, id: &str) -> Option<String> {
    let value = load(dir, id);
    value.get("tool")?.as_str().map(str::to_owned)
}

fn forget_tool(dir: &Path, id: &str) {
    let mut value = load(dir, id);
    value.as_object_mut().unwrap().remove("tool");
    save(dir, id, &value);
    assert_eq!(saved_tool(dir, id), None);
}

fn count(dir: &Path, name: &str) -> usize {
    let prefix = format!("{name}.");
    let mut found = 0;
    for entry in fs::read_dir(dir.join("bin")).unwrap() {
        let file = entry.unwrap().file_name();
        if file.to_string_lossy().starts_with(&prefix) {
            found += 1;
        }
    }
    found
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

// Point 1: the lock holds the process number.

#[test]
fn the_lock_file_holds_the_process_number_of_the_command() {
    let dir = project();
    tool(&dir, "claude", CLAUDE_LOCK);
    write_sheet(&dir, THREE);
    let run = cu(&dir, &["run", "a task"]);
    says(&run, "Status: Waiting");
    let lock = fs::read_to_string(dir.join("bin/lockcopy")).unwrap();
    let parent = fs::read_to_string(dir.join("bin/ppid")).unwrap();
    assert_eq!(lock.trim(), parent.trim(), "the lock: {lock:?}");
    // The lock goes when the command ends.
    assert!(!run_dir(&dir, &id(&run)).join("lock").exists());
}

// Point 2: `status --wait`.

#[test]
fn status_wait_with_a_run_that_is_not_running_prints_the_status_text_at_once() {
    let dir = setup();
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    let before = bytes(&dir, &id);
    let plain = cu(&dir, &["status", &id]);
    let wait = finish(spawn(&dir, &["status", "--wait", &id]), 20);
    assert!(wait.status.success(), "{}", out(&wait));
    assert_eq!(out(&wait), out(&plain));
    assert_eq!(bytes(&dir, &id), before);
}

/// Waits until the one run of the project has a lock file.
fn locked_run(dir: &Path) -> String {
    let runs = dir.join(".codeumbrella/runs");
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let first = fs::read_dir(&runs).ok().and_then(|mut d| d.next());
        let entry = first.and_then(Result::ok);
        if let Some(entry) = entry.filter(|e| e.path().join("lock").exists()) {
            return entry.file_name().to_string_lossy().into_owned();
        }
        assert!(Instant::now() < end, "the run did not start");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn status_wait_waits_for_a_live_run_and_then_prints_the_status_text() {
    let dir = setup();
    tool(&dir, "claude", CLAUDE_SLOW);
    let runner = spawn(&dir, &["run", "a task"]);
    let id = locked_run(&dir);
    let start = Instant::now();
    let wait = finish(spawn(&dir, &["status", "--wait", &id]), 30);
    let waited = start.elapsed();
    assert!(wait.status.success(), "{}", out(&wait));
    says(&wait, "Status: Waiting");
    assert!(waited >= Duration::from_millis(500), "it did not wait");
    let runner = finish(runner, 30);
    assert!(runner.status.success(), "{}", out(&runner));
    let plain = cu(&dir, &["status", &id]);
    assert_eq!(out(&wait), out(&plain));
}

#[test]
fn status_wait_stops_with_a_message_when_the_command_of_the_run_is_dead() {
    let dir = setup();
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    // A process that is gone: it ended and its parent collected it.
    let mut gone = Command::new("true").spawn().unwrap();
    gone.wait().unwrap();
    set_status(&dir, &id, "Running");
    let lock = run_dir(&dir, &id).join("lock");
    fs::write(&lock, gone.id().to_string()).unwrap();
    let before = bytes(&dir, &id);
    let wait = finish(spawn(&dir, &["status", "--wait", &id]), 20);
    assert!(!wait.status.success(), "{}", out(&wait));
    let text = out(&wait).to_lowercase();
    assert!(text.contains("dead"), "{text}");
    assert!(text.contains("lock"), "{text}");
    assert!(text.contains("remove"), "{text}");
    // It never changes the notebook and never removes the lock.
    assert_eq!(bytes(&dir, &id), before);
    assert!(lock.exists(), "the lock was removed");
    // A second call does the same, so nothing changed.
    let again = finish(spawn(&dir, &["status", "--wait", &id]), 20);
    assert!(!again.status.success());
}

#[test]
fn status_wait_does_not_stop_when_the_command_of_the_run_is_alive() {
    let dir = setup();
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    // The test itself is alive: its number is in the lock.
    set_status(&dir, &id, "Running");
    let lock = run_dir(&dir, &id).join("lock");
    fs::write(&lock, std::process::id().to_string()).unwrap();
    let mut wait = spawn(&dir, &["status", "--wait", &id]);
    thread::sleep(Duration::from_millis(1500));
    assert!(wait.try_wait().unwrap().is_none(), "it stopped too early");
    // The run ends: the notebook says Done and the lock goes.
    set_status(&dir, &id, "Done");
    fs::remove_file(&lock).unwrap();
    let wait = finish(wait, 20);
    assert!(wait.status.success(), "{}", out(&wait));
    says(&wait, "Status: Done");
}

/// The names of all files in a folder, with their sizes.
fn listing(dir: &Path) -> Vec<(String, u64)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let size = entry.metadata().unwrap().len();
        found.push((entry.file_name().to_string_lossy().into_owned(), size));
    }
    found.sort();
    found
}

#[test]
fn status_wait_stops_when_the_run_says_running_and_has_no_lock() {
    let dir = setup();
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    set_status(&dir, &id, "Running");
    let folder = run_dir(&dir, &id);
    assert!(!folder.join("lock").exists());
    let before = bytes(&dir, &id);
    let files = listing(&folder);
    let start = Instant::now();
    let wait = finish(spawn(&dir, &["status", "--wait", &id]), 20);
    assert!(!wait.status.success(), "{}", out(&wait));
    // It waits about 2 seconds with no lock before it stops.
    assert!(start.elapsed() >= Duration::from_millis(1500), "too early");
    let text = out(&wait).to_lowercase();
    assert!(text.contains("running"), "{text}");
    assert!(text.contains("no command"), "{text}");
    let resume = format!("codeumbrella resume {id}");
    assert!(text.contains(&resume), "{text}");
    assert!(text.contains("yes"), "{text}");
    // It changes no file and makes no lock.
    assert_eq!(bytes(&dir, &id), before);
    assert_eq!(listing(&folder), files);
    assert!(!folder.join("lock").exists());
}

#[test]
fn status_wait_starts_its_count_again_when_the_lock_comes_back() {
    let dir = setup();
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    set_status(&dir, &id, "Running");
    let lock = run_dir(&dir, &id).join("lock");
    let start = Instant::now();
    let mut wait = spawn(&dir, &["status", "--wait", &id]);
    // No lock for 1.5 seconds, a live lock for 1 second, then no lock.
    thread::sleep(Duration::from_millis(1500));
    fs::write(&lock, std::process::id().to_string()).unwrap();
    thread::sleep(Duration::from_millis(1000));
    fs::remove_file(&lock).unwrap();
    // The first absence would have ended at 2 seconds. The count is new.
    thread::sleep(Duration::from_millis(1100));
    assert!(wait.try_wait().unwrap().is_none(), "the count went on");
    let wait = finish(wait, 20);
    assert!(!wait.status.success(), "{}", out(&wait));
    assert!(start.elapsed() >= Duration::from_millis(4000), "too early");
    says(&wait, &format!("codeumbrella resume {id}"));
}

// Point 3: a worker folder that is not there or is empty.

const ONE_WORKER: &str = r"start: one
steps:
  one:
    role: planner
    writes: [spec]
";

const REMOVE_WORK: &str = r#"start: gone
steps:
  gone:
    type: command
    run:
      - [rm, -r, ".codeumbrella/runs/{run}/work"]
    next: one
  one:
    role: planner
    writes: [spec]
"#;

#[test]
fn a_worker_step_stops_the_run_when_the_work_folder_is_empty() {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    write_sheet(&dir, ONE_WORKER);
    let run = cu(&dir, &["run", "a task"]);
    says(&run, "Status: Stopped");
    says(&run, "0.1.2");
    says(&run, "codeumbrella init --update");
    assert_eq!(count(&dir, "claude"), 0, "a worker started");
}

#[test]
fn a_worker_step_stops_the_run_when_the_work_folder_is_not_there() {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    write_sheet(&dir, REMOVE_WORK);
    let run = cu(&dir, &["run", "a task"]);
    says(&run, "Status: Stopped");
    says(&run, "0.1.2");
    says(&run, "codeumbrella init --update");
    assert_eq!(count(&dir, "claude"), 0, "a worker started");
}

#[test]
fn a_worker_step_with_a_filled_work_folder_starts_the_worker() {
    let dir = setup();
    let run = cu(&dir, &["run", "a task"]);
    says(&run, "Status: Waiting");
    assert_eq!(count(&dir, "claude"), 1);
}

// Point 4: run ids.

const TRUE_SHEET: &str = r"start: a
steps:
  a:
    type: command
    run: [[true]]
";

#[test]
fn runs_that_start_in_the_same_second_get_different_ids() {
    let dir = project();
    write_sheet(&dir, TRUE_SHEET);
    // All four commands start at once, so they meet in one second.
    let children: Vec<Child> = (0..4).map(|_| spawn(&dir, &["run", "a task"])).collect();
    let mut ids = BTreeSet::new();
    for child in children {
        let run = finish(child, 60);
        says(&run, "Status: Done");
        ids.insert(id(&run));
    }
    assert_eq!(ids.len(), 4, "{ids:?}");
    for id in &ids {
        assert!(run_dir(&dir, id).join("state.json").is_file());
    }
}

#[test]
fn run_stops_with_a_message_when_it_cannot_make_the_run_folder() {
    let dir = project();
    write_sheet(&dir, TRUE_SHEET);
    let runs = dir.join(".codeumbrella/runs");
    fs::create_dir_all(&runs).unwrap();
    fs::set_permissions(&runs, fs::Permissions::from_mode(0o555)).unwrap();
    // A person with full rights (for example root) can write anyway.
    let can_write = fs::create_dir(runs.join("probe")).is_ok();
    let run = (!can_write).then(|| spawn(&dir, &["run", "a task"]));
    let result = run.map(|child| finish(child, 20));
    fs::set_permissions(&runs, fs::Permissions::from_mode(0o755)).unwrap();
    let Some(run) = result else { return };
    assert!(!run.status.success(), "{}", out(&run));
    says(&run, "runs");
    assert!(out(&run).to_lowercase().contains("permission denied"));
}

// Point 5: the tool goes into the notebook only after the sheet is good.

#[test]
fn a_bad_sheet_does_not_write_the_tool_into_a_notebook_with_no_tool() {
    let dir = setup();
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    forget_tool(&dir, &id);
    write_sheet(&dir, "this: [is: not a sheet");
    let answer = cu(&dir, &["--tool", "codex", "answer", &id, "ask1", "yes"]);
    assert!(!answer.status.success(), "{}", out(&answer));
    assert_eq!(saved_tool(&dir, &id), None, "the tool was written");
    let resume = cu(&dir, &["--tool", "codex", "resume", &id]);
    assert!(!resume.status.success(), "{}", out(&resume));
    assert_eq!(saved_tool(&dir, &id), None, "the tool was written");
}

// Point 6: a role file that is not there.

#[test]
fn a_missing_rules_page_says_to_run_init_update() {
    let dir = setup();
    fs::remove_file(dir.join(".codeumbrella/roles/builder.md")).unwrap();
    let run = cu(&dir, &["run", "a task"]);
    assert!(!run.status.success(), "{}", out(&run));
    says(&run, "codeumbrella init --update");
}

fn missing_settings(args: &[&str], file: &str) {
    let dir = setup();
    fs::remove_file(dir.join(".codeumbrella/roles").join(file)).unwrap();
    let run = cu(&dir, args);
    says(&run, "Status: Stopped");
    says(&run, "codeumbrella init --update");
}

#[test]
fn a_missing_settings_file_says_to_run_init_update_for_claude() {
    missing_settings(&["run", "a task"], "planner.claude.json");
}

#[test]
fn a_missing_settings_file_says_to_run_init_update_for_codex() {
    let args = ["--tool", "codex", "run", "a task"];
    missing_settings(&args, "planner.codex.txt");
}

// Point 7: the stop for a changed paper.

/// A sheet that changes the paper `spec` after its writer left it.
const TAMPER: &str = r#"start: copy
steps:
  copy:
    type: command
    run:
      - [git, clone, --quiet, --no-hardlinks, ., ".codeumbrella/runs/{run}/work"]
    next: one
  one:
    role: planner
    writes: [spec]
    next: tamper
  tamper:
    type: command
    run:
      - [touch, ".codeumbrella/runs/{run}/papers/spec/extra.md"]
    next: two
  two:
    role: builder
    reads: [spec]
"#;

fn puts_paper_back(o: &Output) {
    let text = out(o).to_lowercase();
    assert!(text.contains("put the paper back"), "{text}");
}

#[test]
fn a_stop_for_a_changed_paper_has_no_resume_line_and_says_to_put_the_paper_back() {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    write_sheet(&dir, TAMPER);
    let run = cu(&dir, &["run", "a task"]);
    says(&run, "Status: Stopped");
    says(&run, "not as its writer left it");
    lacks(&run, "To continue:");
    puts_paper_back(&run);
    let id = id(&run);
    // `status` and a `resume` that stops again print the same.
    for args in [["status", id.as_str()], ["resume", id.as_str()]] {
        let again = cu(&dir, &args);
        says(&again, "Status: Stopped");
        lacks(&again, "To continue:");
        puts_paper_back(&again);
    }
}

const FALSE_SHEET: &str = r"start: a
steps:
  a:
    type: command
    run: [[false]]
";

#[test]
fn a_stop_for_a_different_cause_keeps_the_two_resume_lines() {
    let dir = project();
    write_sheet(&dir, FALSE_SHEET);
    let run = cu(&dir, &["run", "a task"]);
    let line = format!("To continue: codeumbrella resume {}\n", id(&run));
    says(&run, &line);
    let rule = "Resume only after the person says yes.";
    says(&run, &format!("{rule} An AI must not give this approval."));
}

// Point 8: a notebook with no tool and a command that uses Claude.

const NO_TOOL_LINE: &str = "This run has no tool, so the command uses Claude.";

#[test]
fn a_command_that_uses_claude_on_a_notebook_with_no_tool_says_so_in_one_line() {
    let dir = setup();
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    lacks(&run, NO_TOOL_LINE);
    forget_tool(&dir, &id);
    let answer = cu(&dir, &["answer", &id, "ask1", "yes"]);
    says(&answer, "Status: Waiting");
    let text = out(&answer);
    let lines = text.lines().filter(|l| *l == NO_TOOL_LINE).count();
    assert_eq!(lines, 1, "{text}");
    // The tool of the notebook is still not set by this command.
    forget_tool(&dir, &id);
    // A command with `--tool` writes the tool, so it says nothing.
    let answer = cu(&dir, &["--tool", "codex", "answer", &id, "ask2", "yes"]);
    lacks(&answer, NO_TOOL_LINE);
    assert_eq!(saved_tool(&dir, &id).as_deref(), Some("codex"));
}

#[test]
fn status_and_a_notebook_with_a_tool_print_no_line_about_the_tool() {
    let dir = setup();
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    let answer = cu(&dir, &["answer", &id, "ask1", "yes"]);
    lacks(&answer, NO_TOOL_LINE);
    forget_tool(&dir, &id);
    lacks(&cu(&dir, &["status", &id]), NO_TOOL_LINE);
}

// Point 10: tests that must be there.

#[test]
fn status_with_a_different_tool_shows_the_run_and_does_not_change_the_notebook() {
    // Decision 1 in `spec.md`: `status` ignores `--tool`. It gives no error.
    let dir = setup();
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    let id = id(&run);
    let before = bytes(&dir, &id);
    let status = cu(&dir, &["--tool", "claude", "status", &id]);
    assert!(status.status.success(), "{}", out(&status));
    says(&status, "Status: Waiting");
    assert_eq!(bytes(&dir, &id), before);
}

/// A worker that gives a bad result for the step `simplify`.
struct BadSimplify;

impl Worker for BadSimplify {
    fn run(&self, job: &Job) -> Result<Outcome> {
        let papers = job.dir.join("papers");
        for paper in job.writes {
            fs::write(papers.join(paper).join("file.md"), job.step)?;
        }
        Ok(Outcome {
            ok: job.step != "simplify",
            summary: format!("{} ran", job.step),
        })
    }
}

#[test]
fn a_bad_result_of_the_step_simplify_stops_the_run() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let roles = root.join("roles");
    let mut graph = Graph::load(&root.join("graph.example.yaml"), &roles).unwrap();
    for step in graph.steps.values_mut().filter(|s| !s.run.is_empty()) {
        step.run = vec![vec!["true".to_owned()]];
    }
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let dir = env::temp_dir().join(format!("cu-simplify-{}-{n}", std::process::id()));
    let mut run = Run::start(dir.clone(), "a task", &graph).unwrap();
    // The folder of the workers is not empty.
    fs::write(dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, &BadSimplify).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &BadSimplify).unwrap();
    assert_eq!(run.state.status, Status::Stopped);
    assert_eq!(run.state.at.as_deref(), Some("simplify"));
    assert!(run.state.note.contains("simplify"), "{}", run.state.note);
    assert!(!run.state.results.contains_key("check_tests"));
}
