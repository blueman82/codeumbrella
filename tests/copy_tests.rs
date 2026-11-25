//! Tests: the step `copy_tests`, the list of the changed papers, the checker
//! page and the documents. The workers are simulated. No Claude, no Codex,
//! no network.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::{Graph, Kind};
use serde_json::Value;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

const STEP: &str = "copy_tests";
const BRANCH: &str = "codeumbrella/run-1";
const BEFORE: &str = "refs/codeumbrella/before";

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
    let name = format!("cu-copy-{}-{n}", std::process::id());
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
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

fn head(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"])
}

/// Writes a file and the folders above it.
fn put(base: &Path, rel: &str, body: &str) {
    let path = base.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn read(base: &Path, rel: &str) -> String {
    let read = fs::read_to_string(base.join(rel));
    read.unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// Makes the work copy of a run: a git folder with one commit and the git
/// name that the step `mark` writes.
fn make_work(work: &Path) {
    fs::create_dir_all(work).unwrap();
    git(work, &["init", "--quiet", "--initial-branch=master"]);
    git(work, &["config", "user.name", "t"]);
    git(work, &["config", "user.email", "t@t"]);
    put(work, "README.md", "mine");
    git(work, &["add", "-A"]);
    git(work, &["commit", "--quiet", "-m", "first"]);
    git(work, &["update-ref", BEFORE, "HEAD"]);
}

// ---------------------------------------------------------------------
// The commands of the step
// ---------------------------------------------------------------------

/// A project folder with the run `run-1`: its work copy and its paper
/// `tests`, which has only `README.md`.
struct World {
    dir: PathBuf,
    work: PathBuf,
    paper: PathBuf,
}

fn world() -> World {
    let dir = folder();
    let run = dir.join(".codeumbrella/runs/run-1");
    let work = run.join("work");
    let paper = run.join("papers/tests");
    make_work(&work);
    put(&paper, "README.md", "how to run the tests");
    World { dir, work, paper }
}

/// Runs the commands of the step as the supervisor does: in the project
/// folder, with the run id in each word. Gives `true` for a good result.
fn copy(w: &World) -> bool {
    let graph = default_sheet();
    let step = graph.step(STEP).unwrap();
    for words in &step.run {
        let with_id = words.iter().map(|x| x.replace("{run}", "run-1"));
        let words: Vec<String> = with_id.collect();
        let mut command = Command::new(&words[0]);
        command.args(&words[1..]).current_dir(&w.dir);
        if !command.output().unwrap().status.success() {
            return false;
        }
    }
    true
}

#[test]
fn the_sheet_has_copy_tests_between_mark_and_build() {
    let graph = default_sheet();
    let step = graph.step(STEP).unwrap();
    assert_eq!(step.kind, Kind::Command);
    assert_eq!(step.next.as_deref(), Some("build"));
    assert_eq!(step.on_bad, None);
    let mark = graph.step("mark").unwrap();
    assert_eq!(mark.next.as_deref(), Some(STEP));
    let ways = [(STEP, "mark"), ("build", STEP)];
    for (name, other) in &graph.steps {
        let targets = [&other.next, &other.on_bad];
        for (to, from) in ways {
            let goes = targets.contains(&&Some(to.to_owned()));
            assert_eq!(goes, name == from, "{name} to {to}");
        }
    }
}

#[test]
fn it_copies_each_file_but_the_readme_and_commits_on_the_run_branch() {
    let w = world();
    let first = head(&w.work);
    put(&w.paper, "tests/a.rs", "a");
    put(&w.paper, "tests/data/README.md", "data");
    put(&w.paper, "tests/with space.txt", "s");
    put(&w.paper, "top_test.txt", "t");
    assert!(copy(&w));
    assert_eq!(read(&w.work, "tests/a.rs"), "a");
    assert_eq!(read(&w.work, "tests/data/README.md"), "data");
    assert_eq!(read(&w.work, "tests/with space.txt"), "s");
    assert_eq!(read(&w.work, "top_test.txt"), "t");
    assert_eq!(read(&w.work, "README.md"), "mine");
    let on = git(&w.work, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(on, BRANCH);
    assert_eq!(git(&w.work, &["rev-parse", "master"]), first);
    assert_eq!(git(&w.work, &["rev-parse", "HEAD~1"]), first);
    let args = ["show", "--name-only", "--format=", "HEAD"];
    let names = git(&w.work, &args);
    let want = [
        "tests/a.rs",
        "tests/data/README.md",
        "tests/with space.txt",
        "top_test.txt",
    ];
    assert_eq!(names.lines().collect::<Vec<_>>(), want);
    assert_eq!(git(&w.work, &["status", "--porcelain"]), "");
}

#[test]
fn after_its_commit_the_mark_is_at_that_commit() {
    let w = world();
    let first = head(&w.work);
    put(&w.paper, "tests/x.rs", "one");
    assert!(copy(&w));
    assert_ne!(head(&w.work), first);
    assert_eq!(git(&w.work, &["rev-parse", BEFORE]), head(&w.work));
}

#[test]
fn a_corrected_paper_test_replaces_the_version_of_before() {
    let w = world();
    put(&w.paper, "tests/x.rs", "one");
    assert!(copy(&w));
    let one = head(&w.work);
    put(&w.paper, "tests/x.rs", "two");
    assert!(copy(&w));
    assert_eq!(read(&w.work, "tests/x.rs"), "two");
    assert_eq!(git(&w.work, &["rev-parse", "HEAD~1"]), one);
    assert_eq!(git(&w.work, &["status", "--porcelain"]), "");
}

#[test]
fn with_no_change_it_commits_nothing_and_is_good() {
    let w = world();
    put(&w.paper, "tests/x.rs", "one");
    assert!(copy(&w));
    let one = head(&w.work);
    assert!(copy(&w));
    assert_eq!(head(&w.work), one);
    assert_eq!(git(&w.work, &["rev-parse", BEFORE]), one);
}

#[test]
fn a_paper_with_only_the_readme_commits_nothing() {
    let w = world();
    let first = head(&w.work);
    put(&w.work, "staged.txt", "s");
    git(&w.work, &["add", "staged.txt"]);
    assert!(copy(&w));
    assert_eq!(head(&w.work), first);
    assert_eq!(read(&w.work, "README.md"), "mine");
}

#[test]
fn its_commit_has_no_other_change_of_the_work_copy() {
    let w = world();
    put(&w.work, "staged.txt", "s");
    git(&w.work, &["add", "staged.txt"]);
    put(&w.work, "README.md", "changed");
    put(&w.paper, "tests/x.rs", "one");
    assert!(copy(&w));
    let args = ["show", "--name-only", "--format=", "HEAD"];
    assert_eq!(git(&w.work, &args), "tests/x.rs");
    let status = git(&w.work, &["status", "--porcelain"]);
    assert!(status.contains("A  staged.txt"), "{status}");
    assert!(status.contains("M README.md"), "{status}");
}

#[test]
fn it_commits_a_test_file_that_gitignore_names() {
    let w = world();
    put(&w.work, ".gitignore", "*.snap\n");
    git(&w.work, &["add", "-A"]);
    git(&w.work, &["commit", "--quiet", "-m", "ignore"]);
    put(&w.paper, "tests/x.snap", "one");
    assert!(copy(&w));
    let args = ["show", "HEAD:tests/x.snap"];
    assert_eq!(git(&w.work, &args), "one");
}

/// Commits `tests/x.sh` with the right to run, and puts a paper version with
/// no such right.
fn script_world(paper: &str) -> World {
    let w = world();
    put(&w.work, "tests/x.sh", "one");
    let open = fs::Permissions::from_mode(0o755);
    fs::set_permissions(w.work.join("tests/x.sh"), open).unwrap();
    git(&w.work, &["add", "-A"]);
    git(&w.work, &["commit", "--quiet", "-m", "script"]);
    put(&w.paper, "tests/x.sh", paper);
    let plain = fs::Permissions::from_mode(0o644);
    fs::set_permissions(w.paper.join("tests/x.sh"), plain).unwrap();
    w
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn a_target_with_the_same_content_keeps_its_mode() {
    let w = script_world("one");
    let first = head(&w.work);
    assert!(copy(&w));
    assert_eq!(mode(&w.work.join("tests/x.sh")), 0o755);
    assert_eq!(head(&w.work), first);
    assert_eq!(git(&w.work, &["status", "--porcelain"]), "");
}

#[test]
fn a_target_with_new_content_keeps_its_mode() {
    let w = script_world("two");
    assert!(copy(&w));
    assert_eq!(read(&w.work, "tests/x.sh"), "two");
    assert_eq!(mode(&w.work.join("tests/x.sh")), 0o755);
    let entry = git(&w.work, &["ls-tree", "HEAD", "tests/x.sh"]);
    assert!(entry.starts_with("100755 "), "{entry}");
    assert_eq!(git(&w.work, &["status", "--porcelain"]), "");
}

#[test]
fn a_name_with_brackets_is_not_a_pattern() {
    let w = world();
    put(&w.work, "tests/i.js", "mine");
    git(&w.work, &["add", "-A"]);
    git(&w.work, &["commit", "--quiet", "-m", "i"]);
    put(&w.work, "tests/i.js", "changed");
    put(&w.work, "tests/d.js", "new");
    put(&w.paper, "tests/[id].js", "one");
    assert!(copy(&w));
    let args = ["show", "--name-only", "--format=", "HEAD"];
    assert_eq!(git(&w.work, &args), "tests/[id].js");
    let one = head(&w.work);
    // A second copy with no change also reads the name as plain text.
    assert!(copy(&w));
    assert_eq!(head(&w.work), one);
    let staged = ["diff", "--cached", "--name-only"];
    assert_eq!(git(&w.work, &staged), "");
    let status = git(&w.work, &["status", "--porcelain"]);
    assert!(status.contains("M tests/i.js"), "{status}");
    assert!(status.contains("?? tests/d.js"), "{status}");
}

#[test]
fn with_no_paper_folder_the_result_is_bad() {
    let w = world();
    fs::remove_dir_all(&w.paper).unwrap();
    assert!(!copy(&w));
}

// ---------------------------------------------------------------------
// A test change from master
// ---------------------------------------------------------------------

const FIRST: &str = "1\n2\n3\n4\n5\n6\n7\n";
const PAPER: &str = "one\n2\n3\n4\n5\n6\n7\n";

/// The project has `tests/x.rs`. The step copies the first paper version,
/// which changes line 1. Then master changes line 7, and a merge brings
/// that change into the run branch.
fn merged_world() -> World {
    let w = world();
    put(&w.work, "tests/x.rs", FIRST);
    git(&w.work, &["add", "-A"]);
    git(&w.work, &["commit", "--quiet", "-m", "x"]);
    put(&w.paper, "tests/x.rs", PAPER);
    assert!(copy(&w));
    git(&w.work, &["checkout", "--quiet", "master"]);
    put(&w.work, "tests/x.rs", "1\n2\n3\n4\n5\n6\nmaster\n");
    git(&w.work, &["commit", "--quiet", "-am", "master"]);
    git(&w.work, &["checkout", "--quiet", BRANCH]);
    git(&w.work, &["merge", "--quiet", "--no-edit", "master"]);
    w
}

/// The work copy and its last commit have the paper version.
fn has_paper_version(w: &World, want: &str) {
    assert_eq!(read(&w.work, "tests/x.rs"), want);
    let args = ["show", "HEAD:tests/x.rs"];
    assert_eq!(git(&w.work, &args), want.trim());
    assert_eq!(git(&w.work, &["status", "--porcelain"]), "");
}

#[test]
fn an_unchanged_paper_test_replaces_the_change_of_master() {
    let w = merged_world();
    assert!(copy(&w));
    has_paper_version(&w, PAPER);
}

#[test]
fn a_corrected_paper_test_replaces_the_change_of_master() {
    let w = merged_world();
    let paper = "one\n2\n3\n4\n5\n6\nplanner\n";
    put(&w.paper, "tests/x.rs", paper);
    assert!(copy(&w));
    has_paper_version(&w, paper);
}

#[test]
fn a_test_that_master_removed_gets_the_paper_version() {
    let w = world();
    put(&w.paper, "tests/x.rs", "one");
    assert!(copy(&w));
    git(&w.work, &["rm", "--quiet", "tests/x.rs"]);
    git(&w.work, &["commit", "--quiet", "-m", "removed"]);
    assert!(copy(&w));
    has_paper_version(&w, "one");
}

#[test]
fn it_keeps_no_version_and_no_record_in_the_run_folder() {
    let w = merged_world();
    assert!(copy(&w));
    let run = w.dir.join(".codeumbrella/runs/run-1");
    let entries = fs::read_dir(run).unwrap();
    let names = entries.map(|e| e.unwrap().file_name());
    let mut names: Vec<_> = names.collect();
    names.sort();
    assert_eq!(names, ["papers", "work"]);
}

// ---------------------------------------------------------------------
// The walls of the step
// ---------------------------------------------------------------------

#[test]
fn it_refuses_a_paper_folder_that_is_a_link() {
    let w = world();
    let first = head(&w.work);
    let home = w.dir.join("home");
    put(&home, "tests/key.rs", "secret");
    fs::remove_dir_all(&w.paper).unwrap();
    symlink(&home, &w.paper).unwrap();
    assert!(!copy(&w));
    assert!(!w.work.join("tests").exists(), "a copy");
    assert_eq!(head(&w.work), first);
}

/// A script that makes the file `ran` in `dir`. Gives the two paths.
fn marker(dir: &Path) -> (PathBuf, PathBuf) {
    let ran = dir.join("ran");
    let script = dir.join("marker.sh");
    let body = format!("#!/bin/sh\ntouch '{}'\n", ran.display());
    fs::write(&script, body).unwrap();
    let open = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&script, open).unwrap();
    (script, ran)
}

#[test]
fn it_runs_no_hook_of_the_work_copy() {
    let w = world();
    let (script, ran) = marker(&w.dir);
    let hooks = w.work.join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    let names = [
        "pre-commit",
        "prepare-commit-msg",
        "commit-msg",
        "post-commit",
        "post-checkout",
        "post-index-change",
        "reference-transaction",
    ];
    for name in names {
        fs::copy(&script, hooks.join(name)).unwrap();
    }
    put(&w.paper, "tests/x.rs", "one");
    assert!(copy(&w));
    assert!(!ran.exists(), "a hook ran");
    assert_eq!(read(&w.work, "tests/x.rs"), "one");
}

#[test]
fn it_refuses_a_config_entry_that_is_not_of_a_clean_clone() {
    let keys = [
        "filter.x.clean",
        "core.fsmonitor",
        "core.hooksPath",
        "core.sshCommand",
        "url.x.insteadOf",
    ];
    for key in keys {
        let w = world();
        let first = head(&w.work);
        let (script, ran) = marker(&w.dir);
        let path = script.display().to_string();
        git(&w.work, &["config", key, &path]);
        put(&w.work, ".git/info/attributes", "* filter=x\n");
        put(&w.paper, "tests/x.rs", "one");
        assert!(!copy(&w), "{key}: a good result");
        assert!(!ran.exists(), "{key}: the command of the entry ran");
        assert!(!w.work.join("tests").exists(), "{key}: a copy");
        git(&w.work, &["config", "--unset", key]);
        assert_eq!(head(&w.work), first, "{key}");
    }
}

#[test]
fn it_refuses_a_paper_with_a_link() {
    for kind in ["symbolic file", "symbolic folder", "hard"] {
        let w = world();
        let secret = w.dir.join("home/key.rs");
        put(&w.dir, "home/key.rs", "secret");
        put(&w.paper, "a.txt", "a");
        let tests = w.paper.join("tests");
        if kind == "symbolic folder" {
            symlink(w.dir.join("home"), &tests).unwrap();
        } else {
            fs::create_dir_all(&tests).unwrap();
            let link = tests.join("key.rs");
            if kind == "hard" {
                fs::hard_link(&secret, link).unwrap();
            } else {
                symlink(&secret, link).unwrap();
            }
        }
        assert!(!copy(&w), "{kind}: a good result");
        assert!(!w.work.join("tests/key.rs").exists(), "{kind}");
        assert!(!w.work.join("a.txt").exists(), "{kind}: a copy");
    }
}

#[test]
fn it_writes_nothing_through_a_linked_folder_of_the_work_copy() {
    let w = world();
    let home = w.dir.join("home");
    fs::create_dir_all(&home).unwrap();
    symlink(&home, w.work.join("tests")).unwrap();
    put(&w.paper, "a.txt", "a");
    put(&w.paper, "tests/x.rs", "one");
    assert!(!copy(&w));
    assert!(!home.join("x.rs").exists());
    assert!(!w.work.join("a.txt").exists(), "a copy");
}

#[test]
fn it_refuses_a_target_that_is_a_link_or_a_folder() {
    for kind in ["link", "folder"] {
        let w = world();
        put(&w.dir, "home/mine.txt", "mine");
        let target = w.work.join("tests/x.rs");
        fs::create_dir_all(w.work.join("tests")).unwrap();
        if kind == "link" {
            let mine = w.dir.join("home/mine.txt");
            symlink(mine, &target).unwrap();
        } else {
            fs::create_dir(&target).unwrap();
        }
        put(&w.paper, "a.txt", "a");
        put(&w.paper, "tests/x.rs", "one");
        assert!(!copy(&w), "{kind}: a good result");
        assert_eq!(read(&w.dir, "home/mine.txt"), "mine");
        assert!(!w.work.join("a.txt").exists(), "{kind}: a copy");
        assert!(!target.join("x.rs").exists(), "{kind}");
    }
}

#[test]
fn it_does_not_write_into_a_file_with_a_second_name() {
    let w = world();
    put(&w.dir, "home/mine.txt", "mine");
    fs::create_dir_all(w.work.join("tests")).unwrap();
    let mine = w.dir.join("home/mine.txt");
    fs::hard_link(mine, w.work.join("tests/x.rs")).unwrap();
    put(&w.paper, "tests/x.rs", "one");
    assert!(copy(&w));
    assert_eq!(read(&w.dir, "home/mine.txt"), "mine");
    assert_eq!(read(&w.work, "tests/x.rs"), "one");
}

#[test]
fn it_refuses_a_path_into_a_locked_folder() {
    let paths = [
        ".git/hooks/pre-commit",
        ".git/config",
        ".GIT/description",
        "a/.claude/settings.json",
        ".codeumbrella/graph.yaml",
        ".github/workflows/x.yml",
    ];
    for rel in paths {
        let w = world();
        let config = read(&w.work, ".git/config");
        let first = head(&w.work);
        put(&w.paper, "a.txt", "a");
        put(&w.paper, rel, "bad");
        assert!(!copy(&w), "{rel}: a good result");
        assert!(!w.work.join("a.txt").exists(), "{rel}: a copy");
        assert_eq!(read(&w.work, ".git/config"), config, "{rel}");
        let made = fs::read_to_string(w.work.join(rel));
        assert_ne!(made.unwrap_or_default(), "bad", "{rel}");
        assert_eq!(head(&w.work), first, "{rel}");
    }
}

// ---------------------------------------------------------------------
// The path of a run, with a simulated worker
// ---------------------------------------------------------------------

/// The default sheet for a run in `dir`. The steps `mark`, `copy_tests` and
/// `work_was_done` run their commands there. Each other command step does
/// nothing.
fn sheet(dir: &Path) -> Graph {
    let mut graph = default_sheet();
    let here = dir.display().to_string();
    for (name, step) in &mut graph.steps {
        if ["mark", STEP, "work_was_done"].contains(&name.as_str()) {
            for words in &mut step.run {
                let go = "cd \"$0\" && exec \"$@\"";
                let front = ["sh", "-c", go, &here].map(str::to_owned);
                let mut all = front.to_vec();
                all.append(words);
                *words = all;
            }
        } else if !step.run.is_empty() {
            step.run = vec![vec!["true".to_owned()]];
        }
    }
    graph
}

/// A simulated worker. The planner writes a new version of one paper test in
/// each of its runs. The step `build` keeps the version that it finds.
struct Sim {
    commits: bool,
    plans: Mutex<u32>,
    seen: Mutex<Vec<String>>,
}

impl Sim {
    fn new(commits: bool) -> Self {
        Self {
            commits,
            plans: Mutex::default(),
            seen: Mutex::default(),
        }
    }
}

impl Worker for Sim {
    fn run(&self, job: &Job) -> Result<Outcome> {
        let papers = job.dir.join("papers");
        let work = job.dir.join("work");
        for paper in job.writes {
            put(&papers.join(paper), "file.md", job.step);
        }
        if job.step == "plan" {
            let mut plans = self.plans.lock().unwrap();
            *plans += 1;
            let body = format!("version {plans}");
            put(&papers, "tests/tests/x.rs", &body);
        }
        let mut ok = true;
        if job.step == "build" {
            let found = fs::read_to_string(work.join("tests/x.rs"));
            let mut seen = self.seen.lock().unwrap();
            seen.push(found.unwrap_or_default());
            // The first build finds a bad paper test.
            ok = !self.commits || seen.len() > 1;
            if self.commits {
                let name = format!("built{}", seen.len());
                put(&work, &name, "b");
                git(&work, &["add", &name]);
                git(&work, &["commit", "--quiet", "-m", &name]);
            }
        }
        Ok(Outcome {
            ok,
            summary: format!("{} ran", job.step),
        })
    }
}

/// The steps in the log, with their results.
fn steps(run: &Run) -> Vec<(String, bool)> {
    let log = read(&run.dir, "log.jsonl");
    let mut found = Vec::new();
    for line in log.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        let step = value["step"].as_str().unwrap().to_owned();
        found.push((step, value["ok"].as_bool().unwrap()));
    }
    found
}

fn start(dir: &Path, graph: &Graph) -> Run {
    let place = dir.join(".codeumbrella/runs/run-1");
    let run = Run::start(place, "a task", graph).unwrap();
    make_work(&run.dir.join("work"));
    run
}

/// Does the run to the next stop, answers yes, and goes on.
fn yes(run: &mut Run, graph: &Graph, worker: &Sim) {
    let roles = root().join("roles");
    run.go(graph, &roles, worker).unwrap();
    let at = run.state.at.as_deref();
    assert_eq!(at, Some("look_at_tests"), "{}", run.state.note);
    run.answer(true, "").unwrap();
    run.go(graph, &roles, worker).unwrap();
}

#[test]
fn after_a_corrected_paper_test_the_build_finds_the_new_version() {
    let dir = folder();
    let graph = sheet(&dir);
    let mut run = start(&dir, &graph);
    let worker = Sim::new(true);
    yes(&mut run, &graph, &worker);
    // The first build was bad, so the planner corrected the test.
    let at = run.state.at.clone();
    assert_eq!(at.as_deref(), Some("look_at_tests"));
    run.answer(true, "").unwrap();
    let roles = root().join("roles");
    run.go(&graph, &roles, &worker).unwrap();
    let seen = worker.seen.lock().unwrap().clone();
    assert_eq!(seen, ["version 1", "version 2"]);
    let at = run.state.at.as_deref();
    assert_eq!(at, Some("approval"), "{}", run.state.note);
}

#[test]
fn work_was_done_counts_no_commit_of_copy_tests() {
    let dir = folder();
    let graph = sheet(&dir);
    let mut run = start(&dir, &graph);
    let work = run.dir.join("work");
    let first = head(&work);
    let worker = Sim::new(false);
    yes(&mut run, &graph, &worker);
    assert_ne!(head(&work), first, "{}", run.state.note);
    let all = steps(&run);
    let copied = (STEP.to_owned(), true);
    assert!(all.contains(&copied), "{all:?}");
    let bad = ("work_was_done".to_owned(), false);
    let good = ("work_was_done".to_owned(), true);
    assert!(all.contains(&bad), "{all:?}");
    assert!(!all.contains(&good), "{all:?}");
    assert_eq!(run.state.status, Status::Stopped);
}

// ---------------------------------------------------------------------
// The list of the changed papers, with the built command
// ---------------------------------------------------------------------

/// A sheet with a person step before and after the builder.
const SHEET: &str = r#"start: copy
steps:
  copy:
    type: command
    run:
      - [git, clone, --quiet, --no-hardlinks, ., ".codeumbrella/runs/{run}/work"]
    next: plan
  plan:
    role: planner
    writes: [spec, tests]
    max_tries: 3
    next: look
  look:
    type: person
    next: build
  build:
    role: builder
    reads: [spec]
    next: check
  check:
    type: fan
    fan: [check_a]
    next: ok
  check_a:
    role: checker
    reads: [spec]
  ok:
    type: person
    on_bad: plan
"#;

/// A simulated `claude`. It keeps each prompt in `bin/<step>.<number>`. The
/// planner runs `bin/plan<number>.sh`, which writes the papers.
const CLAUDE: &str = r#"#!/bin/sh
bin='BIN'
step=$(printf '%s\n' "$2" | sed -n 's/^You do the step "\([^"]*\)".*/\1/p')
n=1
while [ -f "$bin/$step.$n" ]; do n=$((n+1)); done
printf '%s\n' "$2" > "$bin/$step.$n"
if [ "$step" = plan ]; then sh "$bin/plan$n.sh"; fi
echo '{"is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'
"#;

const PLAN_1: &str = "mkdir -p ../papers/tests/tests
echo s > ../papers/spec/spec.md
echo one > ../papers/tests/tests/x.rs
echo same > ../papers/tests/tests/y.rs
echo g > ../papers/tests/tests/gone.rs
";

const PLAN_2: &str = "echo s > ../papers/spec/spec.md
echo two > ../papers/tests/tests/x.rs
echo n > ../papers/tests/tests/new.rs
rm ../papers/tests/tests/gone.rs
";

/// Starts the built command, with the simulated tool first on the PATH.
fn cu(dir: &Path, args: &[&str]) -> Output {
    let old = env::var("PATH").unwrap();
    let path = format!("{}:{old}", dir.join("bin").display());
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(args).current_dir(dir).env("PATH", path);
    let out = command.output().unwrap();
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{args:?}: {error}");
    out
}

/// A run of `SHEET` through a no at `ok`, to the second check.
fn two_builds() -> PathBuf {
    let dir = folder();
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    cu(&dir, &["init"]);
    put(&dir, ".codeumbrella/graph.yaml", SHEET);
    let tool = bin.join("claude");
    let script = CLAUDE.replace("BIN", &bin.display().to_string());
    fs::write(&tool, script).unwrap();
    let open = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&tool, open).unwrap();
    put(&bin, "plan1.sh", PLAN_1);
    put(&bin, "plan2.sh", PLAN_2);
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["commit", "--quiet", "--allow-empty", "-m", "x"]);
    let out = cu(&dir, &["run", "a task"]);
    let shown = String::from_utf8_lossy(&out.stdout).into_owned();
    let line = shown.lines().find_map(|l| l.strip_prefix("Run:"));
    let id = line.expect("no run id").trim().to_owned();
    cu(&dir, &["answer", &id, "look", "yes"]);
    cu(&dir, &["answer", &id, "ok", "no", "again"]);
    cu(&dir, &["answer", &id, "look", "yes"]);
    bin
}

