//! Tests of two runs at the same time: `run --next` with an active run, the
//! line `Run:` at the start, the steps `take_master`, `fix_merge`,
//! `same_master`, `work_was_done` and `do_work`, and the documents. The
//! worker is simulated. A bare git folder in the temp folder is the server
//! copy. No Claude, no Codex and no `gh`.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::{Graph, Kind};
use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

const QUEUE: &str = ".codeumbrella/queue";
const RUNS: &str = ".codeumbrella/runs";
const SKILL: &str = "skills/codeumbrella/SKILL.md";
const BRANCH: &str = "codeumbrella/run-1";
const MASTER: &str = "refs/remotes/origin/master";
const NEW_FILE: &str = "from-master.txt";
const DATE: &str = "20 November 2026";

/// The fan parts. They run at the same time, so their sequence is not set.
const PARTS: [&str; 5] = [
    "check_tests",
    "check_safety",
    "check_review",
    "check_security",
    "check_use",
];

/// The steps of the default sheet that run their commands in a test.
const REAL: [&str; 5] = [
    "mark",
    "work_was_done",
    "take_master",
    "same_master",
    "no_locked_files",
];

/// The three options of each git call of `take_master`.
const OPTIONS: [&str; 3] = [
    "core.fsmonitor=false",
    "core.hooksPath=/dev/null",
    "commit.gpgSign=false",
];

/// The hooks that a merge or a fetch can start.
const HOOKS: [&str; 6] = [
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-merge",
    "post-checkout",
    "reference-transaction",
];

/// A sheet with a person step first. A run of it waits at once.
const SHEET: &str = r#"start: look
steps:
  look:
    type: person
    next: done
  done:
    type: command
    run: [["true"]]
"#;

/// A sheet with one command step. The step copies the output of the command
/// of that moment into `seen.txt`.
const SEE: &str = r#"start: see
steps:
  see:
    type: command
    run: [[sh, -c, "cat out.txt > seen.txt"]]
"#;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let read = fs::read_to_string(root().join(rel));
    read.unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn default_sheet() -> Graph {
    let roles = root().join("roles");
    Graph::load(&root().join("graph.example.yaml"), &roles).unwrap()
}

/// A new empty folder. The path is the full one: on a Mac,
/// `/private/var/...` and not `/var/...`.
fn folder() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-same-{}-{n}", std::process::id());
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

/// Starts the built command in `dir`.
fn cu(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(args).current_dir(dir);
    command.output().unwrap()
}

fn all(out: &Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    format!("{stdout}{stderr}")
}

/// Runs `git` in a folder, with a name and an email. Gives the output text.
fn git(dir: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    command.args(["-c", "user.name=t", "-c", "user.email=t@t"]);
    command.args(["-c", "commit.gpgsign=false"]);
    let out = command.args(args).output().unwrap();
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "git {args:?}: {error}");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Runs `git` in a folder. Gives `true` if git gave no error.
fn git_ok(dir: &Path, args: &[&str]) -> bool {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir).args(args);
    command.stderr(Stdio::null()).stdout(Stdio::null());
    command.status().unwrap().success()
}

/// The commit of `HEAD` in a folder.
fn head(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"])
}

/// Tells if the commit `tip` in a folder has the commit `commit` in it.
fn has(dir: &Path, commit: &str, tip: &str) -> bool {
    git_ok(dir, &["merge-base", "--is-ancestor", commit, tip])
}

