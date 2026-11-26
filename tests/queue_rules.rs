//! Tests of the line `Purpose:` of `queue`, of the line `skip_if` of a person
//! step, of the tests question of the default sheet, of the chat page and of
//! the documents. The worker is simulated. No Claude, no Codex, no `gh`.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::Graph;
use serde_json::Value;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

const SKILL: &str = "skills/codeumbrella/SKILL.md";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let read = fs::read_to_string(root().join(rel));
    read.unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// A new empty folder, with its full path.
fn folder() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-rules-{}-{n}", std::process::id());
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

// ---------------------------------------------------------------------
// The command `queue`
// ---------------------------------------------------------------------

fn cu(dir: &Path, args: &[&str]) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    let out = command.args(args).current_dir(dir).output().unwrap();
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{error}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn queue_shows_the_purpose_line_after_the_files_line() {
    let dir = folder();
    cu(&dir, &["init"]);
    let queue = dir.join(".codeumbrella/queue");
    let first = "Title\n\nPurpose: less work.\n\nFiles: a.rs\n";
    fs::write(queue.join("01-a.txt"), first).unwrap();
    fs::write(queue.join("02-b.txt"), "Files: b.rs").unwrap();
    fs::write(queue.join("03-c.txt"), "Purpose: more.").unwrap();
    let printed = cu(&dir, &["queue"]);
    let lines: Vec<&str> = printed.lines().collect();
    let want = [
        "01-a.txt  Files: a.rs",
        "  Purpose: less work.",
        "02-b.txt  Files: b.rs",
        "03-c.txt  Files: not given",
        "  Purpose: more.",
    ];
    assert_eq!(lines, want);
}

// ---------------------------------------------------------------------
// The line `skip_if` of a person step
// ---------------------------------------------------------------------

const ASK: &str = r#"start: one
steps:
  one:
    type: command
    run: [["echo", "hello"]]
    next: look
  look:
    type: person
    skip_if: SKIP
    next: done
  done:
    type: command
    run: [["true"]]
"#;

fn load(dir: &Path, sheet: &str) -> Result<Graph> {
    let path = dir.join("sheet.yaml");
    fs::write(&path, sheet).unwrap();
    Graph::load(&path, &root().join("roles"))
}

/// A run of `ASK` in `dir`, with these commands in `skip_if`.
fn ask_run(dir: &Path, skip: &str) -> (Run, Graph) {
    let graph = load(dir, &ASK.replace("SKIP", skip)).unwrap();
    let mut run = Run::start(dir.join("run-7"), "a task", &graph).unwrap();
    go(&mut run, &graph, &Planner::new(Change::Named));
    (run, graph)
}

fn go(run: &mut Run, graph: &Graph, worker: &Planner) {
    run.go(graph, &root().join("roles"), worker).unwrap();
}

/// The lines of the log for the step `step`.
fn log_of(run: &Run, step: &str) -> Vec<Value> {
    let log = fs::read_to_string(run.dir.join("log.jsonl")).unwrap();
    let all = log.lines().map(|l| serde_json::from_str(l).unwrap());
    all.filter(|v: &Value| v["step"] == step).collect()
}

#[test]
fn good_commands_of_skip_if_go_on_with_no_question() {
    let (run, _) = ask_run(&folder(), r#"[["echo", "skip {run}"]]"#);
    assert_eq!(run.state.status, Status::Done, "{}", run.state.note);
    let lines = log_of(&run, "look");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["ok"], true);
    assert_eq!(lines[0]["summary"], "skip run-7");
}

#[test]
fn one_bad_command_of_skip_if_keeps_the_question_and_the_result() {
    let skip = r#"[["true"], ["false"]]"#;
    let (run, _) = ask_run(&folder(), skip);
    assert_eq!(run.state.status, Status::Waiting, "{}", run.state.note);
    assert_eq!(run.state.at.as_deref(), Some("look"));
    let note = &run.state.note;
    assert!(note.contains("Result before: hello"), "{note}");
}

