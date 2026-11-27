//! Tests: the steps `after_no` and `check_no` of the default sheet. After a
//! no of the person, a worker checks each point of the no right after the
//! build. The worker is simulated. Only `after_no` runs its own command.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::{Graph, Kind};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// The report of a simulated `check_no` with a bad result.
const REPORT: &str = "Point 1: found file:1. Point 2: not found.";

/// The summary of each simulated build. It quotes a log line of a no.
const QUOTE: &str = r#"{"step":"approval","ok":false}"#;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let read = fs::read_to_string(root().join(rel));
    read.unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn default_sheet() -> Graph {
    let path = root().join("graph.example.yaml");
    Graph::load(&path, &root().join("roles")).unwrap()
}

/// A new empty project folder, with its full path.
fn project() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-checkno-{}-{n}", std::process::id());
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

/// The default sheet for the project `dir`. The step `after_no` reads the
/// run folder there. Each other command step does nothing.
fn sheet(dir: &Path) -> Graph {
    let mut graph = default_sheet();
    let full = format!("{}/.codeumbrella/runs/{{run}}", dir.display());
    for (name, step) in &mut graph.steps {
        if name == "after_no" {
            for word in step.run.iter_mut().flatten() {
                *word = word.replace(".codeumbrella/runs/{run}", &full);
            }
        } else if !step.run.is_empty() {
            step.run = vec![vec!["true".to_owned()]];
        }
    }
    graph
}

fn start(dir: &Path, graph: &Graph) -> Run {
    let folder = dir.join(".codeumbrella/runs/run-1");
    let run = Run::start(folder, "a task", graph).unwrap();
    fs::write(run.dir.join("work/file"), "x").unwrap();
    run
}

/// A simulated worker. It writes its papers. It gives one bad result for
/// each step name in `bad`. Each other result is good.
#[derive(Default)]
struct Sim {
    bad: Mutex<Vec<&'static str>>,
    got: Mutex<Vec<(String, String)>>,
}

impl Sim {
    /// The results before each call of the step `step`.
    fn got(&self, step: &str) -> Vec<String> {
        let got = self.got.lock().unwrap();
        let mine = got.iter().filter(|(s, _)| s == step);
        mine.map(|(_, before)| before.clone()).collect()
    }

    fn count(&self, step: &str) -> usize {
        self.got(step).len()
    }

    fn set_bad(&self, steps: &[&'static str]) {
        self.bad.lock().unwrap().extend_from_slice(steps);
    }

    fn is_bad(&self, step: &str) -> bool {
        let mut bad = self.bad.lock().unwrap();
        let at = bad.iter().position(|s| *s == step);
        at.map(|at| bad.remove(at)).is_some()
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
        let ok = !self.is_bad(job.step);
        let summary = match (job.step, ok) {
            ("check_no", false) => REPORT.to_owned(),
            ("build", _) => QUOTE.to_owned(),
            _ => format!("{} ran", job.step),
        };
        Ok(Outcome { ok, summary })
    }
}

/// The steps in the log, with their results.
fn steps(run: &Run) -> Vec<(String, bool)> {
    let log = fs::read_to_string(run.dir.join("log.jsonl")).unwrap();
    let mut found = Vec::new();
    for line in log.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        let step = value["step"].as_str().unwrap().to_owned();
        let ok = value["ok"].as_bool().unwrap();
        found.push((step, ok));
    }
    found
}

/// The first `count` steps after the last answer at `look_at_tests`.
fn after_yes(run: &Run, count: usize) -> Vec<(String, bool)> {
    let all = steps(run);
    let at = all.iter().rposition(|(s, _)| s == "look_at_tests");
    let from = at.expect("no answer at look_at_tests") + 1;
    all.into_iter().skip(from).take(count).collect()
}

fn pairs(list: &[(&str, bool)]) -> Vec<(String, bool)> {
    list.iter().map(|(s, ok)| ((*s).to_owned(), *ok)).collect()
}

/// The step where the run is.
fn at(run: &Run) -> &str {
    run.state.at.as_deref().unwrap_or("")
}

