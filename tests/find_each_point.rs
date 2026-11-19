//! Tests: the steps `mark` and `work_was_done`, the new lines of the rules
//! pages and the documents, and the repair that changes no file. The worker
//! is simulated. The two new steps run `git` in a test project.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::{Graph, Kind};
use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// The fan parts. They run at the same time, so their order is not fixed.
const PARTS: [&str; 4] = [
    "check_tests",
    "check_safety",
    "check_review",
    "check_security",
];

const WORK: &str = ".codeumbrella/runs/{run}/work";

/// A simulated `ps` that gives an error and no text.
const PS_BAD: &str = "#!/bin/sh\necho 'ps: operation not permitted' >&2\nexit 1\n";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap()
}

fn default_sheet() -> Graph {
    let roles = root().join("roles");
    Graph::load(&root().join("graph.example.yaml"), &roles).unwrap()
}

/// A new empty project folder. The path is the full one: on a Mac,
/// `/private/var/...` and not `/var/...`.
fn project() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-point-{}-{n}", std::process::id());
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

/// The default sheet for a run in the project `dir`. The steps `mark` and
/// `work_was_done` run their `git` commands there. Each other command step
/// does nothing.
fn sheet(dir: &Path) -> Graph {
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

/// A run in the project `dir`. Its work copy has one commit.
fn start(dir: &Path, graph: &Graph) -> Run {
    let folder = dir.join(".codeumbrella/runs/run-1");
    let run = Run::start(folder, "a task", graph).unwrap();
    let work = run.dir.join("work");
    git(&work, &["init", "--quiet"]);
    fs::write(work.join("file"), "x").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "--quiet", "-m", "first"]);
    run
}

/// A simulated worker. It writes its papers and gives a good result. It
/// makes one commit in the work copy for each step name in `commits`, in
/// that order.
struct Sim {
    commits: Mutex<Vec<&'static str>>,
    made: Mutex<Vec<String>>,
    got: Mutex<Vec<(String, String)>>,
}