#[test]
fn skip_if_does_not_run_when_the_person_answered() {
    let dir = folder();
    let count = dir.join("count");
    let script = format!("echo x >> {}; false", count.display());
    let skip = format!(r#"[["sh", "-c", "{script}"]]"#);
    let (mut run, graph) = ask_run(&dir, &skip);
    run.answer(true, "").unwrap();
    go(&mut run, &graph, &Planner::new(Change::Named));
    assert_eq!(run.state.status, Status::Done, "{}", run.state.note);
    assert_eq!(fs::read_to_string(&count).unwrap(), "x\n");
}

#[test]
fn a_skip_if_command_that_cannot_start_stops_the_run() {
    let (run, _) = ask_run(&folder(), r#"[["cu-no-such-command"]]"#);
    assert_eq!(run.state.status, Status::Stopped, "{}", run.state.note);
    assert_eq!(run.state.at.as_deref(), Some("look"));
}

#[test]
fn skip_if_is_only_for_a_person_step_and_has_words() {
    let dir = folder();
    let good = ASK.replace("SKIP", r#"[["true"]]"#);
    assert!(load(&dir, &good).is_ok(), "a person step with skip_if");
    let empty = ASK.replace("SKIP", "[[]]");
    assert!(load(&dir, &empty).is_err(), "a command with no words");
    let on_command = "run: [[\"true\"]]\n    skip_if: [[\"true\"]]\n";
    let bad = good.replace("run: [[\"true\"]]\n", on_command);
    assert_ne!(bad, good);
    assert!(load(&dir, &bad).is_err(), "skip_if on a command step");
}

// ---------------------------------------------------------------------
// The tests question of the default sheet
// ---------------------------------------------------------------------

/// What the planner changes in its second plan.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Change {
    Named,
    TwoTimes,
    Spec,
    Other,
    Added,
    Removed,
    Readme,
    PrText,
    Nothing,
    Link,
    /// Only the test file with this name changes.
    File(&'static str),
}

/// A simulated worker. `bad` has the bad results, in sequence. `second`
/// is the result of the second `attack_plan`.
struct Planner {
    change: Change,
    bad: Mutex<Vec<(&'static str, &'static str)>>,
    second: (bool, &'static str),
    /// `correct_plan` changes `b.rs`, which no bad result names.
    fix_b: bool,
    plans: AtomicU32,
    attacks: AtomicU32,
}

const NAMED: &str = "Bad test:\ntests/a.rs, line 3.";

const ALL: &str = "tests/a.rs, tests/b.rs, tests/c.rs and README.md are bad.";

impl Planner {
    fn new(change: Change) -> Self {
        Self {
            change,
            bad: Mutex::new(vec![("build", NAMED)]),
            second: (true, "looked"),
            fix_b: false,
            plans: AtomicU32::new(0),
            attacks: AtomicU32::new(0),
        }
    }

    fn bad(self, list: &[(&'static str, &'static str)]) -> Self {
        *self.bad.lock().unwrap() = list.to_vec();
        self
    }

    fn plan(&self, papers: &Path) {
        let n = self.plans.fetch_add(1, Ordering::SeqCst);
        let put = |rel: &str, body: &str| {
            let path = papers.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, body).unwrap();
        };
        if n == 0 {
            put("spec/spec.md", "the spec");
            put("tests/README.md", "how");
            put("tests/tests/a.rs", "a1");
            put("tests/tests/b.rs", "b1");
            put("tests/tests/[ab].rs", "g1");
            put("tests/tests/$(echo a).rs", "s1");
            put("pr/title.txt", "title");
            put("pr/body.md", "body");
            return;
        }
        if n == 2 {
            put("tests/tests/b.rs", "b2");
            return;
        }
        let a = papers.join("tests/tests/a.rs");
        if !matches!(self.change, Change::Nothing | Change::File(_)) {
            put("tests/tests/a.rs", "a2");
        }
        match self.change {
            Change::File(name) => put(&format!("tests/tests/{name}"), "new"),
            Change::Spec => put("spec/spec.md", "a new decision"),
            Change::Other => put("tests/tests/b.rs", "b2"),
            Change::Added => put("tests/tests/c.rs", "c1"),
            Change::Removed => {
                fs::remove_file(papers.join("tests/tests/b.rs")).unwrap();
            }
            Change::Readme => put("tests/README.md", "how, new"),
            Change::PrText => put("pr/body.md", "body, new"),
            Change::Link => {
                let target = papers.with_file_name("target.rs");
                fs::write(&target, "a2").unwrap();
                fs::remove_file(&a).unwrap();
                symlink(&target, &a).unwrap();
            }
            Change::Named | Change::TwoTimes | Change::Nothing => {}
        }
    }
}

impl Worker for Planner {
    fn run(&self, job: &Job) -> Result<Outcome> {
        let papers = job.dir.join("papers");
        let mut result = (true, "done");
        match job.step {
            "plan" => self.plan(&papers),
            "learn" => fs::write(papers.join("lessons/lessons.md"), "l")?,
            "write_pr" => fs::write(papers.join("pr/body.md"), "full")?,
            "attack_plan" => {
                result = (true, "looked");
                if self.attacks.fetch_add(1, Ordering::SeqCst) == 1 {
                    result = self.second;
                }
            }
            "attack_plan_2" => result = (true, "looked"),
            "correct_plan" if self.fix_b => fs::write(papers.join("tests/tests/b.rs"), "b2")?,
            _ => {}
        }
        let mut bad = self.bad.lock().unwrap();
        if bad.first().is_some_and(|(step, _)| *step == job.step) {
            result = (false, bad.remove(0).1);
        }
        Ok(Outcome {
            ok: result.0,
            summary: result.1.to_owned(),
        })
    }
}

fn git(dir: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    command.args(["-c", "user.name=t", "-c", "user.email=t@t"]);
    command.args(["-c", "commit.gpgsign=false"]);
    let out = command.args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}");
}

/// The default sheet for a run in `dir`. The step `mark` and the line
/// `skip_if` run there. Each other command step does nothing.
fn quiet_sheet(dir: &Path) -> Graph {
    let full = format!("{}/.codeumbrella/runs/", dir.display());
    let sheet = text("graph.example.yaml");
    let sheet = sheet.replace(".codeumbrella/runs/", &full);
    let mut graph = load(dir, &sheet).unwrap();
    for (name, step) in &mut graph.steps {
        if name != "mark" && !step.run.is_empty() {
            step.run = vec![vec!["true".to_owned()]];
        }
    }
    graph
}

/// A run at the first tests question.
fn at_first_question(worker: &Planner) -> (Run, Graph) {
    let dir = folder();
    let graph = quiet_sheet(&dir);
    let home = dir.join(".codeumbrella/runs/run-1");
    let mut run = Run::start(home, "a task", &graph).unwrap();
    let work = run.dir.join("work");
    git(&work, &["init", "--quiet"]);
    fs::write(work.join("file"), "x").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "--quiet", "-m", "first"]);
    go(&mut run, &graph, worker);
    let at = run.state.at.as_deref();
    assert_eq!(at, Some("look_at_tests"), "{}", run.state.note);
    (run, graph)
}

