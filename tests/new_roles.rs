//! Tests: the roles `reviewer`, `security` and `simplifier`, their steps in the
//! default sheet, and their rules pages. No Claude, no Codex, no network.

use codeumbrella::graph::{Graph, Kind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs, process};

const ROLES: [&str; 3] = ["reviewer", "security", "simplifier"];
const EXTENSIONS: [&str; 3] = [".md", ".claude.json", ".codex.txt"];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn graph() -> Graph {
    Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap()
}

fn folder() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    env::temp_dir().join(format!("codeumbrella-newroles-{}-{n}", process::id()))
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
fn the_nine_role_files_exist_and_init_writes_them() {
    let dir = folder();
    fs::create_dir_all(&dir).unwrap();
    assert!(cu(&dir, &["init"]));
    for role in ROLES {
        for ext in EXTENSIONS {
            let rel = format!("roles/{role}{ext}");
            assert!(!text(&rel).trim().is_empty(), "{rel} is empty");
            let written = fs::read_to_string(dir.join(".codeumbrella").join(&rel));
            assert_eq!(written.unwrap(), text(&rel), "{rel}");
        }
    }
    // `init --update` puts a missing file back.
    let gone = dir.join(".codeumbrella/roles/simplifier.codex.txt");
    fs::remove_file(&gone).unwrap();
    assert!(cu(&dir, &["init", "--update"]));
    assert_eq!(
        fs::read_to_string(&gone).unwrap(),
        text("roles/simplifier.codex.txt")
    );
}

#[test]
fn the_settings_of_the_new_roles_are_the_same_text_as_the_settings_of_their_model_role() {
    for (role, model) in [
        ("reviewer", "safety"),
        ("security", "safety"),
        ("simplifier", "builder"),
    ] {
        for ext in [".claude.json", ".codex.txt"] {
            let new = text(&format!("roles/{role}{ext}"));
            assert_eq!(new, text(&format!("roles/{model}{ext}")), "{role}{ext}");
        }
    }
}

#[test]
fn the_default_sheet_has_the_step_simplify_between_build_and_check() {
    let graph = graph();
    let step = graph.step("simplify").unwrap();
    assert_eq!(step.kind, Kind::Worker);
    assert_eq!(step.role.as_deref(), Some("simplifier"));
    assert_eq!(step.reads, ["spec"]);
    assert_eq!(step.writes, [] as [String; 0]);
    assert_eq!(step.next.as_deref(), Some("check"));
    assert_eq!(
        graph.step("build").unwrap().next.as_deref(),
        Some("simplify")
    );
    // Only `build` goes to `simplify`. The step `repair` goes to `check`.
    for (name, step) in &graph.steps {
        let to_simplify = [&step.next, &step.on_bad].contains(&&Some("simplify".to_owned()));
        assert_eq!(to_simplify, name == "build", "{name}");
    }
    assert_eq!(graph.step("repair").unwrap().next.as_deref(), Some("check"));
}

#[test]
fn the_check_is_a_fan_of_four_parts() {
    let graph = graph();
    let check = graph.step("check").unwrap();
    assert_eq!(check.kind, Kind::Fan);
    let parts = [
        "check_tests",
        "check_safety",
        "check_review",
        "check_security",
    ];
    assert_eq!(check.fan, parts);
    assert_eq!(check.on_bad.as_deref(), Some("repair"));
    assert_eq!(check.next.as_deref(), Some("no_locked_files"));
    let role = |name| graph.step(name).unwrap().role.clone().unwrap();
    assert_eq!(role("check_tests"), "checker");
    assert_eq!(role("check_safety"), "safety");
    assert_eq!(role("check_review"), "reviewer");
    assert_eq!(role("check_security"), "security");
    for name in ["check_review", "check_security"] {
        assert_eq!(graph.step(name).unwrap().reads, ["spec"], "{name}");
        assert!(graph.step(name).unwrap().writes.is_empty(), "{name}");
    }
}

/// The comment lines right above a step, in small letters.
fn comment_above(sheet: &str, step: &str) -> String {
    let lines: Vec<&str> = sheet.lines().collect();
    let head = format!("  {step}:");
    let at = lines.iter().position(|l| *l == head).expect(step);
    let above = lines[..at].iter().rev();
    let block: Vec<&str> = above
        .take_while(|l| l.trim_start().starts_with('#'))
        .copied()
        .collect();
    block.join("\n").to_lowercase()
}

