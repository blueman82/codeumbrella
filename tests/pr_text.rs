//! Tests of the PR text of version 0.1.26: the paper `pr`, the role
//! `writer`, the step `write_pr`, the steps `find_pr` and `open_pr` with a
//! simulated `gh`, the rules pages and the documents. No Claude, no Codex.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::{Graph, Kind};
use serde_json::Value;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// The fan parts of `check`, in the sequence of the sheet.
const PARTS: [&str; 5] = [
    "check_tests",
    "check_safety",
    "check_review",
    "check_security",
    "check_use",
];

/// The three files of the role `writer`.
const FILES: [&str; 3] = [
    "roles/writer.md",
    "roles/writer.claude.json",
    "roles/writer.codex.txt",
];

/// The word that makes `gh` read the clean copy.
const GIT_DIR: &str = "GIT_DIR=.codeumbrella/runs/{run}/clean/.git";

/// The first command of the step `find_pr`, as today.
const VIEW: [&str; 10] = [
    "env",
    GIT_DIR,
    "gh",
    "pr",
    "view",
    "codeumbrella/{run}",
    "--json",
    "url",
    "--jq",
    ".url",
];

/// The title file of the paper `pr`, from the project folder.
const TITLE: &str = ".codeumbrella/runs/{run}/papers/pr/title.txt";

/// The text file of the paper `pr`, from the project folder.
const BODY: &str = ".codeumbrella/runs/{run}/papers/pr/body.md";

/// The folder of the paper `pr`, after `/` in a Claude rule.
const PR_PATH: &str = "{project}/.codeumbrella/runs/{run}/papers/pr";

/// The write entry of the paper `pr` in the Claude settings.
const CLAUDE_ENTRY: &str = "Edit(/{project}/.codeumbrella/runs/{run}/papers/pr/**)";

/// The deny rule of the work copy in the Claude settings of the writer.
const WORK_DENY: &str = "Edit(/{project}/.codeumbrella/runs/{run}/work/**)";

/// The deny rule of the reviewer that the writer does not have.
const ALL_DENY: &str = "Edit(/{project}/**)";

/// The write entry of the paper `pr` in the Codex settings.
const CODEX_ENTRY: &str = r#""{project}/.codeumbrella/runs/{run}/papers/pr"="write""#;

/// The 10 items of part B of the task, with no list mark.
const CHECKLIST: [&str; 10] = [
    "Purpose: what the change is for, in general words.",
    "Cause: the general problem, with no run id and no date.",
    "Example: one real case, with its facts, with no run id and no date.",
    "How this PR prevents it: one line for each part of the fix.",
    "Other corrections in the PR: one line each.",
    "Version line.",
    "One part for each area of the change, with the file names.",
    "Only the work of this PR. No plans, no other PRs.",
    "\"The person\", not \"you\".",
    "Each sentence compared with the diff.",
];

/// The first sentence of the first reading rule of each page.
const FIRST: &str = "If the project has `AGENTS.md` or `CLAUDE.md`, read them first.";

/// The second sentence of that rule.
const D1: &str = "Read `../../../lessons.md` from the work copy if it is there.";

/// The third sentence of that rule.
const D2: &str = "The spec, `AGENTS.md`, `CLAUDE.md` and this page come before a lesson.";

const SKILL: &str = "skills/codeumbrella/SKILL.md";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn roles() -> PathBuf {
    root().join("roles")
}

