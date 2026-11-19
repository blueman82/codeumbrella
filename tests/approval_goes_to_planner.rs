//! Tests: a no at the PR stop goes to the planner. The worker is simulated.
//! A bad build goes to the planner too.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::Graph;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs, process};

/// A worker that always gives a good result and records the steps that it did.
#[derive(Default)]
struct Script {
    calls: Mutex<Vec<String>>,
}

impl Worker for Script {
    fn run(&self, job: &Job) -> Result<Outcome> {
        // The parts of the fan run at the same time, so their order is not fixed.
        if ![
            "check_safety",
            "check_review",
            "check_security",
            "check_use",
        ]
        .contains(&job.step)
        {
            self.calls.lock().unwrap().push(job.step.to_owned());
        }
        for paper in job.writes {
            fs::write(job.dir.join("papers").join(paper).join("file.md"), job.step)?;
        }
        Ok(Outcome {
            ok: true,
            summary: format!("{} ran", job.step),
        })
    }
}

/// A worker whose step `build` gives a bad result a set number of times.
struct BadBuilds {
    left: Mutex<u32>,
    calls: Mutex<Vec<String>>,
    before_plan: Mutex<Vec<String>>,
}

impl BadBuilds {
    fn new(bad: u32) -> Self {
        Self {
            left: Mutex::new(bad),
            calls: Mutex::default(),
            before_plan: Mutex::default(),
        }
    }

    fn count(&self, step: &str) -> usize {
        let calls = self.calls.lock().unwrap();
        calls.iter().filter(|c| c.as_str() == step).count()
    }
}

