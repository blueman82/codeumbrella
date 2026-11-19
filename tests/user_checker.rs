//! Tests of the fifth checker `check_use` and its role `user`: the sheet,
//! the settings files, `init`, the rules pages and the documents. The worker
//! is simulated. No Claude and no Codex.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::{Graph, Kind};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// The fan parts of `check`, in the sequence of the sheet.
const PARTS: [&str; 5] = [
    "check_tests",
    "check_safety",
    "check_review",
    "check_security",
    "check_use",
];

/// The three files of the role `user`.
const FILES: [&str; 3] = [
    "roles/user.md",
    "roles/user.claude.json",
    "roles/user.codex.txt",
];

/// The network entry of the Codex settings of `user`.
const NETWORK: &str = r#"network={enabled=true, mode="limited", allow_local_binding=true}, "#;

/// The start of the Codex settings of the attacker.
const READ_ONLY: &str = r#"{extends=":read-only", "#;

/// The words of point L of the task. The words `true` and `false` are not
/// in it, because a rules page gives `ok: true` and `ok: false`.
const NOT_PERMITTED: [&str; 38] = [
    "program",
    "never",
    "security",
    "process",
    "already",
    "way",
    "case",
    "need",
    "return",
    "place",
    "permit",
    "now",
    "real",
    "exact",
    "later",
    "confirm",
    "decide",
    "few",
    "leaves",
    "choice",
    "take",
    "avoid",
    "aim",
    "old",
    "ask",
    "both",
    "wrong",
    "pass",
    "whose",
    "outside",
    "still",
    "missing",
    "absent",
    "preparation",
    "rest",
    "yet",
    "ready",
    "conflict",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn roles() -> PathBuf {
    root().join("roles")
}

fn text(rel: &str) -> String {
    let path = root().join(rel);
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn json(rel: &str) -> Value {
    serde_json::from_str(&text(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn default_sheet() -> Graph {
    let path = root().join("graph.example.yaml");
    Graph::load(&path, &roles()).unwrap()
}

/// The default sheet. Each command step gets a command that does nothing.
fn quiet_sheet() -> Graph {
    let mut graph = default_sheet();
    for step in graph.steps.values_mut() {
        if !step.run.is_empty() {
            step.run = vec![vec!["true".to_owned()]];
        }
    }
    graph
}

/// A new empty folder. The path is the full one: on a Mac,
/// `/private/var/...` and not `/var/...`.
fn folder() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-user-{}-{n}", std::process::id());
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

// ---------------------------------------------------------------------
// The sheet
// ---------------------------------------------------------------------

#[test]
fn the_check_is_a_fan_of_five_parts_in_this_sequence() {
    let graph = default_sheet();
    let check = graph.step("check").unwrap();
    assert_eq!(check.kind, Kind::Fan);
    assert_eq!(check.fan, PARTS);
    assert_eq!(check.on_bad.as_deref(), Some("repair"));
    assert_eq!(check.next.as_deref(), Some("work_was_done"));
}

#[test]
fn check_use_has_the_role_user_reads_only_spec_and_writes_no_paper() {
    let graph = default_sheet();
    let part = graph.step("check_use").unwrap();
    assert_eq!(part.kind, Kind::Worker);
    assert_eq!(part.role.as_deref(), Some("user"));
    assert_eq!(part.reads, ["spec"]);
    assert!(part.writes.is_empty(), "{:?}", part.writes);
    assert!(part.run.is_empty() && part.fan.is_empty());
    assert!(part.next.is_none() && part.on_bad.is_none());
    assert!(part.max_tries.is_none());
    assert!(part.tool.is_none());
    assert!(part.model.is_none() && part.effort.is_none());
}

#[test]
fn the_sheet_has_one_new_step_and_no_other_step_goes_or_comes() {
    let graph = default_sheet();
    let mut names: Vec<&str> = graph.steps.keys().map(String::as_str).collect();
    names.sort_unstable();
    let mut want = [
        "copy",
        "plan",
        "attack_plan",
        "correct_plan",
        "attack_plan_2",
        "correct_plan_2",
        "attack_plan_3",
        "correct_plan_3",
        "look_at_tests",
        "mark",
        "build",
        "simplify",
        "learn",
        "write_pr",
        "check",
        "work_was_done",
        "check_tests",
        "check_safety",
        "check_review",
        "check_security",
        "check_use",
        "repair",
        "no_locked_files",
        "push",
        "find_pr",
        "open_pr",
        "approval",
        "merge",
        "save_lessons",
        "clean_up",
    ];
    want.sort_unstable();
    assert_eq!(names, want);
    // Only the fan of `check` names the new step.
    for (name, step) in &graph.steps {
        let targets = [&step.next, &step.on_bad];
        let goes = targets.contains(&&Some("check_use".to_owned()));
        assert!(!goes, "{name} goes to check_use");
        let fans = step.fan.iter().any(|f| f == "check_use");
        assert_eq!(fans, name == "check", "{name}");
    }
}

/// The comment lines right above the step `name` in the default sheet.
fn comment_above(name: &str) -> String {
    let sheet = text("graph.example.yaml");
    let lines: Vec<&str> = sheet.lines().collect();
    let head = format!("  {name}:");
    let at = lines.iter().position(|l| *l == head);
    let at = at.unwrap_or_else(|| panic!("no line {head}"));
    let mut found = Vec::new();
    for line in lines[..at].iter().rev() {
        if !line.trim_start().starts_with('#') {
            break;
        }
        found.push(*line);
    }
    found.reverse();
    found.join("\n").to_lowercase()
}

#[test]
fn the_comment_of_check_use_tells_what_it_does_and_how_to_remove_it() {
    let comment = comment_above("check_use");
    assert!(comment.contains("example"), "{comment}");
    assert!(comment.contains("faster run"), "{comment}");
    assert!(comment.contains("remove"), "{comment}");
    assert!(comment.contains("fan"), "{comment}");
}

#[test]
fn the_comment_of_the_check_names_five_checkers() {
    let comment = comment_above("check");
    assert!(comment.contains("five checkers"), "{comment}");
    let sheet = text("graph.example.yaml");
    assert!(!sheet.contains("Four checkers"), "graph.example.yaml");
}

// ---------------------------------------------------------------------
// The settings files of the role `user`
// ---------------------------------------------------------------------

#[test]
fn the_claude_settings_of_user_are_those_of_the_attacker() {
    let user = text("roles/user.claude.json");
    assert_eq!(user, text("roles/attacker.claude.json"));
    // The file is correct JSON with the sandbox on.
    let file = json("roles/user.claude.json");
    assert_eq!(file["sandbox"]["enabled"], true);
    assert_eq!(file["disableAllHooks"], true);
}

#[test]
fn the_codex_settings_of_user_are_those_of_the_attacker_and_the_network_entry() {
    let user = text("roles/user.codex.txt");
    let attacker = text("roles/attacker.codex.txt");
    assert_eq!(user.matches(NETWORK).count(), 1, "{user}");
    assert_eq!(user.replacen(NETWORK, "", 1), attacker);
    let after = user.strip_prefix(READ_ONLY);
    let first = after.is_some_and(|a| a.starts_with(NETWORK));
    assert!(first, "the entry is not after the first entry: {user}");
    assert!(user.contains("allow_local_binding=true"), "{user}");
    assert!(user.contains(r#"":tmpdir"="write""#), "{user}");
    assert!(!user.contains("danger"), "{user}");
}

// ---------------------------------------------------------------------
// `init` and `init --update`
// ---------------------------------------------------------------------

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

fn written(dir: &Path, rel: &str) -> String {
    let path = dir.join(".codeumbrella").join(rel);
    let read = fs::read_to_string(&path);
    read.unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

#[test]
fn init_writes_the_three_files_of_user() {
    let dir = folder();
    let out = cu(&dir, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    for rel in FILES {
        assert_eq!(written(&dir, rel), text(rel), "{rel}");
    }
}

#[test]
fn init_update_writes_the_three_files_into_a_project_of_before() {
    let dir = folder();
    let out = cu(&dir, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    // A project of before has no files of the role `user`.
    for rel in FILES {
        let path = dir.join(".codeumbrella").join(rel);
        fs::remove_file(path).unwrap();
    }
    let out = cu(&dir, &["init", "--update"]);
    assert!(out.status.success(), "{}", all(&out));
    let printed = all(&out);
    for rel in FILES {
        assert_eq!(written(&dir, rel), text(rel), "{rel}");
        assert!(printed.contains(rel), "{rel}: {printed}");
    }
}

// ---------------------------------------------------------------------
// The rules page of the role `user`
// ---------------------------------------------------------------------

#[test]
fn the_user_page_has_the_commands_and_the_words_of_the_task() {
    let page = text("roles/user.md");
    let words = [
        r#"mktemp -d "$TMPDIR/scratch.XXXXXX""#,
        "git clone",
        "codeumbrella/<run id>",
        "rm -r",
        "kill -0",
        "HOME",
        "Not run:",
        "No example:",
        "Examples for a user",
        "agrees",
        "differs",
        "AGENTS.md",
        "CLAUDE.md",
        "ok: true",
        "ok: false",
    ];
    for word in words {
        assert!(page.contains(word), "{word}");
    }
}

/// The words of a text: letters and digits, split at each other character.
fn words(text: &str) -> Vec<&str> {
    let parts = text.split(|c: char| !c.is_alphanumeric());
    parts.filter(|w| !w.is_empty()).collect()
}

#[test]
fn the_user_page_names_no_language_and_no_tool_of_one_language() {
    let page = text("roles/user.md");
    let found = words(&page);
    let lower = page.to_lowercase();
    let low = words(&lower);
    for tool in ["cargo", "npm", "pip", "mvn", "gradle", "rustc"] {
        assert!(!low.contains(&tool), "{tool}");
    }
    let pages = fs::read_dir(roles().join("languages")).unwrap();
    let mut seen = 0;
    for entry in pages {
        let path = entry.unwrap().path();
        let stem = path.file_stem().unwrap().to_string_lossy();
        let stem = stem.to_lowercase();
        seen += 1;
        if stem == "go" {
            // `go` is also a plain word. Only the name `Go` counts.
            assert!(!found.contains(&"Go"), "Go");
        } else {
            assert!(!low.contains(&stem.as_str()), "{stem}");
        }
    }
    assert!(seen > 0, "no language page");
}

/// A text with no part in code marks.
fn plain(text: &str) -> String {
    let parts = text.split('`').step_by(2);
    parts.collect::<Vec<&str>>().join(" ")
}

#[test]
fn the_user_page_has_none_of_the_words_that_are_not_permitted() {
    let page = plain(&text("roles/user.md")).to_lowercase();
    let found = words(&page);
    for bad in NOT_PERMITTED {
        assert!(!found.contains(&bad), "{bad}");
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
fn each_sentence_of_the_user_page_has_20_words_at_the_maximum() {
    for line in text("roles/user.md").lines() {
        for sentence in body(line).split(". ") {
            let count = sentence.split_whitespace().count();
            assert!(count <= 20, "{count} words: {sentence}");
        }
    }
}

/// The numbers at the start of the lines of a page, in sequence.
fn numbers(page: &str) -> Vec<usize> {
    let lines = page.lines();
    lines
        .filter_map(|l| l.split_once(". ")?.0.parse().ok())
        .collect()
}

#[test]
fn the_rules_of_the_user_page_attacker_page_and_planner_page_have_no_hole() {
    for rel in ["roles/user.md", "roles/attacker.md", "roles/planner.md"] {
        let found = numbers(&text(rel));
        assert!(!found.is_empty(), "{rel}: no rule");
        let want: Vec<usize> = (1..=found.len()).collect();
        assert_eq!(found, want, "{rel}");
    }
}

/// The rule with this number in the text of a page, with its sub-points.
/// An empty text if the page has no such rule.
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

/// The rule of a page that has the words `Examples for a user`.
fn examples_rule(rel: &str) -> String {
    let page = text(rel);
    let count = numbers(&page).len();
    let rules = (1..=count).map(|n| rule_in(&page, n));
    let mut found = rules.filter(|r| r.contains("Examples for a user"));
    let rule = found.next();
    rule.unwrap_or_else(|| panic!("{rel}: no rule with Examples for a user"))
}

#[test]
fn the_planner_page_has_a_rule_for_the_examples() {
    let rule = examples_rule("roles/planner.md");
    let want = ["## Examples for a user", "E1", "No example:"];
    for word in want {
        assert!(rule.contains(word), "{word}: {rule}");
    }
    assert!(rule.contains("spec.md"), "{rule}");
}

#[test]
fn the_attacker_page_has_a_rule_for_the_examples() {
    let rule = examples_rule("roles/attacker.md");
    for word in ["No example:", "spec.md", "problem"] {
        assert!(rule.contains(word), "{word}: {rule}");
    }
}

// ---------------------------------------------------------------------
// The chat page and the documents
// ---------------------------------------------------------------------

/// The list items and the paragraphs of a page.
fn items(page: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut open = false;
    for line in page.lines() {
        let new = line.starts_with("- ") || line.starts_with('#');
        if line.trim().is_empty() {
            open = false;
        } else if open && !new {
            if let Some(last) = found.last_mut() {
                last.push(' ');
                last.push_str(line.trim());
            }
        } else {
            found.push(line.to_owned());
            open = true;
        }
    }
    found
}

#[test]
fn the_chat_tells_each_not_run_line_of_check_use_at_the_approval() {
    let page = text("skills/codeumbrella/SKILL.md");
    let words = ["`approval`", "check_use", "log.jsonl", "Not run:"];
    let all = items(&page);
    let found = all.iter().any(|i| words.iter().all(|w| i.contains(w)));
    assert!(found, "no item of SKILL.md has {words:?}");
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
fn the_safety_rules_of_agents_md_name_five_checkers_and_check_use() {
    let rules = section("AGENTS.md", "Safety rules");
    assert!(rules.contains("fan of five checkers"), "five");
    assert!(!rules.contains("fan of four checkers"), "four");
    assert!(rules.contains("check_use"), "check_use");
    let old = "The four checkers work in the same copy.";
    assert!(!rules.contains(old), "{old}");
    assert!(rules.contains("18 November 2026"), "date");
}

#[test]
fn the_structure_of_agents_md_names_the_test_file() {
    let part = section("AGENTS.md", "Structure");
    let name = "`tests/user_checker.rs`";
    let line = part.lines().find(|l| l.contains(name));
    assert!(line.is_some_and(|l| l.starts_with('|')), "no table line");
}

#[test]
fn the_readme_and_the_architecture_name_the_fifth_checker() {
    let readme = text("README.md");
    assert!(!readme.contains("Four checkers"), "README.md");
    assert!(readme.contains("Five checkers"), "README.md");
    let path = section("ARCHITECTURE.md", "The path of a task");
    assert!(!path.contains("FAN of 4"), "ARCHITECTURE.md path");
    assert!(path.contains("check_use"), "ARCHITECTURE.md path");
    let parts = section("ARCHITECTURE.md", "The parts");
    let role = parts.lines().any(|l| l.starts_with("│ user "));
    assert!(role, "ARCHITECTURE.md: no line for the role user");
}

// ---------------------------------------------------------------------
// Runs with a simulated worker
// ---------------------------------------------------------------------

/// What the simulated worker saw in one call of a step.
#[derive(Clone, Debug)]
struct Seen {
    step: String,
    rules: PathBuf,
    reads: Vec<String>,
    writes: Vec<String>,
    before: String,
}

/// A simulated worker. It writes its papers. Each step in `bad` gives one
/// bad result, and each other call gives a good result.
struct Sim {
    bad: Mutex<Vec<&'static str>>,
    seen: Mutex<Vec<Seen>>,
}

impl Sim {
    fn new(bad: &[&'static str]) -> Self {
        Self {
            bad: Mutex::new(bad.to_vec()),
            seen: Mutex::default(),
        }
    }

    /// Each call of the step `step`, in sequence.
    fn calls(&self, step: &str) -> Vec<Seen> {
        let seen = self.seen.lock().unwrap();
        seen.iter().filter(|s| s.step == step).cloned().collect()
    }
}

impl Worker for Sim {
    fn run(&self, job: &Job) -> Result<Outcome> {
        let before = job.before.map_or_else(String::new, |o| o.summary.clone());
        let seen = Seen {
            step: job.step.to_owned(),
            rules: job.rules.to_path_buf(),
            reads: job.reads.to_vec(),
            writes: job.writes.to_vec(),
            before,
        };
        self.seen.lock().unwrap().push(seen);
        for paper in job.writes {
            let path = job.dir.join("papers").join(paper);
            fs::write(path.join("file.md"), job.step)?;
        }
        let mut bad = self.bad.lock().unwrap();
        let at = bad.iter().position(|s| *s == job.step);
        if let Some(at) = at {
            bad.remove(at);
        }
        Ok(Outcome {
            ok: at.is_none(),
            summary: format!("{} ran", job.step),
        })
    }
}

/// A run with the quiet default sheet. It goes to `look_at_tests`, gets a
/// yes, and goes on.
fn run_with(sim: &Sim) -> Run {
    let graph = quiet_sheet();
    let dir = folder().join(".codeumbrella/runs/run-1");
    let mut run = Run::start(dir, "a task", &graph).unwrap();
    fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles(), sim).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    run.answer(true, "").unwrap();
    run.go(&graph, &roles(), sim).unwrap();
    run
}

/// The lines of the log: each step with its result.
fn log(run: &Run) -> Vec<(String, bool)> {
    let text = fs::read_to_string(run.dir.join("log.jsonl")).unwrap();
    let mut found = Vec::new();
    for line in text.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        let step = value["step"].as_str().unwrap().to_owned();
        found.push((step, value["ok"].as_bool().unwrap()));
    }
    found
}

/// The steps of the log after the yes at `look_at_tests`. The fan parts
/// are not in it.
fn after_yes(run: &Run) -> Vec<(String, bool)> {
    let all = log(run);
    let at = all.iter().rposition(|(s, _)| s == "look_at_tests");
    let at = at.unwrap_or_else(|| panic!("no look_at_tests: {all:?}"));
    let after = all[at + 1..].iter();
    let steps = after.filter(|(s, _)| !PARTS.contains(&s.as_str()));
    steps.cloned().collect()
}

/// The results of one fan part in the log, in sequence.
fn part_results(run: &Run, part: &str) -> Vec<bool> {
    let all = log(run);
    let mine = all.iter().filter(|(s, _)| s == part);
    mine.map(|(_, ok)| *ok).collect()
}

fn pairs(list: &[(&str, bool)]) -> Vec<(String, bool)> {
    list.iter().map(|(s, ok)| ((*s).to_owned(), *ok)).collect()
}

/// Checks the job of each call of `check_use`.
fn check_use_jobs(sim: &Sim, times: usize) {
    let calls = sim.calls("check_use");
    assert_eq!(calls.len(), times, "{calls:?}");
    for call in calls {
        assert!(call.rules.ends_with("user.md"), "{:?}", call.rules);
        assert_eq!(call.reads, ["spec"]);
        assert!(call.writes.is_empty(), "{:?}", call.writes);
    }
}

#[test]
fn a_bad_check_use_alone_goes_to_repair_with_its_report() {
    let sim = Sim::new(&["check_use"]);
    let run = run_with(&sim);
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    let path = pairs(&[
        ("mark", true),
        ("build", true),
        ("simplify", true),
        ("learn", true),
        ("write_pr", true),
        ("check", false),
        ("repair", true),
        ("write_pr", true),
        ("check", true),
        ("work_was_done", true),
        ("no_locked_files", true),
        ("push", true),
        ("find_pr", true),
    ]);
    assert_eq!(after_yes(&run), path);
    assert_eq!(part_results(&run, "check_use"), [false, true]);
    for part in &PARTS[..4] {
        assert_eq!(part_results(&run, part), [true, true], "{part}");
    }
    // The builder in `repair` gets the report of the fan.
    let repair = sim.calls("repair");
    assert_eq!(repair.len(), 1, "{repair:?}");
    let got = &repair[0].before;
    assert!(got.contains("check_use (bad)"), "{got}");
    assert!(got.contains("check_tests (good)"), "{got}");
    check_use_jobs(&sim, 2);
}

#[test]
fn with_good_workers_the_run_goes_from_check_to_work_was_done() {
    let sim = Sim::new(&[]);
    let run = run_with(&sim);
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    let path = pairs(&[
        ("mark", true),
        ("build", true),
        ("simplify", true),
        ("learn", true),
        ("write_pr", true),
        ("check", true),
        ("work_was_done", true),
        ("no_locked_files", true),
        ("push", true),
        ("find_pr", true),
    ]);
    assert_eq!(after_yes(&run), path);
    for part in PARTS {
        assert_eq!(part_results(&run, part), [true], "{part}");
    }
    assert!(sim.calls("repair").is_empty());
    check_use_jobs(&sim, 1);
}
