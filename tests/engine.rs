//! Tests of the supervisor with a simulated worker. No Claude and no Codex.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::Graph;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs, process};

/// A worker that follows a script and records the steps that it did.
#[derive(Default)]
struct Script {
    /// How many times each step gives a bad result before a good one.
    bad: RefCell<BTreeMap<String, u32>>,
    /// A step that changes the paper `tests`, which it must not do.
    cheat: Option<String>,
    /// The steps that ran, in sequence.
    calls: RefCell<Vec<String>>,
}

impl Worker for Script {
    fn run(&self, job: &Job) -> Result<Outcome> {
        self.calls.borrow_mut().push(job.step.to_owned());
        let papers = job.dir.join("papers");
        for paper in job.writes {
            fs::write(papers.join(paper).join("file.md"), job.step)?;
        }
        if self.cheat.as_deref() == Some(job.step) {
            fs::write(papers.join("tests").join("file.md"), "easy tests")?;
        }
        let mut bad = self.bad.borrow_mut();
        let left = bad.entry(job.step.to_owned()).or_default();
        let ok = *left == 0;
        *left = left.saturating_sub(1);
        Ok(Outcome {
            ok,
            summary: format!("{} ran", job.step),
        })
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The sheet and the rules pages that come with the project.
/// Each command step gets a command that does nothing, so no test can push.
fn sheet() -> (Graph, PathBuf) {
    let roles = root().join("roles");
    let mut graph = Graph::load(&root().join("graph.example.yaml"), &roles).unwrap();
    for step in graph.steps.values_mut().filter(|s| !s.run.is_empty()) {
        step.run = vec![vec!["true".to_owned()]];
    }
    (graph, roles)
}

/// A new empty folder for one test run.
fn folder() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let name = format!(
        "codeumbrella-test-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    );
    env::temp_dir().join(name)
}

fn bad(step: &str, times: u32) -> RefCell<BTreeMap<String, u32>> {
    RefCell::new(BTreeMap::from([(step.to_owned(), times)]))
}

/// Parses a sheet from text and checks it.
fn check(text: &str) -> Result<()> {
    let graph: Graph = yaml_serde::from_str(text)?;
    graph.check(&root().join("roles"))
}

#[test]
fn a_good_run_stops_two_times_for_the_person_and_then_ends() {
    let (graph, roles) = sheet();
    let worker = Script::default();
    let dir = folder();
    let mut run = Run::start(dir.clone(), "a task", &graph).unwrap();

    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));

    // Open the run from the notebook, as a new start of the command does.
    let mut run = Run::open(dir.clone()).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("approval"));

    let mut run = Run::open(dir).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Done);
    assert_eq!(*worker.calls.borrow(), ["plan", "build", "check"]);
    // The PR opens before the approval. The merge comes after it.
    assert!(run.state.results.contains_key("find_pr"));
    assert!(run.state.results.contains_key("merge"));
}

#[test]
fn a_bad_check_goes_to_repair_and_back() {
    let (graph, roles) = sheet();
    let worker = Script {
        bad: bad("check", 2),
        ..Script::default()
    };
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    assert_eq!(
        *worker.calls.borrow(),
        [
            "plan", "build", "check", "repair", "check", "repair", "check"
        ]
    );
}

#[test]
fn the_repair_stops_at_its_maximum() {
    let (graph, roles) = sheet();
    let worker = Script {
        bad: bad("check", 99),
        ..Script::default()
    };
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Stopped);
    assert!(
        run.state.note.contains("maximum of 3"),
        "{}",
        run.state.note
    );
    let repairs = worker
        .calls
        .borrow()
        .iter()
        .filter(|c| *c == "repair")
        .count();
    assert_eq!(repairs, 3);

    // After `resume`, the run can do the stopped step again.
    run.state.tries.remove("repair");
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(worker.calls.borrow().len(), 3 + 2 * 3 + 2 * 3);
}

#[test]
fn a_no_from_the_person_goes_back_to_the_plan() {
    let (graph, roles) = sheet();
    let worker = Script::default();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(false, "The tests are too easy.").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(*worker.calls.borrow(), ["plan", "plan"]);
}

#[test]
fn a_no_at_the_approval_sends_the_work_back_to_the_builder() {
    let (graph, roles) = sheet();
    let worker = Script::default();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(false, "Change the heading.").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    // The builder repairs, the checker looks again, and the run comes back to the person.
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    assert_eq!(
        *worker.calls.borrow(),
        ["plan", "build", "check", "repair", "check"]
    );
    assert!(!run.state.results.contains_key("merge"));
}

