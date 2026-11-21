//! Tests of the rules for plain words. They read the rules pages, the
//! default sheet, `AGENTS.md` and the README, and they start the built
//! command `init`. No worker, no network.

use codeumbrella::graph::Graph;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

const DATE: &str = "19 November 2026";

/// The four roles that write text for a person.
const WRITERS: [&str; 4] = ["planner", "writer", "learner", "builder"];

/// The words that each new rule has, in small letters.
const RULE_WORDS: [&str; 9] = [
    "20 words",
    "6 sentences",
    "active voice",
    "semicolon",
    "refused words",
    "agents.md",
    "commit messages",
    "test names",
    "code marks",
];

/// The words of the list of part C of the task.
const TASK_WORDS: [&str; 74] = [
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
    "true",
    "false",
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
    "split",
    "save",
    "guard",
    "among",
    "under",
    "separate",
    "another",
    "yourself",
    "past",
    "little",
    "exist",
    "lost",
    "approve",
    "search",
    "form",
    "whole",
    "otherwise",
    "any",
    "trace",
    "gap",
    "zero",
    "fail",
    "loses",
    "splits",
    "harmful",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let path = root().join(rel);
    let read = fs::read_to_string(&path);
    let msg = format!("cannot read {}", path.display());
    read.expect(&msg)
}

fn page(role: &str) -> String {
    text(&format!("roles/{role}.md"))
}

/// A new empty folder in the temp folder. It gives the full path.
fn folder() -> PathBuf {
    let me = std::process::id();
    let name = format!("cu-plainwords-{me}");
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("make the folder");
    fs::canonicalize(&dir).expect("full path of the folder")
}

/// The text of the part `## title` of a document, with its `###` parts.
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

/// The text of the part `### Refused words` of `AGENTS.md`, up to the
/// next title.
fn refused_part() -> String {
    let page = text("AGENTS.md");
    let mut found = Vec::new();
    let mut inside = false;
    for line in page.lines() {
        if line.starts_with('#') {
            inside = line == "### Refused words";
        } else if inside {
            found.push(line);
        }
    }
    assert!(!found.is_empty(), "AGENTS.md: no part Refused words");
    found.join("\n")
}

/// The list of `AGENTS.md`: each word in code marks on the line
/// `- Words:` of the part, in small letters.
fn refused() -> Vec<String> {
    let part = refused_part();
    let line = part.lines().find(|l| l.starts_with("- Words:"));
    let line = line.expect("no line `- Words:` in the part");
    let marks = line.split('`').skip(1).step_by(2);
    marks.map(str::to_lowercase).collect()
}

/// The lines of the part `### Refused words` other than `- Words:`.
fn notes() -> String {
    let part = refused_part();
    let kept = part.lines().filter(|l| !l.starts_with("- Words:"));
    kept.collect::<Vec<&str>>().join("\n")
}

/// A text with no fenced block and no part in code marks.
fn plain(text: &str) -> String {
    let mut kept = Vec::new();
    let mut fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        } else if !fence {
            kept.push(line);
        }
    }
    let joined = kept.join("\n");
    let parts = joined.split('`').step_by(2);
    parts.collect::<Vec<&str>>().join(" ")
}

/// The words of a text, in small letters, with `_` as a part of a word.
fn words(text: &str) -> Vec<String> {
    let low = text.to_lowercase();
    let split = low.split(|c: char| !(c.is_alphanumeric() || c == '_'));
    split.filter(|w| !w.is_empty()).map(str::to_owned).collect()
}