fn text(rel: &str) -> String {
    let path = root().join(rel);
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn json(rel: &str) -> Value {
    serde_json::from_str(&text(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn list(value: &Value) -> Vec<&str> {
    let items = value.as_array().expect("a list");
    items.iter().map(|v| v.as_str().expect("text")).collect()
}

fn default_sheet() -> Graph {
    let path = root().join("graph.example.yaml");
    Graph::load(&path, &roles()).unwrap()
}

/// The default sheet. Each command step gets a command that does nothing.
fn quiet_sheet() -> Graph {
    let mut graph = default_sheet();
    for step in graph.steps.values_mut() {
        if !step.run.is_empty() {
            step.run = vec![vec!["true".to_owned()]];
        }
    }
    graph
}

/// A new empty folder. The path is the full one: on a Mac,
/// `/private/var/...` and not `/var/...`.
fn folder() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-pr-{}-{n}", std::process::id());
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

// ---------------------------------------------------------------------
// The sheet
// ---------------------------------------------------------------------

/// True if a step of the sheet goes to `to`: by `next` or by `on_bad`.
fn goes_to(graph: &Graph, from: &str, to: &str) -> bool {
    let step = graph.step(from).unwrap();
    let target = Some(to.to_owned());
    step.next == target || step.on_bad == target
}

#[test]
fn no_command_of_the_sheet_uses_fill() {
    let graph = default_sheet();
    for (name, step) in &graph.steps {
        for word in step.run.iter().flatten() {
            assert!(!word.starts_with("--fill"), "{name}: {word}");
        }
    }
}

#[test]
fn the_planner_steps_write_spec_tests_and_pr() {
    let graph = default_sheet();
    let mut names = vec!["plan".to_owned(), "correct_plan".to_owned()];
    names.extend([2, 3].map(|n| format!("correct_plan_{n}")));
    for name in &names {
        let step = graph.step(name).unwrap();
        assert_eq!(step.role.as_deref(), Some("planner"), "{name}");
        assert_eq!(step.writes, ["spec", "tests", "pr"], "{name}");
    }
}

#[test]
fn write_pr_is_a_worker_step_of_the_writer_after_learn_and_repair() {
    let graph = default_sheet();
    let step = graph.step("write_pr").unwrap();
    assert_eq!(step.kind, Kind::Worker);
    assert_eq!(step.role.as_deref(), Some("writer"));
    assert_eq!(step.reads, ["spec"]);
    assert_eq!(step.writes, ["pr"]);
    assert_eq!(step.next.as_deref(), Some("check"));
    assert!(step.on_bad.is_none(), "{:?}", step.on_bad);
    assert!(step.max_tries.is_none(), "{:?}", step.max_tries);
    let learn = graph.step("learn").unwrap();
    assert_eq!(learn.next.as_deref(), Some("set_version"));
    let repair = graph.step("repair").unwrap();
    assert_eq!(repair.next.as_deref(), Some("write_pr"));
    assert_eq!(repair.on_bad.as_deref(), Some("plan"));
    assert_eq!(repair.max_tries, Some(3));
    assert_eq!(repair.role.as_deref(), Some("builder"));
    assert_eq!(repair.reads, ["spec"]);
    for name in graph.steps.keys() {
        let goes = goes_to(&graph, name, "write_pr");
        let want = name == "set_version" || name == "repair";
        assert_eq!(goes, want, "{name}");
    }
}

#[test]
fn check_review_reads_spec_and_pr_and_the_other_parts_keep_their_reads() {
    let graph = default_sheet();
    let reads = |name: &str| graph.step(name).unwrap().reads.clone();
    assert_eq!(reads("check_review"), ["spec", "pr", "lessons"]);
    assert_eq!(reads("check_tests"), ["spec", "tests"]);
    assert_eq!(reads("check_safety"), ["spec", "lessons"]);
    assert_eq!(reads("check_security"), ["spec"]);
    assert_eq!(reads("check_use"), ["spec"]);
}

/// True if a command uses `gh`: as a word, or in a script word.
fn uses_gh(words: &[String]) -> bool {
    let mut all = words.iter();
    all.any(|w| w == "gh" || w.starts_with("gh ") || w.contains(" gh "))
}

#[test]
fn each_command_with_gh_starts_with_the_word_git_dir() {
    let graph = default_sheet();
    let mut count = 0;
    for (name, step) in &graph.steps {
        for words in step.run.iter().filter(|w| uses_gh(w)) {
            count += 1;
            let head: Vec<&str> = words.iter().take(2).map(String::as_str).collect();
            assert_eq!(head, ["env", GIT_DIR], "{name}: {words:?}");
        }
    }
    assert!(count >= 4, "{count} commands with gh");
}

/// All the words of the commands of a step, from command `from` on.
fn words_of(graph: &Graph, name: &str, from: usize) -> Vec<String> {
    let step = graph.step(name).unwrap();
    let run = step.run.iter().skip(from).flatten();
    run.cloned().collect()
}

#[test]
fn find_pr_views_the_pr_first_and_then_puts_the_paper_into_it() {
    let graph = default_sheet();
    let find = graph.step("find_pr").unwrap();
    assert_eq!(find.run[0], VIEW);
    assert!(find.run.len() >= 2, "{:?}", find.run);
    assert_eq!(find.on_bad.as_deref(), Some("open_pr"));
    assert_eq!(find.next.as_deref(), Some("approval"));
    assert!(find.reads.is_empty(), "{:?}", find.reads);
    let later = words_of(&graph, "find_pr", 1);
    for path in [TITLE, BODY] {
        let names = later.iter().any(|w| w.contains(path));
        assert!(names, "{path}: {later:?}");
    }
}

#[test]
fn open_pr_makes_the_pr_of_the_run_branch_with_the_paper() {
    let graph = default_sheet();
    let open = graph.step("open_pr").unwrap();
    assert_eq!(open.next.as_deref(), Some("approval"));
    assert!(open.reads.is_empty(), "{:?}", open.reads);
    let words = words_of(&graph, "open_pr", 0);
    for path in [TITLE, BODY, "codeumbrella/{run}"] {
        let names = words.iter().any(|w| w.contains(path));
        assert!(names, "{path}: {words:?}");
    }
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
    found.join("\n").to_lowercase()
}

#[test]
fn the_step_write_pr_has_a_comment_with_the_writer_and_the_paper() {
    let comment = comment_above("write_pr");
    assert!(comment.contains("writer"), "{comment}");
    assert!(comment.contains("`pr`"), "{comment}");
}

// ---------------------------------------------------------------------
// Runs of the quiet default sheet with a simulated worker
// ---------------------------------------------------------------------

/// What the simulated worker saw in one call of a step.
#[derive(Clone, Debug)]
struct Seen {
    step: String,
    rules: PathBuf,
    reads: Vec<String>,
    writes: Vec<String>,
}

/// A simulated worker. It writes its papers. Each step in `bad` gives one
/// bad result, and each other call gives a good result.
struct Sim {
    bad: Mutex<Vec<&'static str>>,
    seen: Mutex<Vec<Seen>>,
}

impl Sim {
    fn new(bad: &[&'static str]) -> Self {
        Self {
            bad: Mutex::new(bad.to_vec()),
            seen: Mutex::default(),
        }
    }

    /// Each call of the step `step`, in sequence.
    fn calls(&self, step: &str) -> Vec<Seen> {
        let seen = self.seen.lock().unwrap();
        seen.iter().filter(|s| s.step == step).cloned().collect()
    }
}

impl Worker for Sim {
    fn run(&self, job: &Job) -> Result<Outcome> {
        let seen = Seen {
            step: job.step.to_owned(),
            rules: job.rules.to_path_buf(),
            reads: job.reads.to_vec(),
            writes: job.writes.to_vec(),
        };
        self.seen.lock().unwrap().push(seen);
        for paper in job.writes {
            let path = job.dir.join("papers").join(paper);
            fs::write(path.join("file.md"), job.step)?;
        }
        let mut bad = self.bad.lock().unwrap();
        let at = bad.iter().position(|s| *s == job.step);
        if let Some(at) = at {
            bad.remove(at);
        }
        Ok(Outcome {
            ok: at.is_none(),
            summary: format!("{} ran", job.step),
        })
    }
}

/// A run with the quiet default sheet. It goes to `look_at_tests`, gets a
/// yes, and goes on.
fn run_with(sim: &Sim) -> Run {
    let graph = quiet_sheet();
    let dir = folder().join(".codeumbrella/runs/run-1");
    let mut run = Run::start(dir, "a task", &graph).unwrap();
    fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles(), sim).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    run.answer(true, "").unwrap();
    run.go(&graph, &roles(), sim).unwrap();
    run
}

/// The lines of the log: each step with its result.
fn log(run: &Run) -> Vec<(String, bool)> {
    let text = fs::read_to_string(run.dir.join("log.jsonl")).unwrap();
    let mut found = Vec::new();
    for line in text.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        let step = value["step"].as_str().unwrap().to_owned();
        found.push((step, value["ok"].as_bool().unwrap()));
    }
    found
}

/// The steps of the log after the last yes at `look_at_tests`. The fan
/// parts are not in it.
fn after_yes(run: &Run) -> Vec<(String, bool)> {
    let all = log(run);
    let at = all.iter().rposition(|(s, _)| s == "look_at_tests");
    let at = at.unwrap_or_else(|| panic!("no look_at_tests: {all:?}"));
    let after = all[at + 1..].iter();
    let steps = after.filter(|(s, _)| !PARTS.contains(&s.as_str()));
    steps.cloned().collect()
}

fn pairs(list: &[(&str, bool)]) -> Vec<(String, bool)> {
    list.iter().map(|(s, ok)| ((*s).to_owned(), *ok)).collect()
}

/// Checks the job of each call of `write_pr`.
fn writer_jobs(sim: &Sim, times: usize) {
    let calls = sim.calls("write_pr");
    assert_eq!(calls.len(), times, "{calls:?}");
    for call in calls {
        assert!(call.rules.ends_with("writer.md"), "{:?}", call.rules);
        assert_eq!(call.reads, ["spec"]);
        assert_eq!(call.writes, ["pr"]);
    }
}

#[test]
fn a_run_does_write_pr_after_learn_and_before_check() {
    let sim = Sim::new(&[]);
    let run = run_with(&sim);
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    let path = pairs(&[
        ("mark", true),
        ("build", true),
        ("simplify", true),
        ("learn", true),
        ("set_version", true),
        ("write_pr", true),
        ("check", true),
        ("work_was_done", true),
        ("no_locked_files", true),
        ("push", true),
        ("find_pr", true),
    ]);
    assert_eq!(after_yes(&run), path);
    writer_jobs(&sim, 1);
    let plan = sim.calls("plan");
    assert_eq!(plan.len(), 1, "{plan:?}");
    assert_eq!(plan[0].writes, ["spec", "tests", "pr"]);
    let review = sim.calls("check_review");
    assert_eq!(review.len(), 1, "{review:?}");
    assert_eq!(review[0].reads, ["spec", "pr", "lessons"]);
}

#[test]
fn a_repair_goes_to_write_pr_and_then_to_check() {
    let sim = Sim::new(&["check_tests"]);
    let run = run_with(&sim);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    let path = pairs(&[
        ("mark", true),
        ("build", true),
        ("simplify", true),
        ("learn", true),
        ("set_version", true),
        ("write_pr", true),
        ("check", false),
        ("repair", true),
        ("write_pr", true),
        ("check", true),
        ("work_was_done", true),
        ("no_locked_files", true),
        ("push", true),
        ("find_pr", true),
    ]);
    assert_eq!(after_yes(&run), path);
    writer_jobs(&sim, 2);
    assert_eq!(sim.calls("learn").len(), 1);
}

#[test]
fn a_bad_repair_still_goes_to_the_plan() {
    let sim = Sim::new(&["check_tests", "repair"]);
    let run = run_with(&sim);
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    let all = log(&run);
    let at = all.iter().position(|(s, ok)| s == "repair" && !ok);
    let at = at.unwrap_or_else(|| panic!("no bad repair: {all:?}"));
    assert_eq!(all[at + 1], ("plan".to_owned(), true), "{all:?}");
    writer_jobs(&sim, 1);
}

#[test]
fn a_bad_result_of_write_pr_stops_the_run() {
    let sim = Sim::new(&["write_pr"]);
    let run = run_with(&sim);
    assert_eq!(run.state.status, Status::Stopped);
    assert_eq!(run.state.at.as_deref(), Some("write_pr"));
    assert!(run.state.note.contains("write_pr"), "{}", run.state.note);
    assert!(sim.calls("check_tests").is_empty());
    writer_jobs(&sim, 1);
}

#[test]
fn after_a_no_at_approval_the_planner_and_the_writer_write_pr_again() {
    let graph = quiet_sheet();
    let sim = Sim::new(&[]);
    let mut run = run_with(&sim);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    run.answer(false, "Say why.").unwrap();
    run.go(&graph, &roles(), &sim).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    run.answer(true, "").unwrap();
    run.go(&graph, &roles(), &sim).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    let plan = sim.calls("plan");
    assert_eq!(plan.len(), 2, "{plan:?}");
    for call in plan {
        assert_eq!(call.writes, ["spec", "tests", "pr"]);
    }
    writer_jobs(&sim, 2);
}

// ---------------------------------------------------------------------
// The steps `find_pr` and `open_pr` with a simulated `gh`
// ---------------------------------------------------------------------

/// A simulated `gh`. It writes each call into `LOG`: the value of
/// `GIT_DIR`, each word, and the text of each file that a word names.
/// A call that makes a PR makes the file `OPEN`. `pr view` is good only
/// if that file is there.
const GH: &str = r#"#!/bin/sh
echo "CALL GIT_DIR=$GIT_DIR" >> "LOG"
for a in "$@"; do
  echo "ARG $a" >> "LOG"
  for f in "$a" "${a##*@}" "${a#*=}"; do
    if [ -f "$f" ]; then
      cat "$f" >> "LOG"
      echo >> "LOG"
      break
    fi
  done
done
case " $* " in
  *" create "*|*"/pulls "*) touch "OPEN" ;;
esac
if [ "$1 $2" = "pr view" ]; then
  if [ ! -f "OPEN" ]; then
    echo "no pull requests found" >&2
    exit 1
  fi
  case "$*" in
    *.number*) echo 7 ;;
    *) echo https://github.com/o/r/pull/7 ;;
  esac
