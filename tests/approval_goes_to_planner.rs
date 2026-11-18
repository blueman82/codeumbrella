//! Tests: a no at the PR stop goes to the planner. The worker is simulated.

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
        if !["check_safety", "check_review", "check_security"].contains(&job.step) {
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
fn a_no_at_the_approval_goes_to_the_planner_and_then_to_the_person_for_the_tests() {
    let (graph, roles) = sheet();
    let worker = Script::default();
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
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
fn the_numbers_of_the_rules_are_in_sequence() {
    for path in ["roles/builder.md", "roles/checker.md", "roles/planner.md"] {
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
