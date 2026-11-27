//! Tests of the seven standards pages, `init`, the three rules
//! pages and the documents. No worker is started.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs, process};

const PAGES: [(&str, &[&str]); 7] = [
    ("python", &["PEP 8", "PEP 20", "PEP 257"]),
    ("go", &["Effective Go", "Code Review Comments"]),
    ("javascript", &["MDN", "Google JavaScript Style Guide"]),
    (
        "typescript",
        &["TypeScript Handbook", "Google TypeScript Style Guide"],
    ),
    ("java", &["Google Java Style Guide", "Effective Java"]),
    ("rust", &["Rust API Guidelines", "Rust Style Guide"]),
    ("general", &["KISS"]),
];

const FOLDER: &str = "../../../roles/languages/";
const WRITE: &str = "## Write the code like this";
const BREAK: &str = "## A break";

/// The six lines of rule 4 of the simplifier.
const SIX: [&str; 6] = [
    "   - KISS: write the simplest code that works.",
    "   - DRY: put the same logic in one position only.",
    "   - YAGNI: remove code that the task does not use.",
    "   - One source of truth: give one value one home.",
    "   - Plain direct code: use no clever trick.",
    "   - No boilerplate: remove code that adds nothing.",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn folder() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = env::temp_dir().join(format!("codeumbrella-lang-{}-{n}", process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
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

fn has_all(found: &str, words: &[&str]) {
    for word in words {
        assert!(found.contains(word), "{word}: {found}");
    }
}

/// Each sentence of a line has 20 words at the maximum.
fn short(line: &str) {
    let body = line.trim_start().trim_start_matches("- ");
    for sentence in body.split(". ") {
        let words = sentence.split_whitespace().count();
        assert!(words <= 20, "{words} words: {sentence}");
    }
}

/// The text of a numbered rule of a rules page, with its sub-points.
fn rule_block(role: &str, number: usize) -> String {
    let page = text(&format!("roles/{role}.md"));
    let head = format!("\n{number}. ");
    let next = format!("\n{}. ", number + 1);
    let after = page.split(&head).nth(1).expect("the rule");
    let block = after.split(&next).next().expect("a block");
    block.to_owned()
}

/// The one line of a rules page that names the folder of the pages.
fn path_line(role: &str) -> String {
    let page = text(&format!("roles/{role}.md"));
    let found: Vec<&str> = page.lines().filter(|l| l.contains(FOLDER)).collect();
    assert_eq!(found.len(), 1, "{role}: one line names the folder");
    found[0].to_owned()
}

#[test]
fn each_page_is_there_with_its_sources_at_the_top_and_300_lines_at_the_maximum() {
    for (name, guides) in PAGES {
        let rel = format!("roles/languages/{name}.md");
        let page = text(&rel);
        assert!(page.lines().count() <= 300, "{rel} has too many lines");
        assert!(page.lines().count() >= 8, "{rel} is too short");
        let top: Vec<&str> = page.lines().take(6).collect();
        let top = top.join("\n");
        assert!(
            guides.iter().any(|g| top.contains(g)),
            "{rel} names no guide at the top"
        );
    }
}

#[test]
fn the_folder_of_the_pages_has_the_seven_files_and_no_other() {
    let dir = root().join("roles/languages");
    let entries = fs::read_dir(dir).unwrap();
    let mut found: Vec<String> = entries
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n != ".DS_Store")
        .collect();
    found.sort();
    let mut seven: Vec<String> = PAGES.iter().map(|(n, _)| format!("{n}.md")).collect();
    seven.sort();
    assert_eq!(found, seven);
}

#[test]
fn the_general_page_names_its_six_principles_at_the_top() {
    let page = text("roles/languages/general.md");
    let top: Vec<&str> = page.lines().take(6).collect();
    let top = top.join("\n").to_lowercase();
    let six = [
        "kiss",
        "dry",
        "yagni",
        "one source of truth",
        "plain direct code",
        "no boilerplate",
    ];
    has_all(&top, &six);
}

#[test]
fn each_page_has_the_part_to_write_and_after_it_the_part_a_break() {
    for (name, _) in PAGES {
        let page = text(&format!("roles/languages/{name}.md"));
        let lines: Vec<&str> = page.lines().collect();
        let first = lines.iter().position(|l| *l == WRITE);
        let second = lines.iter().position(|l| *l == BREAK);
        let first = first.unwrap_or_else(|| panic!("{name}: {WRITE}"));
        let second = second.unwrap_or_else(|| panic!("{name}: {BREAK}"));
        assert!(first < second, "{name}: the parts are in this sequence");
    }
}

#[test]
fn the_sentences_of_a_page_have_20_words_at_the_maximum() {
    for (name, _) in PAGES {
        let page = text(&format!("roles/languages/{name}.md"));
        for line in page.lines().filter(|l| !l.trim_start().starts_with('#')) {
            for sentence in line.split(". ") {
                let words = sentence.split_whitespace().count();
                assert!(words <= 20, "{name}: {words} words in: {sentence}");
            }
        }
    }
}

#[test]
fn init_puts_the_seven_pages_into_a_new_project() {
    let dir = folder();
    assert!(cu(&dir, &["init"]));
    for (name, _) in PAGES {
        let rel = format!("roles/languages/{name}.md");
        let written = dir.join(".codeumbrella").join(&rel);
        let copy = fs::read_to_string(&written);
        let copy = copy.unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(copy, text(&rel), "{rel}");
    }
}

#[test]
fn init_update_replaces_a_changed_page_and_keeps_a_before_copy() {
    let dir = folder();
    assert!(cu(&dir, &["init"]));
    let page = dir.join(".codeumbrella/roles/languages/go.md");
    fs::write(&page, "changed by a person\n").unwrap();
    assert!(cu(&dir, &["init", "--update"]));
    assert_eq!(
        fs::read_to_string(&page).unwrap(),
        text("roles/languages/go.md")
    );
    let before = dir.join(".codeumbrella/roles/languages/go.md.before");
    let kept = fs::read_to_string(before).unwrap();
    assert_eq!(kept, "changed by a person\n");
}

#[test]
fn three_rules_pages_name_the_folder_and_others_do_not() {
    for role in ["reviewer", "simplifier", "builder"] {
        let page = text(&format!("roles/{role}.md"));
        assert!(page.contains(FOLDER), "{role}");
    }
    for role in [
        "planner", "attacker", "checker", "safety", "security", "user",
    ] {
        let page = text(&format!("roles/{role}.md"));
        assert!(!page.contains("roles/languages"), "{role}");
    }
}

#[test]
fn the_reviewer_does_not_run_the_code_and_leaves_weak_paper_tests_to_the_summary() {
    let page = text("roles/reviewer.md");
    let no_run = page.contains("does not run");
    assert!(no_run, "no rule: do not run the code");
    let no_build = page.contains("build");
    assert!(no_build, "no rule: do not build");
    let paper = page.contains("`tests`");
    assert!(paper, "no rule about the paper tests");
}

#[test]
fn rule_5_of_the_reviewer_has_six_items_and_one_is_a_break_of_a_page() {
    let block = rule_block("reviewer", 5);
    let items = block.matches("\n   - ").count();
    assert_eq!(items, 6, "{block}");
    let new = "breaks a point of a standards page";
    let found = block
        .lines()
        .any(|l| l.starts_with("   - ") && l.contains(new));
    assert!(found, "{block}");
    let kept = "Rules 1, 11 and 13 can give `ok: false`.";
    assert!(block.contains(kept), "{block}");
}

fn rule_six(page: &str) -> &str {
    let found = page.lines().find(|l| l.starts_with("6. "));
    found.expect("rule 6")
}

#[test]
fn the_simplifier_puts_its_changes_back_after_a_bad_test_and_gives_ok_true() {
    let page = text("roles/simplifier.md");
    let rule = rule_six(&page);
    assert!(rule.contains("ok: true"), "{rule}");
    assert!(!rule.contains("ok: false"), "{rule}");
    assert!(rule.contains("test"), "{rule}");
    let back = rule.contains("back") || rule.contains("restore") || rule.contains("reset");
    assert!(back, "rule 6 does not put the changes back: {rule}");
    assert!(rule.contains("changed nothing"), "{rule}");
}

#[test]
fn both_rules_pages_keep_the_words_of_the_language_rule() {
    for role in ["reviewer", "simplifier"] {
        let line = path_line(role);
        has_all(&line, &["AGENTS.md", "CLAUDE.md", "no problem"]);
    }
}

#[test]
fn the_language_rule_of_the_reviewer_gives_ok_false_for_a_break() {
    let line = path_line("reviewer");
    assert!(line.starts_with("8. "), "{line}");
    let words = ["file", "line", "summary", "`ok: false`", BREAK];
    has_all(&line, &words);
}

#[test]
fn rule_8_of_the_reviewer_tells_how_to_check_each_point() {
    let block = rule_block("reviewer", 8).to_lowercase();
    let words = [
        "general.md",
        "does the change follow this point, yes or no",
        "the run changed",
        "not a break",
        "note",
        "no page",
    ];
    has_all(&block, &words);
    let page = text("roles/reviewer.md");
    let gone = "Put a point from a page into the summary";
    assert!(!page.contains(gone), "{gone}");
}

#[test]
fn rule_8_of_the_reviewer_has_the_three_sentences_of_the_result() {
    let block = rule_block("reviewer", 8);
    let changed = "A line that the run changed and that breaks a point gives `ok: false`.";
    let before = "A break in code from before the run is a note.";
    let write = "A point of the part `## Write the code like this` does not give `ok: false`.";
    has_all(&block, &[changed, before, write]);
}

#[test]
fn a_break_in_a_test_that_the_builder_cannot_change_is_a_note() {
    let block = rule_block("reviewer", 8);
    let note = "A break in a test file of the paper `tests` or in a lesson test is a note.";
    has_all(&block, &[note, "`tests.txt`"]);
}

#[test]
fn rule_4_of_the_simplifier_names_the_general_page_and_keeps_six_lines() {
    let block = rule_block("simplifier", 4);
    assert!(block.contains("general.md"), "{block}");
    let lines = block.lines().filter(|l| l.starts_with("   - "));
    assert_eq!(lines.collect::<Vec<_>>(), SIX);
}

#[test]
fn rule_9_of_the_simplifier_names_the_general_page_and_the_part() {
    let line = path_line("simplifier");
    assert!(line.starts_with("9. "), "{line}");
    has_all(&line, &["general.md", WRITE]);
}

#[test]
fn rule_3_of_the_builder_has_one_line_that_names_the_pages() {
    let line = path_line("builder");
    let block = rule_block("builder", 3);
    assert!(block.contains(&line), "{block}");
    let words = ["general.md", "language", WRITE];
    has_all(&line, &words);
    short(&line);
}

#[test]
fn the_builder_page_gets_no_new_numbered_rule() {
    let page = text("roles/builder.md");
    let numbered = |l: &&str| {
        let head = l.split_once(". ").map(|(n, _)| n);
        head.is_some_and(|n| n.parse::<u32>().is_ok())
    };
    assert_eq!(page.lines().filter(numbered).count(), 15);
}

#[test]
fn agents_md_has_the_new_language_rule_and_not_the_old_sentence() {
    let agents = text("AGENTS.md");
    let old = "Put no knowledge of one language into the supervisor, the default sheet or the default rules pages.";
    assert!(!agents.contains(old));
    assert!(agents.contains("roles/languages"));
    assert!(agents.contains("only in the language pages"));
}

/// The text of the part `## title` of `AGENTS.md`.
fn section(title: &str) -> String {
    let page = text("AGENTS.md");
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
    found.join("\n")
}

/// The first row of the table of `Structure` that starts with `start`.
fn row(start: &str) -> String {
    let part = section("Structure");
    let found = part.lines().find(|l| l.starts_with(start));
    let found = found.unwrap_or_else(|| panic!("no row {start}"));
    found.to_owned()
}

#[test]
fn agents_md_names_seven_page_file_names() {
    let part = section("What this project is");
    assert!(part.contains("seven page file names"), "{part}");
    assert!(!part.contains("six page file names"), "{part}");
}

#[test]
fn the_table_row_of_the_pages_tells_the_new_facts() {
    let line = row("| `roles/languages/");
    let words = [
        "general.md",
        "300 lines",
        "## Write the code like this",
        "## A break",
        "builder",
    ];
    has_all(&line, &words);
    let gone = "Only the reviewer and the simplifier read them";
    assert!(!line.contains(gone), "{line}");
}

#[test]
fn the_table_row_of_this_test_file_names_seven_pages() {
    let line = row("| `tests/language_pages.rs`");
    has_all(&line, &["seven", "three rules pages"]);
}

/// A line of `Safety rules` with each of `words`, in small letters.
fn safety_line(words: &[&str]) {
    let rules = section("Safety rules").to_lowercase();
    let has = |l: &&str| words.iter().all(|w| l.contains(w));
    let line = rules.lines().find(has);
    let line = line.unwrap_or_else(|| panic!("no line with {words:?}"));
    short(line);
}

#[test]
fn the_safety_rules_tell_the_new_cause_of_a_bad_result() {
    safety_line(&["standards page", "bad result", "25 november 2026"]);
}

#[test]
fn the_safety_rules_tell_the_question_of_the_check() {
    safety_line(&["does the change follow this point, yes or no"]);
}

#[test]
fn the_safety_rules_say_that_no_worker_changes_a_point() {
    safety_line(&["no worker writes or changes a point of a page"]);
}

#[test]
fn agents_md_names_the_simplifier_in_the_three_safety_sentences() {
    let agents = text("AGENTS.md");
    let lines: Vec<&str> = agents
        .lines()
        .filter(|l| {
            l.contains("can run all commands")
                || l.contains("can connect to a server on the same computer")
                || l.contains("run in the Claude sandbox")
        })
        .collect();
    assert_eq!(lines.len(), 3, "expected 3 safety lines: {lines:?}");
    for line in lines {
        assert!(line.contains("simplifier"), "{line}");
        // The learner runs commands in the sandbox too.
        assert!(line.contains("learner"), "{line}");
    }
}

#[test]
fn the_sheet_and_the_documents_name_the_pages() {
    let sheet = text("graph.example.yaml");
    assert!(sheet.contains("languages"));
    for doc in ["README.md", "ARCHITECTURE.md"] {
        assert!(text(doc).contains("language pages"), "{doc}");
    }
}

#[test]
fn the_documents_name_the_builder_and_the_general_page() {
    for doc in ["README.md", "ARCHITECTURE.md"] {
        let page = text(doc);
        let found = page.lines().find(|l| l.contains("language pages"));
        let line = found.expect("a line with language pages");
        has_all(line, &["builder", "general.md"]);
    }
}