fn say_yes(run: &mut Run, graph: &Graph, worker: &Planner) {
    run.answer(true, "").unwrap();
    go(run, graph, worker);
}

/// A run after the first yes for the tests, at its next question.
fn after_first_yes(worker: &Planner) -> (Run, Graph) {
    let (mut run, graph) = at_first_question(worker);
    say_yes(&mut run, &graph, worker);
    (run, graph)
}

/// The summaries of the tests questions that stayed away.
fn skipped(run: &Run) -> Vec<String> {
    let lines = log_of(run, "look_at_tests").into_iter();
    let all = lines.map(|v| v["summary"].as_str().unwrap().to_owned());
    all.filter(|s| s.starts_with("No tests question."))
        .collect()
}

/// The run went to the PR question with no second tests question.
fn assert_goes_on(worker: &Planner) {
    goes_on_for(worker, "tests/a.rs");
}

/// As `assert_goes_on`, for a change of the test file `file`.
fn goes_on_for(worker: &Planner, file: &str) {
    let (run, _) = after_first_yes(worker);
    let at = run.state.at.as_deref();
    assert_eq!(at, Some("approval"), "{}", run.state.note);
    assert_eq!(worker.plans.load(Ordering::SeqCst), 2);
    let lines = skipped(&run);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains(file), "{lines:?}");
}

