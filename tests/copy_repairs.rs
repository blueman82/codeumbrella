//! Tests: three repairs of the step `copy_tests`. The run branch keeps its
//! commits, a lesson test has its own file, and a merge that did not
//! complete gives a clear message. The workers are simulated.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::Graph;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

const STEP: &str = "copy_tests";
const BRANCH: &str = "codeumbrella/run-1";
const BEFORE: &str = "refs/codeumbrella/before";
const MERGE: &str = "a merge that did not complete";
const MESSAGE: &str = "refused, a merge that did not complete";

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

/// A new empty folder, with its full path.
fn folder() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-repairs-{}-{n}", std::process::id());
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

/// Runs `git` in a folder, with a name and an email.
fn git_output(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    command.args(["-c", "user.name=t", "-c", "user.email=t@t"]);
    command.args(["-c", "commit.gpgsign=false"]);
    command.args(args).output().unwrap()
}

/// Runs a good `git` command. Gives the output text.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = git_output(dir, args);
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "git {args:?}: {error}");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn head(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"])
}

fn on(dir: &Path) -> String {
    git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
}

fn put(base: &Path, rel: &str, body: &str) {
    let path = base.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn commit(work: &Path, rel: &str, body: &str) {
    put(work, rel, body);
    git(work, &["add", rel]);
    git(work, &["commit", "--quiet", "-m", body]);
}

/// The work copy of a run: one commit and the git name of the step `mark`.
fn make_work(work: &Path) {
    fs::create_dir_all(work).unwrap();
    git(work, &["init", "--quiet", "--initial-branch=master"]);
    git(work, &["config", "user.name", "t"]);
    git(work, &["config", "user.email", "t@t"]);
    commit(work, "README.md", "mine");
    git(work, &["update-ref", BEFORE, "HEAD"]);
}

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

/// Runs the commands of the step. Gives `true` and the error text.
fn copy(w: &World) -> (bool, String) {
    let graph = default_sheet();
    let step = graph.step(STEP).unwrap();
    for words in &step.run {
        let with_id = words.iter().map(|x| x.replace("{run}", "run-1"));
        let words: Vec<String> = with_id.collect();
        let mut command = Command::new(&words[0]);
        command.args(&words[1..]).current_dir(&w.dir);
        let out = command.output().unwrap();
        if !out.status.success() {
            let error = String::from_utf8_lossy(&out.stderr);
            return (false, error.into_owned());
        }
    }
    (true, String::new())
}

// ---------------------------------------------------------------------
// A run branch at a commit that is not `HEAD`.
// These three tests are good before the work too. They keep the commits.
// ---------------------------------------------------------------------

/// The run branch has one commit more than `master`. `HEAD` is on
/// `master`. Gives that commit.
fn branch_with_a_commit(w: &World) -> String {
    git(&w.work, &["checkout", "--quiet", "-b", BRANCH]);
    commit(&w.work, "built.txt", "built");
    let kept = head(&w.work);
    git(&w.work, &["checkout", "--quiet", "master"]);
    kept
}

#[test]
fn a_run_branch_ahead_of_head_keeps_its_commits() {
    let w = world();
    let first = head(&w.work);
    let kept = branch_with_a_commit(&w);
    put(&w.paper, "tests/x.rs", "one");
    let (good, error) = copy(&w);
    assert!(good, "{error}");
    assert_eq!(on(&w.work), BRANCH);
    assert_eq!(git(&w.work, &["rev-parse", "HEAD~1"]), kept);
    assert_eq!(git(&w.work, &["show", "HEAD:built.txt"]), "built");
    assert_eq!(git(&w.work, &["rev-parse", "master"]), first);
}

#[test]
fn a_run_branch_that_differs_from_head_keeps_its_commits() {
    let w = world();
    let kept = branch_with_a_commit(&w);
    commit(&w.work, "other.txt", "other");
    let master = head(&w.work);
    put(&w.paper, "tests/x.rs", "one");
    let (good, error) = copy(&w);
    assert!(good, "{error}");
    assert_eq!(on(&w.work), BRANCH);
    assert_eq!(git(&w.work, &["rev-parse", "HEAD~1"]), kept);
    assert_eq!(git(&w.work, &["rev-parse", "master"]), master);
    assert_eq!(git(&w.work, &["rev-parse", BEFORE]), head(&w.work));
}

#[test]
fn with_no_file_to_copy_the_run_branch_does_not_move() {
    let w = world();
    let kept = branch_with_a_commit(&w);
    commit(&w.work, "other.txt", "other");
    let (good, error) = copy(&w);
    assert!(good, "{error}");
    assert_eq!(on(&w.work), BRANCH);
    assert_eq!(head(&w.work), kept);
}

// ---------------------------------------------------------------------
// A merge that did not complete
// ---------------------------------------------------------------------

/// Commits on `side` and on the branch of this moment. With the same file,
/// a merge of `side` stops for two changes to one line.
fn two_sides(work: &Path, theirs: &str, ours: &str) {
    let start = on(work);
    git(work, &["checkout", "--quiet", "-b", "side"]);
    commit(work, theirs, "theirs");
    git(work, &["checkout", "--quiet", &start]);
    commit(work, ours, "ours");
}

/// A merge with two changes to one line. `kind` is `--no-ff` or `--squash`.
fn stopped_merge(work: &Path, kind: &str) {
    two_sides(work, "README.md", "README.md");
    let out = git_output(work, &["merge", kind, "side"]);
    assert!(!out.status.success(), "the merge did complete");
}

/// A merge with no line with two changes, and no commit.
fn open_merge(work: &Path) {
    two_sides(work, "theirs.txt", "ours.txt");
    git(work, &["merge", "--no-ff", "--no-commit", "side"]);
}

/// The step gives a bad result with the cause, and changes nothing.
fn refuses_and_changes_nothing(w: &World, name: &str) {
    let first = head(&w.work);
    let branch = on(&w.work);
    let before = git(&w.work, &["rev-parse", BEFORE]);
    let status = git(&w.work, &["status", "--porcelain"]);
    let merge_head = w.work.join(".git/MERGE_HEAD").exists();
    let (good, error) = copy(w);
    assert!(!good, "{name}: a good result");
    assert!(error.contains(MESSAGE), "{name}: {error}");
    assert_eq!(head(&w.work), first, "{name}");
    assert_eq!(on(&w.work), branch, "{name}");
    assert_eq!(git(&w.work, &["rev-parse", BEFORE]), before, "{name}");
    let after = git(&w.work, &["status", "--porcelain"]);
    assert_eq!(after, status, "{name}");
    let kept = w.work.join(".git/MERGE_HEAD").exists();
    assert_eq!(kept, merge_head, "{name}");
    assert!(!w.work.join("tests").exists(), "{name}: a copy");
}

#[test]
fn a_merge_with_two_changes_to_a_line_gives_the_cause() {
    let w = world();
    stopped_merge(&w.work, "--no-ff");
    put(&w.paper, "tests/x.rs", "one");
    refuses_and_changes_nothing(&w, "two changes");
}

#[test]
fn a_merge_with_no_commit_gives_the_cause() {
    let w = world();
    open_merge(&w.work);
    put(&w.paper, "tests/x.rs", "one");
    refuses_and_changes_nothing(&w, "no commit");
}

#[test]
fn a_squash_merge_with_two_changes_gives_the_cause() {
    let w = world();
    stopped_merge(&w.work, "--squash");
    put(&w.paper, "tests/x.rs", "one");
    refuses_and_changes_nothing(&w, "squash");
}

#[test]
fn a_merge_on_the_run_branch_is_bad_also_with_no_file_to_copy() {
    let w = world();
    git(&w.work, &["checkout", "--quiet", "-b", BRANCH]);
    open_merge(&w.work);
    refuses_and_changes_nothing(&w, "run branch");
}

// ---------------------------------------------------------------------
// The note of a stopped run
// ---------------------------------------------------------------------

/// The default sheet for a run in `dir`. The steps `mark` and `copy_tests`
/// run their commands there. Each other command step does nothing.
fn sheet(dir: &Path) -> Graph {
    let mut graph = default_sheet();
    let here = dir.display().to_string();
    for (name, step) in &mut graph.steps {
        if ["mark", STEP].contains(&name.as_str()) {
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

struct Sim;

impl Worker for Sim {
    fn run(&self, job: &Job) -> Result<Outcome> {
        let papers = job.dir.join("papers");
        for paper in job.writes {
            put(&papers.join(paper), "file.md", job.step);
        }
        if job.step == "plan" {
            put(&papers, "tests/tests/x.rs", "one");
        }
        Ok(Outcome {
            ok: true,
            summary: format!("{} ran", job.step),
        })
    }
}

#[test]
fn the_first_300_characters_of_the_note_have_the_cause() {
    let dir = folder();
    let graph = sheet(&dir);
    let place = dir.join(".codeumbrella/runs/run-1");
    let mut run = Run::start(place, "a task", &graph).unwrap();
    let work = run.dir.join("work");
    make_work(&work);
    stopped_merge(&work, "--no-ff");
    git(&work, &["update-ref", BEFORE, "HEAD"]);
    let roles = root().join("roles");
    run.go(&graph, &roles, &Sim).unwrap();
    let at = run.state.at.as_deref();
    assert_eq!(at, Some("look_at_tests"), "{}", run.state.note);
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &Sim).unwrap();
    assert_eq!(run.state.status, Status::Stopped);
    let note = &run.state.note;
    // The status text shows only the first 300 characters.
    let shown: String = note.chars().take(300).collect();
    assert!(shown.contains(STEP), "{shown}");
    assert!(shown.contains(MESSAGE), "{shown}");
    // The note keeps the command, after the cause.
    assert!(note.contains("core.fsmonitor=false"), "{note}");
}

// ---------------------------------------------------------------------
// The rules pages and the documents
// ---------------------------------------------------------------------

/// The text of rule `n` of a rules page.
fn rule(rel: &str, n: u32) -> String {
    let page = text(rel);
    let start = format!("\n{n}. ");
    let end = format!("\n{}. ", n + 1);
    let (_, from) = page.split_once(&start).expect("no rule");
    let (found, _) = from.split_once(&end).expect("no next rule");
    found.to_owned()
}

#[test]
fn the_learner_puts_a_lesson_test_into_a_new_file() {
    let six = rule("roles/learner.md", 6);
    let lines = [
        "Put each lesson test into a new file of its own.",
        "Do not add a lesson test to a file of the paper `tests`.",
    ];
    for line in lines {
        assert!(six.contains(line), "{line}: {six}");
    }
}

#[test]
fn rule_9_of_the_builder_has_no_two_demands_for_one_file() {
    let nine = rule("roles/builder.md", 9);
    let lines = [
        "A file of the paper `tests` is not a lesson test, \
         also if `tests.txt` names it.",
        "For such a file, the paper file wins.",
    ];
    for line in lines {
        assert!(nine.contains(line), "{line}: {nine}");
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
fn agents_tells_the_new_rules() {
    let part = section("AGENTS.md", "The sheet");
    assert!(part.contains(MERGE), "{part}");
    let page = text("AGENTS.md");
    let lesson = |l: &str| l.contains("lesson test") && l.contains("file of its own");
    assert!(page.lines().any(lesson), "no lesson line");
    let table = section("AGENTS.md", "Structure");
    let row = |l: &str| l.starts_with('|') && l.contains("`tests/copy_repairs.rs`");
    assert!(table.lines().any(row), "{table}");
}

#[test]
fn the_comment_of_the_step_tells_the_merge_rule() {
    let sheet = text("graph.example.yaml");
    let (above, _) = sheet.split_once("\n  copy_tests:").unwrap();
    let lines = above.lines().rev();
    let comment = lines.take_while(|l| l.trim_start().starts_with('#'));
    let mut comment: Vec<&str> = comment
        .map(|l| l.trim_start().trim_start_matches('#').trim())
        .collect();
    comment.reverse();
    let comment = comment.join(" ");
    assert!(comment.contains(MERGE), "{comment}");
}