fi
exit 0
"#;

/// A sheet with the steps `find_pr` and `open_pr`. `FIND_RUN` and
/// `OPEN_RUN` get the commands of these steps of the default sheet.
const SHEET: &str = r"start: wait
steps:
  wait:
    type: person
    max_tries: 3
    next: find_pr
  find_pr:
    type: command
    run: FIND_RUN
    on_bad: open_pr
    next: approval
  open_pr:
    type: command
    run: OPEN_RUN
    max_tries: 2
    next: approval
  approval:
    type: person
    on_bad: wait
";

/// The title of the first try.
const FIRST_TITLE: &str = "Each PR gets a set structure and a checklist";

/// The title of the second try.
const SECOND_TITLE: &str = "The PR text gets a writer and a checklist";

/// A PR text with the 8 parts. `word` is in its first part.
fn pr_text(word: &str) -> String {
    let parts = [
        format!("## Purpose\n\nThe person sees the purpose. {word}\n"),
        "## Cause\n\nThe PR text was the commit text.\n".to_owned(),
        "Example: a PR had a branch name as its title.\n".to_owned(),
        "## How this PR prevents it\n\n- The sheet reads the paper.\n".to_owned(),
        "- The tests get a simulated `gh`.\n".to_owned(),
        "Version 0.1.26.\n".to_owned(),
        "## The sheet\n\n- `graph.example.yaml`\n".to_owned(),
        "## PR text checklist\n\n- [x] Version line.\n".to_owned(),
    ];
    parts.join("\n")
}