/// The names in a folder, in sequence.
fn names_in(dir: &Path) -> Vec<String> {
    let found = fs::read_dir(dir).unwrap();
    let mut names: Vec<String> = found
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------
// `run --next` while a different run is active
// ---------------------------------------------------------------------

/// A project after `init`, with the sheet that waits at once.
fn project() -> PathBuf {
    let dir = folder();
    let out = cu(&dir, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    fs::write(dir.join(".codeumbrella/graph.yaml"), SHEET).unwrap();
    dir
}

/// Puts a task file into the queue of the project.
fn put(dir: &Path, name: &str, body: &str) {
    let queue = dir.join(QUEUE);
    fs::create_dir_all(&queue).unwrap();
    fs::write(queue.join(name), body).unwrap();
}

/// The file names in the queue, in sequence.
fn queued(dir: &Path) -> Vec<String> {
    names_in(&dir.join(QUEUE))
}

/// The id of the run that an output names.
fn id(out: &Output) -> String {
    let text = all(out);
    let line = text.lines().find(|l| l.starts_with("Run:      "));
    let line = line.unwrap_or_else(|| panic!("no run: {text}"));
    line["Run:      ".len()..].to_owned()
}

fn run_dir(dir: &Path, id: &str) -> PathBuf {
    dir.join(RUNS).join(id)
}

/// The names of the run folders of the project. A new project has none.
fn run_folders(dir: &Path) -> Vec<String> {
    let found = fs::read_dir(dir.join(RUNS)).into_iter().flatten();
    let mut names: Vec<String> = found
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The status of each run of the project that has a notebook.
fn statuses(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(runs) = fs::read_dir(dir.join(RUNS)) else {
        return found;
    };
    for entry in runs {
        let path = entry.unwrap().path().join("state.json");
        if let Ok(json) = fs::read_to_string(path) {
            let value: Value = serde_json::from_str(&json).unwrap();
            found.push(value["status"].as_str().unwrap().to_owned());
        }
    }
    found
}

/// The count of the runs with the status `Running` or `Waiting`.
fn active(dir: &Path) -> usize {
    let mut count = 0;
    for status in statuses(dir) {
        if status == "Running" || status == "Waiting" {
            count += 1;
        }
    }
    count
}

/// Starts `run --next` in `dir` and does not wait for its end.
fn start_next(dir: &Path) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(["run", "--next"]).current_dir(dir);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.spawn().unwrap()
}

#[test]
fn run_next_starts_while_a_different_run_waits() {
    let dir = project();
    let first = cu(&dir, &["run", "a task"]);
    let shown = all(&first);
    assert!(shown.contains("Status:   Waiting"), "{shown}");
    let waiting = id(&first);
    let notebook = run_dir(&dir, &waiting).join("state.json");
    let before = fs::read(&notebook).unwrap();
    // An empty queue stays an error, and it starts no run.
    let empty = cu(&dir, &["run", "--next"]);
    assert!(!empty.status.success(), "{}", all(&empty));
    let said = all(&empty).to_lowercase();
    assert!(said.contains("empty"), "{said}");
    assert_eq!(statuses(&dir).len(), 1, "a run started");
    // With a task in the queue, the second run starts.
    put(&dir, "01-next.txt", "Next task");
    let out = cu(&dir, &["run", "--next"]);
    assert!(out.status.success(), "{}", all(&out));
    assert!(all(&out).contains("Status:   Waiting"), "{}", all(&out));
    let new = id(&out);
    assert_ne!(new, waiting);
    let moved = run_dir(&dir, &new).join("task.txt");
    assert_eq!(fs::read_to_string(moved).unwrap(), "Next task");
    assert!(queued(&dir).is_empty(), "the task is in the queue");
    assert_eq!(active(&dir), 2, "{:?}", statuses(&dir));
    assert_eq!(fs::read(&notebook).unwrap(), before);
}

#[test]
fn run_next_starts_while_a_different_run_is_running() {
    let dir = project();
    let first = cu(&dir, &["run", "a task"]);
    let running = id(&first);
    let notebook = run_dir(&dir, &running).join("state.json");
    let json = fs::read_to_string(&notebook).unwrap();
    let changed = json.replace("\"Waiting\"", "\"Running\"");
    assert_ne!(json, changed, "no status Waiting in the notebook");
    fs::write(&notebook, &changed).unwrap();
    put(&dir, "01-next.txt", "Next task");
    let out = cu(&dir, &["run", "--next"]);
    assert!(out.status.success(), "{}", all(&out));
    assert_ne!(id(&out), running);
    assert!(queued(&dir).is_empty(), "the task is in the queue");
    assert_eq!(active(&dir), 2, "{:?}", statuses(&dir));
    assert_eq!(fs::read_to_string(&notebook).unwrap(), changed);
}

#[test]
fn a_run_next_that_cannot_move_the_task_file_makes_no_run() {
    let dir = project();
    put(&dir, "01-a.txt", "A task");
    let queue = dir.join(QUEUE);
    // A folder that is read-only refuses the move of its file.
    let closed = fs::Permissions::from_mode(0o555);
    fs::set_permissions(&queue, closed).unwrap();
    let out = cu(&dir, &["run", "--next"]);
    let open = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&queue, open).unwrap();
    assert!(!out.status.success(), "{}", all(&out));
    assert_eq!(queued(&dir), ["01-a.txt"]);
    assert_eq!(active(&dir), 0, "{:?}", statuses(&dir));
    // The command removed its run folder.
    let kept = run_folders(&dir);
    assert!(kept.is_empty(), "a run folder stays: {kept:?}");
    // With the folder open again, the same command starts the run.
    let out = cu(&dir, &["run", "--next"]);
    assert!(out.status.success(), "{}", all(&out));
    assert_eq!(active(&dir), 1, "{:?}", statuses(&dir));
    assert_eq!(run_folders(&dir).len(), 1);
}

#[test]
fn two_run_next_commands_for_one_task_file_start_one_run() {
    let dir = project();
    let first = cu(&dir, &["run", "a task"]);
    let waiting = run_dir(&dir, &id(&first));
    let before = fs::read(waiting.join("state.json")).unwrap();
    put(&dir, "01-only.txt", "The only task");
    let mut children = Vec::new();
    for _ in 0..6 {
        children.push(start_next(&dir));
    }
    let mut good = 0;
    for child in children {
        let out = child.wait_with_output().unwrap();
        good += u32::from(out.status.success());
    }
    assert_eq!(good, 1, "one command must get the task");
    assert!(queued(&dir).is_empty(), "the task is in the queue");
    // The run from before and the new run. No third run is active.
    assert_eq!(active(&dir), 2, "{:?}", statuses(&dir));
    let after = fs::read(waiting.join("state.json")).unwrap();
    assert_eq!(after, before, "the other run changed");
    assert!(!waiting.join("lock").exists(), "a lock in the other run");
    // A command that did not get the task removed its run folder.
    let kept = run_folders(&dir);
    assert_eq!(kept.len(), 2, "{kept:?}");
    assert_eq!(statuses(&dir).len(), 2, "a folder with no notebook");
    // So `status` with no id shows a run and gives no error.
    let shown = cu(&dir, &["status"]);
    assert!(shown.status.success(), "{}", all(&shown));
}

#[test]
fn run_prints_the_run_line_before_the_first_step() {
    let dir = folder();
    let out = cu(&dir, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    fs::write(dir.join(".codeumbrella/graph.yaml"), SEE).unwrap();
    let file = fs::File::create(dir.join("out.txt")).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(["run", "a task"]).current_dir(&dir);
    command.stderr(Stdio::null());
    let status = command.stdout(file).status().unwrap();
    assert!(status.success(), "the run gave an error");
    let printed = fs::read_to_string(dir.join("out.txt")).unwrap();
    assert!(printed.contains("Status:   Done"), "{printed}");
    // The first step saw the line with the id of the run.
    let names = names_in(&dir.join(RUNS));
    assert_eq!(names.len(), 1, "{names:?}");
    let want = format!("Run:      {}", names[0]);
    let seen = fs::read_to_string(dir.join("seen.txt")).unwrap();
    assert!(seen.lines().any(|l| l == want), "{seen}");
}

// ---------------------------------------------------------------------
// The default sheet
// ---------------------------------------------------------------------

/// The step after a good result of the step `name`.
fn next(graph: &Graph, name: &str) -> String {
    let step = graph.step(name).unwrap();
    step.next.clone().unwrap_or_default()
}

/// The step after a bad result of the step `name`.
fn on_bad(graph: &Graph, name: &str) -> String {
    let step = graph.step(name).unwrap();
    step.on_bad.clone().unwrap_or_default()
}

/// The names of the steps that go to `to`, by `next` or by `on_bad`.
fn ways_to(graph: &Graph, to: &str) -> Vec<String> {
    let target = Some(to.to_owned());
    let mut found = Vec::new();
    for (name, step) in &graph.steps {
        if step.next == target || step.on_bad == target {
            found.push(name.clone());
        }
    }
    found
}

/// Each word of each command of a step, in one text.
fn words(graph: &Graph, name: &str) -> String {
    let step = graph.step(name).unwrap();
    let lines: Vec<String> = step.run.iter().map(|c| c.join(" ")).collect();
    lines.join("\n")
}

#[test]
fn the_default_sheet_has_take_master_between_simplify_and_learn() {
    let graph = default_sheet();
    assert_eq!(next(&graph, "simplify"), "work_was_done");
    assert_eq!(next(&graph, "work_was_done"), "take_master");
    assert_eq!(next(&graph, "take_master"), "learn");
    assert_eq!(on_bad(&graph, "take_master"), "fix_merge");
    let step = graph.step("take_master").unwrap();
    assert_eq!(step.kind, Kind::Command);
    assert_eq!(step.max_tries, Some(3));
    assert_eq!(ways_to(&graph, "learn"), ["take_master"]);
    let into = ["fix_merge", "same_master", "work_was_done"];
    assert_eq!(ways_to(&graph, "take_master"), into);
}

#[test]
fn fix_merge_and_do_work_are_builder_steps_with_a_next_step() {
    let graph = default_sheet();
    // `do_work` goes back to the check, so a builder with no commit
    // does not get to the PR question.
    assert_eq!(next(&graph, "do_work"), "work_was_done");
    for name in ["fix_merge", "do_work"] {
        let step = graph.step(name).unwrap();
        assert_eq!(step.kind, Kind::Worker, "{name}");
        assert_eq!(step.role.as_deref(), Some("builder"), "{name}");
        assert_eq!(step.reads, ["spec"], "{name}");
        assert!(step.writes.is_empty(), "{name}");
        assert_eq!(step.max_tries, Some(3), "{name}");
        assert_eq!(step.on_bad.as_deref(), Some("plan"), "{name}");
    }
    assert_eq!(next(&graph, "fix_merge"), "take_master");
    assert_eq!(ways_to(&graph, "fix_merge"), ["take_master"]);
    assert_eq!(ways_to(&graph, "do_work"), ["work_was_done"]);
    let into = ["do_work", "simplify"];
    assert_eq!(ways_to(&graph, "work_was_done"), into);
}

#[test]
fn the_default_sheet_has_same_master_between_approval_and_merge() {
    let graph = default_sheet();
    assert_eq!(next(&graph, "approval"), "same_master");
    assert_eq!(on_bad(&graph, "approval"), "plan");
    assert_eq!(next(&graph, "same_master"), "merge");
    assert_eq!(on_bad(&graph, "same_master"), "take_master");
    let step = graph.step("same_master").unwrap();
    assert_eq!(step.kind, Kind::Command);
    assert_eq!(ways_to(&graph, "merge"), ["same_master"]);
    assert_eq!(ways_to(&graph, "same_master"), ["approval"]);
    // The wall: only the yes of the person gets to the merge.
    let approval = graph.step("approval").unwrap();
    assert_eq!(approval.kind, Kind::Person);
    assert_ne!(graph.start, "same_master");
    assert_ne!(graph.start, "merge");
    // `same_master` only compares. It does not merge and does not push.
    let said = words(&graph, "same_master");
    for word in ["gh", "push"] {
        assert!(!has_word(&said, word), "{word}: {said}");
    }
}

#[test]
fn work_was_done_comes_after_simplify_and_check_goes_to_the_locked_files() {
    let graph = default_sheet();
    assert_eq!(on_bad(&graph, "work_was_done"), "do_work");
    assert_eq!(next(&graph, "do_work"), "work_was_done");
    assert_eq!(next(&graph, "check"), "no_locked_files");
    assert_eq!(on_bad(&graph, "check"), "repair");
    assert_eq!(next(&graph, "repair"), "write_pr");
    assert_eq!(ways_to(&graph, "no_locked_files"), ["check"]);
}

#[test]
fn the_push_stays_plain_and_take_master_has_no_rebase() {
    let graph = default_sheet();
    let push = graph.step("push").unwrap();
    for word in push.run.iter().flatten() {
        let forced = word.starts_with("--force") || word == "-f";
        assert!(!forced && !word.starts_with('+'), "{word}");
    }
    let merge = words(&graph, "take_master");
    for word in ["rebase", "--force", "push"] {
        assert!(!merge.contains(word), "{word}: {merge}");
    }
    for word in ["--no-edit", "--no-verify", "/clean"] {
        assert!(merge.contains(word), "{word}: {merge}");
    }
}

#[test]
fn each_git_command_of_take_master_has_the_three_options() {
    let graph = default_sheet();
    let step = graph.step("take_master").unwrap();
    let lines = step.run.iter().map(|c| c.join(" "));
    let mut count = 0;
    for line in lines.filter(|l| l.contains("git")) {
        count += 1;
        for option in OPTIONS {
            assert!(line.contains(option), "{option}: {line}");
        }
    }
    assert!(count > 0, "take_master has no git command");
}

// ---------------------------------------------------------------------
// `take_master` and `same_master`, with a server copy in the temp folder
// ---------------------------------------------------------------------

/// A server copy, a project folder with that `origin`, and a run with a
/// work copy on the branch of the run.
struct World {
    base: PathBuf,
    server: PathBuf,
    project: PathBuf,
    work: PathBuf,
    run: Run,
}

/// Clones a git folder with no hooks and no questions.
fn clone(from: &Path, to: &Path, more: &[&str]) {
    let mut command = Command::new("git");
    command.args(["clone", "--quiet"]).args(more);
    let out = command.arg(from).arg(to).output().unwrap();
    assert!(out.status.success(), "{}", all(&out));
}

fn world() -> World {
    let base = folder();
    let server = base.join("server.git");
    let project = base.join("project");
    fs::create_dir_all(&server).unwrap();
    fs::create_dir_all(&project).unwrap();
    let branch = "--initial-branch=master";
    git(&server, &["init", "--quiet", "--bare", branch]);
    git(&project, &["init", "--quiet", branch]);
    fs::write(project.join("file"), "one\ntwo\nthree\n").unwrap();
    git(&project, &["add", "-A"]);
    git(&project, &["commit", "--quiet", "-m", "first"]);
    let url = server.display().to_string();
    git(&project, &["remote", "add", "origin", &url]);
    git(&project, &["push", "--quiet", "origin", "master"]);
    git(&project, &["fetch", "--quiet", "origin"]);
    let folder = project.join(RUNS).join("run-1");
    let run = Run::start(folder, "a task", &default_sheet()).unwrap();
    // The step `copy` clones the project folder into `work`.
    let work = run.dir.join("work");
    clone(&project, &work, &["--no-hardlinks"]);
    git(&work, &["config", "user.name", "t"]);
    git(&work, &["config", "user.email", "t@t"]);
    git(&work, &["checkout", "--quiet", "-b", BRANCH]);
    World {
        base,
        server,
        project,
        work,
        run,
    }
}

/// Makes a commit with this file in a git folder. Gives the commit.
fn commit(dir: &Path, name: &str, body: &str) -> String {
    fs::write(dir.join(name), body).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "--quiet", "-m", name]);
    head(dir)
}