#[test]
fn a_command_step_obeys_its_maximum() {
    let text = "start: a\nsteps:\n  a: {type: command, run: [[false]], on_bad: a, max_tries: 2}\n";
    let graph: Graph = yaml_serde::from_str(text).unwrap();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &root().join("roles"), &Script::default())
        .unwrap();
    assert_eq!(run.state.status, Status::Stopped);
    assert!(
        run.state.note.contains("maximum of 2"),
        "{}",
        run.state.note
    );
}

#[test]
fn a_bad_command_goes_to_its_on_bad_step() {
    let text = "start: a\nsteps:\n  a: {type: command, run: [[false]], on_bad: b}\n  b: {type: command, run: [[true]], max_tries: 1}\n";
    let graph: Graph = yaml_serde::from_str(text).unwrap();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &root().join("roles"), &Script::default())
        .unwrap();
    assert_eq!(run.state.status, Status::Done);
    assert!(run.state.results.contains_key("b"));
}

/// Does a sheet with one command step, and gives back the run.
fn command_run(words: &str) -> Run {
    let text = format!("start: a\nsteps:\n  a: {{type: command, run: [[{words}]]}}\n");
    let graph: Graph = yaml_serde::from_str(&text).unwrap();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &root().join("roles"), &Script::default())
        .unwrap();
    run
}

#[test]
fn a_command_step_runs_its_commands_and_a_bad_command_stops_the_run() {
    let run = command_run("echo, \"id {run}\"");
    assert_eq!(run.state.status, Status::Done);
    let id = run.dir.file_name().unwrap().to_string_lossy();
    assert_eq!(run.state.results["a"].summary, format!("id {id}"));

    assert_eq!(command_run("false").state.status, Status::Stopped);
    assert_eq!(
        command_run("no-such-command-here").state.status,
        Status::Stopped
    );
}

#[test]
fn a_worker_that_changes_the_tests_stops_the_run() {
    let (graph, roles) = sheet();
    let worker = Script {
        cheat: Some("build".to_owned()),
        ..Script::default()
    };
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Stopped);
    assert!(run.state.note.contains("paper tests"), "{}", run.state.note);
    assert!(!worker.calls.borrow().contains(&"check".to_owned()));
}

#[test]
fn a_paper_that_the_person_changes_at_a_stop_does_not_stop_the_run() {
    let (graph, roles) = sheet();
    let worker = Script::default();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    fs::write(run.dir.join("papers/tests/file.md"), "better tests").unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
}

#[test]
fn a_worker_change_to_a_paper_stops_the_run_before_the_question_for_the_person() {
    let (graph, roles) = sheet();
    let worker = Script {
        cheat: Some("check".to_owned()),
        ..Script::default()
    };
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    // The run does not get to the approval step, where an answer makes new fingerprints.
    assert_eq!(run.state.status, Status::Stopped);
    assert!(run.state.note.contains("paper tests"), "{}", run.state.note);
    assert!(!run.state.results.contains_key("find_pr"));
}

#[test]
fn an_answer_with_no_open_question_is_an_error() {
    let (graph, _) = sheet();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    assert!(run.answer(true, "").is_err());
}

#[test]
fn the_sheet_check_finds_errors() {
    let unknown_line = "start: a\nsteps:\n  a: {role: planner, colour: red}\n";
    assert!(check(unknown_line).is_err());
    let no_such_step = "start: a\nsteps:\n  a: {role: planner, next: b}\n";
    assert!(check(no_such_step).is_err());
    let no_start = "start: z\nsteps:\n  a: {role: planner}\n";
    assert!(check(no_start).is_err());
    let no_rules_page = "start: a\nsteps:\n  a: {role: nobody}\n";
    assert!(check(no_rules_page).is_err());
    let way_back_with_no_maximum =
        "start: a\nsteps:\n  a: {role: planner, next: b}\n  b: {role: checker, on_bad: a}\n";
    assert!(check(way_back_with_no_maximum).is_err());
    let command_with_no_run = "start: a\nsteps:\n  a: {type: command}\n";
    assert!(check(command_with_no_run).is_err());
    let worker_with_run = "start: a\nsteps:\n  a: {role: planner, run: [[ls]]}\n";
    assert!(check(worker_with_run).is_err());
    let circle = "start: a\nsteps:\n  a: {role: planner, next: b}\n  b: {role: checker, next: a}\n";
    assert!(check(circle).is_err());
    let good = "start: a\nsteps:\n  a: {role: planner, max_tries: 2, next: b}\n  b: {role: checker, on_bad: a}\n";
    check(good).unwrap();
}

#[test]
fn the_sheet_of_the_project_is_correct() {
    let (graph, _) = sheet();
    assert_eq!(graph.steps.len(), 10);
    assert!(Path::new(&root().join("roles/planner.md")).is_file());
}
