//! Tests of the queue: `queue`, `run --next`, the steps `set_version` and
//! `work_was_done`, `init`, the script `scripts/set-version` and the
//! documents. No Claude, no Codex and no `gh`.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Worker};
use codeumbrella::graph::{Graph, Kind};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

const QUEUE: &str = ".codeumbrella/queue";
const SCRIPT: &str = ".codeumbrella/set-version";
const SKILL: &str = "skills/codeumbrella/SKILL.md";
const WORK: &str = ".codeumbrella/runs/{run}/work";

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

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let path = root().join(rel);
    let read = fs::read_to_string(path);
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
    let name = format!("cu-queue-{}-{n}", std::process::id());
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

fn lines_of(out: &Output) -> Vec<String> {
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout.lines().map(str::to_owned).collect()
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
    let found = fs::read_dir(dir.join(QUEUE)).unwrap();
    let mut names: Vec<String> = found
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The id of the run that an output names.
fn id(out: &Output) -> String {
    let text = all(out);
    let line = text.lines().find(|l| l.starts_with("Run:      "));
    let line = line.unwrap_or_else(|| panic!("no run: {text}"));
    line["Run:      ".len()..].to_owned()
}

fn run_dir(dir: &Path, id: &str) -> PathBuf {
    dir.join(".codeumbrella/runs").join(id)
}

// ---------------------------------------------------------------------
// The command `queue`
// ---------------------------------------------------------------------

#[test]
fn queue_with_an_empty_folder_prints_one_line_that_says_empty() {
    let dir = project();
    assert!(dir.join(QUEUE).is_dir(), "init made no queue folder");
    let out = cu(&dir, &["queue"]);
    assert!(out.status.success(), "{}", all(&out));
    let lines = lines_of(&out);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].to_lowercase().contains("empty"), "{lines:?}");
}

#[test]
fn queue_prints_one_line_for_each_file_in_the_sequence_of_the_names() {
    let dir = project();
    put(&dir, "02-pr-text.txt", "Second task");
    put(&dir, "01-clean-up.txt", "First task");
    let out = cu(&dir, &["queue"]);
    assert!(out.status.success(), "{}", all(&out));
    let lines = lines_of(&out);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("01-clean-up.txt"), "{lines:?}");
    assert!(lines[1].contains("02-pr-text.txt"), "{lines:?}");
    for line in &lines {
        assert!(line.contains("Files: not given"), "{line}");
    }
}

#[test]
fn queue_shows_the_files_line_of_a_task_file() {
    let dir = project();
    let body = "Do the work.\nFiles: src/a.rs, README.md\nMore text.\n";
    put(&dir, "01-a.txt", body);
    put(&dir, "02-b.txt", "No such line here.");
    let out = cu(&dir, &["queue"]);
    assert!(out.status.success(), "{}", all(&out));
    let lines = lines_of(&out);
    assert_eq!(lines.len(), 2, "{lines:?}");
    let given = "Files: src/a.rs, README.md";
    assert!(lines[0].contains(given), "{lines:?}");
    assert!(!lines[0].contains("not given"), "{lines:?}");
    assert!(lines[1].contains("Files: not given"), "{lines:?}");
}

#[test]
fn queue_changes_no_file() {
    let dir = project();
    put(&dir, "01-a.txt", "Task\nFiles: x");
    let out = cu(&dir, &["queue"]);
    assert!(out.status.success(), "{}", all(&out));
    assert!(all(&out).contains("01-a.txt"), "{}", all(&out));
    assert_eq!(queued(&dir), ["01-a.txt"]);
    let kept = fs::read_to_string(dir.join(QUEUE).join("01-a.txt"));
    assert_eq!(kept.unwrap(), "Task\nFiles: x");
}

// ---------------------------------------------------------------------
// The option `run --next`
// ---------------------------------------------------------------------