/// Starts the built command in `dir`, with `bin` first on the PATH.
fn cu(dir: &Path, bin: &Path, args: &[&str]) -> Output {
    let path = env::var("PATH").unwrap_or_default();
    let path = format!("{}:{path}", bin.display());
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(args).current_dir(dir).env("PATH", path);
    command.output().unwrap()
}

fn all(out: &Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    format!("{stdout}{stderr}")
}

/// Runs `git` in a folder, with a name and an email.
fn git(dir: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    command.args(["-c", "user.name=t", "-c", "user.email=t@t"]);
    command.args(["-c", "commit.gpgsign=false"]);
    let out = command.args(args).output().unwrap();
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "git {args:?}: {error}");
}

/// A folder with the simulated `gh`.
fn bin() -> PathBuf {
    let dir = folder();
    let log = dir.join("gh.log").display().to_string();
    let open = dir.join("open").display().to_string();
    let script = GH.replace("\"LOG\"", &format!("\"{log}\""));
    let script = script.replace("\"OPEN\"", &format!("\"{open}\""));
    let path = dir.join("gh");
    fs::write(&path, script).unwrap();
    let mode = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&path, mode).unwrap();
    dir
}

/// The commands of a step of the default sheet, as a list for a sheet.
fn commands(name: &str) -> String {
    let graph = default_sheet();
    let step = graph.step(name).unwrap();
    serde_json::to_string(&step.run).unwrap()
}

