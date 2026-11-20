//! Tests of a full check that cannot run in the sandbox of a worker.
//! They read the chat page, the rules pages and the documents, and
//! they start the built command `init`. No worker, no network.

use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

const SKILL: &str = "skills/codeumbrella/SKILL.md";

const DATE: &str = "19 November 2026";

const FULL: &str = "Full check:";

const NOT_RUN: &str = "Not run: full check";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let path = root().join(rel);
    let read = fs::read_to_string(&path);
    let msg = format!("cannot read {}", path.display());
    read.expect(&msg)
}

/// A new empty folder in the temp folder. It gives the full path.
fn folder() -> PathBuf {
    let me = std::process::id();
    let name = format!("cu-fullcheck-{me}");
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("make the folder");
    fs::canonicalize(&dir).expect("full path of the folder")
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

/// The part of a document, from one `## ` title to the next, with
/// the word `word`.
fn part_with(rel: &str, word: &str) -> String {
    let page = text(rel);
    let mut parts = page.split("\n## ");
    let found = parts.find(|p| p.contains(word));
    let found = found.unwrap_or_else(|| panic!("{rel}: no part with {word}"));
    found.to_owned()
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

/// The items of the part `title` of the chat page with the word `word`,
/// with no difference between a capital and a small letter. The part
/// must have one.
fn named_items(title: &str, word: &str) -> String {
    let part = section(SKILL, title);
    let mut found = Vec::new();
    for item in items(&part) {
        if item.to_lowercase().contains(word) {
            found.push(item);
        }
    }
    assert!(!found.is_empty(), "{title}: no item with {word}");
    found.join("\n")
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

/// Each sentence of a new text has 20 words at the maximum.
fn short(name: &str, new: &str) {
    for line in new.lines() {
        for sentence in body(line).split(". ") {
            let count = sentence.split_whitespace().count();
            assert!(count <= 20, "{name}: {count} words: {sentence}");
        }
    }
}

/// The first line of the safety rules of `AGENTS.md` with each word of
/// `words`, with no difference between a capital and a small letter.
/// Each sentence of that line has 20 words at the maximum.
fn safety_line(words: &[&str]) {
    let rules = section("AGENTS.md", "Safety rules");
    let has_all = |l: &&str| {
        let low = l.to_lowercase();
        words.iter().all(|w| low.contains(&w.to_lowercase()))
    };
    let line = rules.lines().find(has_all);
    let line = line.unwrap_or_else(|| panic!("no line with {words:?}"));
    short("AGENTS.md", line);
}

// The chat page.

#[test]
fn the_packages_part_tells_the_sandbox_and_the_two_lines() {
    let part = section(SKILL, "Packages");
    let words = [FULL, "Test command:", "sandbox"];
    for word in words {
        assert!(part.contains(word), "{word}: {part}");
    }
    for word in ["clone", "git hook"] {
        assert!(part.contains(word), "{word}: {part}");
    }
    short("chat page", &named_items("Packages", "full check"));
    short("chat page", &named_items("Packages", "sandbox"));
}

#[test]
fn the_chat_offers_the_full_check_at_the_approval() {
    let found = named_items("After each command", "full check");
    let words = [
        FULL,
        "approval",
        "state.json",
        "no sandbox",
        "person says yes",
        "temp folder",
        "clone",
    ];
    for word in words {
        assert!(found.contains(word), "{word}: {found}");
    }
    short("chat page", &found);
}

#[test]
fn a_bad_full_check_is_told_and_is_not_an_answer() {
    let found = named_items("After each command", "full check");
    let words = [
        "before the person answers",
        "not an answer",
        "planner",
        "result",
    ];
    for word in words {
        assert!(found.contains(word), "{word}: {found}");
    }
}

// The rules pages.

#[test]
fn the_builder_and_the_checker_do_not_run_the_full_check() {
    for rel in ["roles/builder.md", "roles/checker.md"] {
        let found = rule_with(rel, FULL);
        let words = [NOT_RUN, "test command", "not a bad result"];
        for word in words {
            assert!(found.contains(word), "{rel}: {word}: {found}");
        }
        short(rel, &found);
    }
}

#[test]
fn rule_8_of_the_builder_gives_a_not_run_line_for_a_check() {
    let eight = rule("roles/builder.md", 8);
    for word in ["change no code", "Not run:", "sandbox", "cause"] {
        assert!(eight.contains(word), "{word}: {eight}");
    }
    let tells = "does not tell the person";
    assert!(eight.contains(tells), "{tells}: {eight}");
    let old = "what the person must do";
    assert!(!eight.contains(old), "{old}: {eight}");
    short("roles/builder.md", &eight);
}

#[test]
fn the_attacker_names_a_test_that_reads_the_open_changes() {
    let rel = "roles/attacker.md";
    let found = rule_with(rel, "open changes");
    for word in ["open changes", "git status", "git diff", "merge"] {
        assert!(found.contains(word), "{word}: {found}");
    }
    short(rel, &found);
}

#[test]
fn init_writes_the_new_rules_pages() {
    let dir = folder();
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    let out = command.arg("init").current_dir(&dir).output();
    let out = out.expect("start codeumbrella");
    assert!(out.status.success(), "{out:?}");
    for role in ["builder", "checker", "attacker"] {
        let rel = format!("roles/{role}.md");
        let written = dir.join(".codeumbrella").join(&rel);
        let msg = format!("cannot read {}", written.display());
        let page = fs::read_to_string(&written).expect(&msg);
        assert_eq!(page, text(&rel), "{rel}");
    }
    let builder = dir.join(".codeumbrella/roles/builder.md");
    let msg = format!("cannot read {}", builder.display());
    let page = fs::read_to_string(&builder).expect(&msg);
    assert!(page.contains(NOT_RUN), "{page}");
    let _ = fs::remove_dir_all(&dir);
}

// The documents.

#[test]
fn agents_md_tells_the_new_rules_with_their_date() {
    safety_line(&[FULL, "Test command:", DATE]);
    safety_line(&[NOT_RUN, "test command", DATE]);
    safety_line(&["git status", "merge", DATE]);
    let chat = ["full check", "no sandbox", "person says yes", DATE];
    safety_line(&chat);
}

#[test]
fn the_readme_tells_the_two_lines_of_the_task() {
    let part = part_with("README.md", FULL);
    for word in ["Full check: <command>", "sandbox"] {
        assert!(part.contains(word), "{word}: {part}");
    }
    for line in part.lines() {
        if line.to_lowercase().contains("full check") {
            short("README.md", line);
        }
    }
}

#[test]
fn the_structure_of_agents_md_names_the_new_test_file() {
    let part = section("AGENTS.md", "Structure");
    let has = |l: &&str| l.contains("`tests/full_check.rs`");
    let line = part.lines().find(has);
    let line = line.expect("no line for tests/full_check.rs");
    assert!(line.starts_with('|'), "{line}");
}
