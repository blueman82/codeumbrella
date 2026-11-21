//! Tests: the step `repair` of the default sheet goes to the plan after a
//! bad result. Also the new rules of the checker, the builder, the planner
//! and the attacker, the chat page and the documents. No Claude, no Codex.

use codeumbrella::graph::Graph;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
    ("fix_merge", "plan"),
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