/// The id of the run that an output names.
fn id(out: &Output) -> String {
    let text = all(out);
    let line = text.lines().find(|l| l.starts_with("Run:      "));
    let line = line.unwrap_or_else(|| panic!("no run: {text}"));
    line["Run:      ".len()..].to_owned()
}

/// A project with the test sheet, a folder with the simulated `gh`, and
/// a run that waits at the step `wait`.
fn start() -> (PathBuf, PathBuf, String) {
    let dir = folder();
    let bin = bin();
    let out = cu(&dir, &bin, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    fs::write(dir.join("keep.txt"), "x").unwrap();
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "--quiet", "-m", "first"]);
    let sheet = SHEET.replace("FIND_RUN", &commands("find_pr"));
    let sheet = sheet.replace("OPEN_RUN", &commands("open_pr"));
    fs::write(dir.join(".codeumbrella/graph.yaml"), sheet).unwrap();
    let run = cu(&dir, &bin, &["run", "a task"]);
    let printed = all(&run);
    assert!(printed.contains("Status:   Waiting"), "{printed}");
    let id = id(&run);
    (dir, bin, id)
}

/// Writes the two files of the paper `pr` of the run `id`.
fn write_paper(dir: &Path, id: &str, title: &str, body: &str) {
    let paper = dir.join(".codeumbrella/runs").join(id).join("papers/pr");
    fs::create_dir_all(&paper).unwrap();
    fs::write(paper.join("title.txt"), format!("{title}\n")).unwrap();
    fs::write(paper.join("body.md"), body).unwrap();
}

/// The text that the simulated `gh` wrote. Empty if it had no call.
fn gh_log(bin: &Path) -> String {
    fs::read_to_string(bin.join("gh.log")).unwrap_or_default()
}

/// Checks that each call of `gh` read the clean copy of the run `id`.
fn reads_the_clean_copy(log: &str, id: &str) {
    let want = format!("CALL GIT_DIR=.codeumbrella/runs/{id}/clean/.git");
    let calls: Vec<&str> = log.lines().filter(|l| l.starts_with("CALL ")).collect();
    assert!(!calls.is_empty(), "no call of gh: {log}");
    for call in calls {
        assert_eq!(call, want, "{log}");
    }
}

/// The names of the steps of the log of a run, with each result.
fn logged(dir: &Path, id: &str) -> Vec<(String, bool)> {
    let path = dir.join(".codeumbrella/runs").join(id).join("log.jsonl");
    let log = fs::read_to_string(path).unwrap();
    let mut found = Vec::new();
    for line in log.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        let step = value["step"].as_str().unwrap().to_owned();
        found.push((step, value["ok"].as_bool().unwrap()));
    }
    found
}

/// Sends a yes at `wait`. The run must then wait at `approval`.
fn yes_at_wait(dir: &Path, bin: &Path, id: &str) {
    let yes = cu(dir, bin, &["answer", id, "wait", "yes"]);
    let printed = all(&yes);
    assert!(printed.contains("Status:   Waiting"), "{printed}");
    assert!(printed.contains("approval"), "{printed}");
}