/// The words of the list of `AGENTS.md` that a text has.
fn listed(text: &str) -> Vec<String> {
    let list = refused();
    let all = words(text);
    all.into_iter().filter(|w| list.contains(w)).collect()
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

/// Each sentence of a text has 20 words at the maximum. The text has no
/// semicolon out of code marks.
fn short(name: &str, new: &str) {
    for line in new.lines() {
        for sentence in body(line).split(". ") {
            let count = sentence.split_whitespace().count();
            assert!(count <= 20, "{name}: {count} words: {sentence}");
        }
    }
    let semi = plain(new).contains(';');
    assert!(!semi, "{name}: a semicolon: {new}");
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

/// The one rule of a page with the words `refused words`, and its
/// number. It must be the last numbered rule of the page.
fn new_rule(who: &str) -> (usize, String) {
    let text_of = page(who);
    let mut found = Vec::new();
    for at in 1..60 {
        let one = rule_in(&text_of, at);
        if one.to_lowercase().contains("refused words") {
            found.push((at, one));
        }
    }
    assert_eq!(found.len(), 1, "{who}: {found:?}");
    let (at, one) = found.remove(0);
    let next = rule_in(&text_of, at + 1);
    // The builder page has the rule for `fix_merge` after it.
    assert!(
        who == "builder" || next.is_empty(),
        "{who}: rule {at} is not the last"
    );
    (at, one)
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

// The rules pages.

#[test]
fn each_page_that_writes_for_a_person_has_one_new_rule() {
    for who in WRITERS {
        let (_, found) = new_rule(who);
        let low = found.to_lowercase();
        for word in RULE_WORDS {
            assert!(low.contains(word), "{who}: {word}: {found}");
        }
    }
}

#[test]
fn the_reviewer_has_one_new_rule_that_gives_ok_false() {
    let (_, found) = new_rule("reviewer");
    let low = found.to_lowercase();
    for word in RULE_WORDS {
        assert!(low.contains(word), "{word}: {found}");
    }
    let more = ["ok: false", "`pr`", "`lessons`", "summary"];
    for word in more {
        assert!(found.contains(word), "{word}: {found}");
    }
    let list = "PR text checklist";
    assert!(found.contains(list), "{list}: {found}");
}

/// The sentences of a text. A sentence ends with `. `.
fn sentences(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(body).collect();
    let joined = lines.join(" ");
    let split = joined.split(". ");
    split.map(str::to_owned).collect()
}

#[test]
fn the_reviewer_gives_only_a_note_for_the_paper_lessons() {
    let (_, found) = new_rule("reviewer");
    let all = sentences(&found);
    let note = all.iter().any(|s| {
        let low = s.to_lowercase();
        low.contains("`lessons`") && words(&low).contains(&"note".into())
    });
    assert!(note, "no note for `lessons`: {found}");
    for s in &all {
        let both = s.contains("`lessons`") && s.contains("ok: false");
        let not = words(s).contains(&"not".into());
        assert!(!both || not, "`lessons` gives ok: false: {s}");
    }
    let low = found.to_lowercase();
    assert!(low.contains("approval"), "no approval: {found}");
}

#[test]
fn rule_5_of_the_reviewer_names_the_new_rule() {
    let (at, _) = new_rule("reviewer");
    let five = rule_in(&page("reviewer"), 5);
    let all = words(&five);
    for want in ["1".to_owned(), "11".to_owned(), at.to_string()] {
        assert!(all.contains(&want), "{want}: {five}");
    }
}

#[test]
fn the_new_rules_name_no_word_of_the_list_and_are_short() {
    let mut list = WRITERS.to_vec();
    list.push("reviewer");
    for who in list {
        let (_, found) = new_rule(who);
        let found = found.replace("ok: false", "");
        let found = found.replace("ok: true", "");
        let bad = listed(&found);
        assert!(bad.is_empty(), "{who}: {bad:?}: {found}");
        short(who, &found);
    }
    no_page_has_a_list();
}

/// No rules page in `roles` has a list of refused words.
fn no_page_has_a_list() {
    let entries = fs::read_dir(root().join("roles"));
    let entries = entries.expect("read the folder roles");
    let mut seen = 0;
    for entry in entries {
        let path = entry.expect("an entry of roles").path();
        if path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        seen += 1;
        let loaded = fs::read_to_string(&path);
        let found = loaded.expect("read a rules page");
        let has = |l: &str| l.starts_with("- Words:");
        assert!(!found.lines().any(has), "{}", path.display());
        let head = found.contains("### Refused words");
        assert!(!head, "{}", path.display());
    }
    assert!(seen >= 5, "{seen} rules pages");
}

#[test]
fn init_writes_the_pages_with_the_new_rules() {
    let dir = folder();
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    let out = command.arg("init").current_dir(&dir).output();
    let out = out.expect("start codeumbrella");
    assert!(out.status.success(), "{out:?}");
    let mut list = WRITERS.to_vec();
    list.push("reviewer");
    for who in list {
        let name = format!("roles/{who}.md");
        let written = dir.join(".codeumbrella").join(&name);
        let msg = format!("cannot read {}", written.display());
        let copy = fs::read_to_string(&written).expect(&msg);
        assert_eq!(copy, page(who), "{name}");
        let low = copy.to_lowercase();
        assert!(low.contains("refused words"), "{name}");
    }
    let _ = fs::remove_dir_all(&dir);
}

// The default sheet.

#[test]
fn check_review_reads_spec_pr_and_lessons() {
    let graph = default_sheet();
    let review = graph.step("check_review").unwrap();
    assert_eq!(review.role.as_deref(), Some("reviewer"));
    assert_eq!(review.reads, ["spec", "pr", "lessons"]);
    let safety = graph.step("check_safety").unwrap();
    assert_eq!(safety.reads, ["spec", "lessons"]);
}

#[test]
fn the_comment_of_learn_names_the_reads_of_check_review() {
    let comment = comment_above("learn");
    let want = ["check_review", "check_safety", "lessons", "faster run"];
    for word in want {
        assert!(comment.contains(word), "{word}: {comment}");
    }
}

/// A text as one line: no `#`, and one space between two words.
fn flat(text: &str) -> String {
    let low = text.to_lowercase().replace('#', " ");
    let all: Vec<&str> = low.split_whitespace().collect();
    all.join(" ")
}

#[test]
fn the_comment_of_learn_counts_four_parts() {
    let comment = flat(&comment_above("learn"));
    let four = comment.contains("four parts");
    assert!(four, "no four parts: {comment}");
    let three = comment.contains("three parts");
    assert!(!three, "three parts: {comment}");
}

#[test]
fn architecture_md_counts_four_parts_for_a_faster_run() {
    let all = items(&text("ARCHITECTURE.md"));
    let has = |i: &&String| {
        let low = i.to_lowercase();
        low.contains("faster run") && low.contains("together")
    };
    let found: Vec<&String> = all.iter().filter(has).collect();
    let msg = "ARCHITECTURE.md: no line for a faster run";
    assert!(!found.is_empty(), "{msg}");
    for item in found {
        let line = flat(item);
        assert!(line.contains("four parts"), "{line}");
        assert!(!line.contains("three parts"), "{line}");
        assert!(line.contains("check_review"), "{line}");
    }
}

// AGENTS.md.

#[test]
fn agents_md_has_the_part_refused_words_in_how_to_write() {
    let part = section("AGENTS.md", "How to write for the person");
    let has = part.lines().any(|l| l == "### Refused words");
    assert!(has, "{part}");
}

#[test]
fn the_list_of_agents_md_has_the_words_of_the_task() {
    let mut found = refused();
    found.sort();
    let mut want: Vec<String> = TASK_WORDS.iter().map(|w| String::from(*w)).collect();
    want.sort();
    assert_eq!(found, want);
}

#[test]
fn the_part_has_the_notes_of_code_marks() {
    let found = notes();
    let marks = ["`security`", "`process`", "`trace`", "`program`"];
    for word in marks {
        assert!(found.contains(word), "{word}: {found}");
    }
    for word in ["`command`", "code marks"] {
        assert!(found.contains(word), "{word}: {found}");
    }
    short("AGENTS.md", &found);
}

#[test]
fn the_part_says_that_the_workers_obey_the_list() {
    let found = notes();
    let obey = items(&found).into_iter().any(|i| {
        let low = i.to_lowercase();
        let all = ["worker", "obey", "person"];
        all.iter().all(|w| low.contains(w))
    });
    assert!(obey, "no sentence: the workers obey: {found}");
    let low = found.to_lowercase();
    assert!(low.contains("other projects"), "{found}");
}

#[test]
fn agents_md_has_no_word_of_its_list_out_of_code_marks() {
    let found = listed(&plain(&text("AGENTS.md")));
    assert!(found.is_empty(), "AGENTS.md: {found:?}");
}

#[test]
fn the_safety_rules_tell_the_limits_and_the_check_with_the_date() {
    safety_line(&["20 words", DATE]);
    safety_line(&["reviewer", "refused words", DATE]);
    safety_line(&["lessons", "note", "approval"]);
}

#[test]
fn the_sheet_part_of_agents_md_tells_the_reads_of_check_review() {
    let part = section("AGENTS.md", "The sheet");
    let has = |l: &str| l.contains("check_review") && l.contains("lessons");
    assert!(part.lines().any(has), "{part}");
}

#[test]
fn the_structure_of_agents_md_names_the_new_test_file() {
    let part = section("AGENTS.md", "Structure");
    let has = |l: &&str| l.contains("`tests/plain_words.rs`");
    let line = part.lines().find(has);
    let line = line.expect("no line for tests/plain_words.rs");
    assert!(line.starts_with('|'), "{line}");
}

// The README.

#[test]
fn the_readme_has_a_short_paragraph_about_refused_words() {
    let all = items(&text("README.md"));
    let found = all.iter().find(|i| i.contains("refused words"));
    let found = found.expect("README.md: no paragraph with refused words");
    assert!(found.contains("AGENTS.md"), "{found}");
    short("README.md", found);
    let bad = listed(&plain(found));
    assert!(bad.is_empty(), "README.md: {bad:?}: {found}");
}
