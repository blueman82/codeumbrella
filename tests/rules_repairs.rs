//! Tests: the repairs of the chat page, the checker page and the builder page.

use std::fs;
use std::path::PathBuf;

fn text(rel: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

const SKILL: &str = "skills/codeumbrella/SKILL.md";

/// The version text of a file: the first `version = "x"` or `"version": "x"`.
fn version(rel: &str) -> String {
    let t = text(rel);
    let mut parts = t.split("version");
    parts.next();
    let rest = parts.next().unwrap();
    let rest = rest.trim_start_matches(['"', ' ', '=', ':']);
    rest.split('"').next().unwrap().to_owned()
}

#[test]
fn the_chat_starts_commands_so_that_they_outlive_the_shell() {
    let s = text(SKILL);
    assert!(s.contains("nohup"), "no example with nohup");
    let line = s.lines().find(|l| l.contains("nohup")).unwrap();
    assert!(line.contains("codeumbrella"), "{line}");
    assert!(line.contains('>'), "the output must go to a file: {line}");
    assert!(
        line.trim_end().trim_end_matches('`').ends_with('&'),
        "{line}"
    );
    let low = s.to_lowercase();
    assert!(low.contains("`answer`") || low.contains("answer"));
    assert!(low.contains("`resume`"));
    assert!(low.contains("time limit"));
}

#[test]
fn the_chat_polls_status_until_it_is_not_running() {
    let s = text(SKILL);
    assert!(s.contains("Running"));
    assert!(s.to_lowercase().contains("each 3 minutes"));
    assert!(s.to_lowercase().contains("output file"));
}

#[test]
fn the_time_limit_of_10_minutes_is_gone() {
    let s = text(SKILL);
    assert!(!s.contains("10 minutes"));
    assert!(!s.contains("Give it a time limit"));
}

#[test]
fn a_running_status_with_no_live_command_gets_a_resume() {
    let s = text(SKILL);
    let line = s
        .lines()
        .find(|l| l.contains("repairs a run with a dead command"))
        .expect("no line about the repair of a dead command");
    assert!(line.contains("resume"), "{line}");
    assert!(line.to_lowercase().contains("tell the person"), "{line}");
}

#[test]
fn the_checker_copies_the_paper_tests_in_each_try() {
    let c = text("roles/checker.md");
    let low = c.to_lowercase();
    assert!(low.contains("compare"));
    assert!(low.contains("each test file"));
    assert!(low.contains("each try"));
    assert!(low.contains("different or is not there") || low.contains("different or not there"));
    assert!(low.contains("paper wins"));
    assert!(low.contains("approval"));
    assert!(low.contains("commit"));
}

#[test]
fn the_checker_gives_no_ok_while_a_test_file_differs() {
    let c = text("roles/checker.md");
    let line = c
        .lines()
        .find(|l| l.contains("ok: true") && l.contains("different") && l.contains("paper"))
        .expect("no rule that stops ok: true for a different test file");
    assert!(line.to_lowercase().contains("not give"), "{line}");
}

#[test]
fn the_builder_does_not_change_a_paper_test_file() {
    let b = text("roles/builder.md");
    let line = b
        .lines()
        .find(|l| l.contains("test file") && l.contains("paper `tests`"))
        .expect("no rule about test files of the paper");
    assert!(
        line.contains("does not change") || line.contains("not change"),
        "{line}"
    );
    assert!(line.contains("changes no code"), "{line}");
    assert!(line.contains("ok: false"), "{line}");
}

#[test]
fn the_builder_rule_8_still_exists_and_is_numbered_8() {
    let b = text("roles/builder.md");
    let rule8 = b.lines().find(|l| l.starts_with("8. ")).unwrap();
    assert!(rule8.contains("change no code"), "{rule8}");
}

#[test]
fn the_builder_reads_the_spec_again_after_a_correction() {
    let b = text("roles/builder.md");
    let low = b.to_lowercase();
    assert!(low.contains("reads the spec again") || low.contains("read the spec again"));
    assert!(low.contains("compare"));
    assert!(low.contains("only because a commit"));
}

#[test]
fn each_rules_page_keeps_the_summary_rule() {
    for path in ["roles/builder.md", "roles/checker.md", "roles/planner.md"] {
        assert!(
            text(path).contains("Your summary must have 100 words or fewer."),
            "{path}"
        );
    }
}

#[test]
fn the_sentences_of_the_pages_have_20_words_or_fewer() {
    for path in ["roles/builder.md", "roles/checker.md", "roles/planner.md"] {
        let t = text(path).replace('`', "");
        for line in t.lines().filter(|l| !l.starts_with('#')) {
            let line = line
                .split_once(". ")
                .filter(|(n, _)| n.parse::<u32>().is_ok())
                .map_or(line, |(_, rest)| rest);
            for sentence in line.split(['.', '?']) {
                let n = sentence.split_whitespace().count();
                assert!(n <= 20, "{path}: {n} words: {sentence}");
            }
        }
    }
}

#[test]
fn the_numbers_of_the_rules_stay_in_sequence() {
    for path in ["roles/builder.md", "roles/checker.md"] {
        let t = text(path);
        let numbers: Vec<usize> = t
            .lines()
            .filter_map(|l| l.split_once(". ")?.0.parse().ok())
            .collect();
        assert_eq!(numbers, (1..=numbers.len()).collect::<Vec<_>>(), "{path}");
    }
}

#[test]
fn agents_md_has_a_line_for_each_repair() {
    let a = text("AGENTS.md");
    assert!(
        a.lines()
            .any(|l| l.starts_with("- ") && l.contains("nohup") && l.contains("lock")),
        "no line about a command that outlives the shell"
    );
    assert!(
        a.lines().any(|l| l.starts_with("- ")
            && l.contains("checker")
            && l.contains("`tests`")
            && l.to_lowercase().contains("paper")
            && l.contains("different")),
        "no line about the test files of the paper"
    );
}

#[test]
fn the_line_limits_in_agents_md_are_the_agreed_ones() {
    let a = text("AGENTS.md");
    assert!(a.contains("`src/lib.rs`): 1100 lines of code."));
    assert!(a.contains("Each tool part: 200 lines of code."));
}

#[test]
fn the_four_files_have_the_same_version_with_three_numbers() {
    let v = version("Cargo.toml");
    assert_ne!(v, "");
    assert_eq!(version(".claude-plugin/plugin.json"), v);
    assert_eq!(version(".codex-plugin/plugin.json"), v);
    let lock = text("Cargo.lock");
    let at = lock.find("name = \"codeumbrella\"\nversion = \"").unwrap();
    let lock_v = lock[at..].lines().nth(1).unwrap();
    assert_eq!(lock_v, format!("version = \"{v}\""));
    let parts: Vec<u32> = v.split('.').map(|p| p.parse().unwrap()).collect();
    assert_eq!(parts.len(), 3, "{v}");
}
