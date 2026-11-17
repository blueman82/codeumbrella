//! Tests of the plan attacker: its step in the sheet, its role files, its
//! settings and its rules page. The worker is simulated. No Claude, no Codex.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::{Graph, Kind};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs, process};

const FILES: [&str; 3] = [
    "roles/attacker.md",
    "roles/attacker.claude.json",
    "roles/attacker.codex.txt",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn json(rel: &str) -> Value {
    serde_json::from_str(&text(rel)).unwrap_or_else(|e| panic!("{rel} is not JSON: {e}"))
}

fn folder() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    env::temp_dir().join(format!("codeumbrella-attacker-{}-{n}", process::id()))
}

fn cu(dir: &Path, args: &[&str]) -> bool {
    Command::new(env!("CARGO_BIN_EXE_codeumbrella"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("start codeumbrella")
        .status
        .success()
}

fn default_graph() -> Graph {
    Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap()
}

#[test]
fn the_default_sheet_has_the_attack_step_between_the_plan_and_the_person() {
    let graph = default_graph();
    let attack = graph.step("attack_plan").unwrap();
    assert_eq!(attack.kind, Kind::Worker);
    assert_eq!(attack.role.as_deref(), Some("attacker"));
    assert_eq!(attack.reads, ["spec", "tests"]);
    assert_eq!(attack.writes, [] as [String; 0]);
    assert_eq!(attack.on_bad.as_deref(), Some("plan"));
    assert_eq!(attack.next.as_deref(), Some("look_at_tests"));
    let plan = graph.step("plan").unwrap();
    assert_eq!(plan.next.as_deref(), Some("attack_plan"));
    assert_eq!(plan.max_tries, Some(5));
    // Only the attack step leads to the person for the tests.
    for (name, step) in &graph.steps {
        let to_person = [&step.next, &step.on_bad].contains(&&Some("look_at_tests".to_owned()));
        assert_eq!(to_person, name == "attack_plan", "{name}");
    }
}

#[test]
fn the_three_role_files_exist_and_init_writes_them() {
    for rel in FILES {
        assert!(!text(rel).trim().is_empty(), "{rel} is empty");
    }
    let dir = folder();
    fs::create_dir_all(&dir).unwrap();
    assert!(cu(&dir, &["init"]));
    for rel in FILES {
        let written = dir.join(".codeumbrella").join(rel);
        assert_eq!(fs::read_to_string(&written).unwrap(), text(rel), "{rel}");
    }
    // `init --update` puts a missing file back.
    let gone = dir.join(".codeumbrella/roles/attacker.codex.txt");
    fs::remove_file(&gone).unwrap();
    assert!(cu(&dir, &["init", "--update"]));
    assert_eq!(
        fs::read_to_string(&gone).unwrap(),
        text("roles/attacker.codex.txt")
    );
}

fn list(value: &Value) -> Vec<&str> {
    let items = value.as_array().expect("a list");
    items.iter().map(|v| v.as_str().expect("text")).collect()
}

#[test]
fn the_attacker_has_a_shell_in_the_sandbox_and_cannot_write_a_file() {
    let file = json("roles/attacker.claude.json");
    let allow = list(&file["permissions"]["allow"]);
    assert_eq!(allow, ["Read", "Glob", "Grep", "Bash"]);
    let deny = list(&file["permissions"]["deny"]);
    assert!(deny.contains(&"Edit(/{project}/**)"), "{deny:?}");
    assert!(deny.contains(&"Bash(git push *)"), "{deny:?}");
    for rule in ["WebFetch", "WebSearch"] {
        assert!(deny.contains(&rule), "{rule}");
    }
    assert!(!allow.iter().any(|a| a.starts_with("Edit")), "{allow:?}");
    assert!(!allow.contains(&"Write"), "{allow:?}");
    assert_eq!(file["disableAllHooks"], true);
    assert_eq!(file["sandbox"]["enabled"], true);
    assert_eq!(file["sandbox"]["allowUnsandboxedCommands"], false);
    assert_eq!(file["sandbox"]["failIfUnavailable"], true);
    let network = file["sandbox"]["network"].as_object().expect("network");
    assert_eq!(network.keys().collect::<Vec<_>>(), ["allowLocalBinding"]);
    let keys = list(&file["sandbox"]["filesystem"]["denyRead"]);
    assert_eq!(keys.len(), 5, "{keys:?}");
    for key in ["~/.ssh", "~/.config/gh", "~/.netrc", "~/.git-credentials"] {
        assert!(keys.contains(&key), "{key}");
    }
    assert!(keys.contains(&"~/Library/Keychains"));
    // The settings are the same as the settings of the safety checker.
    assert_eq!(file, json("roles/safety.claude.json"));
    let codex = text("roles/attacker.codex.txt");
    assert!(codex.starts_with("{extends=\":read-only\""), "{codex}");
    assert!(!codex.contains("\"write\"") && !codex.contains("danger"));
    assert_eq!(codex, text("roles/safety.codex.txt"));
}

/// The numbers at the start of the rules of a page.
fn numbers(rel: &str) -> Vec<usize> {
    let page = text(rel);
    let lines = page.lines();
    lines
        .filter_map(|l| l.split_once(". ")?.0.parse().ok())
        .collect()
}

#[test]
fn the_rules_page_of_the_attacker_names_the_three_methods() {
    let page = text("roles/attacker.md").to_lowercase();
    for method in ["pre-mortem", "pre-parade", "red team"] {
        assert!(page.contains(method), "{method}");
    }
    for word in ["agents.md", "claude.md", "ok: false", "ok: true"] {
        assert!(page.contains(word), "{word}");
    }
    assert!(page.contains("100 words or fewer"));
    for rel in ["roles/attacker.md", "roles/safety.md", "roles/planner.md"] {
        let found = numbers(rel);
        let expected: Vec<usize> = (1..=found.len()).collect();
        assert!(!found.is_empty(), "{rel}");
        assert_eq!(found, expected, "{rel}");
    }
}

#[test]
fn the_planner_and_the_safety_checker_have_their_new_rules() {
    let planner = text("roles/planner.md").to_lowercase();
    assert!(planner.contains("attacker"));
    let safety = text("roles/safety.md");
    assert!(safety.contains("settings files"));
    assert!(safety.contains("17 November 2026") || safety.contains("use the changed lines"));
}

/// A worker that gives a bad result for one step a set number of times.
struct Script {
    bad_step: &'static str,
    bad_times: Mutex<u32>,
    calls: Mutex<Vec<String>>,
}

impl Script {
    fn new(bad_step: &'static str, bad_times: u32) -> Self {
        Self {
            bad_step,
            bad_times: Mutex::new(bad_times),
            calls: Mutex::default(),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Worker for Script {
    fn run(&self, job: &Job) -> Result<Outcome> {
        if job.step != "check_safety" {
            self.calls.lock().unwrap().push(job.step.to_owned());
        }
        for paper in job.writes {
            fs::write(job.dir.join("papers").join(paper).join("file.md"), job.step)?;
        }
        let mut left = self.bad_times.lock().unwrap();
        let mut ok = true;
        if job.step == self.bad_step {
            ok = *left == 0;
            *left = left.saturating_sub(1);
        }
        Ok(Outcome {
            ok,
            summary: format!("{} ran", job.step),
        })
    }
}

/// The default sheet, with a command that does nothing in each command step.
fn sheet() -> (Graph, PathBuf) {
    let roles = root().join("roles");
    let mut graph = default_graph();
    for step in graph.steps.values_mut().filter(|s| !s.run.is_empty()) {
        step.run = vec![vec!["true".to_owned()]];
    }
    (graph, roles)
}

#[test]
fn a_good_attack_result_goes_to_the_person() {
    let (graph, roles) = sheet();
    let worker = Script::new("attack_plan", 0);
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    assert_eq!(worker.calls(), ["plan", "attack_plan"]);
}

#[test]
fn a_bad_attack_result_goes_to_the_planner_and_then_back_to_the_attacker() {
    let (graph, roles) = sheet();
    let worker = Script::new("attack_plan", 1);
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    // The person sees the tests only after a good attack result.
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    assert_eq!(
        worker.calls(),
        ["plan", "attack_plan", "plan", "attack_plan"]
    );
    assert!(!run.state.results.contains_key("build"));
}

#[test]
fn the_planner_has_five_tries_and_the_run_stops_after_them() {
    let (graph, roles) = sheet();
    let worker = Script::new("attack_plan", 99);
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Stopped);
    assert!(
        run.state.note.contains("maximum of 5"),
        "{}",
        run.state.note
    );
    let plans = worker.calls().iter().filter(|c| *c == "plan").count();
    assert_eq!(plans, 5);
    assert!(!run.state.results.contains_key("build"));
}

#[test]
fn a_no_from_the_person_still_goes_to_the_planner_and_the_attacker() {
    let (graph, roles) = sheet();
    let worker = Script::new("attack_plan", 0);
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(false, "The tests are too easy.").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    assert_eq!(
        worker.calls(),
        ["plan", "attack_plan", "plan", "attack_plan"]
    );
}

#[test]
fn the_four_version_numbers_are_the_same() {
    let cargo = text("Cargo.toml");
    let line = cargo
        .lines()
        .find(|l| l.starts_with("version = \""))
        .expect("version line");
    let version = line
        .trim_start_matches("version = \"")
        .trim_end_matches('"');
    let lock = text("Cargo.lock");
    let (_, after) = lock
        .split_once("name = \"codeumbrella\"\n")
        .expect("package");
    let locked = after.lines().next().expect("version line");
    assert_eq!(locked, format!("version = \"{version}\""));
    for rel in [".claude-plugin/plugin.json", ".codex-plugin/plugin.json"] {
        assert_eq!(json(rel)["version"], version, "{rel}");
    }
}

/// Counts as `grep -v -E '^\s*(//|$)' <file> | wc -l` does.
fn code_lines(rel: &str) -> usize {
    let page = text(rel);
    let lines = page.lines().map(str::trim);
    lines
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .count()
}

#[test]
fn the_code_obeys_the_limits_for_lines() {
    let supervisor = ["graph", "engine", "main", "log", "lib"];
    let total: usize = supervisor
        .iter()
        .map(|name| code_lines(&format!("src/{name}.rs")))
        .sum();
    assert!(total <= 560, "the supervisor has {total} lines");
    for part in ["claude", "codex"] {
        let lines = code_lines(&format!("src/{part}.rs"));
        assert!(lines <= 100, "{part} has {lines} lines");
    }
}
