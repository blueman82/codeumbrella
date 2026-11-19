//! Tests of the run id with four digits, of `status` on an id of the
//! form of before, of the chat page and of `AGENTS.md`. They start the
//! built command with a simulated `claude`. No worker, no network.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs, thread};

static COUNT: AtomicU32 = AtomicU32::new(0);

const SKILL: &str = "skills/codeumbrella/SKILL.md";

const BEFORE: &str = "Before the first command";

const DATE: &str = "19 November 2026";

/// The words of point K of the task.
const NOT_PERMITTED: [&str; 69] = [
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
];

/// A simulated `claude` that gives a good result.
const CLAUDE: &str = r#"true
echo '{"is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

/// A sheet: copy, then one worker step.
const WORKER: &str = r#"start: copy
steps:
  copy:
    type: command
    run:
      - [git, clone, --quiet, --no-hardlinks, ., ".codeumbrella/runs/{run}/work"]
    next: one
  one:
    role: planner
    writes: [spec]
"#;

/// A sheet with one command step and no worker.
const TRUE_SHEET: &str = r"start: a
steps:
  a:
    type: command
    run: [[true]]
";

/// A notebook as a version before this one wrote it.
const NOTEBOOK: &str = r#"{
  "task": "a task",
  "status": "Done",
  "at": "build",
  "note": "",
  "answer": null,
  "tries": {},
  "results": {},
  "prints": {},
  "last": null
}"#;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let read = fs::read_to_string(root().join(rel));
    read.unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// A new empty folder with a folder `bin`. It gives the full path.
fn folder(name: &str) -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let me = std::process::id();
    let dir = env::temp_dir().join(format!("cu-runid-{name}-{me}-{n}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
    fs::canonicalize(&dir).unwrap()
}

fn git(dir: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.args(args).current_dir(dir);
    command.env("GIT_AUTHOR_NAME", "t");
    command.env("GIT_AUTHOR_EMAIL", "t@t");
    command.env("GIT_COMMITTER_NAME", "t");
    command.env("GIT_COMMITTER_EMAIL", "t@t");
    assert!(command.status().unwrap().success());
}

/// The built command, with the folder `bin` first on the `PATH`.
fn cu(dir: &Path, args: &[&str]) -> Output {
    let bin = dir.join("bin").display().to_string();
    let old = env::var("PATH").unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(args).current_dir(dir);
    command.env("PATH", format!("{bin}:{old}"));
    command.output().expect("start codeumbrella")
}

fn out(o: &Output) -> String {
    let stdout = String::from_utf8_lossy(&o.stdout);
    let stderr = String::from_utf8_lossy(&o.stderr);
    format!("{stdout}{stderr}")
}

fn says(o: &Output, needle: &str) {
    assert!(out(o).contains(needle), "{}", out(o));
}

/// The run id of the line `Run:` of the output.
fn id(o: &Output) -> String {
    let all = out(o);
    let line = all.lines().find(|l| l.starts_with("Run:      "));
    let line = line.unwrap_or_else(|| panic!("no line Run: {all}"));
    line["Run:      ".len()..].to_owned()
}

/// A new project with the factory files, a sheet and one commit.
fn project(name: &str, sheet: &str) -> PathBuf {
    let dir = folder(name);
    let init = cu(&dir, &["init"]);
    assert!(init.status.success(), "{}", out(&init));
    let sheet_file = dir.join(".codeumbrella/graph.yaml");
    fs::write(sheet_file, sheet).unwrap();
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "--quiet", "-m", "x"]);
    dir
}

/// Writes a simulated tool into the folder `bin` of the project.
fn tool(dir: &Path, name: &str, script: &str) {
    let path = dir.join("bin").join(name);
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    let mode = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&path, mode).unwrap();
}

/// The seconds and the four digits of an id `run-<seconds>-<digits>`.
/// It fails for an id with a different shape.
fn parts(id: &str) -> (u64, String) {
    let rest = id.strip_prefix("run-");
    let rest = rest.unwrap_or_else(|| panic!("no run-: {id}"));
    let split = rest.split_once('-');
    let (seconds, digits) = split.unwrap_or_else(|| panic!("{id}"));
    let all_digits = |t: &str| t.chars().all(|c| c.is_ascii_digit());
    assert!(all_digits(seconds), "{id}");
    assert_eq!(digits.len(), 4, "{id}");
    assert!(all_digits(digits), "{id}");
    (seconds.parse().unwrap(), digits.to_owned())
}

/// The Unix seconds of this moment.
fn now() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    since.unwrap().as_secs()
}

// The id of `run`.

#[test]
fn run_with_a_simulated_claude_gives_an_id_with_four_digits() {
    let dir = project("claude", WORKER);
    tool(&dir, "claude", CLAUDE);
    let before = now();
    let run = cu(&dir, &["run", "a task"]);
    let after = now();
    assert!(run.status.success(), "{}", out(&run));
    says(&run, "Status:   Done");
    let id = id(&run);
    let (seconds, _) = parts(&id);
    assert!((before..=after).contains(&seconds), "{id}");
    let run_folder = dir.join(".codeumbrella/runs").join(&id);
    assert!(run_folder.join("state.json").is_file(), "{id}");
}

/// One run in a project. The barrier starts it at the same moment as
/// the run of the other thread.
fn run_at(dir: &Path, start: &std::sync::Barrier) -> String {
    start.wait();
    let run = cu(dir, &["run", "a task"]);
    says(&run, "Status:   Done");
    id(&run)
}

