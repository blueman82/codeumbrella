//! Tests of the writer that corrects a sentence of the PR text after a bad
//! check: the run path, the two rules pages, the sheet and `AGENTS.md`.
//! No Claude, no Codex.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::Graph;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// The report of the simulated reviewer.
const REPORT: &str = "body.md: the sentence with `still` has a refused word.";

/// The fan parts of `check`.
const PARTS: [&str; 5] = [
    "check_tests",
    "check_safety",
    "check_review",
    "check_security",
    "check_use",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let path = root().join(rel);
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// A new empty folder, with its full path.
fn folder() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-writerfix-{}-{n}", std::process::id());
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

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

/// The lines of a log file.
fn log(path: &Path) -> Result<Vec<Value>> {
    let mut found = Vec::new();
    for line in fs::read_to_string(path)?.lines() {
        found.push(serde_json::from_str(line)?);
    }
    Ok(found)
}

/// A simulated worker. The reviewer gives one bad result with `REPORT`.
/// Each call of `write_pr` keeps the log of that moment.
#[derive(Default)]
struct Sim {
    refused: Mutex<bool>,
    logs: Mutex<Vec<Vec<Value>>>,
}

impl Worker for Sim {
    fn run(&self, job: &Job) -> Result<Outcome> {
        for paper in job.writes {
            let path = job.dir.join("papers").join(paper);
            fs::write(path.join("file.md"), job.step)?;
        }
        if job.step == "write_pr" {
            let lines = log(&job.dir.join("log.jsonl"))?;
            self.logs.lock().unwrap().push(lines);
        }
        let mut done = self.refused.lock().unwrap();
        let bad = job.step == "check_review" && !*done;
        *done |= bad;
        let summary = if bad { REPORT } else { "good work" };
        Ok(Outcome {
            ok: !bad,
            summary: summary.to_owned(),
        })
    }
}

/// The names of the steps of log lines, with no fan part.
fn names(lines: &[Value]) -> Vec<&str> {
    let all = lines.iter().filter_map(|l| l["step"].as_str());
    all.filter(|s| !PARTS.contains(s)).collect()
}

#[test]
fn a_refused_word_goes_through_repair_and_write_pr_to_check() {
    let graph = quiet_sheet();
    let sim = Sim::default();
    let roles = root().join("roles");
    let dir = folder().join(".codeumbrella/runs/run-1");
    let mut run = Run::start(dir, "a task", &graph).unwrap();
    fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, &sim).unwrap();
    run.answer(true, "").unwrap();
    run.go(&graph, &roles, &sim).unwrap();
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    assert_eq!(run.state.tries.get("plan"), Some(&1));

    let lines = log(&run.dir.join("log.jsonl")).unwrap();
    let all = names(&lines);
    let from = all.iter().position(|s| *s == "write_pr").unwrap();
    let want = ["write_pr", "check", "repair", "write_pr", "check"];
    assert_eq!(all[from..from + 5], want, "{all:?}");
    let plans = all.iter().filter(|s| **s == "plan").count();
    assert_eq!(plans, 1, "{all:?}");
    let questions = all.iter().filter(|s| **s == "look_at_tests");
    assert_eq!(questions.count(), 1, "{all:?}");

    // What the writer can read in its second call.
    let logs = sim.logs.lock().unwrap();
    assert_eq!(logs.len(), 2);
    let second = &logs[1];
    assert_eq!(names(second).last(), Some(&"repair"));
    let check = second.iter().rev().find(|l| l["step"] == "check");
    let check = check.expect("no line of check");
    assert_eq!(check["ok"], false);
    let summary = check["summary"].as_str().unwrap();
    assert!(summary.contains(REPORT), "{summary}");
}

/// The rule with this number of a page, with its sub-points.
fn rule(rel: &str, number: usize) -> String {
    let head = format!("{number}. ");
    let next = format!("{}. ", number + 1);
    let mut found = Vec::new();
    let mut inside = false;
    for line in text(rel).lines() {
        let goes_on = inside && !line.is_empty() && !line.starts_with(&next);
        inside = line.starts_with(&head) || goes_on;
        if inside {
            found.push(line.to_owned());
        }
    }
    assert!(!found.is_empty(), "{rel}: no rule {number}");
    found.join("\n")
}