#[test]
fn run_next_starts_the_first_file_and_moves_it_to_task_txt() {
    let dir = project();
    put(&dir, "02-second.txt", "Second task");
    put(&dir, "01-first.txt", "First task\nFiles: a.rs");
    let out = cu(&dir, &["run", "--next"]);
    let printed = all(&out);
    assert!(out.status.success(), "{printed}");
    assert!(printed.contains("Status:   Waiting"), "{printed}");
    let run = run_dir(&dir, &id(&out));
    let moved = fs::read_to_string(run.join("task.txt")).unwrap();
    assert_eq!(moved, "First task\nFiles: a.rs");
    // The task is not in the queue two times. The second file stays.
    assert_eq!(queued(&dir), ["02-second.txt"]);
    let kept = fs::read_to_string(dir.join(QUEUE).join("02-second.txt"));
    assert_eq!(kept.unwrap(), "Second task");
}

#[test]
fn run_next_gives_the_text_of_the_file_to_the_run() {
    let dir = project();
    put(&dir, "01-first.txt", "First task text");
    let out = cu(&dir, &["run", "--next"]);
    assert!(out.status.success(), "{}", all(&out));
    let run = run_dir(&dir, &id(&out));
    let state = fs::read_to_string(run.join("state.json")).unwrap();
    assert!(state.contains("First task text"), "{state}");
}

#[test]
fn run_next_with_an_empty_queue_is_an_error_and_starts_nothing() {
    let dir = project();
    let out = cu(&dir, &["run", "--next"]);
    assert!(!out.status.success(), "{}", all(&out));
    let said = all(&out).to_lowercase();
    assert!(said.contains("empty"), "{said}");
    assert!(!dir.join(".codeumbrella/runs").exists(), "a run started");
}

#[test]
fn run_next_with_a_waiting_run_is_an_error_and_the_queue_stays() {
    let dir = project();
    let first = cu(&dir, &["run", "a task"]);
    let shown = all(&first);
    assert!(shown.contains("Status:   Waiting"), "{shown}");
    let waiting = id(&first);
    put(&dir, "01-next.txt", "Next task");
    let out = cu(&dir, &["run", "--next"]);
    assert!(!out.status.success(), "{}", all(&out));
    assert!(all(&out).contains(&waiting), "{}", all(&out));
    assert_eq!(queued(&dir), ["01-next.txt"]);
    let runs = fs::read_dir(dir.join(".codeumbrella/runs")).unwrap();
    assert_eq!(runs.count(), 1, "a second run started");
}

#[test]
fn run_next_with_a_running_run_is_an_error_and_the_queue_stays() {
    let dir = project();
    let first = cu(&dir, &["run", "a task"]);
    let running = id(&first);
    let state = run_dir(&dir, &running).join("state.json");
    let json = fs::read_to_string(&state).unwrap();
    let changed = json.replace("\"Waiting\"", "\"Running\"");
    assert_ne!(json, changed, "no status Waiting in the notebook");
    fs::write(&state, changed).unwrap();
    put(&dir, "01-next.txt", "Next task");
    let out = cu(&dir, &["run", "--next"]);
    assert!(!out.status.success(), "{}", all(&out));
    assert!(all(&out).contains(&running), "{}", all(&out));
    assert_eq!(queued(&dir), ["01-next.txt"]);
}

#[test]
fn run_next_after_a_done_run_is_allowed() {
    let dir = project();
    let first = cu(&dir, &["run", "a task"]);
    let old = id(&first);
    let answer = cu(&dir, &["answer", &old, "look", "yes"]);
    let shown = all(&answer);
    assert!(shown.contains("Status:   Done"), "{shown}");
    put(&dir, "01-next.txt", "Next task");
    let out = cu(&dir, &["run", "--next"]);
    assert!(out.status.success(), "{}", all(&out));
    assert_ne!(id(&out), old);
    assert_eq!(queued(&dir).len(), 0);
}

