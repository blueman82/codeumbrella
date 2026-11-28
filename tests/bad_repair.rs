//! Tests: the step `repair` of the default sheet goes to the plan after a
//! bad result. Also the new rules of the checker, the builder, the planner
//! and the attacker, the chat page and the documents. Also the path of a
//! check with a computer error (`No check:`). No Claude, no Codex.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::Graph;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{self, Command};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

/// The words of point K of the task. The words `true` and `false` are not
/// in it, because a rules page gives `ok: true` and `ok: false`.
const NOT_PERMITTED: [&str; 47] = [
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
    "once",
    "just",
    "size",
    "state",
    "round",
    "early",
    "findings",
    "carry",
    "order",
];

/// Each step of the default sheet with an `on_bad` line, and its target.
const ON_BAD: [(&str, &str); 14] = [
    ("attack_plan", "correct_plan"),
    ("attack_plan_2", "correct_plan_2"),
    ("attack_plan_3", "correct_plan_3"),
    ("look_at_tests", "plan"),
    ("build", "plan"),
    ("check", "repair"),
    ("work_was_done", "do_work"),
    ("take_master", "fix_merge"),
    ("fix_merge", "fix_merge"),
    ("do_work", "plan"),
    ("repair", "plan"),
    ("find_pr", "open_pr"),
    ("approval", "plan"),
    ("same_master", "take_master"),
];

/// Each step of the default sheet with a `max_tries` line, and its number.
const MAX_TRIES: [(&str, u32); 9] = [
    ("plan", 3),
    ("correct_plan", 3),
    ("correct_plan_2", 3),
    ("correct_plan_3", 3),
    ("repair", 3),
    ("take_master", 3),
    ("fix_merge", 3),
    ("do_work", 3),
    ("open_pr", 2),
];