/// Makes a commit with this file on `master` of a bare folder, as a
/// different run or a person does. Gives the commit.
fn push_to(base: &Path, server: &Path, name: &str, body: &str) -> String {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let other = base.join(format!("other-{n}"));
    clone(server, &other, &[]);
    let tip = commit(&other, name, body);
    git(&other, &["push", "--quiet", "origin", "HEAD:master"]);
    tip
}

/// A command that makes one commit with a new line in `version.txt`.
fn version_command(runs: &str) -> Vec<String> {
    let work = format!("{runs}/run-1/work");
    let add = "echo v >> version.txt && git add version.txt";
    let commit = "git -c commit.gpgSign=false commit -q -m version";
    let text = format!("cd {work} && {add} && {commit}");
    vec!["sh".to_owned(), "-c".to_owned(), text]
}

/// The default sheet for a run in `project`. The steps of `REAL` run their
/// commands there. `set_version` makes one commit in the work copy. Each
/// other command step does nothing.
fn sheet(project: &Path) -> Graph {
    let mut graph = default_sheet();
    let runs = format!("{}/{RUNS}", project.display());
    let full = format!("{runs}/{{run}}");
    for (name, step) in &mut graph.steps {
        if REAL.contains(&name.as_str()) {
            for word in step.run.iter_mut().flatten() {
                *word = word.replace(".codeumbrella/runs/{run}", &full);
            }
        } else if name == "set_version" {
            step.run = vec![version_command(&runs)];
        } else if !step.run.is_empty() {
            step.run = vec![vec!["true".to_owned()]];
        }
    }
    graph
}

