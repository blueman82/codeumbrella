//! Tests of the step `clean_up`: the default sheet, runs with the built
//! command in a temp project, and the documents. No Claude, no Codex and
//! no `gh`.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::{Graph, Kind};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// The one command of the step `clean_up`.
const CLEAN: [&str; 4] = [
    "rm",
    "-r",
    ".codeumbrella/runs/{run}/work",
    ".codeumbrella/runs/{run}/clean",
];

/// The folder of a different run in a test project.
const OTHER: &str = ".codeumbrella/runs/run-other";

/// A command that makes the folder `clean` of the run.
const MAKE_CLEAN: [&str; 2] = ["mkdir", ".codeumbrella/runs/{run}/clean"];

/// A command that makes the file `new-work.bundle` of the run.
const MAKE_BUNDLE: [&str; 2] = ["touch", ".codeumbrella/runs/{run}/new-work.bundle"];

/// The row of "Paper" in the part "Words" of `AGENTS.md`.
const PAPER: &str = concat!(
    "| Paper | A folder in `.codeumbrella/runs/<run-id>/papers/` ",
    "that one step writes and other steps read. |"
);

/// The row of "Notebook" in the part "Words" of `AGENTS.md`.
const NOTEBOOK: &str = concat!(
    "| Notebook | `.codeumbrella/runs/<run-id>/state.json`: ",
    "where the run is, and the tool of the run. ",
    "`since` is the start time of the step of this moment. ",
    "`total` has the sum of the numbers of the workers: ",
    "`workers`, `seconds`, `dollars`, `tokens_in`, ",
    "`tokens_out` and `no_cost`. |"
);

/// The first cell of each row of the part "Words" of `AGENTS.md`.
const WORDS: [&str; 17] = [
    "Sheet",
    "Step",
    "Fan",
    "Supervisor",
    "Tool part",
    "Notebook",
    "Tool of the run",
    "Log",
    "`worker.<number>`",
    "`trace`",
    "Paper",
    "Paper `pr`",
    "Lesson",
    "`.codeumbrella/lessons.md`",
    "Role",
    "Writer",
    "Queue",
];

/// The words of point J of the task.
const NOT_PERMITTED: [&str; 40] = [
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
];

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
    let name = format!("cu-clean-{}-{n}", std::process::id());
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
fn save_lessons_goes_to_clean_up_and_the_run_ends_after_it() {
    let graph = default_sheet();
    let merge = graph.step("merge").unwrap();
    assert_eq!(merge.next.as_deref(), Some("save_lessons"));
    let save = graph.step("save_lessons").unwrap();
    assert_eq!(save.next.as_deref(), Some("clean_up"));
    assert!(save.on_bad.is_none(), "{:?}", save.on_bad);
    let clean = graph.step("clean_up").unwrap();
    assert_eq!(clean.kind, Kind::Command);
    assert_eq!(clean.run.len(), 1, "{:?}", clean.run);
    assert!(clean.next.is_none(), "{:?}", clean.next);
    assert!(clean.on_bad.is_none(), "{:?}", clean.on_bad);
    assert!(clean.max_tries.is_none(), "{:?}", clean.max_tries);
    assert!(clean.reads.is_empty() && clean.writes.is_empty());
    assert!(clean.role.is_none() && clean.fan.is_empty());
    assert_ne!(graph.start, "clean_up");
    // Only `save_lessons` goes to `clean_up`.
    for name in graph.steps.keys() {
        let goes = goes_to(&graph, name, "clean_up");
        assert_eq!(goes, name == "save_lessons", "{name}");
    }
}