/// A text as one line, in small letters, with no `#`.
fn flat(text: &str) -> String {
    let low = text.to_lowercase().replace('#', " ");
    let all: Vec<&str> = low.split_whitespace().collect();
    all.join(" ")
}

fn has_all(text: &str, words: &[&str]) {
    for word in words {
        assert!(text.contains(word), "{word}: {text}");
    }
}

#[test]
fn rule_5_of_the_writer_reads_the_report_of_the_bad_check() {
    let five = flat(&rule("roles/writer.md", 5));
    let words = ["`../log.jsonl`", "`write_pr`", "`repair`", "`check`"];
    has_all(&five, &words);
}

#[test]
fn rule_5_of_the_writer_corrects_the_named_sentences() {
    let five = flat(&rule("roles/writer.md", 5));
    let words = [
        "parts 1 to 5",
        "refused word",
        "sentence limit",
        "disagree",
        "meaning",
        "title",
        "purpose",
        "point of the no",
    ];
    has_all(&five, &words);
    let gone = "keep parts 1 to 5 as the planner wrote them.";
    assert!(!five.contains(gone), "{five}");
}

#[test]
fn rules_7_and_9_of_the_writer_agree_with_rule_5() {
    let seven = flat(&rule("roles/writer.md", 7));
    has_all(&seven, &["rule 5"]);
    let gone = "you do not change that sentence";
    assert!(!seven.contains(gone), "{seven}");
    let nine = flat(&rule("roles/writer.md", 9));
    has_all(&nine, &["rule 5", "do not change `title.txt`"]);
    let gone = "and only parts 6 to 8";
    assert!(!nine.contains(gone), "{nine}");
}

#[test]
fn the_checklist_of_the_writer_has_the_new_example_line() {
    let page = text("roles/writer.md");
    let want = "- Example: one example, with its facts, with no run id and no date.";
    assert!(page.lines().any(|l| l.trim() == want), "{page}");
    assert!(!page.contains("one real case"), "{page}");
}

#[test]
fn rule_8_of_the_builder_gives_a_good_result_for_the_paper_pr() {
    let eight = rule("roles/builder.md", 8);
    let words = ["paper `pr`", "`body.md`", "no file", "`ok: true`"];
    has_all(&eight, &words);
    let title = |l: &str| l.contains("`title.txt`") && l.contains("`ok: false`");
    assert!(eight.lines().any(title), "{eight}");
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
    flat(&found.join("\n"))
}

#[test]
fn the_comments_of_the_sheet_tell_the_new_path() {
    let writer = comment_above("write_pr");
    has_all(&writer, &["bad check", "parts 1 to 5"]);
    let repair = comment_above("repair");
    has_all(&repair, &["paper `pr`", "no file", "write_pr"]);
}

/// The lines of the part `## title` of `AGENTS.md`.
fn section(title: &str) -> Vec<String> {
    let head = format!("## {title}");
    let mut found = Vec::new();
    let mut inside = false;
    for line in text("AGENTS.md").lines() {
        if line.starts_with("## ") {
            inside = line == head;
        } else if inside {
            found.push(line.to_owned());
        }
    }
    assert!(!found.is_empty(), "AGENTS.md: no part {title}");
    found
}

/// The part `title` of `AGENTS.md` must have a line with each word.
fn one_line(title: &str, words: &[&str]) {
    let has = |l: &String| words.iter().all(|w| l.contains(w));
    let found = section(title).iter().any(has);
    assert!(found, "{title}: no line with {words:?}");
}

#[test]
fn the_words_of_agents_md_tell_what_the_writer_corrects() {
    one_line("Words", &["| Paper `pr` |", "bad check"]);
    one_line("Words", &["| Writer |", "bad check"]);
}

#[test]
fn agents_md_tells_the_new_path() {
    let path = ["`repair`", "paper `pr`", "no file"];
    let sheet = ["`repair`", "`write_pr`", "paper `pr`", "no file"];
    one_line("The sheet", &sheet);
    one_line("Safety rules", &path);
    let writer = ["writer", "bad check", "parts 1 to 5"];
    one_line("Safety rules", &writer);
    one_line("Structure", &["`tests/writer_fix.rs`"]);
}