#[test]
fn two_runs_in_two_folders_at_one_moment_get_different_ids() {
    // The four digits come from the clock and the process number. Two
    // runs can get the same four digits by chance. The test permits 2
    // pairs of 20 with the same id, so it is not bad by chance.
    let mut same = 0;
    for _ in 0..20 {
        let alpha = project("alpha", TRUE_SHEET);
        let beta = project("beta", TRUE_SHEET);
        let start = std::sync::Barrier::new(2);
        let (first, second) = thread::scope(|s| {
            let left = s.spawn(|| run_at(&alpha, &start));
            let right = s.spawn(|| run_at(&beta, &start));
            (left.join().unwrap(), right.join().unwrap())
        });
        parts(&first);
        parts(&second);
        if first == second {
            same += 1;
        }
    }
    assert!(same <= 2, "{same} pairs of 20 got the same id");
}

// `status` on a run folder that the test writes by hand. There is no
// `lock` file, so the repair does nothing.

/// Writes a run folder with this id by hand, and runs `status` on it.
fn status_of(id: &str) -> Output {
    let dir = folder("status");
    let run = dir.join(".codeumbrella/runs").join(id);
    fs::create_dir_all(&run).unwrap();
    fs::write(run.join("state.json"), NOTEBOOK).unwrap();
    let o = cu(&dir, &["status", id]);
    assert!(o.status.success(), "{}", out(&o));
    o
}

#[test]
fn status_works_on_a_run_folder_with_an_id_of_the_form_of_before() {
    let o = status_of("run-1795090396");
    says(&o, "Run:      run-1795090396\n");
    says(&o, "Status:   Done\n");
}

#[test]
fn status_works_on_a_run_folder_with_an_id_with_four_digits() {
    let o = status_of("run-1795095322-0417");
    says(&o, "Run:      run-1795095322-0417\n");
    says(&o, "Status:   Done\n");
}

// The documents.

/// The text of the part `## title` of a document.
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

/// The first line of a text with the word `word`.
fn line_with(part: &str, word: &str) -> String {
    let line = part.lines().find(|l| l.contains(word));
    let line = line.unwrap_or_else(|| panic!("no line with {word}"));
    line.to_owned()
}

/// A text with no part in code marks.
fn plain(text: &str) -> String {
    let parts = text.split('`').step_by(2);
    parts.collect::<Vec<&str>>().join(" ")
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

/// Checks a new line against point K: no word of the list out of code
/// marks, no semicolon, and 20 words at the maximum for a sentence.
fn plain_and_short(line: &str) {
    let low = plain(line).to_lowercase();
    let split = low.split(|c: char| !(c.is_alphanumeric() || c == '_'));
    let found: Vec<&str> = split.filter(|w| !w.is_empty()).collect();
    for bad in NOT_PERMITTED {
        assert!(!found.contains(&bad), "{bad}: {line}");
    }
    assert!(!low.contains(';'), "a semicolon: {line}");
    for sentence in body(line).split(". ") {
        let count = sentence.split_whitespace().count();
        assert!(count <= 20, "{count} words: {sentence}");
    }
}

#[test]
fn the_chat_page_stops_in_a_linked_git_worktree() {
    let part = section(SKILL, BEFORE);
    let line = line_with(&part, "test -d .git");
    for word in ["git clone", "linked git worktree", "stop"] {
        assert!(line.contains(word), "{word}: {line}");
    }
    plain_and_short(&line);
    // The step comes before the first `codeumbrella` command.
    let at = |w: &str| part.find(w).unwrap();
    let first = at("test -d .git") < at("codeumbrella init");
    assert!(first, "{part}");
}

#[test]
fn the_chat_page_tells_about_a_run_that_is_not_complete() {
    let part = section(SKILL, BEFORE);
    let line = line_with(&part, "Waiting");
    for word in ["codeumbrella status", "Running", "person"] {
        assert!(line.contains(word), "{word}: {line}");
    }
    plain_and_short(&line);
}

#[test]
fn the_chat_page_names_the_open_prs_before_the_start() {
    let part = section(SKILL, BEFORE);
    let line = line_with(&part, "gh pr list");
    for word in ["number", "title", "person"] {
        assert!(line.contains(word), "{word}: {line}");
    }
    plain_and_short(&line);
}

#[test]
fn agents_md_tells_the_new_id_its_chance_and_its_cause() {
    let rules = section("AGENTS.md", "Safety rules");
    let shape = line_with(&rules, "`run-<Unix seconds>-<four digits>`");
    plain_and_short(&shape);
    let chance = line_with(&rules, "not zero");
    plain_and_short(&chance);
    let has = |l: &&str| l.contains("same id") && l.contains(DATE);
    let cause = rules.lines().find(has);
    let cause = cause.unwrap_or_else(|| panic!("no line: same id"));
    plain_and_short(cause);
}

#[test]
fn agents_md_says_that_the_chat_page_stops_in_a_linked_git_worktree() {
    let rules = section("AGENTS.md", "Safety rules");
    let line = line_with(&rules, "linked git worktree");
    assert!(line.contains("chat page"), "{line}");
    let before = "before the first command";
    assert!(line.contains(before), "{line}");
    plain_and_short(&line);
}

#[test]
fn agents_md_says_that_the_chat_tells_about_a_run_that_is_not_complete() {
    let rules = section("AGENTS.md", "Safety rules");
    let line = line_with(&rules, "run that is not complete");
    assert!(line.contains("chat"), "{line}");
    assert!(line.contains("new run"), "{line}");
    plain_and_short(&line);
}

#[test]
fn the_structure_of_agents_md_names_the_new_test_file() {
    let part = section("AGENTS.md", "Structure");
    let line = line_with(&part, "`tests/run_id.rs`");
    assert!(line.starts_with('|'), "{line}");
}