#[test]
fn run_with_a_text_works_as_before() {
    let dir = project();
    put(&dir, "01-next.txt", "Queued task");
    let out = cu(&dir, &["run", "Task by hand"]);
    assert!(out.status.success(), "{}", all(&out));
    assert!(all(&out).contains("Status:   Waiting"), "{}", all(&out));
    assert_eq!(queued(&dir), ["01-next.txt"]);
}

#[test]
fn run_next_does_not_follow_a_link_out_of_the_queue() {
    let dir = project();
    let secret = dir.join("secret.txt");
    fs::write(&secret, "A secret").unwrap();
    let queue = dir.join(QUEUE);
    symlink(&secret, queue.join("01-link.txt")).unwrap();
    let out = cu(&dir, &["run", "--next"]);
    assert!(!out.status.success(), "{}", all(&out));
    assert!(!dir.join(".codeumbrella/runs").exists(), "a run started");
    assert_eq!(fs::read_to_string(&secret).unwrap(), "A secret");
    assert!(queue.join("01-link.txt").is_symlink(), "the link moved");
}

// ---------------------------------------------------------------------
// The default sheet
// ---------------------------------------------------------------------

/// True if a step of the sheet goes to `to`: by `next` or by `on_bad`.
fn goes_to(graph: &Graph, from: &str, to: &str) -> bool {
    let step = graph.step(from).unwrap();
    let target = Some(to.to_owned());
    step.next == target || step.on_bad == target
}

#[test]
fn learn_goes_to_set_version_and_set_version_goes_to_write_pr() {
    let graph = default_sheet();
    let learn = graph.step("learn").unwrap();
    assert_eq!(learn.next.as_deref(), Some("set_version"));
    let set = graph.step("set_version").unwrap();
    assert_eq!(set.next.as_deref(), Some("write_pr"));
    let repair = graph.step("repair").unwrap();
    assert_eq!(repair.next.as_deref(), Some("write_pr"));
    for name in graph.steps.keys() {
        let goes = goes_to(&graph, name, "set_version");
        assert_eq!(goes, name == "learn", "{name}");
    }
}

#[test]
fn set_version_has_one_command_and_no_on_bad_or_max_tries() {
    let graph = default_sheet();
    let set = graph.step("set_version").unwrap();
    assert_eq!(set.kind, Kind::Command);
    let want = ["sh", SCRIPT, WORK];
    assert_eq!(set.run, [want], "{:?}", set.run);
    assert!(set.on_bad.is_none(), "{:?}", set.on_bad);
    assert!(set.max_tries.is_none(), "{:?}", set.max_tries);
    assert!(set.reads.is_empty() && set.writes.is_empty());
    assert!(set.role.is_none() && set.fan.is_empty());
}

#[test]
fn set_version_does_not_push() {
    let graph = default_sheet();
    let set = graph.step("set_version").unwrap();
    assert!(!set.run.iter().flatten().any(|w| w == "push"));
    let push = graph.step("push").unwrap();
    assert!(push.run.iter().flatten().any(|w| w == "push"));
}

#[test]
fn work_was_done_has_a_second_command_that_writes_files_txt() {
    let graph = default_sheet();
    let done = graph.step("work_was_done").unwrap();
    assert_eq!(done.run.len(), 2, "{:?}", done.run);
    let second = &done.run[1];
    let words = ["git", "diff", "--name-only", "--no-ext-diff"];
    for word in words {
        assert!(has(second, word), "{word}: {second:?}");
    }
    let wanted = ["--no-textconv", "--output", "files.txt", "HEAD"];
    for word in wanted {
        assert!(has(second, word), "{word}: {second:?}");
    }
    assert!(has(second, "refs/codeumbrella/before"), "{second:?}");
}

/// True if one word of a command has `part` in it.
fn has(words: &[String], part: &str) -> bool {
    words.iter().any(|w| w.contains(part))
}