#[test]
fn each_new_step_has_a_comment_that_tells_how_to_remove_it() {
    let sheet = text("graph.example.yaml");
    for step in ["simplify", "check_review", "check_security"] {
        let comment = comment_above(&sheet, step);
        assert!(comment.len() > 20, "{step} has no comment");
        assert!(comment.contains("remove"), "{step}: {comment}");
    }
}

/// The numbers at the start of the rules of a page.
fn numbers(page: &str) -> Vec<usize> {
    let lines = page.lines();
    lines
        .filter_map(|l| l.split_once(". ")?.0.parse().ok())
        .collect()
}

#[test]
fn each_new_page_has_numbered_rules_and_the_last_two_paragraphs() {
    for role in ROLES {
        let page = text(&format!("roles/{role}.md"));
        assert!(page.starts_with("# "), "{role}");
        let found = numbers(&page);
        assert!(found.len() >= 3, "{role}");
        assert_eq!(found, (1..=found.len()).collect::<Vec<_>>(), "{role}");
        let low = page.to_lowercase();
        for word in ["agents.md", "claude.md"] {
            assert!(low.contains(word), "{role} {word}");
        }
        let paragraphs: Vec<&str> = page.trim_end().split("\n\n").collect();
        let last = paragraphs[paragraphs.len() - 1];
        let ok = paragraphs[paragraphs.len() - 2];
        let summary = "Your summary must have 100 words or fewer. Give only the result and what the next step must know.";
        assert_eq!(last, summary, "{role}");
        assert!(
            ok.contains("ok: true") && ok.contains("ok: false"),
            "{role}"
        );
    }
}

#[test]
fn the_sentences_of_the_new_pages_have_20_words_or_fewer() {
    for role in ROLES {
        let page = text(&format!("roles/{role}.md")).replace('`', "");
        for line in page.lines().filter(|l| !l.starts_with('#')) {
            let line = line
                .split_once(". ")
                .filter(|(n, _)| n.parse::<u32>().is_ok())
                .map_or(line, |(_, rest)| rest);
            for sentence in line.split(['.', '?', ':']) {
                let n = sentence.split_whitespace().count();
                assert!(n <= 20, "{role}: {n} words: {sentence}");
            }
        }
    }
}

#[test]
fn a_new_page_has_no_knowledge_of_one_programming_language() {
    let names = [
        "rust",
        "cargo",
        "python",
        "pytest",
        "javascript",
        "typescript",
        "node",
        "npm",
        "golang",
        "java",
        "kotlin",
        "ruby",
        "swift",
    ];
    for role in ROLES {
        let page = text(&format!("roles/{role}.md")).to_lowercase();
        let words = page.split(|c: char| !c.is_alphanumeric());
        for word in words {
            assert!(!names.contains(&word), "{role} names {word}");
        }
    }
}

/// Each group is one item of a list. A page must have one word of each group.
fn names_items(role: &str, items: &[&[&str]]) {
    let page = text(&format!("roles/{role}.md")).to_lowercase();
    for group in items {
        assert!(group.iter().any(|w| page.contains(w)), "{role}: {group:?}");
    }
}

#[test]
fn the_reviewer_page_names_its_list_and_cannot_write() {
    names_items(
        "reviewer",
        &[
            &["incorrect result"],
            &["default value"],
            &["shows nothing"],
            &["agree"],
            &["breaks a rule"],
            &[
                "file and the line",
                "file and its line",
                "its file and its line",
            ],
            &["summary of a good result", "good result"],
        ],
    );
    let page = text("roles/reviewer.md").to_lowercase();
    assert!(page.contains("write no file"), "no rule about files");
    assert!(page.contains("`spec`"), "the page does not name the paper");
}

#[test]
fn the_security_page_names_its_list_and_does_not_run_the_code() {
    names_items(
        "security",
        &[
            &["input"],
            &["query"],
            &["file path"],
            &["who a person is", "who a person"],
            &["password"],
            &["package"],
            &["safety setting"],
            &["write no file"],
            &["do not run", "does not run"],
        ],
    );
}

#[test]
fn the_simplifier_page_names_its_six_rules_and_its_work() {
    let page = text("roles/simplifier.md");
    for rule in [
        "KISS",
        "DRY",
        "YAGNI",
        "one source of truth",
        "plain direct code",
        "no boilerplate",
    ] {
        assert!(page.to_lowercase().contains(&rule.to_lowercase()), "{rule}");
    }
    assert!(page.contains("KISS") && page.contains("DRY") && page.contains("YAGNI"));
    for word in [
        "codeumbrella/<run id>",
        "commit",
        "ok: true",
        "ok: false",
        "test command",
        "paper `tests`",
    ] {
        assert!(page.contains(word), "{word}");
    }
}