/// Does one step of the default sheet and no step after it. Gives `true`
/// for a good result. The note of the run has the error of a bad result.
fn alone(w: &mut World, name: &str) -> bool {
    let mut graph = sheet(&w.project);
    let step = graph.steps.get_mut(name);
    let step = step.unwrap_or_else(|| panic!("no step {name}"));
    step.next = None;
    step.on_bad = None;
    w.run.state.at = Some(name.to_owned());
    w.run.state.tries.clear();
    let roles = root().join("roles");
    w.run.go(&graph, &roles, &Sim::new(&[])).unwrap();
    w.run.state.status == Status::Done
}

/// A script in `base` that makes the file `name` in `base`. Gives the path
/// of the script and of that file.
fn marker(base: &Path, name: &str) -> (String, PathBuf) {
    let file = base.join(name);
    let path = base.join(format!("{name}.sh"));
    let body = format!("#!/bin/sh\ntouch {}\n", file.display());
    fs::write(&path, body).unwrap();
    let open = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&path, open).unwrap();
    (path.display().to_string(), file)
}

#[test]
fn take_master_merges_a_new_commit_of_the_server_into_the_branch() {
    let mut w = world();
    let work = w.work.clone();
    let built = commit(&work, "built.txt", "b");
    let tip = push_to(&w.base, &w.server, NEW_FILE, "m");
    let refs = git(&w.project, &["for-each-ref"]);
    assert!(alone(&mut w, "take_master"), "{}", w.run.state.note);
    assert!(work.join(NEW_FILE).is_file(), "no merge");
    assert!(work.join("built.txt").is_file(), "the work is gone");
    // A merge commit: the work first, then the commit of the server.
    let line = git(&work, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    let parents: Vec<&str> = line.split(' ').skip(1).collect();
    assert_eq!(parents, [built.as_str(), tip.as_str()]);
    assert_eq!(git(&work, &["rev-parse", MASTER]), tip);
    let on = git(&work, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(on, BRANCH);
    assert_eq!(git(&work, &["status", "--porcelain"]), "");
    // The project folder does not change.
    assert_eq!(git(&w.project, &["for-each-ref"]), refs);
    // The second time, nothing changes and the result is good.
    let merged = head(&work);
    assert!(alone(&mut w, "take_master"), "{}", w.run.state.note);
    assert_eq!(head(&work), merged);
}

#[test]
fn take_master_with_the_newest_master_changes_nothing_and_is_good() {
    let mut w = world();
    let work = w.work.clone();
    let built = commit(&work, "built.txt", "b");
    assert!(alone(&mut w, "take_master"), "{}", w.run.state.note);
    assert_eq!(head(&work), built);
    assert_eq!(git(&work, &["status", "--porcelain"]), "");
}

#[test]
fn take_master_with_two_changes_to_one_line_is_bad_and_ends_the_merge() {
    let mut w = world();
    let work = w.work.clone();
    let mine = commit(&work, "file", "one\nmine\nthree\n");
    let theirs = "one\ntheirs\nthree\n";
    let tip = push_to(&w.base, &w.server, "file", theirs);
    assert!(!alone(&mut w, "take_master"), "a good result");
    assert!(!work.join(".git/MERGE_HEAD").exists(), "a merge stays");
    assert_eq!(head(&work), mine);
    assert_eq!(git(&work, &["status", "--porcelain"]), "");
    let kept = fs::read_to_string(work.join("file")).unwrap();
    assert_eq!(kept, "one\nmine\nthree\n");
    // The ref of the default branch is there for the builder.
    assert_eq!(git(&work, &["rev-parse", MASTER]), tip);
    // The builder of `fix_merge` merges, repairs the line and commits.
    assert!(!git_ok(&work, &["merge", "--no-edit", MASTER]));
    commit(&work, "file", "one\nmine and theirs\nthree\n");
    assert!(alone(&mut w, "take_master"), "{}", w.run.state.note);
    assert!(has(&work, &tip, "HEAD"), "master is not in the branch");
}

/// `take_master` with this config entry in the work copy gives a bad result
/// and no merge. With no such entry, the same step is good.
fn refuses(key: &str) {
    let mut w = world();
    let work = w.work.clone();
    let built = commit(&work, "built.txt", "b");
    let tip = push_to(&w.base, &w.server, NEW_FILE, "m");
    let (script, ran) = marker(&w.base, "ran");
    git(&work, &["config", key, &script]);
    fs::create_dir_all(work.join(".git/info")).unwrap();
    let rule = "* filter=x merge=x\n";
    fs::write(work.join(".git/info/attributes"), rule).unwrap();
    assert!(!alone(&mut w, "take_master"), "{key}: a good result");
    assert!(!ran.exists(), "{key}: the command of the entry ran");
    assert_eq!(head(&work), built, "{key}");
    assert!(!work.join(NEW_FILE).exists(), "{key}: a merge");
    git(&work, &["config", "--unset", key]);
    assert!(alone(&mut w, "take_master"), "{}", w.run.state.note);
    assert!(has(&work, &tip, "HEAD"), "{key}: no merge");
}

#[test]
fn take_master_refuses_a_work_copy_with_a_filter_entry() {
    refuses("filter.x.smudge");
}

#[test]
fn take_master_refuses_a_work_copy_with_a_merge_driver() {
    refuses("merge.x.driver");
}

#[test]
fn take_master_refuses_a_work_copy_with_a_url_entry() {
    refuses("url.x.insteadOf");
}

#[test]
fn take_master_refuses_a_work_copy_with_a_protocol_entry() {
    refuses("protocol.x.allow");
}

#[test]
fn take_master_runs_no_hook_no_fsmonitor_and_no_sign_command() {
    let mut w = world();
    let work = w.work.clone();
    commit(&work, "built.txt", "b");
    push_to(&w.base, &w.server, NEW_FILE, "m");
    let (hook, hooked) = marker(&w.base, "hook-ran");
    let other = w.base.join("other-hooks");
    for folder in [work.join(".git/hooks"), other.clone()] {
        fs::create_dir_all(&folder).unwrap();
        for name in HOOKS {
            fs::copy(&hook, folder.join(name)).unwrap();
        }
    }
    let (monitor, watched) = marker(&w.base, "monitor-ran");
    let (sign, signed) = marker(&w.base, "sign-ran");
    let path = other.display().to_string();
    git(&work, &["config", "core.hooksPath", &path]);
    git(&work, &["config", "core.fsmonitor", &monitor]);
    git(&work, &["config", "commit.gpgSign", "true"]);
    git(&work, &["config", "gpg.program", &sign]);
    let good = alone(&mut w, "take_master");
    assert!(!hooked.exists(), "a hook ran");
    assert!(!watched.exists(), "the fsmonitor ran");
    assert!(!signed.exists(), "the sign command ran");
    assert!(good, "{}", w.run.state.note);
    assert!(work.join(NEW_FILE).is_file(), "no merge");
}

#[test]
fn take_master_does_not_fetch_from_the_origin_of_the_work_copy() {
    let mut w = world();
    let work = w.work.clone();
    commit(&work, "built.txt", "b");
    // A second server copy with a commit that master does not have.
    let decoy = w.base.join("decoy.git");
    clone(&w.server, &decoy, &["--bare"]);
    let bad = push_to(&w.base, &decoy, "decoy.txt", "d");
    let tip = push_to(&w.base, &w.server, NEW_FILE, "m");
    // The origin of the work copy is that copy. A `url.` entry is refused
    // by a different test, so none is set here.
    let decoy = decoy.display().to_string();
    git(&work, &["config", "remote.origin.url", &decoy]);
    assert!(alone(&mut w, "take_master"), "{}", w.run.state.note);
    assert_eq!(git(&work, &["rev-parse", MASTER]), tip);
    assert!(has(&work, &tip, "HEAD"), "master is not in the branch");
    assert!(!work.join("decoy.txt").exists(), "the decoy came in");
    let got = git_ok(&work, &["cat-file", "-e", &bad]);
    assert!(!got, "the work copy fetched from its origin");
}

#[test]
fn same_master_is_good_only_if_the_pushed_branch_has_the_newest_master() {
    let mut w = world();
    let work = w.work.clone();
    commit(&work, "built.txt", "b");
    for step in ["take_master", "no_locked_files", "same_master"] {
        assert!(alone(&mut w, step), "{step}: {}", w.run.state.note);
    }
    let tip = push_to(&w.base, &w.server, NEW_FILE, "m");
    assert!(!alone(&mut w, "same_master"), "good with a new commit");
    // The merge is in the work copy. The clean copy does not have it.
    assert!(alone(&mut w, "take_master"), "{}", w.run.state.note);
    assert!(has(&work, &tip, "HEAD"), "no merge");
    assert!(!alone(&mut w, "same_master"), "it read the work copy");
    // `no_locked_files` gets the branch into the clean copy.
    for step in ["no_locked_files", "same_master"] {
        assert!(alone(&mut w, step), "{step}: {}", w.run.state.note);
    }
}

// ---------------------------------------------------------------------
// The path of a run, with a simulated worker
// ---------------------------------------------------------------------

/// A simulated worker. It writes its papers and gives a good result. It
/// makes one commit in the work copy for each step name in `commits`, in
/// that sequence.
struct Sim {
    commits: Mutex<Vec<&'static str>>,
    got: Mutex<Vec<(String, String)>>,
}

impl Sim {
    fn new(commits: &[&'static str]) -> Self {
        Self {
            commits: Mutex::new(commits.to_vec()),
            got: Mutex::default(),
        }
    }

    /// The results before each call of the step `step`.
    fn got(&self, step: &str) -> Vec<String> {
        let got = self.got.lock().unwrap();
        let mine = got.iter().filter(|(s, _)| s == step);
        mine.map(|(_, before)| before.clone()).collect()
    }
}

impl Worker for Sim {
    fn run(&self, job: &Job) -> Result<Outcome> {
        let before = job.before.map_or_else(String::new, |o| o.summary.clone());
        let entry = (job.step.to_owned(), before);
        self.got.lock().unwrap().push(entry);
        for paper in job.writes {
            let path = job.dir.join("papers").join(paper);
            fs::write(path.join("file.md"), job.step)?;
        }
        let mut commits = self.commits.lock().unwrap();
        if commits.first().is_some_and(|s| *s == job.step) {
            commits.remove(0);
            let work = job.dir.join("work");
            fs::write(work.join(job.step), job.step)?;
            git(&work, &["add", "-A"]);
            git(&work, &["commit", "--quiet", "-m", job.step]);
        }
        Ok(Outcome {
            ok: true,
            summary: format!("{} ran", job.step),
        })
    }
}

/// The steps in the log, with their results. The fan parts are not in it.
fn steps(run: &Run) -> Vec<(String, bool)> {
    let log = fs::read_to_string(run.dir.join("log.jsonl")).unwrap();
    let mut found = Vec::new();
    for line in log.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        let step = value["step"].as_str().unwrap().to_owned();
        if !PARTS.contains(&step.as_str()) {
            found.push((step, value["ok"].as_bool().unwrap()));
        }
    }
    found
}

/// The steps after the last answer at the person step `name`.
fn after(run: &Run, name: &str) -> Vec<(String, bool)> {
    let all = steps(run);
    let at = all.iter().rposition(|(s, _)| s == name);
    all[at.expect("no answer at that step") + 1..].to_vec()
}

fn pairs(list: &[(&str, bool)]) -> Vec<(String, bool)> {
    list.iter().map(|(s, ok)| ((*s).to_owned(), *ok)).collect()
}

/// Does the run to `look_at_tests`, answers yes, and goes on.
fn to_build(w: &mut World, graph: &Graph, worker: &Sim) {
    let roles = root().join("roles");
    w.run.go(graph, &roles, worker).unwrap();
    assert_eq!(w.run.state.at.as_deref(), Some("look_at_tests"));
    w.run.answer(true, "").unwrap();
    w.run.go(graph, &roles, worker).unwrap();
}

#[test]
fn a_run_with_work_goes_through_take_master_to_the_approval() {
    let mut w = world();
    let graph = sheet(&w.project);
    let tip = push_to(&w.base, &w.server, NEW_FILE, "m");
    let worker = Sim::new(&["build"]);
    to_build(&mut w, &graph, &worker);
    let path = pairs(&[
        ("mark", true),
        ("build", true),
        ("simplify", true),
        ("work_was_done", true),
        ("take_master", true),
        ("learn", true),
        ("set_version", true),
        ("write_pr", true),
        ("check", true),
        ("no_locked_files", true),
        ("push", true),
        ("find_pr", true),
    ]);
    let ran = after(&w.run, "look_at_tests");
    assert_eq!(ran, path, "{}", w.run.state.note);
    assert_eq!(w.run.state.status, Status::Waiting);
    assert_eq!(w.run.state.at.as_deref(), Some("approval"));
    assert!(has(&w.work, &tip, "HEAD"), "master is not in the branch");
    assert!(w.work.join("version.txt").is_file(), "no version commit");
    assert!(w.run.dir.join("new-work.bundle").is_file(), "no bundle");
}

#[test]
fn files_txt_does_not_name_a_file_that_only_the_merge_changed() {
    let mut w = world();
    let graph = sheet(&w.project);
    push_to(&w.base, &w.server, NEW_FILE, "m");
    let worker = Sim::new(&["build"]);
    to_build(&mut w, &graph, &worker);
    assert_eq!(w.run.state.at.as_deref(), Some("approval"));
    assert!(w.work.join(NEW_FILE).is_file(), "no merge");
    let listed = fs::read_to_string(w.run.dir.join("files.txt"));
    let listed = listed.expect("no files.txt in the run folder");
    let names: Vec<&str> = listed.lines().collect();
    // The simulated builder commits one file with the name `build`.
    assert!(names.contains(&"build"), "{names:?}");
    assert!(!names.contains(&NEW_FILE), "{names:?}");
}

#[test]
fn with_no_commit_of_any_builder_the_run_stops_before_the_pr() {
    let mut w = world();
    let graph = sheet(&w.project);
    push_to(&w.base, &w.server, NEW_FILE, "m");
    let worker = Sim::new(&[]);
    to_build(&mut w, &graph, &worker);
    // `do_work` goes back to `work_was_done`. With no commit the
    // run stops after the tries of `do_work`.
    assert_eq!(w.run.state.status, Status::Stopped, "{}", w.run.state.note);
    let ran = after(&w.run, "look_at_tests");
    let names: Vec<&str> = ran.iter().map(|(s, _)| s.as_str()).collect();
    for gone in ["take_master", "set_version", "write_pr", "push", "find_pr"] {
        assert!(!names.contains(&gone), "{gone}: {names:?}");
    }
    let tries = names.iter().filter(|s| **s == "do_work").count();
    assert_eq!(tries, 3, "{names:?}");
    // The builder of `do_work` gets the error text of git.
    let got = worker.got("do_work");
    let first = got[0].to_lowercase();
    assert!(first.contains("empty bundle"), "{got:?}");
    assert!(!w.run.dir.join("new-work.bundle").exists());
}

#[test]
fn a_commit_of_do_work_passes_the_second_check_and_goes_to_take_master() {
    let mut w = world();
    let graph = sheet(&w.project);
    let tip = push_to(&w.base, &w.server, NEW_FILE, "m");
    let worker = Sim::new(&["do_work"]);
    to_build(&mut w, &graph, &worker);
    let path = pairs(&[
        ("mark", true),
        ("build", true),
        ("simplify", true),
        ("work_was_done", false),
        ("do_work", true),
        ("work_was_done", true),
        ("take_master", true),
        ("learn", true),
        ("set_version", true),
        ("write_pr", true),
        ("check", true),
        ("no_locked_files", true),
        ("push", true),
        ("find_pr", true),
    ]);
    let ran = after(&w.run, "look_at_tests");
    assert_eq!(ran, path, "{}", w.run.state.note);
    assert!(w.run.dir.join("new-work.bundle").is_file(), "no bundle");
    // The merge and the version are commits after `mark`.
    assert!(has(&w.work, &tip, "HEAD"), "master is not in the branch");
    assert!(w.work.join("version.txt").is_file(), "no version commit");
}

#[test]
fn after_a_no_the_commits_of_the_merge_and_the_version_do_not_count() {
    let mut w = world();
    let graph = sheet(&w.project);
    let roles = root().join("roles");
    push_to(&w.base, &w.server, NEW_FILE, "m");
    let worker = Sim::new(&["build"]);
    to_build(&mut w, &graph, &worker);
    assert_eq!(w.run.state.at.as_deref(), Some("approval"));
    push_to(&w.base, &w.server, "second.txt", "s");
    w.run.answer(false, "Change the heading.").unwrap();
    w.run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(w.run.state.at.as_deref(), Some("look_at_tests"));
    w.run.answer(true, "").unwrap();
    w.run.go(&graph, &roles, &worker).unwrap();
    // The builder made no commit after the second yes. The commits
    // of the merge and the version do not count, so `do_work` runs
    // and, with no commit, the run stops before `take_master`.
    let ran = after(&w.run, "look_at_tests");
    let names: Vec<&str> = ran.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(&names[..4], ["mark", "build", "simplify", "work_was_done"]);
    assert!(!ran[3].1, "{ran:?}");
    assert!(names.contains(&"do_work"), "{names:?}");
    assert!(!names.contains(&"write_pr"), "{names:?}");
    assert_eq!(w.run.state.status, Status::Stopped);
}

#[test]
fn a_new_commit_of_master_at_the_approval_sends_the_run_to_take_master() {
    let mut w = world();
    let graph = sheet(&w.project);
    let roles = root().join("roles");
    let worker = Sim::new(&["build"]);
    to_build(&mut w, &graph, &worker);
    assert_eq!(w.run.state.at.as_deref(), Some("approval"));
    let tip = push_to(&w.base, &w.server, NEW_FILE, "m");
    w.run.answer(true, "").unwrap();
    w.run.go(&graph, &roles, &worker).unwrap();
    let path = pairs(&[
        ("same_master", false),
        ("take_master", true),
        ("learn", true),
        ("set_version", true),
        ("write_pr", true),
        ("check", true),
        ("no_locked_files", true),
        ("push", true),
        ("find_pr", true),
    ]);
    let ran = after(&w.run, "approval");
    assert_eq!(ran, path, "{}", w.run.state.note);
    // The person gets the PR question again.
    assert_eq!(w.run.state.status, Status::Waiting);
    assert_eq!(w.run.state.at.as_deref(), Some("approval"));
    assert!(has(&w.work, &tip, "HEAD"), "master is not in the branch");
    // Master did not change again, so the second yes ends the run.
    w.run.answer(true, "").unwrap();
    w.run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(w.run.state.status, Status::Done, "{}", w.run.state.note);
    let ran = after(&w.run, "approval");
    assert_eq!(ran[0], ("same_master".to_owned(), true));
    assert_eq!(ran[1], ("merge".to_owned(), true));
}

// ---------------------------------------------------------------------
// The limit, the rules page and the documents
// ---------------------------------------------------------------------

/// The rule with this number in the text of a page, or an empty text.
fn rule_in(page: &str, number: usize) -> String {
    let head = format!("{number}. ");
    let next = format!("{}. ", number + 1);
    let mut found = Vec::new();
    let mut inside = false;
    for line in page.lines() {
        let goes_on = inside && !line.is_empty() && !line.starts_with(&next);
        inside = line.starts_with(&head) || goes_on;
        if inside {
            found.push(line);
        }
    }
    found.join("\n")
}

#[test]
fn the_builder_page_has_one_rule_for_fix_merge() {
    let page = text("roles/builder.md");
    let rules: Vec<String> = (1..30).map(|n| rule_in(&page, n)).collect();
    let mut mine = rules.iter().filter(|r| r.contains("`fix_merge`"));
    let rule = mine.next().expect("no rule for fix_merge");
    assert!(mine.next().is_none(), "two rules for fix_merge");
    for word in ["`take_master`", "paper test", "lesson test"] {
        assert!(rule.contains(word), "{word}: {rule}");
    }
    assert!(rule.to_lowercase().contains("commit"), "{rule}");
}

/// The comment lines right above the step `name` in the default sheet.
fn comment_above(name: &str) -> String {
    let sheet = text("graph.example.yaml");
    let lines: Vec<&str> = sheet.lines().collect();
    let head = format!("  {name}:");
    let at = lines.iter().position(|l| *l == head).expect(&head);
    let mut found = Vec::new();
    for line in lines[..at].iter().rev() {
        if !line.trim_start().starts_with('#') {
            break;
        }
        found.push(*line);
    }
    found.join("\n").to_lowercase()
}

#[test]
fn the_comment_of_take_master_names_each_option_and_the_tries() {
    let comment = comment_above("take_master");
    for option in OPTIONS {
        let option = option.to_lowercase();
        assert!(comment.contains(&option), "{option}: {comment}");
    }
    let words = ["--no-edit", "--no-verify", "filter", "driver"];
    for word in words {
        assert!(comment.contains(word), "{word}: {comment}");
    }
    assert!(comment.contains("third try"), "{comment}");
    assert!(comment.contains("clean copy"), "{comment}");
}

#[test]
fn each_new_step_of_the_sheet_has_a_comment() {
    for name in ["fix_merge", "same_master", "do_work"] {
        let comment = comment_above(name);
        assert!(!comment.is_empty(), "{name}: no comment");
    }
    let done = comment_above("work_was_done");
    assert!(done.contains("not a wall"), "{done}");
    assert!(done.contains("simplify"), "{done}");
}

/// The text of the part `## title` of a document.
fn section(rel: &str, title: &str) -> String {
    let page = text(rel);
    let head = format!("## {title}");
    let mut found = Vec::new();
    let mut inside = false;
    for line in page.lines() {
        if line.starts_with("## ") {
            inside = line == head;
        } else if inside {
            found.push(line);
        }
    }
    assert!(!found.is_empty(), "{rel}: no part {title}");
    found.join("\n")
}

fn has_word(text: &str, word: &str) -> bool {
    let mut words = text.split(|c: char| !(c.is_alphanumeric() || c == '_'));
    words.any(|w| w == word)
}

/// The position of the first line that starts with `start`.
fn line_at(lines: &[&str], start: &str) -> usize {
    let at = lines.iter().position(|l| l.starts_with(start));
    at.unwrap_or_else(|| panic!("no line {start}"))
}

#[test]
fn agents_md_tells_the_new_steps_and_run_next_with_an_active_run() {
    let sheet = section("AGENTS.md", "The sheet");
    let names = ["take_master", "fix_merge", "same_master", "do_work"];
    for name in names {
        assert!(has_word(&sheet, name), "The sheet: {name}");
    }
    // The sentence from before is not there: the step moved.
    assert!(!sheet.contains("comes after a good `check`"), "{sheet}");
    let commands = section("AGENTS.md", "Commands");
    let mut lines = commands.lines();
    let line = lines.find(|l| l.contains("codeumbrella run --next"));
    let line = line.expect("no line for run --next");
    assert!(!line.contains("or a run is"), "{line}");
}

#[test]
fn agents_md_tells_that_two_runs_can_work_at_the_same_time() {
    let part = section("AGENTS.md", "Not in version 1");
    let before = "- Builders that work at the same time.";
    assert!(!part.contains(before), "{part}");
    assert!(part.contains("Two runs"), "{part}");
    assert!(part.contains("fan of builders"), "{part}");
}

#[test]
fn the_safety_rules_have_a_line_with_the_date_for_each_new_rule() {
    let rules = section("AGENTS.md", "Safety rules");
    let keys = ["run --next", "take_master", "same_master"];
    for key in keys {
        let mut lines = rules.lines();
        let found = lines.any(|l| l.contains(key) && l.contains(DATE));
        assert!(found, "no line with {key} and {DATE}");
    }
    let mut lines = rules.lines();
    let done = lines.any(|l| l.contains("`do_work`") && l.contains(DATE));
    assert!(done, "no line with do_work and {DATE}");
    // The sentence from before is not there: `take_master` is between.
    let before = "The step `learn` follows `simplify`.";
    assert!(!rules.contains(before), "{before}");
}

#[test]
fn the_architecture_picture_has_the_new_steps_in_sequence() {
    let path = section("ARCHITECTURE.md", "The path of a task");
    let lines: Vec<&str> = path.lines().collect();
    let at = |start: &str| line_at(&lines, start);
    assert!(at("simplify") < at("work_was_done"), "{path}");
    assert!(at("work_was_done") < at("take_master"), "{path}");
    assert!(at("take_master") < at("learn"), "{path}");
    assert!(at("YOU: PR?") < at("same_master"), "{path}");
    assert!(at("same_master") < at("merge"), "{path}");
    for name in ["fix_merge", "do_work"] {
        assert!(has_word(&path, name), "{name}");
    }
    assert!(!path.contains("comes after a good check"), "{path}");
}

#[test]
fn the_readme_tells_about_two_runs_and_the_tries_of_take_master() {
    let page = text("README.md");
    let two = |l: &str| l.contains("Two runs") && l.contains("same time");
    assert!(page.lines().any(two), "no line about two runs");
    assert!(page.contains("two times"), "no PR question two times");
    let tries = |l: &str| l.contains("`take_master`") && l.contains("third try");
    assert!(page.lines().any(tries), "no line about the tries");
    let before = "After a good check, the step `work_was_done`";
    assert!(!page.contains(before), "{before}");
}

#[test]
fn the_chat_page_gives_each_command_its_own_output_file() {
    let page = text(SKILL);
    let file = ".codeumbrella/out-<run id>.txt";
    assert!(page.contains(file), "{file}");
    // One file for each command: the shared name is not there.
    assert!(!page.contains(".codeumbrella/out.txt"), "out.txt");
}

#[test]
fn the_chat_page_picks_a_new_output_file_for_each_run_command() {
    let part = section(SKILL, "The command to run");
    let mut lines = part.lines();
    let line = lines.find(|l| l.starts_with("`nohup codeumbrella run"));
    let line = line.expect("no nohup line for run");
    assert!(line.contains("> .codeumbrella/out-"), "{line}");
    // The chat does not know the id of the run before the start.
    assert!(!line.contains("<run id>"), "{line}");
    // The chat reads the id of the run from the line `Run:`.
    let reads = |l: &str| l.contains("`Run:`") && l.contains("run id");
    assert!(part.lines().any(reads), "no line about `Run:`");
}

#[test]
fn the_chat_page_tells_that_the_pr_question_can_come_a_second_time() {
    let part = section(SKILL, "After each command");
    let again = |l: &str| l.contains("`approval`") && l.contains("second time");
    assert!(part.lines().any(again), "no line about a second question");
}

#[test]
fn the_chat_page_starts_a_queue_task_while_a_run_is_active() {
    let page = text(SKILL);
    let mut lines = page.lines();
    let row = lines.find(|l| l.contains("while a run of the project is"));
    let row = row.expect("no row for a task while a run is active");
    assert!(row.contains("run --next"), "{row}");
    assert!(row.contains(".codeumbrella/queue/"), "{row}");
    assert!(!row.contains("Do not start a run"), "{row}");
    // The chat tells the ids of the active runs before the start.
    let tells = row.find("active runs").expect("no active runs");
    let starts = row.find("run --next").unwrap();
    assert!(tells < starts, "{row}");
    // Step 6 stays for a task that is not in the queue.
    let first = section(SKILL, "Before the first command");
    let six = first.lines().find(|l| l.starts_with("6. "));
    let six = six.expect("no step 6");
    assert!(six.contains("codeumbrella status"), "{six}");
    assert!(six.contains("`Running` or `Waiting`"), "{six}");
}