// ---------------------------------------------------------------------
// `work_was_done` writes files.txt (simulated worker, real git)
// ---------------------------------------------------------------------

/// The default sheet for a run in the project `dir`. The steps `mark` and
/// `work_was_done` run their commands there. Each other command step does
/// nothing.
fn quiet_sheet(dir: &Path) -> Graph {
    let mut graph = default_sheet();
    let full = format!("{}/.codeumbrella/runs/{{run}}", dir.display());
    for (name, step) in &mut graph.steps {
        if ["mark", "work_was_done"].contains(&name.as_str()) {
            for word in step.run.iter_mut().flatten() {
                *word = word.replace(".codeumbrella/runs/{run}", &full);
            }
        } else if !step.run.is_empty() {
            step.run = vec![vec!["true".to_owned()]];
        }
    }
    graph
}

/// A simulated worker. The builder makes a commit with two new files.
struct Sim;

impl Worker for Sim {
    fn run(&self, job: &Job) -> Result<Outcome> {
        for paper in job.writes {
            let path = job.dir.join("papers").join(paper);
            fs::write(path.join("file.md"), job.step)?;
        }
        if job.step == "build" {
            let work = job.dir.join("work");
            fs::create_dir_all(work.join("sub"))?;
            fs::write(work.join("new-one.txt"), "1")?;
            fs::write(work.join("sub/new-two.txt"), "2")?;
            fs::write(work.join("first.txt"), "changed")?;
            git(&work, &["add", "-A"]);
            git(&work, &["commit", "--quiet", "-m", "build"]);
        }
        Ok(Outcome {
            ok: true,
            summary: format!("{} ran", job.step),
        })
    }
}

#[test]
fn work_was_done_writes_the_changed_files_into_files_txt() {
    let dir = folder();
    let graph = quiet_sheet(&dir);
    let folder = dir.join(".codeumbrella/runs/run-1");
    let mut run = Run::start(folder, "a task", &graph).unwrap();
    let work = run.dir.join("work");
    git(&work, &["init", "--quiet"]);
    fs::write(work.join("first.txt"), "x").unwrap();
    fs::write(work.join("untouched.txt"), "y").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "--quiet", "-m", "first"]);
    let roles = root().join("roles");
    run.go(&graph, &roles, &Sim).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &Sim).unwrap();
    let listed = fs::read_to_string(run.dir.join("files.txt"));
    let listed = listed.expect("no files.txt in the run folder");
    let mut names: Vec<&str> = listed.lines().collect();
    names.sort_unstable();
    assert_eq!(names, ["first.txt", "new-one.txt", "sub/new-two.txt"]);
    let stray = work.join("files.txt");
    assert!(!stray.exists(), "files.txt in the work copy");
}

// ---------------------------------------------------------------------
// `init` and the script of the project
// ---------------------------------------------------------------------