impl Sim {
    fn new(commits: &[&'static str]) -> Self {
        Self {
            commits: Mutex::new(commits.to_vec()),
            made: Mutex::default(),
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
            let head = git(&work, &["rev-parse", "HEAD"]);
            self.made.lock().unwrap().push(head);
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

/// The steps after the last answer at `look_at_tests`.
fn after_yes(run: &Run) -> Vec<(String, bool)> {
    let all = steps(run);
    let at = all.iter().rposition(|(s, _)| s == "look_at_tests");
    all[at.expect("no answer at look_at_tests") + 1..].to_vec()
}

fn pairs(list: &[(&str, bool)]) -> Vec<(String, bool)> {
    list.iter().map(|(s, ok)| ((*s).to_owned(), *ok)).collect()
}

/// Does the run to `look_at_tests`, answers yes, and goes on.
fn to_build(run: &mut Run, graph: &Graph, worker: &Sim) {
    let roles = root().join("roles");
    run.go(graph, &roles, worker).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    run.answer(true, "").unwrap();
    run.go(graph, &roles, worker).unwrap();
}

#[test]
fn with_no_commit_work_was_done_is_bad_and_goes_to_repair() {
    let dir = project();
    let graph = sheet(&dir);
    let mut run = start(&dir, &graph);
    let worker = Sim::new(&[]);
    to_build(&mut run, &graph, &worker);
    let path = pairs(&[
        ("mark", true),
        ("build", true),
        ("simplify", true),
        ("check", true),
        ("work_was_done", false),
        ("repair", true),
    ]);
    let after = after_yes(&run);
    assert_eq!(after.get(..6), Some(&path[..]), "{after:?}");
    // The builder in `repair` gets the error text of git.
    let got = worker.got("repair");
    let first = got[0].to_lowercase();
    assert!(first.contains("empty bundle"), "{got:?}");
    // No repair makes a commit, so the run stops at the maximum of `repair`.
    assert_eq!(run.state.status, Status::Stopped);
    assert_eq!(run.state.at.as_deref(), Some("repair"));
    assert!(!run.dir.join("new-work.bundle").exists());
}

#[test]
fn a_commit_of_the_repair_makes_work_was_done_good() {
    let dir = project();
    let graph = sheet(&dir);
    let mut run = start(&dir, &graph);
    let worker = Sim::new(&["repair"]);
    to_build(&mut run, &graph, &worker);
    let path = pairs(&[
        ("mark", true),
        ("build", true),
        ("simplify", true),
        ("check", true),
        ("work_was_done", false),
        ("repair", true),
        ("check", true),
        ("work_was_done", true),
        ("no_locked_files", true),
        ("push", true),
        ("find_pr", true),
    ]);
    assert_eq!(after_yes(&run), path);
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
}

#[test]
fn with_a_new_commit_work_was_done_is_good_and_changes_no_file() {
    let dir = project();
    let graph = sheet(&dir);
    let mut run = start(&dir, &graph);
    let work = run.dir.join("work");
    let first = git(&work, &["rev-parse", "HEAD"]);
    let worker = Sim::new(&["build"]);
    to_build(&mut run, &graph, &worker);
    let path = pairs(&[
        ("mark", true),
        ("build", true),
        ("simplify", true),
        ("check", true),
        ("work_was_done", true),
        ("no_locked_files", true),
        ("push", true),
        ("find_pr", true),
    ]);
    assert_eq!(after_yes(&run), path);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    // `mark` wrote the commit of the yes into the git name.
    let mark = git(&work, &["rev-parse", "refs/codeumbrella/before"]);
    assert_eq!(mark, first);
    // The bundle is in the run folder, not in the work copy.
    assert!(run.dir.join("new-work.bundle").is_file());
    assert!(!work.join("new-work.bundle").exists());
    // The step changes no file of the work copy.
    assert_eq!(git(&work, &["status", "--porcelain"]), "");
    let made = worker.made.lock().unwrap().clone();
    assert_eq!(git(&work, &["rev-parse", "HEAD"]), made[0]);
}

#[test]
fn after_a_no_at_the_approval_a_commit_from_before_does_not_count() {
    let dir = project();
    let graph = sheet(&dir);
    let roles = root().join("roles");
    let mut run = start(&dir, &graph);
    let work = run.dir.join("work");
    let worker = Sim::new(&["build"]);
    to_build(&mut run, &graph, &worker);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    let made = worker.made.lock().unwrap().clone();

    run.answer(false, "Change the heading.").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();

    // `mark` ran again after the second yes.
    let all = steps(&run);
    let marks = all.iter().filter(|(s, _)| s == "mark").count();
    assert_eq!(marks, 2, "{all:?}");
    let mark = git(&work, &["rev-parse", "refs/codeumbrella/before"]);
    assert_eq!(mark, made[0]);
    // The builder made no commit after that yes.
    let path = pairs(&[
        ("mark", true),
        ("build", true),
        ("simplify", true),
        ("check", true),
        ("work_was_done", false),
        ("repair", true),
    ]);
    let after = after_yes(&run);
    assert_eq!(after.get(..6), Some(&path[..]), "{after:?}");
    assert!(!run.state.results["work_was_done"].ok);
}

#[test]
fn the_default_sheet_has_mark_before_build_and_work_was_done_after_check() {
    let graph = default_sheet();
    let mark = graph.step("mark").unwrap();
    assert_eq!(mark.kind, Kind::Command);
    let words = ["git", "-C", WORK, "update-ref"];
    let ends = ["refs/codeumbrella/before", "HEAD"];
    let all: Vec<&str> = words.iter().chain(&ends).copied().collect();
    assert_eq!(mark.run, [all]);
    assert_eq!(mark.next.as_deref(), Some("build"));
    let tests = graph.step("look_at_tests").unwrap();
    assert_eq!(tests.next.as_deref(), Some("mark"));
    assert_eq!(tests.on_bad.as_deref(), Some("plan"));

    let done = graph.step("work_was_done").unwrap();
    assert_eq!(done.kind, Kind::Command);
    let bundle = [
        "git",
        "-C",
        WORK,
        "bundle",
        "create",
        "../new-work.bundle",
        "refs/codeumbrella/before..HEAD",
    ];
    assert_eq!(done.run, [bundle]);
    assert_eq!(done.on_bad.as_deref(), Some("repair"));
    assert_eq!(done.next.as_deref(), Some("no_locked_files"));
    let check = graph.step("check").unwrap();
    assert_eq!(check.next.as_deref(), Some("work_was_done"));
    assert_eq!(check.on_bad.as_deref(), Some("repair"));
    let repair = graph.step("repair").unwrap();
    assert_eq!(repair.next.as_deref(), Some("check"));

    // Each new step has one way in.
    let ways = [
        ("mark", "look_at_tests"),
        ("build", "mark"),
        ("work_was_done", "check"),
        ("no_locked_files", "work_was_done"),
    ];
    for (name, step) in &graph.steps {
        let targets = [&step.next, &step.on_bad];
        for (to, from) in ways {
            let goes = targets.contains(&&Some(to.to_owned()));
            assert_eq!(goes, name == from, "{name} to {to}");
        }
    }
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
fn the_comments_of_the_sheet_tell_what_the_two_steps_do() {
    let mark = comment_above("mark");
    assert!(mark.contains("commit"), "{mark}");
    let done = comment_above("work_was_done");
    assert!(done.contains("commit"), "{done}");
    assert!(done.contains("not a wall"), "{done}");
}

/// The lines of the rule with this number, with its sub-points.
fn rule(path: &str, number: usize) -> String {
    let found = rule_in(&text(path), number);
    assert!(!found.is_empty(), "{path}: no rule {number}");
    found
}

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
fn the_planner_page_has_the_points_of_the_no_and_numbered_points() {
    let page = text("roles/planner.md");
    let rules: Vec<String> = (1..30).map(|n| rule_in(&page, n)).collect();
    let no = rules.iter().find(|r| r.contains("Points of the no"));
    let no = no.expect("no rule for the points of the no");
    assert!(no.contains("spec.md"), "{no}");
    assert!(no.contains("number"), "{no}");
    let numbered = rules.iter().any(|r| {
        let low = r.to_lowercase();
        r != no && low.contains("point") && low.contains("number")
    });
    assert!(numbered, "no rule for the numbers of the points");
}

#[test]
fn the_first_rule_of_the_reviewer_looks_for_each_point() {
    let first = rule("roles/reviewer.md", 1);
    let words = [
        "Points of the no",
        "`found`",
        "`not found`",
        "`kept`",
        "`not kept`",
        "ok: false",
    ];
    for word in words {
        assert!(first.contains(word), "{word}: {first}");
    }
}

#[test]
fn rule_10_of_the_builder_lists_each_point_of_the_no() {
    let ten = rule("roles/builder.md", 10);
    for word in ["Points of the no", "ok: false", "`work_was_done`"] {
        assert!(ten.contains(word), "{word}: {ten}");
    }
}

#[test]
fn rule_5_of_the_attacker_names_a_test_that_is_good_before_the_code() {
    let five = rule("roles/attacker.md", 5);
    let line = "good before the code is written";
    assert!(five.contains(line), "{five}");
}

const PAGES: [&str; 4] = [
    "roles/planner.md",
    "roles/reviewer.md",
    "roles/builder.md",
    "roles/attacker.md",
];

#[test]
fn the_numbers_of_the_rules_of_the_four_pages_are_in_sequence() {
    for path in PAGES {
        let page = text(path);
        let numbers: Vec<usize> = page
            .lines()
            .filter_map(|l| l.split_once(". ")?.0.parse().ok())
            .collect();
        let expected: Vec<usize> = (1..=numbers.len()).collect();
        assert_eq!(numbers, expected, "{path}");
    }
}

/// A line with no list mark at its start.
fn body(line: &str) -> &str {
    let line = line.trim_start();
    if let Some(rest) = line.strip_prefix("- ") {
        return rest;
    }
    match line.split_once(". ") {
        Some((n, rest)) if n.parse::<usize>().is_ok() => rest,
        _ => line,
    }
}

#[test]
fn each_sentence_of_the_four_pages_has_20_words_at_the_maximum() {
    for path in PAGES {
        for line in text(path).lines() {
            for sentence in body(line).split(". ") {
                let words = sentence.split_whitespace().count();
                assert!(words <= 20, "{path}: {words} words: {sentence}");
            }
        }
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

fn has_word(text: &str, word: &str) -> bool {
    let mut words = text.split(|c: char| !(c.is_alphanumeric() || c == '_'));
    words.any(|w| w == word)
}

#[test]
fn the_documents_name_the_two_new_steps() {
    let agents = section("AGENTS.md", "The sheet");
    let path = section("ARCHITECTURE.md", "The path of a task");
    let docs = [
        ("AGENTS.md", agents),
        ("README.md", text("README.md")),
        ("ARCHITECTURE.md", path),
    ];
    for (name, doc) in docs {
        for step in ["mark", "work_was_done"] {
            assert!(has_word(&doc, step), "{name}: {step}");
        }
    }
}

#[test]
fn the_safety_rules_tell_the_new_rules() {
    let rules = section("AGENTS.md", "Safety rules");
    let words = [
        "Points of the no",
        "`not found`",
        "work_was_done",
        "not a wall",
        "18 November 2026",
    ];
    for word in words {
        assert!(rules.contains(word), "{word}");
    }
}

#[test]
fn the_chat_page_tells_the_points_of_the_no_after_a_no() {
    let part = section("skills/codeumbrella/SKILL.md", "After each command");
    assert!(part.contains("Points of the no"), "{part}");
}

/// The number of a command that is dead and collected.
fn dead_number() -> u32 {
    let mut gone = Command::new("true").spawn().unwrap();
    gone.wait().unwrap();
    gone.id()
}

fn alive(pid: u32) -> bool {
    let mut kill = Command::new("kill");
    kill.args(["-0", &pid.to_string()]).stderr(Stdio::null());
    kill.status().unwrap().success()
}

/// Writes a file `worker.<number>` with a dead number. Gives its path.
fn dead_file(dir: &Path) -> PathBuf {
    let path = dir.join(format!("worker.{}", dead_number()));
    fs::write(&path, "Mon Jan  1 00:00:00 2001\n").unwrap();
    path
}

/// The position of a file in the order of `read_dir`.
fn position(dir: &Path, file: &Path) -> Option<usize> {
    let mut entries = fs::read_dir(dir).unwrap();
    entries.position(|e| e.unwrap().path() == file)
}

/// A dead worker file that comes before the live one in the order of
/// `read_dir`. The code of today then removes it before the error.
fn dead_first(dir: &Path, live_file: &Path) -> PathBuf {
    for _ in 0..50 {
        let path = dead_file(dir);
        if position(dir, &path) < position(dir, live_file) {
            return path;
        }
        fs::remove_file(&path).unwrap();
    }
    // The order did not help. The test then checks the files all the same.
    dead_file(dir)
}

/// The command `status <id>` in `dir`, with `bin` first on the PATH.
fn status(dir: &Path, bin: &Path, id: &str) -> Output {
    let path = format!("{}:{}", bin.display(), env::var("PATH").unwrap());
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(["status", id]).current_dir(dir);
    command.env("PATH", path).output().unwrap()
}

#[test]
fn status_with_a_ps_that_fails_removes_no_worker_file() {
    let dir = project();
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let ps = bin.join("ps");
    fs::write(&ps, PS_BAD).unwrap();
    fs::set_permissions(&ps, fs::Permissions::from_mode(0o755)).unwrap();

    let folder = dir.join(".codeumbrella/runs/run-1");
    let mut run = Run::start(folder, "a task", &default_sheet()).unwrap();
    run.state.status = Status::Running;
    run.save().unwrap();
    let lock = run.dir.join("lock");
    fs::write(&lock, dead_number().to_string()).unwrap();
    let mut sleep = Command::new("sleep").arg("30").spawn().unwrap();
    let live_file = run.dir.join(format!("worker.{}", sleep.id()));
    fs::write(&live_file, "Thu Jan  1 00:00:00 1970\n").unwrap();
    let dead = dead_first(&run.dir, &live_file);
    let notebook = run.dir.join("state.json");
    let before = fs::read(&notebook).unwrap();

    let out = status(&dir, &bin, "run-1");
    let lives = alive(sleep.id());
    let _ = sleep.kill();
    let _ = sleep.wait();

    let stdout = String::from_utf8_lossy(&out.stdout);
    let all = format!("{stdout}{}", String::from_utf8_lossy(&out.stderr));
    assert!(!out.status.success(), "{all}");
    assert!(all.to_lowercase().contains("sandbox"), "{all}");
    assert!(dead.exists(), "the dead worker file is gone");
    assert!(live_file.exists(), "the live worker file is gone");
    assert!(lock.exists(), "the lock is gone");
    assert_eq!(fs::read(&notebook).unwrap(), before);
    assert!(lives, "the sleep was ended");
}