#[test]
fn clean_up_names_only_the_two_copies_of_its_own_run() {
    let graph = default_sheet();
    let clean = graph.step("clean_up").unwrap();
    assert_eq!(clean.run, [CLEAN]);
    let words = &clean.run[0];
    // The only option is `-r`. There is no `-f`.
    let options = words.iter().filter(|w| w.starts_with('-'));
    let options: Vec<&String> = options.collect();
    assert_eq!(options, ["-r"], "{words:?}");
    // Each path is a full path in the folder of the run, with its id.
    for path in &words[2..] {
        let name = path.strip_prefix(".codeumbrella/runs/{run}/");
        let name = name.unwrap_or_else(|| panic!("{path}"));
        assert!(["work", "clean"].contains(&name), "{path}");
    }
    // No other step of the sheet removes a file or a folder.
    for (name, step) in &graph.steps {
        let mut all = step.run.iter().flatten();
        let removes = all.any(|w| w == "rm" || w == "rmdir");
        assert_eq!(removes, name == "clean_up", "{name}");
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
fn the_comment_of_clean_up_tells_what_it_does_and_how_to_keep_the_copies() {
    let comment = comment_above("clean_up");
    let words = [
        "`work`",
        "`clean`",
        "master",
        "remove",
        "keep",
        "next: clean_up",
        "stopped",
    ];
    for word in words {
        assert!(comment.contains(word), "{word}: {comment}");
    }
}

// ---------------------------------------------------------------------
// A run of the quiet default sheet with a simulated worker
// ---------------------------------------------------------------------

/// A simulated worker. It writes its papers and gives a good result.
struct Good;

impl Worker for Good {
    fn run(&self, job: &Job) -> Result<Outcome> {
        for paper in job.writes {
            let path = job.dir.join("papers").join(paper);
            fs::write(path.join("file.md"), job.step)?;
        }
        Ok(Outcome {
            ok: true,
            summary: format!("{} ran", job.step),
        })
    }
}

#[test]
fn a_run_of_the_quiet_default_sheet_ends_after_clean_up() {
    let graph = quiet_sheet();
    let dir = folder().join(".codeumbrella/runs/run-1");
    let mut run = Run::start(dir, "a task", &graph).unwrap();
    fs::write(run.dir.join("work/file"), "x").unwrap();
    for at in ["look_at_tests", "approval"] {
        run.go(&graph, &roles(), &Good).unwrap();
        assert_eq!(run.state.at.as_deref(), Some(at));
        run.answer(true, "").unwrap();
    }
    run.go(&graph, &roles(), &Good).unwrap();
    assert_eq!(run.state.status, Status::Done);
    let log = fs::read_to_string(run.dir.join("log.jsonl")).unwrap();
    let mut steps = Vec::new();
    for line in log.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        steps.push(value["step"].as_str().unwrap().to_owned());
    }
    let last = &steps[steps.len() - 3..];
    let want = ["merge", "save_lessons", "clean_up"];
    assert_eq!(last, want, "{steps:?}");
}

// ---------------------------------------------------------------------
// Runs with the built command in a temp project
// ---------------------------------------------------------------------

/// Starts the built command in `dir`.
fn cu(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(args).current_dir(dir);
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

/// A new project: a git repo with the factory files and one file.
fn project() -> PathBuf {
    let dir = folder();
    let out = cu(&dir, &["init"]);
    assert!(out.status.success(), "{}", all(&out));
    fs::write(dir.join("keep.txt"), "x").unwrap();
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "--quiet", "-m", "first"]);
    dir
}

/// A sheet with six steps. `COPY` and `CLEAN` get the commands of the
/// steps `copy` and `clean_up` of the default sheet. `MAKE`, `MERGE` and
/// `SAVE` get the commands of a test.
const SHEET: &str = r"start: copy
steps:
  copy:
    type: command
    run: COPY
    next: make
  make:
    type: command
    run: MAKE
    next: approval
  approval:
    type: person
    next: merge
  merge:
    type: command
    run: MERGE
    next: save_lessons
  save_lessons:
    type: command
    run: SAVE
    next: clean_up
  clean_up:
    type: command
    run: CLEAN
";

/// The commands of a step of the default sheet, as a list for a sheet.
fn commands(name: &str) -> String {
    let graph = default_sheet();
    let step = graph.step(name).unwrap();
    serde_json::to_string(&step.run).unwrap()
}

/// A list of commands for a sheet.
fn list(run: &[&[&str]]) -> String {
    serde_json::to_string(run).unwrap()
}

/// The test sheet. `make` and `merge` are the commands of the steps
/// `make` and `merge`. The step `save_lessons` does nothing.
fn sheet(make: &str, merge: &str) -> String {
    let quiet = list(&[&["true"]]);
    let sheet = SHEET.replace("COPY", &commands("copy"));
    let sheet = sheet.replace("MAKE", make);
    let sheet = sheet.replace("MERGE", merge);
    let sheet = sheet.replace("SAVE", &quiet);
    sheet.replace("CLEAN", &commands("clean_up"))
}

/// The id of the run that an output names.
fn id(out: &Output) -> String {
    let text = all(out);
    let line = text.lines().find(|l| l.starts_with("Run:      "));
    let line = line.unwrap_or_else(|| panic!("no run: {text}"));
    line["Run:      ".len()..].to_owned()
}

/// The folder of the run `id` in the project `dir`.
fn run_dir(dir: &Path, id: &str) -> PathBuf {
    dir.join(".codeumbrella/runs").join(id)
}

/// A project with the test sheet and the two copies of a different run.
/// It starts a run, and gives the project folder and the id of the run.
fn start(make: &str, merge: &str) -> (PathBuf, String) {
    let dir = project();
    for copy in ["work", "clean"] {
        let other = dir.join(OTHER).join(copy);
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("file"), "x").unwrap();
    }
    let path = dir.join(".codeumbrella/graph.yaml");
    fs::write(path, sheet(make, merge)).unwrap();
    let run = cu(&dir, &["run", "a task"]);
    let printed = all(&run);
    assert!(printed.contains("Status:   Waiting"), "{printed}");
    let id = id(&run);
    (dir, id)
}