#[test]
fn init_writes_the_default_script_that_does_nothing_and_is_good() {
    let dir = folder();
    let out = cu(&dir, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    let script = fs::read_to_string(dir.join(SCRIPT));
    let script = script.expect("init wrote no .codeumbrella/set-version");
    let lower = script.to_lowercase();
    assert!(lower.contains("version"), "{script}");
    assert!(lower.contains("work copy"), "{script}");
    assert!(script.lines().any(|l| l.starts_with('#')), "{script}");
    let work = folder();
    let mut command = Command::new("sh");
    command.arg(dir.join(SCRIPT)).arg(&work).current_dir(&dir);
    let run = command.output().unwrap();
    assert!(run.status.success(), "{}", all(&run));
    let count = fs::read_dir(&work).unwrap().count();
    assert_eq!(count, 0, "the script wrote");
}

#[test]
fn init_makes_the_queue_folder_and_update_keeps_its_files() {
    let dir = folder();
    let out = cu(&dir, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    assert!(dir.join(QUEUE).is_dir(), "no queue folder");
    put(&dir, "01-a.txt", "A task");
    let out = cu(&dir, &["init", "--update"]);
    assert!(out.status.success(), "{}", all(&out));
    assert_eq!(queued(&dir), ["01-a.txt"]);
    let kept = fs::read_to_string(dir.join(QUEUE).join("01-a.txt"));
    assert_eq!(kept.unwrap(), "A task");
}

#[test]
fn init_makes_the_queue_folder_if_it_is_not_there() {
    let dir = folder();
    let out = cu(&dir, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    fs::remove_dir(dir.join(QUEUE)).unwrap();
    let out = cu(&dir, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    assert!(dir.join(QUEUE).is_dir(), "no queue folder");
}

#[test]
fn init_update_keeps_a_script_that_the_person_changed() {
    let dir = folder();
    let out = cu(&dir, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    assert!(dir.join(SCRIPT).is_file(), "init wrote no script");
    let mine = "#!/bin/sh\n# my own script\necho mine\n";
    fs::write(dir.join(SCRIPT), mine).unwrap();
    let out = cu(&dir, &["init", "--update"]);
    assert!(out.status.success(), "{}", all(&out));
    assert_eq!(fs::read_to_string(dir.join(SCRIPT)).unwrap(), mine);
    let before = dir.join(".codeumbrella/set-version.before");
    assert!(!before.exists(), "init --update made a copy");
}

#[test]
fn init_update_writes_the_default_script_if_no_file_is_there() {
    let dir = folder();
    let out = cu(&dir, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    let default = fs::read_to_string(dir.join(SCRIPT)).unwrap();
    fs::remove_file(dir.join(SCRIPT)).unwrap();
    let out = cu(&dir, &["init", "--update"]);
    assert!(out.status.success(), "{}", all(&out));
    assert_eq!(fs::read_to_string(dir.join(SCRIPT)).unwrap(), default);
}

// ---------------------------------------------------------------------
// The script of this project, on a temp git copy
// ---------------------------------------------------------------------

const OLD: &str = "0.7.3";
const NEW: &str = "0.7.4";

const TOML: &str = "[package]\nname = \"codeumbrella\"\nversion = \"0.7.3\"\nedition = \"2024\"\n\n[dependencies]\nserde = { version = \"1.0.0\" }\n";

const LOCK: &str = "version = 4\n\n[[package]]\nname = \"anyhow\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"codeumbrella\"\nversion = \"0.7.3\"\ndependencies = [\n \"anyhow\",\n]\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\n";

const PLUGIN: &str = "{\n  \"name\": \"codeumbrella\",\n  \"version\": \"0.7.3\"\n}\n";

/// A bare folder as `origin`, and a clone of it as the work copy. `origin`
/// has the branch master with the four files of the old version.
fn copy_with_origin() -> (PathBuf, PathBuf) {
    let base = folder();
    let origin = base.join("origin.git");
    let seed = base.join("seed");
    fs::create_dir_all(&origin).unwrap();
    git(
        &origin,
        &["init", "--quiet", "--bare", "--initial-branch=master"],
    );
    fs::create_dir_all(seed.join(".claude-plugin")).unwrap();
    fs::create_dir_all(seed.join(".codex-plugin")).unwrap();
    git(&seed, &["init", "--quiet", "--initial-branch=master"]);
    fs::write(seed.join("Cargo.toml"), TOML).unwrap();
    fs::write(seed.join("Cargo.lock"), LOCK).unwrap();
    fs::write(seed.join(".claude-plugin/plugin.json"), PLUGIN).unwrap();
    fs::write(seed.join(".codex-plugin/plugin.json"), PLUGIN).unwrap();
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "--quiet", "-m", "first"]);
    let url = origin.display().to_string();
    git(&seed, &["push", "--quiet", &url, "HEAD:refs/heads/master"]);
    let work = base.join("work");
    let mut clone = Command::new("git");
    clone.args(["clone", "--quiet"]).arg(&origin).arg(&work);
    let clone = clone.output().unwrap();
    assert!(clone.status.success(), "{}", all(&clone));
    git(&work, &["checkout", "--quiet", "-b", "codeumbrella/run-x"]);
    (base, work)
}

/// Starts the script of this project on a work copy.
fn set_version(base: &Path, work: &Path) -> Output {
    let script = root().join("scripts/set-version");
    let mut command = Command::new("sh");
    command.arg(script).arg(work).current_dir(base);
    for who in ["AUTHOR", "COMMITTER"] {
        command.env(format!("GIT_{who}_NAME"), "t");
        command.env(format!("GIT_{who}_EMAIL"), "t@t");
    }
    command.output().unwrap()
}

fn read(work: &Path, rel: &str) -> String {
    let path = work.join(rel);
    let read = fs::read_to_string(&path);
    read.unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_script_has_a_comment_that_tells_the_person_to_copy_it() {
    let script = text("scripts/set-version");
    assert!(script.lines().any(|l| l.starts_with('#')), "{script}");
    assert!(script.contains(".codeumbrella/set-version"), "{script}");
    assert!(script.to_lowercase().contains("copy"), "{script}");
}

#[test]
fn the_script_sets_the_next_patch_version_in_the_four_files() {
    let (base, work) = copy_with_origin();
    let before = git(&work, &["rev-parse", "HEAD"]);
    let out = set_version(&base, &work);
    assert!(out.status.success(), "{}", all(&out));
    for rel in ["Cargo.toml", ".claude-plugin/plugin.json"] {
        let page = read(&work, rel);
        assert!(page.contains(NEW), "{rel}: {page}");
        assert!(!page.contains(OLD), "{rel}: {page}");
    }
    let page = read(&work, ".codex-plugin/plugin.json");
    assert!(page.contains(NEW) && !page.contains(OLD), "{page}");
    // Cargo.lock: only the package codeumbrella changes.
    let lock = read(&work, "Cargo.lock");
    let want = LOCK.replace(OLD, NEW);
    assert_eq!(lock, want, "{lock}");
    // Another package with a version keeps it.
    let toml = read(&work, "Cargo.toml");
    assert_eq!(toml, TOML.replace(OLD, NEW), "{toml}");
    // One commit with the new version.
    let after = git(&work, &["rev-parse", "HEAD"]);
    assert_ne!(before, after, "no commit");
    let range = format!("{before}..HEAD");
    let count = git(&work, &["rev-list", "--count", &range]);
    assert_eq!(count, "1");
    assert_eq!(git(&work, &["status", "--porcelain"]), "");
}

#[test]
fn a_second_run_of_the_script_changes_nothing() {
    let (base, work) = copy_with_origin();
    let first = set_version(&base, &work);
    assert!(first.status.success(), "{}", all(&first));
    let head = git(&work, &["rev-parse", "HEAD"]);
    let toml = read(&work, "Cargo.toml");
    let second = set_version(&base, &work);
    assert!(second.status.success(), "{}", all(&second));
    let now = git(&work, &["rev-parse", "HEAD"]);
    assert_eq!(now, head, "a second commit");
    assert_eq!(read(&work, "Cargo.toml"), toml);
    assert_eq!(git(&work, &["status", "--porcelain"]), "");
}

#[test]
fn the_script_changes_only_the_work_copy_and_does_not_push() {
    let (base, work) = copy_with_origin();
    let origin = base.join("origin.git");
    let refs = git(&origin, &["for-each-ref"]);
    let out = set_version(&base, &work);
    assert!(out.status.success(), "{}", all(&out));
    assert_eq!(git(&origin, &["for-each-ref"]), refs, "origin changed");
    assert!(read(&base.join("seed"), "Cargo.toml").contains(OLD));
}

#[test]
fn the_script_reads_origin_master_and_not_the_branch() {
    let (base, work) = copy_with_origin();
    // The branch has a different version, but origin/master has the old one.
    let toml = read(&work, "Cargo.toml").replace(OLD, "0.9.0");
    fs::write(work.join("Cargo.toml"), toml).unwrap();
    git(&work, &["commit", "--quiet", "-am", "other version"]);
    let out = set_version(&base, &work);
    assert!(out.status.success(), "{}", all(&out));
    let toml = read(&work, "Cargo.toml");
    assert!(toml.contains(NEW) && !toml.contains("0.9.0"), "{toml}");
}

#[test]
fn the_commit_of_the_script_runs_no_hook_of_the_work_copy() {
    let (base, work) = copy_with_origin();
    let marker = base.join("hook-ran");
    let hook = format!("#!/bin/sh\ntouch {}\n", marker.display());
    let other = work.join("other-hooks");
    fs::create_dir_all(&other).unwrap();
    for folder in [work.join(".git/hooks"), other.clone()] {
        for name in ["pre-commit", "commit-msg", "post-commit"] {
            let path = folder.join(name);
            fs::write(&path, &hook).unwrap();
            let mode = fs::Permissions::from_mode(0o755);
            fs::set_permissions(&path, mode).unwrap();
        }
    }
    git(&work, &["config", "core.hooksPath", "other-hooks"]);
    let out = set_version(&base, &work);
    assert!(out.status.success(), "{}", all(&out));
    assert!(read(&work, "Cargo.toml").contains(NEW));
    assert!(!marker.exists(), "a hook ran");
}

/// A script in `base` that makes the file `name` in `base` and gives the
/// path of the script and of that file.
fn marker_script(base: &Path, name: &str) -> (PathBuf, PathBuf) {
    let marker = base.join(name);
    let path = base.join(format!("{name}.sh"));
    let body = format!("#!/bin/sh\ntouch {}\n", marker.display());
    fs::write(&path, body).unwrap();
    let mode = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&path, mode).unwrap();
    (path, marker)
}

#[test]
fn the_script_runs_no_fsmonitor_and_no_sign_program_of_the_work_copy() {
    let (base, work) = copy_with_origin();
    let (monitor, ran) = marker_script(&base, "monitor-ran");
    let (sign, signed) = marker_script(&base, "sign-ran");
    let monitor = monitor.display().to_string();
    let sign = sign.display().to_string();
    git(&work, &["config", "core.fsmonitor", &monitor]);
    git(&work, &["config", "commit.gpgSign", "true"]);
    git(&work, &["config", "gpg.program", &sign]);
    let out = set_version(&base, &work);
    assert!(!ran.exists(), "the fsmonitor ran: {}", all(&out));
    assert!(!signed.exists(), "the sign program ran: {}", all(&out));
}

#[test]
fn the_script_runs_no_filter_of_the_work_copy() {
    let (base, work) = copy_with_origin();
    let (filter, ran) = marker_script(&base, "filter-ran");
    let clean = filter.display().to_string();
    git(&work, &["config", "filter.x.clean", &clean]);
    let attributes = "Cargo.toml filter=x\n";
    fs::write(work.join(".gitattributes"), attributes).unwrap();
    let out = set_version(&base, &work);
    assert!(!ran.exists(), "the filter ran: {}", all(&out));
}

#[test]
fn the_script_writes_through_no_link_in_the_work_copy() {
    let (base, work) = copy_with_origin();
    let outside = base.join("outside.txt");
    symlink(&outside, work.join("Cargo.toml.new")).unwrap();
    let out = set_version(&base, &work);
    assert!(!outside.exists(), "it wrote outside: {}", all(&out));
}

#[test]
fn the_script_gives_an_error_if_the_work_copy_is_not_there() {
    let (base, work) = copy_with_origin();
    let good = set_version(&base, &work);
    assert!(good.status.success(), "{}", all(&good));
    let out = set_version(&base, &base.join("no-such-copy"));
    assert!(!out.status.success(), "{}", all(&out));
}

// ---------------------------------------------------------------------
// The documents
// ---------------------------------------------------------------------

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

/// The position of the first line that starts with `start`.
fn line_at(lines: &[&str], start: &str) -> usize {
    let at = lines.iter().position(|l| l.starts_with(start));
    at.unwrap_or_else(|| panic!("no line {start}"))
}

#[test]
fn agents_md_has_the_two_commands_the_two_rows_and_the_word() {
    let commands = section("AGENTS.md", "Commands");
    assert!(commands.contains("codeumbrella queue"), "{commands}");
    let want = "codeumbrella run --next";
    assert!(commands.contains(want), "{commands}");
    let structure = section("AGENTS.md", "Structure");
    for name in ["`.codeumbrella/queue/`", "set-version"] {
        let row = structure.lines().find(|l| l.contains(name));
        assert!(row.is_some_and(|l| l.starts_with('|')), "{name}");
    }
    let words = section("AGENTS.md", "Words");
    let queue = words.lines().find(|l| l.starts_with("| Queue |"));
    assert!(queue.is_some(), "no row Queue");
}

#[test]
fn agents_md_has_the_new_limit_and_the_safety_line_for_the_script() {
    let agents = text("AGENTS.md");
    assert!(agents.contains("`src/lib.rs`): 1100 lines of code."));
    assert!(!agents.contains("`src/lib.rs`): 1000 lines of code."));
    let rules = section("AGENTS.md", "Safety rules");
    let line = rules.lines().find(|l| l.contains("set-version"));
    let line = line.unwrap_or_else(|| panic!("no safety line: {rules}"));
    assert!(line.contains("sandbox"), "{line}");
    let guards = ["--no-ext-diff", "--no-textconv", "--no-verify", "hooksPath"];
    for word in guards {
        assert!(rules.contains(word), "{word}");
    }
}

#[test]
fn the_default_sheet_comment_of_set_version_tells_what_it_does() {
    let sheet = text("graph.example.yaml");
    let lines: Vec<&str> = sheet.lines().collect();
    let at = line_at(&lines, "  set_version:");
    let mut comment = String::new();
    for line in lines[..at].iter().rev() {
        if !line.trim_start().starts_with('#') {
            break;
        }
        comment.push_str(&line.to_lowercase());
    }
    for word in [".codeumbrella/set-version", "version", "commit"] {
        assert!(comment.contains(word), "{word}: {comment}");
    }
}

#[test]
fn the_architecture_shows_set_version_between_learn_and_write_pr() {
    let path = section("ARCHITECTURE.md", "The path of a task");
    let lines: Vec<&str> = path.lines().collect();
    let learn = line_at(&lines, "learn");
    let set = line_at(&lines, "set_version");
    let write = line_at(&lines, "write_pr");
    assert!(learn < set && set < write, "{path}");
}

#[test]
fn the_readme_and_the_architecture_name_the_queue() {
    for rel in ["README.md", "ARCHITECTURE.md"] {
        let page = text(rel);
        assert!(page.contains("queue"), "{rel}: queue");
        assert!(page.contains("set_version"), "{rel}: set_version");
    }
}

#[test]
fn the_chat_page_runs_queue_after_done_and_starts_the_next_task() {
    let page = text(SKILL);
    assert!(page.contains("codeumbrella queue"), "queue");
    assert!(page.contains("run --next"), "run --next");
    assert!(page.contains("look_at_tests"), "look_at_tests");
}

#[test]
fn the_planner_and_builder_pages_tell_that_a_task_names_no_version() {
    for role in ["planner", "builder"] {
        let page = text(&format!("roles/{role}.md"));
        let lines = page.lines();
        let mut found = lines.filter(|l| l.contains("version"));
        let found = found.any(|l| l.contains("set_version"));
        assert!(found, "{role}: no line with version and set_version");
    }
}