#[test]
fn the_first_try_opens_the_pr_with_the_title_and_the_text_of_the_paper() {
    let (dir, bin, id) = start();
    write_paper(&dir, &id, FIRST_TITLE, &pr_text("first-try"));
    yes_at_wait(&dir, &bin, &id);
    let log = gh_log(&bin);
    assert!(log.contains(FIRST_TITLE), "{log}");
    assert!(log.contains(&pr_text("first-try")), "{log}");
    assert!(log.contains(&format!("codeumbrella/{id}")), "{log}");
    assert!(!log.contains("--fill"), "{log}");
    reads_the_clean_copy(&log, &id);
    let steps = logged(&dir, &id);
    let open = ("open_pr".to_owned(), true);
    assert!(steps.contains(&open), "{steps:?}");
}

#[test]
fn after_a_no_the_second_try_puts_the_new_text_into_the_open_pr() {
    let (dir, bin, id) = start();
    write_paper(&dir, &id, FIRST_TITLE, &pr_text("first-try"));
    yes_at_wait(&dir, &bin, &id);
    // GitHub has the PR of the run at this time.
    fs::write(bin.join("open"), "").unwrap();
    fs::remove_file(bin.join("gh.log")).unwrap();
    let why = "Say why.";
    let no = cu(&dir, &bin, &["answer", &id, "approval", "no", why]);
    let printed = all(&no);
    assert!(printed.contains("Status:   Waiting"), "{printed}");
    write_paper(&dir, &id, SECOND_TITLE, &pr_text("second-try"));
    yes_at_wait(&dir, &bin, &id);
    let log = gh_log(&bin);
    assert!(log.contains(SECOND_TITLE), "{log}");
    assert!(log.contains(&pr_text("second-try")), "{log}");
    assert!(!log.contains(FIRST_TITLE), "{log}");
    assert!(!log.contains("first-try"), "{log}");
    reads_the_clean_copy(&log, &id);
    // The second try makes no new PR.
    for line in log.lines() {
        assert_ne!(line, "ARG create", "{log}");
        let makes = line.starts_with("ARG ") && line.ends_with("/pulls");
        assert!(!makes, "{log}");
    }
    let steps = logged(&dir, &id);
    let opens = steps.iter().filter(|(s, _)| s == "open_pr").count();
    assert_eq!(opens, 1, "{steps:?}");
    let last = steps.iter().rev().find(|(s, _)| s == "find_pr");
    assert_eq!(last, Some(&("find_pr".to_owned(), true)), "{steps:?}");
}

// ---------------------------------------------------------------------
// A link in the paper `pr` gives no text to `gh`
// ---------------------------------------------------------------------

/// The text of a file of the person outside the project.
const SECRET: &str = "SECRET-KEY-of-the-person";

/// A file outside the project with the secret text.
fn secret() -> PathBuf {
    let path = folder().join("id_key");
    fs::write(&path, format!("{SECRET}\n")).unwrap();
    path
}

/// The folder of the paper `pr` of the run `id`.
fn paper_dir(dir: &Path, id: &str) -> PathBuf {
    let runs = dir.join(".codeumbrella/runs");
    runs.join(id).join("papers/pr")
}

/// Sends a yes at `wait`. Then checks that `gh` got no secret text and
/// made or changed no PR, and that the steps of the log from `from` on
/// have no good `find_pr` or `open_pr`, and a bad `open_pr`.
fn refused(dir: &Path, bin: &Path, id: &str, from: usize) {
    let yes = cu(dir, bin, &["answer", id, "wait", "yes"]);
    let printed = all(&yes);
    assert!(!printed.contains(SECRET), "{printed}");
    let log = gh_log(bin);
    assert!(!log.contains(SECRET), "{log}");
    for line in log.lines() {
        for word in ["ARG create", "ARG edit", "ARG api"] {
            assert_ne!(line, word, "{log}");
        }
        let makes = line.starts_with("ARG ") && line.contains("/pulls");
        assert!(!makes, "{log}");
    }
    let steps = logged(dir, id);
    let later = &steps[from..];
    let bad = ("open_pr".to_owned(), false);
    assert!(later.contains(&bad), "{steps:?}");
    for (step, ok) in later {
        let pr = step == "open_pr" || step == "find_pr";
        let good = pr && *ok;
        assert!(!good, "{steps:?}");
    }
}

#[test]
fn a_title_that_is_a_symbolic_link_gives_a_bad_step() {
    let (dir, bin, id) = start();
    write_paper(&dir, &id, FIRST_TITLE, &pr_text("first-try"));
    let title = paper_dir(&dir, &id).join("title.txt");
    fs::remove_file(&title).unwrap();
    symlink(secret(), &title).unwrap();
    refused(&dir, &bin, &id, 0);
}

#[test]
fn a_text_that_is_a_hard_link_gives_a_bad_step() {
    let (dir, bin, id) = start();
    write_paper(&dir, &id, FIRST_TITLE, &pr_text("first-try"));
    let body = paper_dir(&dir, &id).join("body.md");
    fs::remove_file(&body).unwrap();
    fs::hard_link(secret(), &body).unwrap();
    refused(&dir, &bin, &id, 0);
}