fn go(run: &mut Run, graph: &Graph, worker: &Sim) {
    run.go(graph, &root().join("roles"), worker).unwrap();
}

/// Answers the open question and goes on.
fn answer(run: &mut Run, graph: &Graph, worker: &Sim, ok: bool) {
    let words = if ok { "" } else { "Change the heading." };
    run.answer(ok, words).unwrap();
    go(run, graph, worker);
}

/// A run that did its first build and waits at the PR question.
fn first_build(dir: &Path, graph: &Graph, worker: &Sim) -> Run {
    let mut run = start(dir, graph);
    go(&mut run, graph, worker);
    answer(&mut run, graph, worker, true);
    assert_eq!(at(&run), "approval", "{}", run.state.note);
    run
}

/// A no at the PR question, then a yes for the tests.
fn no_then_yes(run: &mut Run, graph: &Graph, worker: &Sim) {
    assert_eq!(at(run), "approval", "{}", run.state.note);
    answer(run, graph, worker, false);
    assert_eq!(at(run), "look_at_tests", "{}", run.state.note);
    answer(run, graph, worker, true);
}

/// The path of a first build, up to the simplifier.
fn no_check_path() -> Vec<(String, bool)> {
    pairs(&[
        ("mark", true),
        ("copy_tests", true),
        ("build", true),
        ("after_no", true),
        ("simplify", true),
    ])
}

/// The path after a no, for a build with each point done.
fn good_check_path() -> Vec<(String, bool)> {
    pairs(&[
        ("mark", true),
        ("copy_tests", true),
        ("build", true),
        ("after_no", false),
        ("check_no", true),
        ("simplify", true),
    ])
}

#[test]
fn a_first_build_goes_to_simplify_with_no_worker_of_check_no() {
    let dir = project();
    let graph = sheet(&dir);
    let worker = Sim::default();
    let run = first_build(&dir, &graph, &worker);
    let path = no_check_path();
    assert_eq!(after_yes(&run, path.len()), path);
    assert_eq!(worker.count("check_no"), 0);
}

#[test]
fn a_bad_build_is_not_a_no_of_the_person() {
    let dir = project();
    let graph = sheet(&dir);
    let worker = Sim::default();
    worker.set_bad(&["build"]);
    let mut run = start(&dir, &graph);
    go(&mut run, &graph, &worker);
    answer(&mut run, &graph, &worker, true);
    assert_eq!(at(&run), "look_at_tests", "{}", run.state.note);
    answer(&mut run, &graph, &worker, true);
    let path = no_check_path();
    assert_eq!(after_yes(&run, path.len()), path);
    assert_eq!(worker.count("check_no"), 0);
}

#[test]
fn after_a_no_a_build_with_each_point_done_goes_to_simplify() {
    let dir = project();
    let graph = sheet(&dir);
    let worker = Sim::default();
    let mut run = first_build(&dir, &graph, &worker);
    no_then_yes(&mut run, &graph, &worker);
    let path = good_check_path();
    assert_eq!(after_yes(&run, path.len()), path);
    assert_eq!(worker.count("check_no"), 1);
    assert_eq!(at(&run), "approval", "{}", run.state.note);
}

#[test]
fn a_no_for_the_tests_before_the_first_build_counts() {
    let dir = project();
    let graph = sheet(&dir);
    let worker = Sim::default();
    let mut run = start(&dir, &graph);
    go(&mut run, &graph, &worker);
    answer(&mut run, &graph, &worker, false);
    answer(&mut run, &graph, &worker, true);
    let path = good_check_path();
    assert_eq!(after_yes(&run, path.len()), path);
    assert_eq!(worker.count("check_no"), 1);
}