/// The line of a prompt about the changes of the paper `tests`.
fn changed_line(prompt: &str) -> &str {
    let head = "the paper `tests` changed: ";
    let found = prompt.lines().find(|l| l.contains(head));
    found.unwrap_or_else(|| panic!("no list: {prompt}"))
}

#[test]
fn a_worker_of_a_second_run_gets_the_changed_files_of_each_paper() {
    let bin = two_builds();
    for name in ["build.2", "check_a.2"] {
        let prompt = read(&bin, name);
        let line = changed_line(&prompt);
        for file in ["x.rs", "new.rs", "gone.rs"] {
            let want = format!("`tests/{file}`");
            assert!(line.contains(&want), "{name}: {line}");
        }
        assert!(!line.contains("y.rs"), "{name}: {line}");
        let spec = "the paper `spec` changed";
        assert!(!prompt.contains(spec), "{name}: {prompt}");
    }
}

#[test]
fn a_first_run_and_a_run_with_no_change_get_no_list() {
    let bin = two_builds();
    // The list is there for the second build, so the code is there.
    changed_line(&read(&bin, "build.2"));
    // `plan` wrote the papers itself, and no one changed them after it.
    for name in ["plan.1", "build.1", "check_a.1", "plan.2"] {
        let prompt = read(&bin, name);
        assert!(!prompt.contains("changed:"), "{name}: {prompt}");
    }
}