/// The steps of the log of a run, each with its result.
fn logged(dir: &Path, id: &str) -> Vec<(String, bool)> {
    let path = run_dir(dir, id).join("log.jsonl");
    let log = fs::read_to_string(path).unwrap();
    let mut found = Vec::new();
    for line in log.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        let step = value["step"].as_str().unwrap().to_owned();
        found.push((step, value["ok"].as_bool().unwrap()));
    }
    found
}

/// The names of the steps of the log of a run.
fn names(dir: &Path, id: &str) -> Vec<String> {
    let steps = logged(dir, id);
    steps.into_iter().map(|(s, _)| s).collect()
}

#[test]
fn after_clean_up_the_copies_are_gone_and_the_record_stays() {
    let make = list(&[&MAKE_CLEAN, &MAKE_BUNDLE]);
    let merge = list(&[&["true"]]);
    let (dir, id) = start(&make, &merge);
    let run = run_dir(&dir, &id);
    assert!(run.join("work/.git").is_dir(), "no work copy");
    assert!(run.join("clean").is_dir(), "no clean copy");
    let yes = cu(&dir, &["answer", &id, "approval", "yes"]);
    let printed = all(&yes);
    assert!(printed.contains("Status:   Done"), "{printed}");
    assert!(!run.join("work").exists(), "work is there");
    assert!(!run.join("clean").exists(), "clean is there");
    for name in ["state.json", "log.jsonl", "new-work.bundle"] {
        assert!(run.join(name).is_file(), "{name}");
    }
    assert!(run.join("papers").is_dir(), "papers");
    let steps = logged(&dir, &id);
    let last = steps.last().unwrap();
    assert_eq!(last, &("clean_up".to_owned(), true), "{steps:?}");
}

#[test]
fn clean_up_keeps_the_project_and_the_copies_of_a_different_run() {
    let make = list(&[&MAKE_CLEAN]);
    let merge = list(&[&["true"]]);
    let (dir, id) = start(&make, &merge);
    let yes = cu(&dir, &["answer", &id, "approval", "yes"]);
    let printed = all(&yes);
    assert!(printed.contains("Status:   Done"), "{printed}");
    for copy in ["work", "clean"] {
        let file = dir.join(OTHER).join(copy).join("file");
        assert!(file.is_file(), "{}", file.display());
    }
    assert!(dir.join("keep.txt").is_file(), "keep.txt");
    assert!(dir.join(".git").is_dir(), ".git");
    let sheet = dir.join(".codeumbrella/graph.yaml");
    assert!(sheet.is_file(), "graph.yaml");
    assert!(run_dir(&dir, &id).is_dir(), "the run folder");
}

#[test]
fn a_run_that_stops_at_the_merge_keeps_its_copies() {
    let make = list(&[&MAKE_CLEAN]);
    let merge = list(&[&["false"]]);
    let (dir, id) = start(&make, &merge);
    let yes = cu(&dir, &["answer", &id, "approval", "yes"]);
    let printed = all(&yes);
    assert!(printed.contains("Status:   Stopped"), "{printed}");
    let run = run_dir(&dir, &id);
    assert!(run.join("work/.git").is_dir(), "work is gone");
    assert!(run.join("clean").is_dir(), "clean is gone");
    let steps = names(&dir, &id);
    assert!(!steps.contains(&"save_lessons".to_owned()), "{steps:?}");
    assert!(!steps.contains(&"clean_up".to_owned()), "{steps:?}");
}

#[test]
fn a_no_at_the_approval_keeps_the_copies() {
    let make = list(&[&MAKE_CLEAN]);
    let merge = list(&[&["true"]]);
    let (dir, id) = start(&make, &merge);
    let run = run_dir(&dir, &id);
    // The run waits for the person. The copies are there.
    assert!(run.join("work/.git").is_dir(), "work is gone");
    let no = cu(&dir, &["answer", &id, "approval", "no"]);
    let printed = all(&no);
    assert!(printed.contains("Status:   Stopped"), "{printed}");
    assert!(run.join("work/.git").is_dir(), "work is gone");
    assert!(run.join("clean").is_dir(), "clean is gone");
    let steps = names(&dir, &id);
    assert!(!steps.contains(&"clean_up".to_owned()), "{steps:?}");
}