#[test]
fn a_paper_folder_that_is_a_symbolic_link_gives_a_bad_step() {
    let (dir, bin, id) = start();
    let other = folder();
    let line = format!("{SECRET}\n");
    fs::write(other.join("title.txt"), &line).unwrap();
    fs::write(other.join("body.md"), &line).unwrap();
    let paper = paper_dir(&dir, &id);
    let _ = fs::remove_dir_all(&paper);
    fs::create_dir_all(paper.parent().unwrap()).unwrap();
    symlink(&other, &paper).unwrap();
    refused(&dir, &bin, &id, 0);
}

#[test]
fn after_a_no_a_linked_title_gives_no_text_to_the_open_pr() {
    let (dir, bin, id) = start();
    write_paper(&dir, &id, FIRST_TITLE, &pr_text("first-try"));
    yes_at_wait(&dir, &bin, &id);
    // GitHub has the PR of the run at this time.
    fs::write(bin.join("open"), "").unwrap();
    fs::remove_file(bin.join("gh.log")).unwrap();
    let why = "Say why.";
    let no = cu(&dir, &bin, &["answer", &id, "approval", "no", why]);
    let printed = all(&no);
    assert!(printed.contains("Status:   Waiting"), "{printed}");
    let from = logged(&dir, &id).len();
    write_paper(&dir, &id, SECOND_TITLE, &pr_text("second-try"));
    let title = paper_dir(&dir, &id).join("title.txt");
    fs::remove_file(&title).unwrap();
    symlink(secret(), &title).unwrap();
    refused(&dir, &bin, &id, from);
    let steps = logged(&dir, &id);
    let bad = ("find_pr".to_owned(), false);
    assert!(steps[from..].contains(&bad), "{steps:?}");
}

// ---------------------------------------------------------------------
// The walls of the writer and of the planner
// ---------------------------------------------------------------------

/// True if a deny rule with a full path holds the folder of the paper.
fn covers_the_paper(rule: &str) -> bool {
    let Some(path) = rule.strip_prefix("Edit(/") else {
        return false;
    };
    let path = path.trim_end_matches(')').trim_end_matches("**");
    PR_PATH.starts_with(path.trim_end_matches('/'))
}

#[test]
fn the_claude_writer_has_the_walls_of_the_reviewer_and_one_write_entry() {
    let writer = json("roles/writer.claude.json");
    let reviewer = json("roles/reviewer.claude.json");
    assert_eq!(writer["sandbox"], reviewer["sandbox"]);
    assert_eq!(writer["disableAllHooks"], true);
    let allow = list(&writer["permissions"]["allow"]);
    let is_edit = |a: &&str| a.starts_with("Edit") || a.starts_with("Write");
    let edits: Vec<&str> = allow.iter().copied().filter(is_edit).collect();
    assert_eq!(edits, [CLAUDE_ENTRY], "{allow:?}");
    let deny = list(&writer["permissions"]["deny"]);
    let theirs = list(&reviewer["permissions"]["deny"]);
    for rule in theirs.iter().filter(|r| **r != ALL_DENY) {
        assert!(deny.contains(rule), "{rule}: {deny:?}");
    }
    assert!(deny.contains(&WORK_DENY), "{deny:?}");
    for rule in &deny {
        assert!(!covers_the_paper(rule), "{rule}");
    }
}

#[test]
fn the_codex_writer_is_the_reviewer_with_one_write_entry() {
    let writer = text("roles/writer.codex.txt");
    let reviewer = text("roles/reviewer.codex.txt");
    assert_eq!(writer.matches("\"write\"").count(), 1, "{writer}");
    assert!(writer.contains(CODEX_ENTRY), "{writer}");
    let before = format!("{CODEX_ENTRY}, ");
    let after = format!(", {CODEX_ENTRY}");
    let without = writer.replacen(&before, "", 1).replacen(&after, "", 1);
    assert_eq!(without, reviewer);
}

#[test]
fn the_planner_can_write_the_paper_pr_and_has_no_shell() {
    let file = json("roles/planner.claude.json");
    let allow = list(&file["permissions"]["allow"]);
    assert!(allow.contains(&CLAUDE_ENTRY), "{allow:?}");
    let deny = list(&file["permissions"]["deny"]);
    assert!(deny.contains(&"Bash"), "{deny:?}");
    let codex = text("roles/planner.codex.txt");
    assert!(codex.contains(CODEX_ENTRY), "{codex}");
    assert!(codex.starts_with("{extends=\":read-only\""), "{codex}");
}