#[test]
fn a_point_that_is_not_done_goes_back_to_build_with_the_list() {
    let dir = project();
    let graph = sheet(&dir);
    let worker = Sim::default();
    let mut run = first_build(&dir, &graph, &worker);
    worker.set_bad(&["check_no"]);
    no_then_yes(&mut run, &graph, &worker);
    let path = pairs(&[
        ("mark", true),
        ("copy_tests", true),
        ("build", true),
        ("after_no", false),
        ("check_no", false),
        ("build", true),
        ("after_no", false),
        ("check_no", true),
        ("simplify", true),
    ]);
    assert_eq!(after_yes(&run, path.len()), path);
    let got = worker.got("build");
    assert_eq!(got.len(), 3, "{got:?}");
    assert_eq!(got[2], REPORT);
    assert_eq!(worker.count("simplify"), 2);
    assert_eq!(at(&run), "approval", "{}", run.state.note);
}

#[test]
fn a_third_bad_result_of_check_no_stops_the_run() {
    let dir = project();
    let graph = sheet(&dir);
    let worker = Sim::default();
    let mut run = first_build(&dir, &graph, &worker);
    worker.set_bad(&["check_no"; 3]);
    no_then_yes(&mut run, &graph, &worker);
    assert_eq!(run.state.status, Status::Stopped);
    assert_eq!(at(&run), "check_no", "{}", run.state.note);
    let note = &run.state.note;
    assert!(note.contains("maximum of 3 tries"), "{note}");
    assert_eq!(worker.count("check_no"), 3);
    // The first build, the build after the no, and one for each bad result.
    assert_eq!(worker.count("build"), 5);
    // Only the first build went on.
    assert_eq!(worker.count("simplify"), 1);
    assert_eq!(worker.count("check_tests"), 1);
    // The last result of the step keeps the points for the chat.
    let result = &run.state.results["check_no"];
    assert!(!result.ok);
    assert_eq!(result.summary, REPORT);
}

#[test]
fn a_good_result_of_check_no_counts_as_a_try() {
    let dir = project();
    let graph = sheet(&dir);
    let worker = Sim::default();
    let mut run = first_build(&dir, &graph, &worker);
    worker.set_bad(&["check_no"]);
    no_then_yes(&mut run, &graph, &worker);
    worker.set_bad(&["check_no"; 2]);
    no_then_yes(&mut run, &graph, &worker);
    assert_eq!(run.state.status, Status::Stopped);
    assert_eq!(at(&run), "check_no", "{}", run.state.note);
    assert_eq!(worker.count("check_no"), 3);
    assert_eq!(worker.count("build"), 5);
}

#[test]
fn the_maximum_of_build_does_not_stop_the_run_before_check_no() {
    let dir = project();
    let graph = sheet(&dir);
    let worker = Sim::default();
    let mut run = first_build(&dir, &graph, &worker);
    // A bad build after the no, then three bad results of `check_no`.
    worker.set_bad(&["build"]);
    worker.set_bad(&["check_no"; 3]);
    no_then_yes(&mut run, &graph, &worker);
    assert_eq!(at(&run), "look_at_tests", "{}", run.state.note);
    answer(&mut run, &graph, &worker, true);
    assert_eq!(run.state.status, Status::Stopped);
    assert_eq!(at(&run), "check_no", "{}", run.state.note);
    assert_eq!(worker.count("check_no"), 3);
    assert_eq!(worker.count("build"), 6);
}

#[test]
fn with_no_readable_log_after_no_goes_to_check_no() {
    let dir = project();
    let mut graph = sheet(&dir);
    let step = graph.steps.get_mut("check_no").unwrap();
    step.next = None;
    step.on_bad = None;
    step.reads.clear();
    let worker = Sim::default();
    let mut run = start(&dir, &graph);
    run.state.at = Some("after_no".to_owned());
    assert!(!run.dir.join("log.jsonl").exists());
    go(&mut run, &graph, &worker);
    let path = pairs(&[("after_no", false), ("check_no", true)]);
    assert_eq!(steps(&run), path);
}