/// The fingerprint of a paper as the supervisor makes it today. `files` are
/// in the sequence of their paths.
fn print_of_today(paper: &Path, files: &[&str]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for file in files {
        Path::new(file).hash(&mut hasher);
        fs::read(paper.join(file)).unwrap().hash(&mut hasher);
    }
    hasher.finish()
}

/// This test is good before the work too. It keeps the value of today.
#[test]
fn a_waiting_notebook_with_the_fingerprint_of_today_continues() {
    let dir = folder();
    let person = "start: ok\nsteps:\n  ok:\n    type: person\n";
    put(&dir, "graph.yaml", person);
    let roles = root().join("roles");
    let graph = Graph::load(&dir.join("graph.yaml"), &roles).unwrap();
    let mut run = Run::start(dir.join("run-1"), "a task", &graph).unwrap();
    let paper = run.dir.join("papers/spec");
    put(&paper, "a/b.txt", "b");
    put(&paper, "spec.md", "s");
    let print = print_of_today(&paper, &["a/b.txt", "spec.md"]);
    run.state.prints.insert("spec".to_owned(), print);
    run.state.status = Status::Waiting;
    run.save().unwrap();
    // No answer comes first, because an answer makes the fingerprints again.
    let mut run = Run::open(run.dir).unwrap();
    run.go(&graph, &roles, &Sim::new(false)).unwrap();
    assert_eq!(run.state.status, Status::Waiting, "{}", run.state.note);
}