#[test]
fn init_puts_the_three_files_of_the_writer_into_a_project() {
    let dir = folder();
    let out = cu(&dir, &bin(), &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    for rel in FILES {
        let path = dir.join(".codeumbrella").join(rel);
        let read = fs::read_to_string(&path);
        let copy = read.unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(copy, text(rel), "{rel}");
    }
}

// ---------------------------------------------------------------------
// The rules pages
// ---------------------------------------------------------------------

#[test]
fn the_writer_page_has_the_checklist_word_for_word_and_the_marks() {
    let page = text("roles/writer.md");
    assert!(page.contains("## PR text checklist"), "{page}");
    for item in CHECKLIST {
        assert!(page.contains(item), "{item}");
    }
    for word in ["[x]", "[ ]", "body.md", "`pr`", "git"] {
        assert!(page.contains(word), "{word}");
    }
}

#[test]
fn the_writer_page_reads_the_project_rules_and_the_lessons() {
    let page = text("roles/writer.md");
    let line = page.lines().find(|l| l.contains(FIRST));
    let line = line.unwrap_or_else(|| panic!("no line: {page}"));
    assert!(line.contains(D1), "{line}");
    assert!(line.contains(D2), "{line}");
}

#[test]
fn the_planner_page_has_the_title_and_parts_1_to_5() {
    let page = text("roles/planner.md");
    let words = [
        "`pr`",
        "title.txt",
        "body.md",
        "72",
        "## Purpose",
        "## Cause",
        "Example:",
        "## How this PR prevents it",
    ];
    for word in words {
        assert!(page.contains(word), "{word}");
    }
}

#[test]
fn the_reviewer_page_compares_the_paper_pr_with_the_change() {
    let page = text("roles/reviewer.md");
    assert!(page.contains("`pr`"), "{page}");
    let line = page.lines().find(|l| l.contains("[x]"));
    let line = line.unwrap_or_else(|| panic!("no [x]: {page}"));
    assert!(line.contains("ok: false"), "{line}");
}

// ---------------------------------------------------------------------
// The documents and the version
// ---------------------------------------------------------------------

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

#[test]
fn agents_md_has_the_paper_pr_and_the_writer_in_its_parts() {
    let words = section("AGENTS.md", "Words");
    for word in ["`pr`", "writer", "title.txt", "body.md"] {
        assert!(words.contains(word), "Words: {word}");
    }
    let structure = section("AGENTS.md", "Structure");
    let files = [FILES[0], FILES[1], FILES[2], "tests/pr_text.rs"];
    for file in files {
        let name = format!("`{file}`");
        assert!(structure.contains(&name), "Structure: {file}");
    }
    let sheet = section("AGENTS.md", "The sheet");
    assert!(sheet.contains("`write_pr`"), "The sheet: write_pr");
    let rules = section("AGENTS.md", "Safety rules");
    for word in ["`write_pr`", "writer", "`open_pr`", "`find_pr`"] {
        assert!(rules.contains(word), "Safety rules: {word}");
    }
    let old = "The planner can write only to the two papers of its run.";
    assert!(!rules.contains(old), "{old}");
}

#[test]
fn the_readme_names_the_writer_and_the_checklist() {
    let readme = text("README.md");
    for word in ["`writer`", "`write_pr`", "checklist"] {
        assert!(readme.contains(word), "README.md: {word}");
    }
}

/// The position of the first line that starts with `start`.
fn line_at(lines: &[&str], start: &str) -> usize {
    let at = lines.iter().position(|l| l.starts_with(start));
    at.unwrap_or_else(|| panic!("no line {start}"))
}

#[test]
fn the_architecture_shows_the_writer_and_write_pr_after_learn() {
    let parts = section("ARCHITECTURE.md", "The parts");
    let box_line = parts.lines().any(|l| l.starts_with("│ writer"));
    assert!(box_line, "no line of the writer: {parts}");
    let path = section("ARCHITECTURE.md", "The path of a task");
    let lines: Vec<&str> = path.lines().collect();
    let learn = line_at(&lines, "learn");
    let write = line_at(&lines, "write_pr");
    let check = line_at(&lines, "check:");
    assert!(learn < write && write < check, "{path}");
    let both = |l: &&str| l.contains("repair") && l.contains("write_pr");
    assert!(lines.iter().any(both), "no repair to write_pr: {path}");
}

/// The list items and the paragraphs of a page.
fn items(page: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut open = false;
    for line in page.lines() {
        let new = line.starts_with("- ") || line.starts_with('#');
        if line.trim().is_empty() {
            open = false;
        } else if open && !new {
            if let Some(last) = found.last_mut() {
                last.push(' ');
                last.push_str(line.trim());
            }
        } else {
            found.push(line.to_owned());
            open = true;
        }
    }
    found
}

#[test]
fn the_chat_shows_the_title_and_each_open_item_at_approval() {
    let part = section(SKILL, "After each command");
    let all = items(&part);
    let item = all.iter().find(|i| i.contains("title.txt"));
    let item = item.unwrap_or_else(|| panic!("no item: {part}"));
    assert!(item.contains("approval"), "{item}");
    assert!(item.contains("[ ]"), "{item}");
}