impl Worker for BadBuilds {
    fn run(&self, job: &Job) -> Result<Outcome> {
        if ![
            "check_safety",
            "check_review",
            "check_security",
            "check_use",
        ]
        .contains(&job.step)
        {
            self.calls.lock().unwrap().push(job.step.to_owned());
        }
        if job.step == "plan" {
            let before = job.before.map(|o| o.summary.clone());
            self.before_plan
                .lock()
                .unwrap()
                .push(before.unwrap_or_default());
        }
        for paper in job.writes {
            fs::write(job.dir.join("papers").join(paper).join("file.md"), job.step)?;
        }
        let mut left = self.left.lock().unwrap();
        if job.step == "build" && *left > 0 {
            *left -= 1;
            return Ok(Outcome {
                ok: false,
                summary: "test is bad in line 7".to_owned(),
            });
        }
        Ok(Outcome {
            ok: true,
            summary: format!("{} ran", job.step),
        })
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn folder() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    env::temp_dir().join(format!(
        "codeumbrella-planner-test-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ))
}

/// The default sheet, with a command that does nothing in each command step.
fn sheet() -> (Graph, PathBuf) {
    let roles = root().join("roles");
    let mut graph = Graph::load(&root().join("graph.example.yaml"), &roles).unwrap();
    for step in graph.steps.values_mut().filter(|s| !s.run.is_empty()) {
        step.run = vec![vec!["true".to_owned()]];
    }
    (graph, roles)
}

fn text(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap()
}

#[test]
fn the_sheet_sends_a_no_at_the_approval_to_the_plan() {
    let graph = Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap();
    assert_eq!(
        graph.step("approval").unwrap().on_bad.as_deref(),
        Some("plan")
    );
}

#[test]
fn the_step_check_still_goes_to_repair() {
    let graph = Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap();
    assert_eq!(
        graph.step("check").unwrap().on_bad.as_deref(),
        Some("repair")
    );
}

#[test]
fn the_default_sheet_sends_a_bad_build_to_the_plan_and_a_bad_repair_nowhere() {
    let graph = Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap();
    assert_eq!(graph.step("build").unwrap().on_bad.as_deref(), Some("plan"));
    assert_eq!(graph.step("repair").unwrap().on_bad, None);
    assert_eq!(graph.step("plan").unwrap().max_tries, Some(3));
}

#[test]
fn a_no_at_the_approval_goes_to_the_planner_and_then_to_the_person_for_the_tests() {
    let (graph, roles) = sheet();
    let worker = Script::default();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    std::fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("approval"));

    run.answer(false, "Change the heading.").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    // The planner works again. The person sees the tests again. Nothing merges.
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

    // After a yes, the work goes on to the builder and the checker, then to the approval.
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    assert_eq!(
        *worker.calls.lock().unwrap(),
        [
            "plan",
            "attack_plan",
            "build",
            "simplify",
            "check_tests",
            "plan",
            "attack_plan",
            "build",
            "simplify",
            "check_tests"
        ]
    );
}

#[test]
fn a_bad_build_goes_to_the_planner_and_waits_at_look_at_tests() {
    let (graph, roles) = sheet();
    let worker = BadBuilds::new(1);
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();

    // The run does not stop. The planner and the attacker work again.
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    assert_eq!(
        *worker.calls.lock().unwrap(),
        ["plan", "attack_plan", "build", "plan", "attack_plan"]
    );
    // The planner gets the bad result of the builder.
    let before = worker.before_plan.lock().unwrap().clone();
    assert_eq!(before.len(), 2);
    assert_eq!(before[1], "test is bad in line 7");
    // A bad build uses one try of the plan, as a no of the person does.
    assert_eq!(run.state.tries.get("plan"), Some(&2));
}

#[test]
fn after_a_yes_at_look_at_tests_the_run_does_the_build_again() {
    let (graph, roles) = sheet();
    let worker = BadBuilds::new(1);
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();

    assert_eq!(worker.count("build"), 2);
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
}

#[test]
fn three_bad_builds_stop_the_run_because_the_plan_has_no_try_left() {
    let (graph, roles) = sheet();
    let worker = BadBuilds::new(3);
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    // The first plan used try 1. Bad build 1 gives try 2. Bad build 2 gives try 3.
    for _ in 0..2 {
        run.answer(true, "").unwrap();
        run.go(&graph, &roles, &worker).unwrap();
        assert_eq!(run.state.status, Status::Waiting);
        assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    }
    assert_eq!(run.state.tries.get("plan"), Some(&3));

    // Bad build 3 finds no try left for the plan.
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Stopped);
    assert_eq!(run.state.at.as_deref(), Some("plan"));
    assert!(run.state.note.contains("maximum of 3 tries"));
    assert_eq!(worker.count("build"), 3);
    assert_eq!(worker.count("plan"), 3);
}

#[test]
fn the_builder_and_the_checker_obey_the_spec_over_the_task() {
    let rule = "If the task and the paper `spec` disagree, obey the spec. The spec has the corrections of the person.";
    for path in ["roles/builder.md", "roles/checker.md"] {
        assert!(text(path).contains(rule), "{path}");
    }
}

#[test]
fn the_builder_has_no_rule_about_a_new_test_file() {
    assert!(!text("roles/builder.md").contains("If the person tells you to add a test"));
}

#[test]
fn the_builder_gives_a_bad_paper_test_to_the_planner() {
    let page = text("roles/builder.md");
    let rule = page.lines().find(|l| l.starts_with("9. ")).unwrap();
    assert!(rule.contains("ok: false"), "{rule}");
    assert!(rule.contains("planner"), "{rule}");
    assert!(rule.contains("line"), "{rule}");
    assert!(rule.contains("cause"), "{rule}");
    assert!(rule.contains("commit"), "{rule}");
    assert!(!rule.contains("what the person must do"), "{rule}");
}

#[test]
fn the_planner_corrects_only_the_bad_test_after_a_bad_build() {
    let page = text("roles/planner.md");
    let rule = page.lines().find(|l| l.contains("bad build")).unwrap();
    assert!(rule.contains("only that test"), "{rule}");
    assert!(rule.contains("character for character"), "{rule}");
    assert!(rule.contains("summary"), "{rule}");
}

#[test]
fn the_numbers_of_the_rules_are_in_sequence() {
    for path in [
        "roles/builder.md",
        "roles/checker.md",
        "roles/planner.md",
        "roles/attacker.md",
    ] {
        let text = text(path);
        let numbers: Vec<usize> = text
            .lines()
            .filter_map(|l| l.split_once(". ")?.0.parse().ok())
            .collect();
        let expected: Vec<usize> = (1..=numbers.len()).collect();
        assert_eq!(numbers, expected, "{path}");
    }
}

#[test]
fn the_planner_has_the_rule_about_what_tests_check() {
    let rule = "Test what the work does, not which words a text has. Look for words only where the task gives the same words. Do not count rules or lines.";
    assert!(text("roles/planner.md").contains(rule));
}

/// Each line about a no at the PR stop names the planner and the tests again.
fn check_lines(path: &str, marker: &dyn Fn(&str) -> bool) {
    let content = text(path);
    let lines: Vec<&str> = content.lines().filter(|l| marker(l)).collect();
    assert!(
        !lines.is_empty(),
        "{path}: no line about a no at the PR stop"
    );
    for l in lines {
        assert!(l.contains("planner"), "{path}: {l}");
        assert!(l.contains("tests"), "{path}: {l}");
        assert!(l.contains("again"), "{path}: {l}");
    }
}

#[test]
fn the_docs_send_a_no_at_the_pr_stop_to_the_planner() {
    check_lines("AGENTS.md", &|l| {
        l.contains("A no with a text sends the work")
    });
    check_lines("README.md", &|l| l.starts_with("| The PR |"));
    check_lines("skills/codeumbrella/SKILL.md", &|l| {
        l.contains("A no at `approval` sends the work")
    });
}

#[test]
fn the_documents_do_not_keep_the_old_count_of_plan_tries() {
    let agents = text("AGENTS.md");
    assert!(!agents.contains("Thus 2 no answers are possible before a stop."));
    assert!(agents.contains("18 November 2026"));
    assert!(agents.contains("on_bad: plan"));
}

#[test]
fn the_chat_page_tells_the_person_what_test_the_planner_changed() {
    let page = text("skills/codeumbrella/SKILL.md");
    let line = page.lines().find(|l| l.contains("bad build")).unwrap();
    assert!(line.contains("look_at_tests"), "{line}");
    assert!(line.contains("planner"), "{line}");
    assert!(line.contains("test"), "{line}");
}