// ---------------------------------------------------------------------
// The checker page and the documents
// ---------------------------------------------------------------------

#[test]
fn the_checker_page_compares_and_does_not_copy_or_commit() {
    let page = text("roles/checker.md");
    assert!(page.contains("compare each test file"), "{page}");
    for gone in ["Write it into the project", "Commit the test files"] {
        assert!(!page.contains(gone), "{gone}");
    }
}

/// The first line of a page that has all the words.
fn line_with(rel: &str, words: &[&str]) -> String {
    let page = text(rel);
    let has = |l: &&str| words.iter().all(|w| l.contains(w));
    let found = page.lines().find(has);
    let found = found.unwrap_or_else(|| panic!("{rel}: no line {words:?}"));
    found.to_owned()
}

#[test]
fn the_checker_page_gives_a_bad_result_for_a_differing_test() {
    let words = ["differs", "not there", "ok: false", "summary"];
    line_with("roles/checker.md", &words);
}

#[test]
fn builder_rule_9_copies_the_paper_file_again() {
    let page = text("roles/builder.md");
    let (_, from) = page.split_once("\n9. ").expect("no rule 9");
    let (nine, _) = from.split_once("\n10. ").expect("no rule 10");
    let words = ["copies", "again", "commits"];
    for word in words {
        let sub = |l: &str| l.trim_start().starts_with("- ");
        let mut lines = nine.lines().filter(|l| sub(l));
        let found = lines.any(|l| l.contains("again") && l.contains(word));
        assert!(found, "{word}: {nine}");
    }
}