/// The person gets the second tests question. A control run with only the
/// named change gets none.
fn assert_asks(worker: &Planner) {
    assert_goes_on(&Planner::new(Change::Named));
    assert_question(worker);
}

/// As `assert_asks`, but the bad build names each file that can change.
/// Thus only the type of the change brings the question.
fn assert_asks_with_all_named(change: Change) {
    let bad = [("build", ALL)];
    assert_goes_on(&Planner::new(Change::Named).bad(&bad));
    assert_question(&Planner::new(change).bad(&bad));
}

/// The run waits at the second tests question.
fn assert_question(worker: &Planner) {
    let (run, _) = after_first_yes(worker);
    assert_eq!(run.state.status, Status::Waiting, "{}", run.state.note);
    let at = run.state.at.as_deref();
    assert_eq!(at, Some("look_at_tests"), "{}", run.state.note);
    let note = &run.state.note;
    assert!(note.contains("Result before: looked"), "{note}");
    assert_eq!(skipped(&run), Vec::<String>::new());
}

#[test]
fn a_named_test_after_a_bad_build_gets_no_tests_question() {
    assert_goes_on(&Planner::new(Change::Named));
}

#[test]
fn a_named_test_after_a_bad_repair_gets_no_tests_question() {
    let bad = [("check_tests", "bad"), ("repair", NAMED)];
    assert_goes_on(&Planner::new(Change::Named).bad(&bad));
}

#[test]
fn a_change_of_the_pr_text_does_not_count() {
    assert_goes_on(&Planner::new(Change::PrText));
}

#[test]
fn a_second_correction_is_compared_with_the_first_one() {
    let second = "tests/b.rs is bad.";
    let bad = [("build", NAMED), ("build", second)];
    let worker = Planner::new(Change::TwoTimes).bad(&bad);
    let (run, _) = after_first_yes(&worker);
    let at = run.state.at.as_deref();
    assert_eq!(at, Some("approval"), "{}", run.state.note);
    let lines = skipped(&run);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[1].contains("tests/b.rs"), "{lines:?}");
    assert!(!lines[1].contains("tests/a.rs"), "{lines:?}");
}

#[test]
fn a_changed_spec_goes_to_the_person() {
    assert_asks(&Planner::new(Change::Spec));
}

#[test]
fn a_changed_test_that_nobody_named_goes_to_the_person() {
    assert_asks(&Planner::new(Change::Other));
}

#[test]
fn a_new_test_file_goes_to_the_person() {
    assert_asks_with_all_named(Change::Added);
}

#[test]
fn a_removed_test_file_goes_to_the_person() {
    assert_asks_with_all_named(Change::Removed);
}

#[test]
fn a_changed_readme_of_the_tests_goes_to_the_person() {
    assert_asks_with_all_named(Change::Readme);
}

#[test]
fn a_file_name_with_brackets_is_not_a_pattern() {
    let named = [("build", "tests/[ab].rs is bad.")];
    let worker = Planner::new(Change::File("[ab].rs")).bad(&named);
    goes_on_for(&worker, "tests/[ab].rs");
    // `[ab].rs` as a pattern fits the named `a.rs`.
    assert_question(&Planner::new(Change::File("[ab].rs")));
}

#[test]
fn a_file_name_with_a_command_is_plain_text() {
    let named = [("build", "tests/$(echo a).rs is bad.")];
    let worker = Planner::new(Change::File("$(echo a).rs")).bad(&named);
    goes_on_for(&worker, "tests/$(echo a).rs");
    // A script that runs the name reads it as the named `a.rs`.
    assert_question(&Planner::new(Change::File("$(echo a).rs")));
}

