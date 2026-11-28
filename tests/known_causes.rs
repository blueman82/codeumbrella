//! Tests of the known causes of a stop: the chat page and the documents.
//! They only read files. No worker and no network.

use std::fs;
use std::path::PathBuf;

const SKILL: &str = "skills/codeumbrella/SKILL.md";

const RULE: &str = "For a known cause only, the chat starts `resume` with no question.";

const CAUSES: [&str; 3] = [
    "`Can't reach the API server`",
    "`ENOTFOUND`",
    "`the base branch policy prohibits the merge`",
];

const OTHER_STOPS: &str =
    "Each other stop goes to the person. Run `resume` only after a yes of the person.";

const README_MERGE: &str = "The merge cause counts only if a GitHub check had no result.";

const README_RULES: [&str; 4] = [
    "The chat first waits until the server answers, or until the GitHub checks end.",
    "A bad check goes to you.",
    "A second stop of the same step with the same cause comes to you.",
    "The chat tells you each such `resume` in one line, at the next question.",
];

fn text(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    let read = fs::read_to_string(path);
    read.unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// The text of the sub-part `### Known causes` in the part of the stop rules.
fn known_causes() -> String {
    let page = text(SKILL);
    let part = page.split_once("\n## A stop with a rule\n");
    let (_, part) = part.expect("SKILL.md: no part A stop with a rule");
    let part = part.split("\n## ").next().unwrap_or_default();
    let sub = part.split_once("\n### Known causes\n");
    let (_, sub) = sub.expect("SKILL.md: no sub-part Known causes");
    sub.to_owned()
}

fn chat_page_has(wanted: &[&str]) {
    let part = known_causes();
    for want in wanted {
        assert!(part.contains(want), "SKILL.md: not there: {want}");
    }
}

/// The first line of `rel` that starts with `start`.
fn line_of(rel: &str, start: &str) -> String {
    let page = text(rel);
    let found = page.lines().find(|l| l.starts_with(start));
    let found = found.unwrap_or_else(|| panic!("{rel}: no line {start}"));
    found.to_owned()
}

#[test]
fn the_chat_page_resumes_with_no_question_for_a_known_cause() {
    chat_page_has(&[
        "For a known cause only, start `codeumbrella resume <run id>` with no question to the person.",
        "The line of the stopped run that says to wait for a yes does not count for a known cause.",
        "Read the full note in `state.json` of the run folder, because the status cuts a long note.",
    ]);
    let page = text(SKILL);
    assert!(page.contains(OTHER_STOPS), "SKILL.md: {OTHER_STOPS}");
}

#[test]
fn the_chat_page_has_the_list_of_known_causes() {
    chat_page_has(&[
        "Network: the note has `claude reported an error:` and also `Can't reach the API server` or `ENOTFOUND`.",
        "A note with `gave a bad result` is not the network cause.",
        "Merge: the note starts with `Step merge gave a bad result:` and has `the base branch policy prohibits the merge`.",
        "For the merge cause, a GitHub check also had no result at the stop.",
        r#"Run `gh pr checks "codeumbrella/<run id>" --json name,bucket,completedAt` to see the checks."#,
        "A check had no result if its `bucket` is `pending`.",
        "It also had no result if its `completedAt` is after the start of the step `merge`.",
        "The line of `merge` in `log.jsonl` gives that start: `time` minus `seconds`.",
        "With no such check, the stop goes to the person.",
    ]);
}

#[test]
fn the_chat_page_waits_before_the_resume() {
    chat_page_has(&[
        "Run `curl` and `gh` with no sandbox, because they use the network.",
        "For the network cause, run `curl -sS -o /dev/null --max-time 10 https://api.anthropic.com` each minute.",
        "Wait until that call ends with no error, which shows that the server gave an answer.",
        r#"For the merge cause, run `gh pr checks "codeumbrella/<run id>" --watch` until the GitHub checks end."#,
        "Then read the checks again with `--json`.",
        "A check is good only with the `bucket` `pass` or `skipping`.",
        "If a check is bad, the stop goes to the person.",
        "If `gh` cannot show the checks, the stop goes to the person.",
    ]);
}

#[test]
fn the_chat_page_has_the_one_time_rule() {
    chat_page_has(&[
        "For a known cause, the chat starts `resume` one time for each stop.",
        "Before the `resume` of a known cause, read `log.jsonl` of the run.",
        "A second stop of the same step with the same cause goes to the person.",
        "The last line of `log.jsonl` is this stop.",
        "In `log.jsonl`, a `summary` does not have the words `Step merge gave a bad result:`.",
        "There, the merge cause is a `summary` with `the base branch policy prohibits the merge`.",
        "There, the network cause is a `summary` with `claude reported an error:` and one network text.",
        r#"An earlier line shows a second stop if it has this `step`, `"ok":false` and the same cause."#,
    ]);
}

#[test]
fn the_architecture_page_has_the_exception() {
    let start = "- `resume` tries a stopped step again.";
    let line = line_of("ARCHITECTURE.md", start);
    let want = "The person must say yes first. A `known cause` is the exception.";
    assert!(line.contains(want), "ARCHITECTURE.md: {line}");
}

#[test]
fn the_chat_page_tells_the_person_each_resume() {
    chat_page_has(&[
        "Tell the person each such `resume` in one line, at the next question.",
        "The line gives the run id, the step and the cause.",
        "If no question comes, tell that line when the run is `Done` or `Stopped`.",
    ]);
}

#[test]
fn the_safety_rules_have_the_exception_with_its_cause_and_date() {
    let page = text("AGENTS.md");
    let rules = page.split_once("\n## Safety rules\n");
    let (_, rules) = rules.expect("AGENTS.md: no part Safety rules");
    let rules = rules.split("\n## ").next().unwrap_or_default();
    let found = rules.lines().find(|l| l.contains(RULE));
    let line = found.expect("AGENTS.md: no safety line with the exception");
    let wanted = [
        "The person decided this on 27 November 2026.",
        "The cause: in this project, a worker could not reach the Claude server.",
        "GitHub also refused a merge while the checks had no result.",
        "The person said yes to each `resume`, and wants no such simple question.",
    ];
    for want in wanted {
        assert!(line.contains(want), "AGENTS.md: not there: {want}");
    }
    let before = "The chat runs `resume` only after the person says yes.";
    assert!(rules.contains(before), "AGENTS.md: {before}");
}

#[test]
fn the_lines_about_resume_name_the_known_cause() {
    let rows = [
        ("AGENTS.md", "codeumbrella resume <run-id>"),
        ("AGENTS.md", "- The stopped run prints `To continue:"),
        (
            "AGENTS.md",
            "- The chat runs `resume` only after the person says yes.",
        ),
        ("README.md", "| `codeumbrella resume <run-id>` |"),
    ];
    for (rel, start) in rows {
        let line = line_of(rel, start);
        assert!(line.contains("known cause"), "{rel}: {line}");
    }
}

#[test]
fn the_readme_tells_the_rule_and_the_causes() {
    let line = line_of("README.md", &format!("- {RULE}"));
    for cause in CAUSES {
        assert!(line.contains(cause), "README.md: not there: {cause}");
    }
    assert!(line.contains(README_MERGE), "README.md: {README_MERGE}");
    let next = line_of("README.md", &format!("- {}", README_RULES[0]));
    for want in README_RULES {
        assert!(next.contains(want), "README.md: not there: {want}");
    }
    line_of("README.md", "- Each other stop comes to you.");
}

#[test]
fn each_sentence_that_starts_a_resume_names_a_known_cause() {
    for rel in [SKILL, "AGENTS.md", "README.md"] {
        let page = text(rel);
        let mut count = 0;
        for sentence in page.lines().flat_map(|l| l.split(". ")) {
            let low = sentence.to_lowercase();
            let starts = low.contains("starts `resume`")
                || low.contains("start `resume`")
                || low.contains("start `codeumbrella resume");
            if starts {
                count += 1;
                assert!(low.contains("known cause"), "{rel}: {sentence}");
            }
        }
        assert!(count > 0, "{rel}: no sentence starts a resume");
    }
}

#[test]
fn the_structure_table_names_this_test_file() {
    line_of("AGENTS.md", "| `tests/known_causes.rs` |");
}
