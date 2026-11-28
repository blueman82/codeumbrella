//! Tests of the plain words check of the attacker. They read the rules
//! page of the attacker and `AGENTS.md`. No worker, no network.

use std::fs;
use std::path::PathBuf;

/// The sub-points that rule 5 of the attacker page gets.
const RULE_LINES: [&str; 10] = [
    "Check each comment and each text in the paper test files for plain words, \
     as the reviewer does.",
    "A sentence has 20 words at the maximum. A paragraph has 6 sentences at the maximum.",
    "The text uses the active voice and no semicolon.",
    "`AGENTS.md` of the project can have a list of refused words. \
     A word of that list is a problem.",
    "A word in code marks is not refused. A project with no such list gets only these limits.",
    "The list stays in `AGENTS.md`. Read it there.",
    "Do not check code, test names, file names, commands or the data of a test.",
    "Check only the lines that a test file adds to the commit from before. \
     Check each new file fully.",
    "Each such problem gives `ok: false`, also a small one. \
     The builder cannot change a paper test.",
    "Give the file, the line and the sentence of each such problem.",
];

/// The lines that the safety rules of `AGENTS.md` get.
const SAFETY_LINES: [&str; 4] = [
    "- The attacker checks the comments and texts of the paper tests \
     for the refused words and sentence limits.",
    "- Such a problem gives a bad result of the attacker. \
     The step `correct_plan` corrects it before the tests question.",
    "- The attacker and the reviewer use one list of refused words. \
     It stays in `AGENTS.md` only.",
    "- A run of this project showed the cause on 25 November 2026. \
     A refused word in a paper test stopped it.",
];

/// The row that the table of the structure gets.
const ROW: &str = "| `tests/attacker_words.rs` | \
    Tests of the plain words check of the attacker and of its documents. |";

fn text(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    let msg = format!("cannot read {}", path.display());
    fs::read_to_string(path).expect(&msg)
}

/// The lines of rule 5 of the attacker page, up to rule 6.
fn rule_5() -> Vec<String> {
    let page = text("roles/attacker.md");
    let from = page.lines().skip_while(|l| !l.starts_with("5. "));
    let rule = from.take_while(|l| !l.starts_with("6. "));
    let found: Vec<String> = rule.map(str::to_owned).collect();
    assert!(!found.is_empty(), "roles/attacker.md: no rule 5");
    found
}

/// The lines of the part `## title` of `AGENTS.md`.
fn part(title: &str) -> Vec<String> {
    let page = text("AGENTS.md");
    let head = format!("## {title}");
    let from = page.lines().skip_while(|l| *l != head).skip(1);
    let lines = from.take_while(|l| !l.starts_with("## "));
    let found: Vec<String> = lines.map(str::to_owned).collect();
    assert!(!found.is_empty(), "AGENTS.md: no part {title}");
    found
}

#[test]
fn rule_5_of_the_attacker_has_each_line_of_the_plain_words_check() {
    let rule = rule_5();
    for want in RULE_LINES {
        let item = format!("- {want}");
        let found = rule.iter().any(|l| l.trim() == item);
        assert!(found, "no line in rule 5: {item}");
    }
}

#[test]
fn the_safety_rules_of_agents_md_tell_the_new_rule() {
    let rules = part("Safety rules");
    for want in SAFETY_LINES {
        let found = rules.iter().any(|l| l == want);
        assert!(found, "no line in the safety rules: {want}");
    }
}

#[test]
fn the_structure_of_agents_md_names_the_new_test_file() {
    let table = part("Structure");
    let found = table.iter().any(|l| l == ROW);
    assert!(found, "no row in the structure: {ROW}");
}
