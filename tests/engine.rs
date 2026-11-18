//! Tests of the supervisor with a simulated worker. No Claude and no Codex.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::Graph;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs, process};

/// A worker that follows a script and records the steps that it did.
#[derive(Default)]
struct Script {
    /// How many times each step gives a bad result before a good one.
    bad: Mutex<BTreeMap<String, u32>>,
    /// A step that changes the paper `tests`, which it must not do.
    cheat: Option<String>,
    /// The steps that ran, in sequence.
    calls: Mutex<Vec<String>>,
}

impl Worker for Script {
    fn run(&self, job: &Job) -> Result<Outcome> {
        // The parts of the fan run at the same time, so their order is not fixed.
        if !["check_safety", "check_review", "check_security"].contains(&job.step) {
            self.calls.lock().unwrap().push(job.step.to_owned());
        }
        let papers = job.dir.join("papers");
        for paper in job.writes {
            fs::write(papers.join(paper).join("file.md"), job.step)?;
        }
        if self.cheat.as_deref() == Some(job.step) {
            fs::write(papers.join("tests").join("file.md"), "easy tests")?;
        }
        let mut bad = self.bad.lock().unwrap();
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

fn bad(step: &str, times: u32) -> Mutex<BTreeMap<String, u32>> {
    Mutex::new(BTreeMap::from([(step.to_owned(), times)]))
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
    assert_eq!(
        *worker.calls.lock().unwrap(),
        ["plan", "attack_plan", "build", "simplify", "check_tests"]
    );
    // The PR opens before the approval. The merge comes after it.
    assert!(run.state.results.contains_key("find_pr"));
    assert!(run.state.results.contains_key("merge"));
}

#[test]
fn a_bad_check_goes_to_repair_and_back() {
    let (graph, roles) = sheet();
    let worker = Script {
        bad: bad("check_tests", 2),
        ..Script::default()
    };
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    // The repair goes back to the check, not through `simplify`.
    assert_eq!(
        *worker.calls.lock().unwrap(),
        [
            "plan",
            "attack_plan",
            "build",
            "simplify",
            "check_tests",
            "repair",
            "check_tests",
            "repair",
            "check_tests"
        ]
    );
}

#[test]
fn a_bad_review_or_security_part_also_goes_to_repair() {
    for part in ["check_review", "check_security"] {
        let (graph, roles) = sheet();
        let worker = Script {
            bad: bad(part, 1),
            ..Script::default()
        };
        let mut run = Run::start(folder(), "a task", &graph).unwrap();
        run.go(&graph, &roles, &worker).unwrap();
        run.answer(true, "").unwrap();
        run.go(&graph, &roles, &worker).unwrap();
        assert_eq!(run.state.at.as_deref(), Some("approval"), "{part}");
        let calls = worker.calls.lock().unwrap().clone();
        let times = |name: &str| calls.iter().filter(|c| *c == name).count();
        assert_eq!(times("repair"), 1, "{part}");
        assert_eq!(times("simplify"), 1, "{part}");
        assert_eq!(times("check_tests"), 2, "{part}");
    }
}

#[test]
fn the_repair_stops_at_its_maximum() {
    let (graph, roles) = sheet();
    let worker = Script {
        bad: bad("check_tests", 99),
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
        .lock()
        .unwrap()
        .iter()
        .filter(|c| *c == "repair")
        .count();
    assert_eq!(repairs, 3);

    // After `resume`, the run can do the stopped step again.
    run.state.tries.remove("repair");
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(worker.calls.lock().unwrap().len(), 5 + 2 * 3 + 2 * 3);
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
    assert_eq!(
        *worker.calls.lock().unwrap(),
        ["plan", "attack_plan", "plan", "attack_plan"]
    );
}

#[test]
fn a_no_at_the_approval_sends_the_work_back_to_the_planner() {
    let (graph, roles) = sheet();
    let worker = Script::default();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(false, "Change the heading.").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    // The planner corrects the spec and the tests, and the person sees the tests again.
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    assert_eq!(
        *worker.calls.lock().unwrap(),
        [
            "plan",
            "attack_plan",
            "build",
            "simplify",
            "check_tests",
            "plan",
            "attack_plan"
        ]
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
    assert!(
        !worker
            .calls
            .lock()
            .unwrap()
            .contains(&"check_tests".to_owned())
    );
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
        cheat: Some("check_tests".to_owned()),
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
    let graph = Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap();
    assert!(Path::new(&root().join("roles/planner.md")).is_file());
    // The check for locked folders comes after each good check and before the push.
    assert_eq!(
        graph.step("check").unwrap().next.as_deref(),
        Some("no_locked_files")
    );
    assert_eq!(
        graph.step("no_locked_files").unwrap().next.as_deref(),
        Some("push")
    );
}

#[test]
fn a_step_can_run_again_after_its_own_try_broke_its_papers() {
    let (graph, roles) = sheet();
    let worker = Script::default();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    // A second planner try wrote a part of its papers and then gave an error.
    // The run stopped at the plan step, and the person runs `resume`.
    run.state.at = Some("plan".to_owned());
    fs::write(run.dir.join("papers/spec/file.md"), "half a plan").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(
        *worker.calls.lock().unwrap(),
        ["plan", "attack_plan", "plan", "attack_plan"]
    );
}

/// A sheet with one fan step `check` that has the parts `a` and `b`.
fn fan_sheet(extra: &str) -> Result<Graph> {
    let text = format!(
        "start: check\nsteps:\n  check:\n    type: fan\n    fan: [a, b]\n    on_bad: fix\n  \
         a:\n    role: checker\n  b:\n    role: safety\n{extra}  \
         fix:\n    role: builder\n    max_tries: 2\n    next: check\n"
    );
    let graph: Graph = yaml_serde::from_str(&text)?;
    graph.check(&root().join("roles"))?;
    Ok(graph)
}

#[test]
fn a_fan_is_good_only_if_all_its_parts_are_good() {
    let graph = fan_sheet("").unwrap();
    let worker = Script {
        bad: bad("b", 1),
        ..Script::default()
    };
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &root().join("roles"), &worker).unwrap();
    assert_eq!(run.state.status, Status::Done);
    // The first fan was bad because of `b`. The repair got the report of each part.
    let mut calls = worker.calls.lock().unwrap().clone();
    assert_eq!(calls.iter().filter(|c| *c == "fix").count(), 1);
    calls.sort();
    assert_eq!(calls, ["a", "a", "b", "b", "fix"]);
    let log = fs::read_to_string(run.dir.join("log.jsonl")).unwrap();
    assert!(log.contains("a (good): a ran\\nb (bad): b ran"), "{log}");
    assert!(run.state.results["check"].ok);
}

#[test]
fn the_parts_of_a_fan_run_at_the_same_time() {
    /// Each part waits until the two parts are at work. One part alone waits with no end.
    struct Meet(std::sync::Barrier);
    impl Worker for Meet {
        fn run(&self, job: &Job) -> Result<Outcome> {
            self.0.wait();
            let summary = job.step.to_owned();
            Ok(Outcome { ok: true, summary })
        }
    }
    let graph = fan_sheet("").unwrap();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    let worker = Meet(std::sync::Barrier::new(2));
    run.go(&graph, &root().join("roles"), &worker).unwrap();
    assert_eq!(run.state.status, Status::Done);
}

#[test]
fn the_sheet_check_refuses_a_fan_part_that_is_not_a_plain_worker_step() {
    assert!(fan_sheet("").is_ok());
    for bad_part in [
        "    next: fix\n",
        "    writes: [spec]\n",
        "    max_tries: 2\n",
    ] {
        // The extra line goes onto the part `b`.
        let error = fan_sheet(bad_part).unwrap_err();
        assert!(format!("{error:#}").contains("the fan part b"), "{error:#}");
    }
    let no_such = "start: c\nsteps:\n  c:\n    type: fan\n    fan: [nothing]\n";
    let graph: Graph = yaml_serde::from_str(no_such).unwrap();
    assert!(graph.check(&root().join("roles")).is_err());
    let no_fan_line = "start: c\nsteps:\n  c:\n    type: fan\n";
    let graph: Graph = yaml_serde::from_str(no_fan_line).unwrap();
    assert!(graph.check(&root().join("roles")).is_err());
}