/// A command that is bad in its first run only. `flag` shows that run.
fn bad_one_time(flag: &Path) -> Vec<Vec<String>> {
    let f = flag.display();
    let script = format!("[ -e '{f}' ] || {{ touch '{f}'; exit 1; }}");
    let words = ["sh", "-c", &script];
    vec![words.map(str::to_owned).to_vec()]
}

/// The step `command` is bad, and then the step `builder` is bad with a
/// named test. The person gets the tests question after the plan.
fn assert_asks_after(command: &str, builder: &'static str) {
    assert_goes_on(&Planner::new(Change::Named));
    let worker = Planner::new(Change::Named).bad(&[(builder, NAMED)]);
    let (mut run, mut graph) = at_first_question(&worker);
    let flag = run.dir.join("flag");
    let step = graph.steps.get_mut(command).unwrap();
    step.run = bad_one_time(&flag);
    say_yes(&mut run, &graph, &worker);
    assert_eq!(log_of(&run, builder)[0]["ok"], false);
    let at = run.state.at.as_deref();
    assert_eq!(at, Some("look_at_tests"), "{}", run.state.note);
    assert_eq!(skipped(&run), Vec::<String>::new());
}

#[test]
fn a_bad_do_work_goes_to_the_person() {
    assert_asks_after("work_was_done", "do_work");
}

#[test]
fn a_bad_fix_merge_goes_to_the_person() {
    assert_asks_after("take_master", "fix_merge");
}

#[test]
fn mark_renews_its_copy_with_a_bundle_in_the_run_folder() {
    let bad = [("build", NAMED), ("build", "tests/b.rs is bad.")];
    let worker = Planner::new(Change::TwoTimes).bad(&bad);
    let (mut run, graph) = at_first_question(&worker);
    // With this file, the first command of `mark` ends early.
    fs::write(run.dir.join("new-work.bundle"), "x").unwrap();
    say_yes(&mut run, &graph, &worker);
    let at = run.state.at.as_deref();
    assert_eq!(at, Some("approval"), "{}", run.state.note);
    let lines = skipped(&run);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(!lines[1].contains("tests/a.rs"), "{lines:?}");
}

#[test]
fn a_correction_with_no_change_goes_to_the_person() {
    assert_asks(&Planner::new(Change::Nothing));
}

#[test]
fn a_link_in_the_paper_tests_goes_to_the_person() {
    assert_asks(&Planner::new(Change::Link));
}

#[test]
fn a_path_that_is_only_a_part_of_a_name_goes_to_the_person() {
    let bad = [("build", "mytests/a.rs and tests/a.rsx are bad.")];
    assert_asks(&Planner::new(Change::Named).bad(&bad));
}

#[test]
fn an_attack_that_did_not_complete_goes_to_the_person() {
    assert_goes_on(&Planner::new(Change::Named));
    let mut worker = Planner::new(Change::Named);
    worker.second = (true, "Not complete: one check. looked");
    let (run, _) = after_first_yes(&worker);
    let at = run.state.at.as_deref();
    assert_eq!(at, Some("look_at_tests"), "{}", run.state.note);
}

#[test]
fn a_problem_of_the_attacker_goes_to_the_person() {
    assert_goes_on(&Planner::new(Change::Named));
    // First with no change after the attack, then with a change of `b.rs`.
    for fix_b in [false, true] {
        let mut worker = Planner::new(Change::Named);
        worker.second = (false, "one problem");
        worker.fix_b = fix_b;
        let (run, _) = after_first_yes(&worker);
        let at = run.state.at.as_deref();
        assert_eq!(at, Some("look_at_tests"), "{}", run.state.note);
        assert_eq!(skipped(&run), Vec::<String>::new());
        let b = run.dir.join("papers/tests/tests/b.rs");
        let want = if fix_b { "b2" } else { "b1" };
        assert_eq!(fs::read_to_string(b).unwrap(), want);
    }
}

