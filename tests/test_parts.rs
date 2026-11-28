//! Tests of the test command in parts: the four rules pages, the Rust
//! language page, the sheet, the documents and the run path.
//! No Claude, no Codex.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::Graph;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// The summary of a simulated worker with a part that did not complete.
const SHORT: &str = "Not complete: the part of tests/engine.rs.";

/// The fan parts of `check`.
const FAN: [&str; 5] = [
    "check_tests",
    "check_safety",
    "check_review",
    "check_security",
    "check_use",
];

/// The sentences that each of the four rules pages has.
const COMMON: [&str; 13] = [
    "Run the test command in parts, when one run can use more than 10 minutes.",
    "One shell command stops after 10 minutes at the maximum.",
    "For each test command, set the longest time limit of the shell tool.",
    "With no set limit, the shell tool can stop a command much sooner.",
    "A full run that stops at this limit is such a run.",
    "The parts together run each test of the test command.",
    "With no such part, make parts that the test tool can run alone.",
    "Run each part as one command and wait for its end.",
    "Do not run a part in the background.",
    "A part that stops at the time limit is not a bad test.",
    "Run that part again one time.",
    "If it stops again, the summary starts with `Not complete:` and names the part.",
    "Then run the other parts.",
];

/// The two sentences of a page that does not name the pages elsewhere.
const FIND: [&str; 2] = [
    "Read the part `## Run the tests` of each language page of the project, if it is there.",
    "The pages are in the folder `languages` of `../../../roles/`.",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let path = root().join(rel);
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// The rule with this number of a page, with its sub-points, as one line.
fn rule(rel: &str, number: usize) -> String {
    let head = format!("{number}. ");
    let next = format!("{}. ", number + 1);
    let mut found = Vec::new();
    let mut inside = false;
    for line in text(rel).lines() {
        let goes_on = inside && !line.is_empty() && !line.starts_with(&next);
        inside = line.starts_with(&head) || goes_on;
        if inside {
            let item = line.trim();
            let item = item.strip_prefix("- ").unwrap_or(item);
            found.push(item.to_owned());
        }
    }
    assert!(!found.is_empty(), "{rel}: no rule {number}");
    found.join(" ")
}

fn has_all(found: &str, words: &[&str]) {
    for word in words {
        assert!(found.contains(word), "{word}: {found}");
    }
}

// The rules pages.

#[test]
fn rule_13_of_the_builder_runs_the_tests_in_parts() {
    let found = rule("roles/builder.md", 13);
    has_all(&found, &COMMON);
    let own = [
        "The part `## Run the tests` of each language page of rule 3 tells the parts.",
        "A `Not complete:` part of a report is not a problem to repair.",
        "With only `Not complete:` parts and no other problem, give `ok: true`.",
        "Commit the work that you have.",
        "Change no code for such a part.",
        "Such a result does not go to the plan, also in the step `repair`.",
        "A bad test or a bad paper test gets its usual rule.",
    ];
    has_all(&found, &own);
    let kept = "A bad test command gives a bad result, as before.";
    has_all(&found, &[kept, "Not run: full check"]);
}

#[test]
fn rule_4_of_the_checker_runs_the_tests_in_parts() {
    let found = rule("roles/checker.md", 4);
    has_all(&found, &COMMON);
    has_all(&found, &FIND);
    let own = [
        "A `Not complete:` part gives `ok: false`, because its tests did not run to their end.",
        "Write no file of the project and commit nothing.",
    ];
    has_all(&found, &own);
}

#[test]
fn rule_10_of_the_checker_keeps_the_line_of_a_paper_test_first() {
    let found = rule("roles/checker.md", 10);
    let own = "A line `Paper test:` stays the first line, before the line `Not complete:`.";
    has_all(&found, &[own]);
}

#[test]
fn rule_5_of_the_attacker_runs_the_tests_in_parts() {
    let found = rule("roles/attacker.md", 5);
    has_all(&found, &COMMON);
    has_all(&found, &FIND);
    let own = [
        "A `Not complete:` part is not a problem by itself.",
        "That is not a problem by itself: the planner cannot repair a time limit.",
    ];
    has_all(&found, &own);
}

#[test]
fn rule_6_of_the_simplifier_runs_the_tests_in_parts() {
    let found = rule("roles/simplifier.md", 6);
    has_all(&found, &COMMON);
    let own = [
        "The part `## Run the tests` of each language page of rule 9 tells the parts.",
        "With a `Not complete:` part, put your changes back and commit nothing.",
        "Then give `ok: true` and say that you changed nothing.",
        "Run the test command of the task before you commit.",
    ];
    has_all(&found, &own);
}

// This test is good before the code. It keeps the rule of `AGENTS.md`.
#[test]
fn the_four_rules_pages_have_no_knowledge_of_one_language() {
    for role in ["builder", "checker", "attacker", "simplifier"] {
        let page = text(&format!("roles/{role}.md"));
        for word in ["cargo", "--lib", "--bins", "--doc", "--test "] {
            assert!(!page.contains(word), "{role}: {word}");
        }
    }
}

// The language page.

#[test]
fn the_rust_page_tells_the_parts_before_the_two_parts_of_the_standards() {
    let page = text("roles/languages/rust.md");
    let run = page.find("\n## Run the tests\n");
    let run = run.expect("no part about the tests");
    let write = page.find("\n## Write the code like this\n");
    let write = write.expect("no part to write");
    assert!(run < write, "the part of the tests comes first");
    let part = &page[run..write];
    let words = [
        "One run of `cargo test` can use more than 10 minutes.",
        "`cargo test --test <name>`",
        "`cargo test --lib --bins`",
        "`cargo test --examples`",
        "`cargo test --doc`",
        "A project with no library has no `--lib` and no `--doc`.",
        "`--offline`",
    ];
    has_all(part, &words);
}

// The sheet.

/// The comment lines right above the step `name`, as one line.
fn comment_above(name: &str) -> String {
    let sheet = text("graph.example.yaml");
    let lines: Vec<&str> = sheet.lines().collect();
    let head = format!("  {name}:");
    let at = lines.iter().position(|l| *l == head);
    let at = at.unwrap_or_else(|| panic!("no line {head}"));
    let mut found = Vec::new();
    for line in lines[..at].iter().rev() {
        let line = line.trim_start();
        let Some(rest) = line.strip_prefix('#') else {
            break;
        };
        found.push(rest.trim());
    }
    found.reverse();
    found.retain(|l| !l.is_empty());
    found.join(" ")
}

#[test]
fn the_sheet_tells_the_path_of_a_part_that_did_not_complete() {
    let build = [
        "A build with only `Not complete:` test parts and no bad test is good.",
        "It does not go to `plan`, and the checkers run the tests after it.",
    ];
    has_all(&comment_above("build"), &build);
    let repair = [
        "A report with only `Not complete:` parts changes no file.",
        "The checkers then look again, with no try of `plan`.",
    ];
    has_all(&comment_above("repair"), &repair);
}

// The documents.

#[test]
fn agents_md_has_the_new_safety_rules() {
    let page = text("AGENTS.md");
    let lines = [
        "- A worker runs the test command in parts when one run can use more than 10 minutes. One shell command stops at that limit.",
        "- The cause: on 28 November 2026, 11 runs of this project worked at the same time. A build stopped at that limit.",
        "- A part that stops two times gets `Not complete:`. A builder with no bad test then gives a good result, with no try of `plan`.",
        "- A checker with a `Not complete:` part gives a bad result. `repair` changes no file for it, and the checkers run again.",
        "- No worker runs a test part in the background. One rules page is for Claude and Codex.",
    ];
    for want in lines {
        assert!(page.lines().any(|l| l == want), "{want}");
    }
}

#[test]
fn the_tables_of_agents_md_name_the_part_and_the_test_file() {
    let page = text("AGENTS.md");
    let row = |start: &str| {
        let found = page.lines().find(|l| l.starts_with(start));
        found.unwrap_or_else(|| panic!("no row {start}")).to_owned()
    };
    let pages = row("| `roles/languages/");
    let words = [
        "Each page has 300 lines at the maximum and the parts `## Write the code like this` and `## A break`.",
        "Only `rust.md` also has the part `## Run the tests`.",
        "The builder, the reviewer and the simplifier read the pages, and `init` copies them.",
        "The checker and the attacker read only the part `## Run the tests`.",
    ];
    has_all(&pages, &words);
    assert!(!pages.contains("two parts"), "{pages}");
    assert!(row("| `tests/test_parts.rs`").contains("parts"));
}

#[test]
fn the_other_documents_name_the_two_new_readers() {
    let new = "The checker and the attacker read only the part `## Run the tests`.";
    let picture = text("ARCHITECTURE.md");
    let kept = "They read `general.md` and the page of each language in the change.";
    has_all(&picture, &[new, kept]);
    assert!(!picture.contains("No other role reads them."));
    let readme = text("README.md");
    let kept = "The builder, the reviewer and the simplifier read them.";
    has_all(&readme, &[new, kept]);
}

// The run path, with a simulated worker. These two tests are good before
// the code. They check the path that the new rules use.

/// The default sheet. Each command step gets a command that does nothing.
fn quiet_sheet() -> Graph {
    let path = root().join("graph.example.yaml");
    let mut graph = Graph::load(&path, &root().join("roles")).unwrap();
    for step in graph.steps.values_mut() {
        if !step.run.is_empty() {
            step.run = vec![vec!["true".to_owned()]];
        }
    }
    graph
}

/// A simulated worker. `build` gives a good result with `SHORT`. With
/// `check_short`, `check_tests` gives one bad result with `SHORT`, and
/// `repair` gives a good result with `SHORT`.
#[derive(Default)]
struct Sim {
    check_short: bool,
    done: AtomicBool,
}

impl Worker for Sim {
    fn run(&self, job: &Job) -> Result<Outcome> {
        for paper in job.writes {
            let path = job.dir.join("papers").join(paper);
            fs::write(path.join("file.md"), job.step)?;
        }
        let checks = self.check_short && job.step == "check_tests";
        let bad = checks && !self.done.swap(true, Ordering::SeqCst);
        let build = job.step == "build" && !self.check_short;
        let short = bad || build || job.step == "repair";
        let summary = if short { SHORT } else { "good work" };
        Ok(Outcome {
            ok: !bad,
            summary: summary.to_owned(),
        })
    }
}

/// The lines of a log file.
fn log(path: &Path) -> Vec<Value> {
    let all = fs::read_to_string(path).unwrap();
    all.lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// A run to the PR question. It gives the names of the steps in the log,
/// with no fan part.
fn to_approval(sim: &Sim) -> Vec<String> {
    let graph = quiet_sheet();
    let roles = root().join("roles");
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-parts-{}-{n}", std::process::id());
    let base = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    let base = fs::canonicalize(&base).unwrap();
    let dir = base.join(".codeumbrella/runs/run-1");
    let mut run = Run::start(dir, "a task", &graph).unwrap();
    fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, sim).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, sim).unwrap();
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    assert_eq!(run.state.tries.get("plan"), Some(&1));
    let lines = log(&run.dir.join("log.jsonl"));
    let all = lines.iter().filter_map(|l| l["step"].as_str());
    let all = all.filter(|s| !FAN.contains(s));
    all.map(str::to_owned).collect()
}

#[test]
fn a_good_build_with_a_short_part_does_not_go_to_the_plan() {
    let all = to_approval(&Sim::default());
    let at = all.iter().position(|s| s == "build");
    let at = at.expect("no build");
    assert_eq!(all[at + 1], "simplify", "{all:?}");
    let plans = all.iter().filter(|s| *s == "plan").count();
    assert_eq!(plans, 1, "{all:?}");
}

#[test]
fn a_check_with_a_short_part_runs_again_with_no_plan() {
    let sim = Sim {
        check_short: true,
        ..Sim::default()
    };
    let all = to_approval(&sim);
    let at = all.iter().position(|s| s == "check");
    let at = at.expect("no check");
    let want = ["check", "repair", "write_pr", "check"];
    assert_eq!(all[at..at + 4], want, "{all:?}");
    let plans = all.iter().filter(|s| *s == "plan").count();
    assert_eq!(plans, 1, "{all:?}");
}