const SKILL: &str = "skills/codeumbrella/SKILL.md";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let path = root().join(rel);
    let read = fs::read_to_string(path);
    read.unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn default_sheet() -> Graph {
    let path = root().join("graph.example.yaml");
    Graph::load(&path, &root().join("roles")).unwrap()
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

/// The rule with this number of a page. The rule must be there.
fn rule(rel: &str, number: usize) -> String {
    let found = rule_in(&text(rel), number);
    assert!(!found.is_empty(), "{rel}: no rule {number}");
    found
}

/// The first rule of a page that has the word `word`.
fn rule_with(rel: &str, word: &str) -> String {
    let page = text(rel);
    let mut rules = (1..40).map(|n| rule_in(&page, n));
    let found = rules.find(|r| r.contains(word));
    found.unwrap_or_else(|| panic!("{rel}: no rule with {word}"))
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

/// The list items and the paragraphs of a text, each on one line.
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

/// The first item of the part `title` of a page with the word `word`.
fn item_with(rel: &str, title: &str, word: &str) -> String {
    let part = section(rel, title);
    let found = items(&part).into_iter().find(|i| i.contains(word));
    found.unwrap_or_else(|| panic!("{rel}: no item with {word} in {title}"))
}

/// A text with no part in code marks.
fn plain(text: &str) -> String {
    let parts = text.split('`').step_by(2);
    parts.collect::<Vec<&str>>().join(" ")
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

/// Checks a new text against point K: no word of the list out of code
/// marks, no semicolon, and 20 words at the maximum for a sentence.
fn plain_and_short(name: &str, new: &str) {
    let low = plain(new).to_lowercase();
    let split = low.split(|c: char| !(c.is_alphanumeric() || c == '_'));
    let found: Vec<&str> = split.filter(|w| !w.is_empty()).collect();
    for bad in NOT_PERMITTED {
        assert!(!found.contains(&bad), "{name}: {bad}: {new}");
    }
    assert!(!plain(new).contains(';'), "{name}: a semicolon: {new}");
    for line in new.lines() {
        for sentence in body(line).split(". ") {
            let count = sentence.split_whitespace().count();
            assert!(count <= 20, "{name}: {count} words: {sentence}");
        }
    }
}

// ---------------------------------------------------------------------
// The sheet
// ---------------------------------------------------------------------

#[test]
fn only_the_step_repair_gets_a_new_line_in_the_sheet() {
    let graph = default_sheet();
    for (name, step) in &graph.steps {
        let to = ON_BAD.iter().find(|(s, _)| *s == name.as_str());
        let to = to.map(|(_, t)| (*t).to_owned());
        assert_eq!(step.on_bad, to, "{name}: on_bad");
        let max = MAX_TRIES.iter().find(|(s, _)| *s == name.as_str());
        let max = max.map(|(_, n)| *n);
        assert_eq!(step.max_tries, max, "{name}: max_tries");
    }
    let repair = graph.step("repair").unwrap();
    assert_eq!(repair.role.as_deref(), Some("builder"));
    assert_eq!(repair.reads, ["spec"]);
    assert_eq!(repair.next.as_deref(), Some("write_pr"));
    let build = graph.step("build").unwrap();
    assert_eq!(build.next.as_deref(), Some("simplify"));
}

#[test]
fn the_comments_of_the_sheet_tell_what_the_new_line_of_repair_does() {
    let comment = comment_above("repair");
    assert!(comment.contains("plan"), "{comment}");
    let sheet = text("graph.example.yaml");
    let gone = "The step `repair` has no `on_bad` line.";
    assert!(!sheet.contains(gone), "graph.example.yaml: {gone}");
}

// ---------------------------------------------------------------------
// The rules pages
// ---------------------------------------------------------------------

#[test]
fn the_checker_names_a_paper_test_that_no_code_can_repair() {
    let rule = rule_with("roles/checker.md", "Paper test:");
    for word in ["summary", "cause", "test file"] {
        assert!(rule.contains(word), "{word}: {rule}");
    }
    let page = text("roles/checker.md");
    assert!(page.contains("Change no paper."), "rule 5 of today");
    plain_and_short("checker", &rule);
}

#[test]
fn rule_9_of_the_builder_is_also_for_the_repair() {
    let nine = rule("roles/builder.md", 9);
    let words = [
        "`repair`",
        "Paper test:",
        "ok: false",
        "planner",
        "each other point",
        "commit",
    ];
    for word in words {
        assert!(nine.contains(word), "{word}: {nine}");
    }
    plain_and_short("builder", &nine);
}

#[test]
fn the_planner_rule_of_a_bad_build_is_also_for_a_bad_repair() {
    let rule = rule_with("roles/planner.md", "bad repair");
    let words = [
        "bad build",
        "only that test",
        "character for character",
        "summary",
    ];
    for word in words {
        assert!(rule.contains(word), "{word}: {rule}");
    }
    plain_and_short("planner", &rule);
}

#[test]
fn rule_5_of_the_attacker_has_the_three_new_lines() {
    let five = rule("roles/attacker.md", 5);
    let wait = [
        "Do not remove the scratch folder while a command of yours runs in it.",
        "First wait until each command ends, or end it.",
    ];
    for sentence in wait {
        assert!(five.contains(sentence), "{sentence}: {five}");
    }
    let low = five.to_lowercase();
    assert!(low.contains("paper test file"), "{five}");
    assert!(low.contains("time limit"), "{five}");
    let line = five.lines().find(|l| l.contains("Not complete:"));
    let line = line.unwrap_or_else(|| panic!("no Not complete: {five}"));
    assert!(line.contains("not a problem"), "{line}");
    plain_and_short("attacker", &five);
}

// ---------------------------------------------------------------------
// The chat page and the help
// ---------------------------------------------------------------------

#[test]
fn the_chat_tells_a_check_that_did_not_complete_at_look_at_tests() {
    let item = item_with(SKILL, "After each command", "Not complete:");
    assert!(item.contains("look_at_tests"), "{item}");
    assert!(item.to_lowercase().contains("offer"), "{item}");
    plain_and_short("SKILL.md", &item);
}

#[test]
fn the_chat_names_step_minutes_with_the_default_of_the_help() {
    let item = item_with(SKILL, "The command to run", "--step-minutes");
    assert!(item.contains("30"), "{item}");
    assert!(item.contains("answer"), "{item}");
    assert!(item.contains("resume"), "{item}");
    plain_and_short("SKILL.md", &item);
}

#[test]
fn the_help_names_step_minutes_and_its_default() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    let out = command.arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{help}");
    assert!(help.contains("--step-minutes"), "{help}");
    assert!(help.contains("[default: 30]"), "{help}");
}

// ---------------------------------------------------------------------
// The documents
// ---------------------------------------------------------------------

#[test]
fn agents_md_counts_a_bad_repair_in_the_tries_of_the_plan() {
    let agents = text("AGENTS.md");
    let today = "A no of the person or a bad build uses one more.";
    assert!(!agents.contains(today), "{today}");
    let none = "The step `repair` has no such line.";
    assert!(!agents.contains(none), "{none}");
    let rules = section("AGENTS.md", "Safety rules");
    let plan_line = |l: &str| {
        let names_plan = l.contains("3 tries") && l.contains("plan");
        names_plan && l.contains("bad repair")
    };
    let tries = rules.lines().any(plan_line);
    assert!(tries, "no line about the tries of plan with a bad repair");
}

#[test]
fn the_safety_rules_tell_the_three_new_rules_with_the_date() {
    let rules = section("AGENTS.md", "Safety rules");
    assert!(rules.contains("19 November 2026"), "date");
    assert!(rules.contains("Paper test:"), "Paper test:");
    assert!(rules.contains("Not complete:"), "Not complete:");
    let repair_line = |l: &str| {
        let names_repair = l.contains("`repair`");
        names_repair && l.contains("on_bad: plan")
    };
    let repair = rules.lines().any(repair_line);
    assert!(repair, "no line for on_bad: plan of repair");
    let marks = [
        "19 November 2026",
        "Paper test:",
        "Not complete:",
        "bad repair",
    ];
    for line in rules.lines() {
        let is_new = marks.iter().any(|m| line.contains(m));
        if is_new {
            plain_and_short("AGENTS.md", line);
        }
    }
}

#[test]
fn the_structure_of_agents_md_names_the_new_test_file() {
    let part = section("AGENTS.md", "Structure");
    let name = "`tests/bad_repair.rs`";
    let line = part.lines().find(|l| l.contains(name));
    assert!(line.is_some_and(|l| l.starts_with('|')), "no table line");
}

#[test]
fn the_architecture_shows_that_a_bad_repair_goes_to_the_plan() {
    let path = section("ARCHITECTURE.md", "The path of a task");
    let both = |l: &str| {
        let names_repair = l.contains("repair");
        names_repair && l.contains("plan")
    };
    let shows = path.lines().any(both);
    assert!(shows, "no line of the path with repair and plan");
    let today = "A no or a bad build uses one more.";
    assert!(!text("ARCHITECTURE.md").contains(today), "{today}");
}

#[test]
fn the_architecture_has_two_sentences_for_the_server() {
    let page = text("ARCHITECTURE.md");
    let two = "The builder, the checker, the simplifier and the user checker can connect to a server on the same computer. An example is a database.";
    assert!(page.contains(two), "no two sentences");
    let one = "on the same computer, for example a database.";
    assert!(!page.contains(one), "{one}");
}

/// The position of the last `│` of a line, in characters.
fn last_bar(line: &str) -> Option<usize> {
    let chars: Vec<char> = line.chars().collect();
    chars.iter().rposition(|c| *c == '│')
}

#[test]
fn the_bar_of_the_line_of_check_use_is_in_line_with_the_line_above() {
    let path = section("ARCHITECTURE.md", "The path of a task");
    let lines: Vec<&str> = path.lines().collect();
    let at = lines.iter().position(|l| l.contains("──► check_use"));
    let at = at.expect("no line of the fan for check_use");
    let above = lines[at - 1];
    assert!(above.contains("check_security"), "{above}");
    let want = last_bar(above);
    assert!(want.is_some(), "no bar: {above}");
    assert_eq!(last_bar(lines[at]), want, "{}", lines[at]);
}

// ---------------------------------------------------------------------
// A check with a computer error: the path of the run
// ---------------------------------------------------------------------

/// A simulated worker on a computer with an error. In its first `broken`
/// times, each part of the fan gives `No check:`. The step `repair` changes
/// no file and gives a bad result. The planner writes only the first time.
struct NoCheck {
    broken: usize,
    calls: Mutex<Vec<String>>,
}

impl NoCheck {
    fn new(broken: usize) -> Self {
        Self {
            broken,
            calls: Mutex::default(),
        }
    }

    fn count(&self, step: &str) -> usize {
        let calls = self.calls.lock().unwrap();
        calls.iter().filter(|c| c.as_str() == step).count()
    }
}

impl Worker for NoCheck {
    fn run(&self, job: &Job) -> Result<Outcome> {
        self.calls.lock().unwrap().push(job.step.to_owned());
        let tries = self.count(job.step);
        if job.step != "plan" || tries == 1 {
            for paper in job.writes {
                let path = job.dir.join("papers").join(paper);
                fs::write(path.join("file.md"), job.step)?;
            }
        }
        let broken = job.step.starts_with("check_") && tries <= self.broken;
        let (ok, summary) = if broken {
            (false, "No check: the shell does not start, ENOTSUP")
        } else if job.step == "repair" {
            (false, "I changed no file.")
        } else {
            (true, "good")
        };
        let summary = summary.to_owned();
        Ok(Outcome { ok, summary })
    }
}

/// The default sheet. Each command step does nothing, and the tests
/// question always comes.
fn quiet_sheet() -> Graph {
    let mut graph = default_sheet();
    for step in graph.steps.values_mut() {
        if !step.run.is_empty() {
            step.run = vec![vec!["true".to_owned()]];
        }
        if !step.skip_if.is_empty() {
            step.skip_if = vec![vec!["false".to_owned()]];
        }
    }
    graph
}

fn folder() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let number = NEXT.fetch_add(1, Ordering::SeqCst);
    let id = process::id();
    let name = format!("codeumbrella-no-check-{id}-{number}");
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Says yes to the tests question and does the steps that follow.
fn yes(run: &mut Run, graph: &Graph, worker: &NoCheck) {
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    run.answer(true, "").unwrap();
    run.go(graph, &root().join("roles"), worker).unwrap();
}

/// Starts a run and says yes to the first tests question.
fn started(graph: &Graph, worker: &NoCheck) -> Run {
    let mut run = Run::start(folder(), "a task", graph).unwrap();
    fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(graph, &root().join("roles"), worker).unwrap();
    yes(&mut run, graph, worker);
    run
}

#[test]
fn a_no_check_result_goes_through_the_plan_back_to_the_build() {
    let graph = quiet_sheet();
    let worker = NoCheck::new(1);
    let mut run = started(&graph, &worker);
    assert_eq!(run.state.status, Status::Waiting, "{}", run.state.note);
    assert_eq!(worker.count("plan"), 2);
    assert_eq!(run.state.tries.get("plan"), Some(&2));
    assert_eq!(run.state.tries.get("repair"), Some(&1));
    // The planner reads the reports of the checks in this log line.
    let log = fs::read_to_string(run.dir.join("log.jsonl")).unwrap();
    let report = "check_tests (bad): No check:";
    assert!(log.contains(report), "{log}");

    yes(&mut run, &graph, &worker);
    assert_eq!(worker.count("build"), 2);
    assert_eq!(worker.count("check_tests"), 2);
    assert_eq!(run.state.status, Status::Waiting, "{}", run.state.note);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
}

#[test]
fn the_third_no_check_result_stops_the_run() {
    let graph = quiet_sheet();
    let worker = NoCheck::new(3);
    let mut run = started(&graph, &worker);
    yes(&mut run, &graph, &worker);
    assert_eq!(run.state.status, Status::Waiting, "{}", run.state.note);
    assert_eq!(run.state.tries.get("plan"), Some(&3));

    yes(&mut run, &graph, &worker);
    assert_eq!(run.state.status, Status::Stopped);
    assert_eq!(run.state.at.as_deref(), Some("plan"));
    let note = &run.state.note;
    assert!(note.contains("maximum of 3 tries"), "{note}");
    assert_eq!(worker.count("build"), 3);
    assert_eq!(worker.count("check_tests"), 3);
    assert_eq!(worker.count("plan"), 3);
    assert_eq!(run.state.tries.get("repair"), Some(&3));
}

// ---------------------------------------------------------------------
// A check with a computer error: the rules pages and the documents
// ---------------------------------------------------------------------

/// The pages of the five parts of the fan of `check`.
const CHECK_PAGES: [&str; 5] = [
    "roles/checker.md",
    "roles/safety.md",
    "roles/reviewer.md",
    "roles/security.md",
    "roles/user.md",
];

#[test]
fn each_page_of_the_fan_tells_the_rule_of_no_check() {
    let sentences = [
        "If a sandbox or disk error stops your commands, start your summary with `No check:` and name the error.",
        "Examples are a full disk and a shell that does not start.",
        "A sandbox that blocks only one test or one example is no such error.",
        "A package or a tool that is not on the computer is no such error.",
        "Then give `ok: false`, and do not say that the code is bad.",
        "Do not use `No check:` if you found a problem in the change.",
        "Give this `ok: false` also if no other rule of this page gives a cause for it.",
    ];
    for page in CHECK_PAGES {
        let rule = rule_with(page, "`No check:`");
        for sentence in sentences {
            assert!(rule.contains(sentence), "{page}: {sentence}");
        }
        plain_and_short(page, &rule);
    }
}

#[test]
fn the_checker_keeps_paper_test_for_one_blocked_test() {
    let new = rule_with("roles/checker.md", "`No check:`");
    let sentence = "One paper test that the sandbox blocks gets `Paper test:`, as rule 10 tells.";
    assert!(new.contains(sentence), "{new}");
    let ten = rule("roles/checker.md", 10);
    assert!(ten.contains("`Paper test:`"), "{ten}");
}

#[test]
fn the_user_checker_keeps_not_run_for_one_blocked_example() {
    let rule = rule_with("roles/user.md", "`No check:`");
    let sentence = "One example that the sandbox blocks gets `Not run:`, as rule 11 tells.";
    assert!(rule.contains(sentence), "{rule}");
}

#[test]
fn the_planner_changes_no_paper_after_reports_with_no_check() {
    let rule = rule_with("roles/planner.md", "`No check:`");
    let sentences = [
        "After a bad repair, read the last line of the step `check` in `../log.jsonl`.",
        "It has one report for each checker, marked `(good)` or `(bad)`.",
        "If each bad report starts with `No check:`, change no paper and give `ok: true`.",
        "Name the error in the first line of your summary.",
        "The run then goes to the build and the checks again, with one more try of the plan.",
        "This comes before rule 4 and before the other lines of this rule.",
        "With a bad report of a different type, correct the plan as before.",
    ];
    for sentence in sentences {
        assert!(rule.contains(sentence), "{sentence}: {rule}");
    }
    plain_and_short("planner", &rule);
}

#[test]
fn the_chat_tells_the_error_of_a_no_check_line() {
    let item = item_with(SKILL, "After each command", "No check:");
    let sentences = [
        "A line of `log.jsonl` can have `No check:`.",
        "At the next question or stop, tell the person its error.",
    ];
    for sentence in sentences {
        assert!(item.contains(sentence), "{sentence}: {item}");
    }
    plain_and_short("SKILL.md", &item);
}

#[test]
fn the_safety_rules_tell_the_rule_of_no_check_with_cause_and_date() {
    let rules = section("AGENTS.md", "Safety rules");
    let sentences = [
        "A checker that cannot run its commands starts its summary with `No check:` and names the error.",
        "It does not say that the code is bad.",
        "If each bad report of the checks starts with `No check:`, the planner changes no paper.",
        "The planner then gives `ok: true`, and the run goes to the build and the checks again.",
        "This uses a try of the plan, so an error that stays stops the run after the third try.",
        "Each such result also uses a try of `repair`.",
        "The cause: on 27 November 2026 each shell call of the checkers stopped with `ENOTSUP`.",
        "The planner found no paper to correct, and the run stopped.",
    ];
    for sentence in sentences {
        assert!(rules.contains(sentence), "{sentence}");
    }
    for line in rules.lines() {
        if line.contains("No check:") || line.contains("27 November 2026") {
            plain_and_short("AGENTS.md", line);
        }
    }
}