#[test]
fn a_no_at_the_pr_question_still_shows_the_tests() {
    assert_goes_on(&Planner::new(Change::Named));
    let worker = Planner::new(Change::Named).bad(&[]);
    let (mut run, graph) = after_first_yes(&worker);
    assert_eq!(run.state.at.as_deref(), Some("approval"));
    run.answer(false, "Change tests/a.rs").unwrap();
    go(&mut run, &graph, &worker);
    let at = run.state.at.as_deref();
    assert_eq!(at, Some("look_at_tests"), "{}", run.state.note);
    assert_eq!(skipped(&run), Vec::<String>::new());
}

// ---------------------------------------------------------------------
// The rules page, the chat page and the documents
// ---------------------------------------------------------------------

/// The list items and the paragraphs of a text, each on one line.
fn items(page: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut open = false;
    for line in page.lines() {
        let new = line.starts_with("- ") || line.starts_with('#');
        let new = new || line.starts_with('|');
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

/// One item of the page has each of these words.
fn has_item(rel: &str, words: &[&str]) {
    let all = items(&text(rel));
    let has = |i: &String| words.iter().all(|w| i.contains(w));
    assert!(all.iter().any(has), "{rel}: no item with {words:?}");
}

#[test]
fn the_planner_page_has_the_two_new_rules() {
    has_item("roles/planner.md", &["named as bad"]);
    has_item("roles/planner.md", &["`no paper to correct`"]);
}

#[test]
fn the_chat_page_has_the_rule_of_the_queue() {
    has_item(SKILL, &["`Done`", "`run --next`", "3 tasks"]);
    has_item(SKILL, &["file name", "first line", "`Purpose:`"]);
}

#[test]
fn the_chat_page_has_the_rule_of_a_stop() {
    let start = ["no paper to correct", "`task.txt`", "`run --next`"];
    has_item(SKILL, &start);
    let cause = ["`Purpose:`", "`Cause:`", "`Example:`"];
    has_item(SKILL, &cause);
    has_item(SKILL, &["`Files:`", "`Test command:`", "`Cause:`"]);
    has_item(SKILL, &["no paper to correct", "one time"]);
    has_item(SKILL, &["rule", "one line", "next question"]);
}

#[test]
fn the_git_call_of_the_chat_page_has_the_two_settings() {
    let words = [
        "refs/codeumbrella/before..HEAD",
        "-c core.fsmonitor=false",
        "-c core.hooksPath=/dev/null",
    ];
    has_item(SKILL, &words);
}

#[test]
fn the_guides_do_not_say_that_the_tests_always_come_again() {
    let gone = "the planner corrects only that test, and you see the tests again";
    assert!(!text("README.md").contains(gone), "README.md: {gone}");
    let gone = "The attacker looks, you see the tests again, and `build` runs again.";
    assert!(!text("ARCHITECTURE.md").contains(gone), "{gone}");
}

#[test]
fn the_chat_page_gives_the_full_paths_at_the_tests_question() {
    let words = ["full path", "`spec.md`", "`tests`", "summary"];
    has_item(SKILL, &words);
}

#[test]
fn the_chat_page_tells_each_test_change_at_the_pr_question() {
    let words = ["`approval`", "`look_at_tests`", "paper test"];
    has_item(SKILL, &words);
    has_item(SKILL, &["No tests question.", "one line"]);
}

#[test]
fn the_documents_tell_the_new_rules() {
    for rel in ["AGENTS.md", "README.md"] {
        has_item(rel, &["3 tasks"]);
    }
    has_item("AGENTS.md", &["no paper to correct", "`task.txt`"]);
    has_item("AGENTS.md", &["`skip_if`"]);
    has_item("AGENTS.md", &["`tests/queue_rules.rs`"]);
    has_item("README.md", &["no paper to correct"]);
    has_item("README.md", &["paper test", "tests question"]);
}