#[test]
fn with_no_folder_clean_the_step_is_bad_and_the_run_stops() {
    let make = list(&[&MAKE_BUNDLE]);
    let merge = list(&[&["true"]]);
    let (dir, id) = start(&make, &merge);
    let run = run_dir(&dir, &id);
    assert!(!run.join("clean").exists(), "clean is there");
    let yes = cu(&dir, &["answer", &id, "approval", "yes"]);
    let printed = all(&yes);
    assert!(printed.contains("Status:   Stopped"), "{printed}");
    let note = printed.lines().find(|l| l.starts_with("Note:     "));
    let note = note.unwrap_or_else(|| panic!("no note: {printed}"));
    assert!(note.contains("clean_up"), "{note}");
    let steps = logged(&dir, &id);
    let last = steps.last().unwrap();
    assert_eq!(last, &("clean_up".to_owned(), false), "{steps:?}");
    // The merge was good before the stop.
    let merge = ("merge".to_owned(), true);
    assert!(steps.contains(&merge), "{steps:?}");
    // The record of the run stays.
    for name in ["state.json", "log.jsonl", "new-work.bundle"] {
        assert!(run.join(name).is_file(), "{name}");
    }
}

// ---------------------------------------------------------------------
// The documents
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

/// The words of a text: letters and digits, split at each other character.
fn words(text: &str) -> Vec<&str> {
    let parts = text.split(|c: char| !c.is_alphanumeric());
    parts.filter(|w| !w.is_empty()).collect()
}

/// A text with no part in code marks.
fn plain(text: &str) -> String {
    let parts = text.split('`').step_by(2);
    parts.collect::<Vec<&str>>().join(" ")
}

/// A line with no list mark at its start.
fn body(line: &str) -> &str {
    let line = line.trim_start();
    line.strip_prefix("- ").unwrap_or(line)
}

/// Checks a text of the task: no semicolon, none of the words of point
/// J, and 20 words at the maximum in a sentence.
fn plain_and_short(text: &str) {
    let lower = plain(text).to_lowercase();
    assert!(!lower.contains(';'), "a semicolon: {text}");
    let found = words(&lower);
    for bad in NOT_PERMITTED {
        assert!(!found.contains(&bad), "{bad}: {text}");
    }
    for sentence in body(text).split(". ") {
        let count = sentence.split_whitespace().count();
        assert!(count <= 20, "{count} words: {sentence}");
    }
}

#[test]
fn agents_md_has_a_safety_line_for_clean_up_with_the_date() {
    let rules = section("AGENTS.md", "Safety rules");
    let date = "19 November 2026";
    let mut lines = rules.lines().filter(|l| l.contains("clean_up"));
    let line = lines.find(|l| l.contains(date));
    let line = line.unwrap_or_else(|| panic!("no line: {rules}"));
    plain_and_short(line);
}

/// The first cell of a row of a table.
fn first_cell(row: &str) -> &str {
    let cell = row.split('|').nth(1).unwrap_or_default();
    cell.trim()
}

#[test]
fn the_words_of_agents_md_keep_paper_and_notebook_and_get_no_new_word() {
    let part = section("AGENTS.md", "Words");
    for row in [PAPER, NOTEBOOK] {
        assert!(part.lines().any(|l| l == row), "{row}");
    }
    let rows = part.lines().filter(|l| l.starts_with("| "));
    let cells: Vec<&str> = rows.map(first_cell).collect();
    let found: Vec<&str> = cells.into_iter().filter(|c| *c != "Word").collect();
    assert_eq!(found, WORDS);
}

#[test]
fn the_readme_names_clean_up_at_the_yes_for_the_pr() {
    let readme = text("README.md");
    let row = readme.lines().find(|l| l.starts_with("| The PR |"));
    let row = row.unwrap_or_else(|| panic!("README.md: no row The PR"));
    assert!(row.contains("`clean_up`"), "{row}");
    let lower = plain(row).to_lowercase();
    let found = words(&lower);
    for bad in NOT_PERMITTED {
        assert!(!found.contains(&bad), "{bad}: {row}");
    }
}

/// The position of the first line that starts with `start`.
fn line_at(lines: &[&str], start: &str) -> usize {
    let at = lines.iter().position(|l| l.starts_with(start));
    at.unwrap_or_else(|| panic!("no line {start}"))
}

#[test]
fn the_architecture_shows_clean_up_after_save_lessons() {
    let path = section("ARCHITECTURE.md", "The path of a task");
    let lines: Vec<&str> = path.lines().collect();
    let save = line_at(&lines, "save_lessons");
    let clean = line_at(&lines, "clean_up");
    assert!(save < clean, "{path}");
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
fn the_chat_tells_at_done_that_the_copies_are_gone_and_the_record_stays() {
    let page = text(SKILL);
    let all = items(&page);
    let done = all.iter().find(|i| i.starts_with("- Status `Done`"));
    let done = done.unwrap_or_else(|| panic!("SKILL.md: no item Done"));
    let marks = ["`work`", "`clean`", "`state.json`", "`log.jsonl`"];
    for word in marks {
        assert!(done.contains(word), "{word}: {done}");
    }
    assert!(done.contains("git pull"), "{done}");
    plain_and_short(done);
}
