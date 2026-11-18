//! Tests of version 0.1.14: the attacker writes only in a scratch folder in
//! the temp folder. No AI runs here.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const ENTRY: &str = "\":tmpdir\"=\"write\"";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn json(rel: &str) -> Value {
    serde_json::from_str(&text(rel)).unwrap_or_else(|e| panic!("{rel} is not JSON: {e}"))
}

/// The lines of rule 5 of the attacker page, up to rule 6.
fn rule_5() -> String {
    let page = text("roles/attacker.md");
    let mut lines = page.lines().skip_while(|l| !l.starts_with("5. "));
    let mut found = Vec::new();
    for line in lines.by_ref() {
        if line.starts_with("6. ") {
            break;
        }
        found.push(line);
    }
    assert!(!found.is_empty(), "rule 5 is missing");
    found.join(" ")
}

#[test]
fn the_codex_attacker_can_write_only_in_the_temp_folder() {
    let codex = text("roles/attacker.codex.txt");
    assert!(codex.contains(ENTRY), "{codex}");
    assert_eq!(codex.matches("\"write\"").count(), 1, "{codex}");
    assert!(codex.starts_with("{extends=\":read-only\""), "{codex}");
    assert!(!codex.contains("danger"), "{codex}");
    // The old locks stay.
    assert!(codex.contains("\".codeumbrella\"=\"read\""), "{codex}");
    for key in [
        ".ssh",
        ".config/gh",
        ".netrc",
        ".git-credentials",
        "Library/Keychains",
    ] {
        let lock = format!("\"{{home}}/{key}\"=\"deny\"");
        assert!(codex.contains(&lock), "{lock}");
    }
}

#[test]
fn no_other_codex_role_gets_the_temp_folder_entry() {
    for role in ["safety", "reviewer", "security", "planner"] {
        let file = text(&format!("roles/{role}.codex.txt"));
        assert!(!file.contains(":tmpdir"), "{role}");
    }
}

#[test]
fn the_claude_attacker_is_the_same_as_the_claude_safety_checker() {
    assert_eq!(
        json("roles/attacker.claude.json"),
        json("roles/safety.claude.json")
    );
}

#[test]
fn the_attacker_page_tells_the_scratch_folder_and_the_good_bad_test() {
    let page = text("roles/attacker.md");
    assert!(page.contains("$TMPDIR"), "$TMPDIR is missing");
    assert!(page.contains("mktemp -d"), "mktemp -d is missing");
    let flat = page.replace('\n', " ").to_lowercase();
    assert!(
        flat.contains("a test that builds and is bad is not a problem"),
        "the rule for a bad test that builds is missing"
    );
    assert!(flat.contains("scratch folder"));
    assert!(flat.contains("readme.md"));
    assert!(flat.contains("cargo fmt") || flat.contains("layout check"));
}

#[test]
fn the_rules_of_the_attacker_page_have_no_hole_and_the_old_words_are_gone() {
    let page = text("roles/attacker.md");
    let numbers: Vec<usize> = page
        .lines()
        .filter_map(|l| l.split_once(". ")?.0.parse().ok())
        .collect();
    assert_eq!(numbers, (1..=10).collect::<Vec<usize>>());
    let rule_2 = page.lines().find(|l| l.starts_with("2. ")).unwrap();
    assert!(!rule_2.contains("Change no file."), "{rule_2}");
    assert!(!page.contains("write no file"), "the intro is old");
    assert!(page.contains("scratch"), "no scratch folder");
}

#[test]
fn each_sentence_of_rule_5_has_20_words_at_the_maximum() {
    let rule = rule_5();
    for sentence in rule.split(". ") {
        let words = sentence.split_whitespace().count();
        assert!(words <= 20, "{words} words: {sentence}");
    }
}

#[test]
fn the_attacker_page_has_none_of_the_words_that_are_not_permitted() {
    let page = text("roles/attacker.md").to_lowercase();
    let words: Vec<&str> = page
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    for bad in [
        "never", "security", "process", "already", "way", "case", "need", "return", "place",
        "permit",
    ] {
        assert!(!words.contains(&bad), "{bad}");
    }
}

#[test]
fn the_documents_no_longer_say_that_the_attacker_cannot_write() {
    for rel in [
        "AGENTS.md",
        "README.md",
        "ARCHITECTURE.md",
        "graph.example.yaml",
    ] {
        let page = text(rel);
        for line in page.lines().filter(|l| l.contains("ttacker")) {
            assert!(!line.contains("cannot write a file"), "{rel}: {line}");
            assert!(!line.contains("writes no file"), "{rel}: {line}");
            assert!(
                !line.contains(
                    "The settings of the attacker are the same as those of the safety checker."
                ),
                "{rel}: {line}"
            );
        }
    }
    let agents = text("AGENTS.md");
    assert!(
        agents
            .lines()
            .any(|l| l.starts_with('|') && l.contains("tests/scratch_folder.rs")),
        "AGENTS.md table line"
    );
    assert!(agents.contains("scratch folder"));
    assert!(agents.contains("temp folder"));
    assert!(agents.contains("one more entry"));
    let flat = text("ARCHITECTURE.md").to_lowercase();
    assert!(flat.contains("scratch"), "ARCHITECTURE.md");
}

fn first_version(page: &str) -> String {
    let line = page.lines().find(|l| l.contains("\"version\"")).unwrap();
    line.split('"').nth(3).unwrap().to_owned()
}

#[test]
fn the_four_version_files_agree() {
    let toml = text("Cargo.toml");
    let line = toml.lines().find(|l| l.starts_with("version")).unwrap();
    let version = line.split('"').nth(1).unwrap().to_owned();
    let lock = text("Cargo.lock");
    let mut lines = lock.lines().skip_while(|l| *l != "name = \"codeumbrella\"");
    lines.next().expect("the package is in Cargo.lock");
    let in_lock = lines.next().unwrap().split('"').nth(1).unwrap().to_owned();
    assert_eq!(in_lock, version);
    for rel in [".claude-plugin/plugin.json", ".codex-plugin/plugin.json"] {
        assert_eq!(first_version(&text(rel)), version, "{rel}");
    }
}