#[test]
fn the_default_sheet_has_the_two_new_steps_and_their_paths() {
    let graph = default_sheet();
    let build = graph.step("build").unwrap();
    assert_eq!(build.next.as_deref(), Some("after_no"));
    assert_eq!(build.on_bad.as_deref(), Some("plan"));
    assert_eq!(build.max_tries, Some(6));

    let gate = graph.step("after_no").unwrap();
    assert_eq!(gate.kind, Kind::Command);
    assert_eq!(gate.next.as_deref(), Some("simplify"));
    assert_eq!(gate.on_bad.as_deref(), Some("check_no"));
    assert_eq!(gate.max_tries, None);

    let check = graph.step("check_no").unwrap();
    assert_eq!(check.kind, Kind::Worker);
    assert_eq!(check.role.as_deref(), Some("reviewer"));
    assert_eq!(check.reads, ["spec"]);
    assert!(check.writes.is_empty(), "{:?}", check.writes);
    assert_eq!(check.next.as_deref(), Some("simplify"));
    assert_eq!(check.on_bad.as_deref(), Some("build"));
    assert_eq!(check.max_tries, Some(3));

    for (name, step) in &graph.steps {
        let targets = [&step.next, &step.on_bad];
        let goes = |to: &str| targets.contains(&&Some(to.to_owned()));
        assert_eq!(goes("after_no"), name == "build", "{name}");
        assert_eq!(goes("check_no"), name == "after_no", "{name}");
        let into = name == "copy_tests" || name == "check_no";
        assert_eq!(goes("build"), into, "{name}");
    }
}

/// The comment lines right above the step `name`, as one line.
fn comment_above(name: &str) -> String {
    let sheet = text("graph.example.yaml");
    let lines: Vec<&str> = sheet.lines().collect();
    let head = format!("  {name}:");
    let found_at = lines.iter().position(|l| *l == head);
    let end = found_at.unwrap_or_else(|| panic!("no line {head}"));
    let mut found = Vec::new();
    for line in lines[..end].iter().rev() {
        if !line.trim_start().starts_with('#') {
            break;
        }
        found.push(*line);
    }
    found.reverse();
    found.join("\n").to_lowercase().replace("\n  #", "")
}

#[test]
fn the_comments_of_the_sheet_tell_the_new_steps() {
    let gate = comment_above("after_no");
    assert!(gate.contains("no of"), "{gate}");
    let check = comment_above("check_no");
    assert!(check.contains("points of the no"), "{check}");
    let simpler = comment_above("simplify");
    let gone = "set `next` of `build` to `work_was_done`";
    assert!(!simpler.contains(gone), "{simpler}");
}

/// Rule 1 of the reviewer page, with its sub-points.
fn first_rule() -> String {
    let page = text("roles/reviewer.md");
    let from = page.lines().skip_while(|l| !l.starts_with("1. "));
    let rule = from.take_while(|l| !l.starts_with("2. "));
    rule.collect::<Vec<&str>>().join("\n")
}

#[test]
fn rule_1_of_the_reviewer_tells_the_step_check_no() {
    let one = first_rule();
    let words = ["`check_no`", "`removed`", "`ok: true`", "100 words"];
    for word in words {
        assert!(one.contains(word), "{word}: {one}");
    }
    let removed = one.lines().any(|l| {
        let low = l.to_lowercase();
        low.contains("`removed`") && low.contains("command")
    });
    assert!(removed, "no command for `removed`: {one}");
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

#[test]
fn the_documents_tell_the_new_step() {
    let sheet_part = section("AGENTS.md", "The sheet");
    for word in ["`after_no`", "`check_no`"] {
        assert!(sheet_part.contains(word), "{word}");
    }
    let rules = section("AGENTS.md", "Safety rules");
    assert!(rules.contains("`check_no`"), "no safety rule");
    let path = section("ARCHITECTURE.md", "The path of a task");
    for word in ["after_no", "check_no"] {
        assert!(path.contains(word), "{word}");
    }
}

#[test]
fn the_chat_page_tells_the_points_after_a_stop_at_check_no() {
    let part = section("skills/codeumbrella/SKILL.md", "After each command");
    let found = part
        .lines()
        .any(|l| l.contains("check_no") && l.contains("`not found`"));
    assert!(found, "{part}");
}

#[test]
fn the_learner_page_tells_that_a_bad_after_no_is_not_a_fault() {
    let page = text("roles/learner.md");
    assert!(page.contains("`after_no`"), "{page}");
}
