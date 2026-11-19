//! Tests of the six language pages, `init`, the two rules
//! pages and `AGENTS.md`. No worker is started.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs, process};

const PAGES: [(&str, &[&str]); 6] = [
    ("python", &["PEP 8", "PEP 20", "PEP 257"]),
    ("go", &["Effective Go", "Code Review Comments"]),
    ("javascript", &["MDN", "Google JavaScript Style Guide"]),
    (
        "typescript",
        &["TypeScript Handbook", "Google TypeScript Style Guide"],
    ),
    ("java", &["Google Java Style Guide", "Effective Java"]),
    ("rust", &["Rust API Guidelines", "Rust Style Guide"]),
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

#[test]
fn each_page_is_there_with_guide_names_at_the_top_and_40_lines_at_the_maximum() {
    for (name, guides) in PAGES {
        let rel = format!("roles/languages/{name}.md");
        let page = text(&rel);
        assert!(page.lines().count() <= 40, "{rel} has too many lines");
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
fn init_puts_the_six_pages_into_a_new_project() {
    let dir = folder();
    assert!(cu(&dir, &["init"]));
    for (name, _) in PAGES {
        let rel = format!("roles/languages/{name}.md");
        let written = dir.join(".codeumbrella").join(&rel);
        assert_eq!(fs::read_to_string(&written).unwrap(), text(&rel), "{rel}");
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
fn the_reviewer_and_the_simplifier_name_the_pages_and_others_do_not() {
    for role in ["reviewer", "simplifier"] {
        let page = text(&format!("roles/{role}.md"));
        assert!(page.contains("roles/languages"), "{role}");
        assert!(page.contains("../../../roles/languages/"), "{role}");
    }
    for role in [
        "planner", "attacker", "builder", "checker", "safety", "security", "user",
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
    // The list of problems for `ok: false` keeps its five items.
    assert_eq!(page.matches("\n   - ").count(), 5);
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
fn both_rules_pages_have_the_language_rule_with_its_three_points() {
    for role in ["reviewer", "simplifier"] {
        let page = text(&format!("roles/{role}.md"));
        let rules: Vec<&str> = page
            .lines()
            .filter(|l| l.contains("../../../roles/languages/"))
            .collect();
        assert_eq!(rules.len(), 1, "{role}: one rule names the folder");
        let rule = rules[0];
        assert!(rule.contains("AGENTS.md"), "{role}: {rule}");
        assert!(rule.contains("CLAUDE.md"), "{role}: {rule}");
        assert!(rule.contains("no problem"), "{role}: {rule}");
    }
    let reviewer = text("roles/reviewer.md");
    let rule = reviewer
        .lines()
        .find(|l| l.contains("../../../roles/languages/"))
        .unwrap();
    assert!(rule.contains("summary"), "{rule}");
    assert!(rule.contains("ok: false"), "{rule}");
}

#[test]
fn agents_md_has_the_new_language_rule_and_not_the_old_sentence() {
    let agents = text("AGENTS.md");
    let old = "Put no knowledge of one language into the supervisor, the default sheet or the default rules pages.";
    assert!(!agents.contains(old));
    assert!(agents.contains("roles/languages"));
    assert!(agents.contains("only in the language pages"));
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