#[test]
fn the_planner_page_tells_the_path_of_a_paper_test() {
    let words = ["paper `tests`", "path", "project folder"];
    line_with("roles/planner.md", &words);
    line_with("roles/planner.md", &["version in the work copy"]);
}

#[test]
fn the_user_page_does_not_say_that_a_checker_commits() {
    let page = text("roles/user.md");
    assert!(page.contains("Do not copy the files of the work copy"));
    for line in page.lines() {
        let says = line.contains("checker") && line.contains("commits");
        assert!(!says, "{line}");
    }
}

/// The text of the part `## title` of a document.
fn section(path: &str, title: &str) -> String {
    let page = text(path);
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
    assert!(!found.is_empty(), "{path}: no part {title}");
    found.join("\n")
}

#[test]
fn the_documents_and_the_sheet_tell_the_new_step() {
    let agents = section("AGENTS.md", "The sheet");
    assert!(agents.contains(STEP), "AGENTS.md");
    let path = section("ARCHITECTURE.md", "The path of a task");
    assert!(path.contains(STEP), "ARCHITECTURE.md");
    let sheet = text("graph.example.yaml");
    let (above, _) = sheet.split_once("\n  copy_tests:").unwrap();
    let lines = above.lines().rev();
    let comment: Vec<&str> = lines
        .take_while(|l| l.trim_start().starts_with('#'))
        .collect();
    let comment = comment.join("\n").to_lowercase();
    assert!(comment.contains("commit"), "{comment}");
    assert!(comment.contains("tests"), "{comment}");
}
