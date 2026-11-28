//! Tests of the numbers of a run: the numbers of each worker, the total in
//! the notebook, and the lines of `status`. They use a simulated worker, a
//! simulated `claude` and a simulated `codex`. No AI, no network.

use anyhow::{Result, anyhow};
use codeumbrella::engine::{Job, Numbers, Outcome, Run, Worker};
use codeumbrella::graph::Graph;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

static COUNT: AtomicU32 = AtomicU32::new(0);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A new empty folder. The path is the full one, as the computer gives it.
fn folder(name: &str) -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let dir = env::temp_dir().join(format!("cu-total-{name}-{pid}-{n}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

/// What a simulated worker gives in one try: ok, seconds and dollars.
type Try = (bool, Option<f64>, Option<f64>);

/// A simulated worker that gives numbers. Each step has a list of tries.
/// A step with no try in the list gives a good result and no numbers.
struct Sim {
    tries: Mutex<BTreeMap<String, Vec<Try>>>,
}

impl Sim {
    fn new(list: &[(&str, Try)]) -> Self {
        let mut tries: BTreeMap<String, Vec<Try>> = BTreeMap::new();
        for (step, one) in list {
            let entry = tries.entry((*step).to_owned());
            entry.or_default().push(*one);
        }
        let tries = Mutex::new(tries);
        Self { tries }
    }
}

impl Worker for Sim {
    fn run(&self, job: &Job) -> Result<Outcome> {
        self.run_counted(job).0
    }

    fn run_counted(&self, job: &Job) -> (Result<Outcome>, Numbers) {
        let mut all = self.tries.lock().unwrap();
        let list = all.entry(job.step.to_owned()).or_default();
        let next = (!list.is_empty()).then(|| list.remove(0));
        let (ok, seconds, dollars) = next.unwrap_or((true, None, None));
        let numbers = Numbers {
            seconds,
            dollars,
            ..Numbers::default()
        };
        let summary = format!("{} ran", job.step);
        (Ok(Outcome { ok, summary }), numbers)
    }
}

/// A simulated worker as the tests of today have it: it gives no numbers.
struct Plain;

impl Worker for Plain {
    fn run(&self, job: &Job) -> Result<Outcome> {
        let summary = format!("{} ran", job.step);
        Ok(Outcome { ok: true, summary })
    }
}

/// A simulated worker that ends with an error after 5 seconds.
struct Broken;

impl Worker for Broken {
    fn run(&self, job: &Job) -> Result<Outcome> {
        self.run_counted(job).0
    }

    fn run_counted(&self, _job: &Job) -> (Result<Outcome>, Numbers) {
        let numbers = Numbers {
            seconds: Some(5.0),
            ..Numbers::default()
        };
        (Err(anyhow!("the worker broke")), numbers)
    }
}

/// A worker, a command and a worker.
const TWO: &str = r#"start: one
steps:
  one:
    role: planner
    writes: [spec]
    next: cmd
  cmd:
    type: command
    run:
      - ["true"]
    next: two
  two:
    role: builder
"#;

/// A fan of three worker steps.
const FAN: &str = r"start: check
steps:
  check:
    type: fan
    fan: [a, b, c]
  a:
    role: checker
  b:
    role: safety
  c:
    role: reviewer
";

/// One worker step that goes back to itself after a bad result.
const AGAIN: &str = r"start: one
steps:
  one:
    role: builder
    on_bad: one
    max_tries: 3
";

/// One worker step.
const ONE: &str = r"start: one
steps:
  one:
    role: builder
";

/// Does a run of the sheet `text` with `worker`. Gives the run folder.
fn go(text: &str, worker: &dyn Worker) -> PathBuf {
    let roles = root().join("roles");
    let graph: Graph = yaml_serde::from_str(text).unwrap();
    graph.check(&roles).unwrap();
    let dir = folder("lib");
    let mut run = Run::start(dir.clone(), "a task", &graph).unwrap();
    fs::write(dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, worker).unwrap();
    dir
}

/// The notebook of the run in `dir`.
fn notebook(dir: &Path) -> Value {
    let text = fs::read_to_string(dir.join("state.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The lines of the log of the run in `dir` for one step.
fn log(dir: &Path, step: &str) -> Vec<Value> {
    let text = fs::read_to_string(dir.join("log.jsonl")).unwrap();
    let parse = |l: &str| serde_json::from_str::<Value>(l).unwrap();
    let all: Vec<Value> = text.lines().map(parse).collect();
    all.into_iter().filter(|v| v["step"] == step).collect()
}

/// The number `key` of `value`. It gives -1 if there is no number.
fn num(value: &Value, key: &str) -> f64 {
    value[key].as_f64().unwrap_or(-1.0)
}

/// Fails if `a` is not `b`, with a small tolerance.
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} is not {b}");
}

/// Fails if `line` has one of the fields in `keys`.
fn none_of(line: &Value, keys: &[&str]) {
    for key in keys {
        assert!(line.get(key).is_none(), "{key}: {line}");
    }
}

/// The fields that only a worker gives.
const NEW: [&str; 6] = [
    "worker_seconds",
    "dollars",
    "turns",
    "model",
    "tokens_in",
    "tokens_out",
];

#[test]
fn the_notebook_keeps_the_total_of_the_workers() {
    let sim = Sim::new(&[
        ("one", (true, Some(60.0), Some(1.25))),
        ("two", (true, Some(30.0), Some(0.5))),
    ]);
    let dir = go(TWO, &sim);
    let state = notebook(&dir);
    assert_eq!(state["status"], "Done", "{state}");
    let total = &state["total"];
    near(num(total, "workers"), 2.0);
    near(num(total, "seconds"), 90.0);
    near(num(total, "dollars"), 1.75);
    near(num(total, "no_cost"), 0.0);
    let one = &log(&dir, "one")[0];
    near(num(one, "dollars"), 1.25);
    near(num(one, "worker_seconds"), 60.0);
    // The field of today: the time that the supervisor measured.
    assert!(one["seconds"].is_number(), "{one}");
    let two = &log(&dir, "two")[0];
    near(num(two, "dollars"), 0.5);
    near(num(two, "worker_seconds"), 30.0);
    // A command step has no worker, so its line gets no new field.
    none_of(&log(&dir, "cmd")[0], &NEW);
}

#[test]
fn a_fan_adds_the_numbers_of_each_part() {
    let sim = Sim::new(&[
        ("a", (true, Some(10.0), Some(0.25))),
        ("b", (true, Some(20.0), Some(0.5))),
        ("c", (true, Some(40.0), Some(1.0))),
    ]);
    let dir = go(FAN, &sim);
    let state = notebook(&dir);
    assert_eq!(state["status"], "Done", "{state}");
    let total = &state["total"];
    near(num(total, "workers"), 3.0);
    near(num(total, "seconds"), 70.0);
    near(num(total, "dollars"), 1.75);
    for (part, dollars) in [("a", 0.25), ("b", 0.5), ("c", 1.0)] {
        let lines = log(&dir, part);
        assert_eq!(lines.len(), 1, "{part}");
        near(num(&lines[0], "dollars"), dollars);
    }
    near(num(&log(&dir, "a")[0], "worker_seconds"), 10.0);
    near(num(&log(&dir, "c")[0], "worker_seconds"), 40.0);
    // The fan step itself has no worker.
    none_of(&log(&dir, "check")[0], &NEW);
}

#[test]
fn a_worker_with_seconds_and_no_dollars_counts_in_no_cost() {
    let sim = Sim::new(&[
        ("one", (true, Some(60.0), Some(1.25))),
        ("two", (true, Some(30.0), None)),
    ]);
    let dir = go(TWO, &sim);
    let total = &notebook(&dir)["total"];
    near(num(total, "workers"), 2.0);
    near(num(total, "seconds"), 90.0);
    near(num(total, "dollars"), 1.25);
    near(num(total, "no_cost"), 1.0);
    let two = &log(&dir, "two")[0];
    near(num(two, "worker_seconds"), 30.0);
    none_of(two, &["dollars"]);
}

#[test]
fn each_try_of_a_step_counts_in_the_total() {
    // The first try gives a bad result. It gives its numbers too.
    let sim = Sim::new(&[
        ("one", (false, Some(60.0), Some(1.25))),
        ("one", (true, Some(30.0), Some(0.5))),
    ]);
    let dir = go(AGAIN, &sim);
    let state = notebook(&dir);
    assert_eq!(state["status"], "Done", "{state}");
    let total = &state["total"];
    near(num(total, "workers"), 2.0);
    near(num(total, "seconds"), 90.0);
    near(num(total, "dollars"), 1.75);
    let lines = log(&dir, "one");
    assert_eq!(lines.len(), 2);
    near(num(&lines[0], "dollars"), 1.25);
    near(num(&lines[1], "dollars"), 0.5);
}

#[test]
fn a_worker_with_an_error_gives_its_numbers() {
    let dir = go(ONE, &Broken);
    let state = notebook(&dir);
    assert_eq!(state["status"], "Stopped", "{state}");
    let total = &state["total"];
    near(num(total, "workers"), 1.0);
    near(num(total, "seconds"), 5.0);
    near(num(total, "no_cost"), 1.0);
    let line = &log(&dir, "one")[0];
    assert_eq!(line["ok"].as_bool(), Some(false), "{line}");
    near(num(line, "worker_seconds"), 5.0);
}

#[test]
fn a_worker_of_today_gives_no_numbers() {
    let dir = go(ONE, &Plain);
    let state = notebook(&dir);
    assert_eq!(state["status"], "Done", "{state}");
    let workers = state["total"]["workers"].as_u64();
    assert_eq!(workers.unwrap_or(0), 0, "{state}");
    none_of(&log(&dir, "one")[0], &NEW);
}

#[test]
fn the_notebook_keeps_the_start_time_of_the_step() {
    let dir = go(ONE, &Plain);
    let state = notebook(&dir);
    let since = state["since"].as_str().unwrap_or_default();
    let time = log(&dir, "one")[0]["time"].clone();
    let time = time.as_str().unwrap_or_default().to_owned();
    // The same time format as the log: RFC 3339 in UTC.
    assert!(since.len() >= 20, "{since}");
    assert_eq!(&since[10..11], "T", "{since}");
    assert!(since.ends_with('Z'), "{since}");
    assert!(time.ends_with('Z'), "{time}");
    assert_eq!(since[..10], time[..10]);
}

// The real command, with a simulated `claude` or `codex`.

fn git(dir: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.args(args).current_dir(dir);
    command.env("GIT_AUTHOR_NAME", "t");
    command.env("GIT_AUTHOR_EMAIL", "t@t");
    command.env("GIT_COMMITTER_NAME", "t");
    command.env("GIT_COMMITTER_EMAIL", "t@t");
    assert!(command.status().unwrap().success());
}

/// A new project with the factory files and a folder for simulated tools.
fn project() -> PathBuf {
    let dir = folder("cli");
    fs::create_dir_all(dir.join("bin")).unwrap();
    assert!(cu(&dir, &["init"]).status.success());
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "--quiet", "-m", "x"]);
    dir
}

/// Writes a simulated tool: a shell script in the `bin` folder.
fn tool(dir: &Path, name: &str, script: &str) {
    let path = dir.join("bin").join(name);
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    let mode = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&path, mode).unwrap();
}

/// The real command, with the simulated tools first on the PATH.
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

/// The lines that the command printed on its standard output.
fn lines(o: &Output) -> Vec<String> {
    let text = String::from_utf8_lossy(&o.stdout);
    text.lines().map(str::to_owned).collect()
}

/// Fails if the output has no line that is `want`.
fn has(o: &Output, want: &str) {
    let found = lines(o).iter().any(|l| l == want);
    assert!(found, "no line {want:?} in:\n{}", out(o));
}

/// Fails if a line of the output starts with `start`.
fn no_line(o: &Output, start: &str) {
    let found = lines(o).iter().any(|l| l.starts_with(start));
    assert!(!found, "a line starts with {start:?}:\n{}", out(o));
}

/// The run id in the line `Run:`.
fn id(o: &Output) -> String {
    let all = lines(o);
    let line = all.iter().find(|l| l.starts_with("Run:"));
    let line = line.expect("a line Run:");
    line["Run:".len()..].trim().to_owned()
}

fn run_dir(dir: &Path, id: &str) -> PathBuf {
    dir.join(".codeumbrella/runs").join(id)
}

fn sheet(dir: &Path, text: &str) {
    let path = dir.join(".codeumbrella/graph.yaml");
    fs::write(path, text).unwrap();
}

/// A sheet: copy, then one worker.
const CLI_ONE: &str = r"start: copy
steps:
  copy:
    type: command
    run:
      - [git, clone, --quiet, --no-hardlinks, ., '.codeumbrella/runs/{run}/work']
    next: one
  one:
    role: planner
    writes: [spec]
";

/// A sheet: copy, a worker, a question, a worker.
const CLI_ASK: &str = r"start: copy
steps:
  copy:
    type: command
    run:
      - [git, clone, --quiet, --no-hardlinks, ., '.codeumbrella/runs/{run}/work']
    next: one
  one:
    role: planner
    writes: [spec]
    next: ask
  ask:
    type: person
    next: two
  two:
    role: builder
";

/// A last line of `claude` with a cost, turns, a model and tokens.
const LAST: &str = concat!(
    r#"{"type":"result","is_error":false,"result":"","#,
    r#""structured_output":{"ok":true,"summary":"done"},"#,
    r#""total_cost_usd":1.25,"num_turns":7,"#,
    r#""modelUsage":{"claude-opus-5-5":{"inputTokens":1}},"#,
    r#""usage":{"input_tokens":10,"#,
    r#""cache_creation_input_tokens":200,"#,
    r#""cache_read_input_tokens":3000,"output_tokens":40}}"#,
);

/// A last line of `claude` with no cost, no turns and no model.
const PLAIN: &str = concat!(
    r#"{"type":"result","is_error":false,"result":"","#,
    r#""structured_output":{"ok":true,"summary":"done"}}"#,
);

/// A simulated `claude` that prints a first line and then `last`.
fn claude(last: &str) -> String {
    let first = r#"echo '{"type":"system"}'"#;
    format!("{first}\necho '{last}'")
}

/// The first `turn.completed` event of `codex`.
const TURN_ONE: &str = concat!(
    r#"{"type":"turn.completed","usage":{"input_tokens":100,"#,
    r#""cached_input_tokens":90,"output_tokens":5,"#,
    r#""reasoning_output_tokens":1}}"#,
);

/// The second `turn.completed` event of `codex`.
const TURN_TWO: &str = concat!(
    r#"{"type":"turn.completed","usage":{"input_tokens":200,"#,
    r#""cached_input_tokens":150,"output_tokens":7,"#,
    r#""reasoning_output_tokens":2}}"#,
);

/// The end of a simulated `codex`: it writes the result file.
const CODEX_END: &str = r#"while [ "$1" != "--output-last-message" ]; do shift; done
echo '{"ok":true,"summary":"done"}' > "$2""#;

/// A simulated `codex` with two `turn.completed` events.
fn codex() -> String {
    let start = r#"echo '{"type":"thread.started"}'"#;
    let one = format!("echo '{TURN_ONE}'");
    let two = format!("echo '{TURN_TWO}'");
    [start, one.as_str(), two.as_str(), CODEX_END].join("\n")
}

#[test]
fn a_claude_worker_gives_its_cost_turns_model_and_tokens() {
    let dir = project();
    tool(&dir, "claude", &claude(LAST));
    sheet(&dir, CLI_ONE);
    let o = cu(&dir, &["run", "a task"]);
    has(&o, "Status:   Done");
    has(&o, "Workers:  1");
    has(&o, "Cost:     1.25 dollars");
    let run = run_dir(&dir, &id(&o));
    let one = &log(&run, "one")[0];
    near(num(one, "dollars"), 1.25);
    near(num(one, "turns"), 7.0);
    assert_eq!(one["model"], "claude-opus-5-5", "{one}");
    near(num(one, "tokens_in"), 3210.0);
    near(num(one, "tokens_out"), 40.0);
    assert!(one["worker_seconds"].is_number(), "{one}");
    none_of(&log(&run, "copy")[0], &NEW);
    let total = &notebook(&run)["total"];
    near(num(total, "workers"), 1.0);
    near(num(total, "dollars"), 1.25);
    near(num(total, "tokens_in"), 3210.0);
    near(num(total, "tokens_out"), 40.0);
    near(num(total, "no_cost"), 0.0);
}

#[test]
fn a_claude_line_with_no_numbers_gives_only_the_time() {
    let dir = project();
    tool(&dir, "claude", &claude(PLAIN));
    sheet(&dir, CLI_ONE);
    let o = cu(&dir, &["run", "a task"]);
    has(&o, "Status:   Done");
    let run = run_dir(&dir, &id(&o));
    let one = &log(&run, "one")[0];
    assert_eq!(one["ok"].as_bool(), Some(true), "{one}");
    assert!(one["worker_seconds"].is_number(), "{one}");
    none_of(one, &["dollars", "turns", "model"]);
    let total = &notebook(&run)["total"];
    near(num(total, "workers"), 1.0);
    near(num(total, "no_cost"), 1.0);
}

#[test]
fn a_codex_worker_gives_the_sums_of_its_tokens_and_no_cost() {
    let dir = project();
    tool(&dir, "codex", &codex());
    sheet(&dir, CLI_ONE);
    let o = cu(&dir, &["--tool", "codex", "run", "a task"]);
    has(&o, "Status:   Done");
    has(&o, "Workers:  1");
    // All workers have no cost: no line about the cost.
    no_line(&o, "Cost:");
    no_line(&o, "No cost:");
    let run = run_dir(&dir, &id(&o));
    let one = &log(&run, "one")[0];
    near(num(one, "tokens_in"), 300.0);
    near(num(one, "tokens_out"), 12.0);
    assert!(one["worker_seconds"].is_number(), "{one}");
    none_of(one, &["dollars", "turns", "model"]);
    let total = &notebook(&run)["total"];
    near(num(total, "workers"), 1.0);
    near(num(total, "no_cost"), 1.0);
    near(num(total, "tokens_in"), 300.0);
    near(num(total, "tokens_out"), 12.0);
}

#[test]
fn a_run_from_before_this_version_starts_its_total_at_zero() {
    let dir = project();
    tool(&dir, "claude", &claude(LAST));
    sheet(&dir, CLI_ASK);
    let o = cu(&dir, &["run", "a task"]);
    has(&o, "Status:   Waiting");
    has(&o, "Workers:  1");
    let id = id(&o);
    let run = run_dir(&dir, &id);
    // Make the notebook as a version before this one wrote it.
    let mut value = notebook(&run);
    let map = value.as_object_mut().unwrap();
    map.remove("total");
    map.remove("since");
    let text = serde_json::to_string_pretty(&value).unwrap();
    fs::write(run.join("state.json"), text).unwrap();
    let o = cu(&dir, &["answer", &id, "ask", "yes"]);
    assert!(o.status.success(), "{}", out(&o));
    has(&o, "Status:   Done");
    has(&o, "Workers:  1");
    has(&o, "Cost:     1.25 dollars");
    let total = &notebook(&run)["total"];
    near(num(total, "workers"), 1.0);
    near(num(total, "dollars"), 1.25);
}

// `status` on a run folder that the test writes by hand. There is no
// `lock` file, so the repair does nothing and makes no call of `ps`.

/// A notebook as a version before this one wrote it.
fn base(status: &str, note: &str) -> Value {
    json!({
        "task": "a task",
        "status": status,
        "at": "build",
        "note": note,
        "answer": null,
        "tries": {},
        "results": {},
        "prints": {},
        "last": null
    })
}

/// The field `total` of a notebook.
fn total(workers: u64, seconds: f64, dollars: f64, no_cost: u64) -> Value {
    json!({
        "workers": workers,
        "seconds": seconds,
        "dollars": dollars,
        "tokens_in": 0,
        "tokens_out": 0,
        "no_cost": no_cost
    })
}

/// The time `seconds` before this moment, in the format of the log.
fn ago(seconds: i64) -> String {
    let then = OffsetDateTime::now_utc() - Duration::seconds(seconds);
    then.format(&Rfc3339).unwrap()
}

/// Writes `value` as the notebook of `run-1` in a new folder.
fn write_run(value: &Value) -> PathBuf {
    let dir = folder("status");
    let run = dir.join(".codeumbrella/runs/run-1");
    fs::create_dir_all(&run).unwrap();
    let text = serde_json::to_string_pretty(value).unwrap();
    fs::write(run.join("state.json"), text).unwrap();
    dir
}

fn status(dir: &Path) -> Output {
    let o = cu(dir, &["status", "run-1"]);
    assert!(o.status.success(), "{}", out(&o));
    o
}

/// The last line of `status`.
fn more(dir: &Path) -> String {
    let log = dir.join(".codeumbrella/runs/run-1/log.jsonl");
    format!("More:     {}", log.display())
}

/// Fails if the lines of `o` are not `want` and then the line `More:`.
fn same(o: &Output, dir: &Path, want: &[&str]) {
    let mut want: Vec<String> = want.iter().map(|l| (*l).to_owned()).collect();
    want.push(more(dir));
    assert_eq!(lines(o), want, "{}", out(o));
}

#[test]
fn status_shows_one_fact_on_each_line() {
    let mut value = base("Done", "All steps are complete.");
    value["total"] = total(2, 90.0, 1.75, 0);
    let dir = write_run(&value);
    let o = status(&dir);
    let want = [
        "Run:      run-1",
        "Status:   Done",
        "Note:     All steps are complete.",
        "Workers:  2",
        "Time:     1.5 worker minutes",
        "Cost:     1.75 dollars",
    ];
    same(&o, &dir, &want);
    no_line(&o, "Step:");
    no_line(&o, "Since:");
}

#[test]
fn each_value_starts_at_character_11() {
    let mut value = base("Done", "All steps are complete.");
    value["total"] = total(2, 90.0, 1.25, 1);
    let dir = write_run(&value);
    let o = status(&dir);
    let all = lines(&o);
    assert_eq!(all.len(), 8, "{}", out(&o));
    for line in all {
        let (name, value) = line.split_at(10);
        assert!(name.contains(':'), "{line}");
        assert!(name.ends_with(' '), "{line}");
        assert!(!value.starts_with(' '), "{line}");
    }
}

#[test]
fn status_tells_the_workers_with_no_cost() {
    let mut value = base("Done", "All steps are complete.");
    value["total"] = total(2, 90.0, 1.25, 1);
    let dir = write_run(&value);
    let o = status(&dir);
    let want = [
        "Run:      run-1",
        "Status:   Done",
        "Note:     All steps are complete.",
        "Workers:  2",
        "Time:     1.5 worker minutes",
        "Cost:     1.25 dollars",
        "No cost:  1 worker",
    ];
    same(&o, &dir, &want);
}

#[test]
fn status_has_no_cost_line_if_no_worker_gave_a_cost() {
    let mut value = base("Done", "All steps are complete.");
    value["total"] = total(2, 90.0, 0.0, 2);
    let dir = write_run(&value);
    let o = status(&dir);
    has(&o, "Workers:  2");
    no_line(&o, "Cost:");
    no_line(&o, "No cost:");
}

#[test]
fn status_of_a_run_with_no_total_has_no_lines_of_numbers() {
    let value = base("Done", "All steps are complete.");
    let dir = write_run(&value);
    let o = status(&dir);
    let want = [
        "Run:      run-1",
        "Status:   Done",
        "Note:     All steps are complete.",
    ];
    same(&o, &dir, &want);
}

#[test]
fn status_of_a_run_with_an_empty_note_has_no_note_line() {
    let mut value = base("Done", "");
    value["total"] = total(2, 90.0, 1.75, 0);
    let dir = write_run(&value);
    let o = status(&dir);
    let want = [
        "Run:      run-1",
        "Status:   Done",
        "Workers:  2",
        "Time:     1.5 worker minutes",
        "Cost:     1.75 dollars",
    ];
    same(&o, &dir, &want);
}

#[test]
fn status_of_a_running_run_shows_the_step_and_its_minutes() {
    let mut value = base("Running", "");
    value["since"] = Value::String(ago(185));
    value["total"] = total(1, 462.0, 2.6, 0);
    let dir = write_run(&value);
    let o = status(&dir);
    let want = [
        "Run:      run-1",
        "Status:   Running",
        "Step:     build",
        "Since:    3 minutes",
        "Workers:  1",
        "Time:     7.7 worker minutes",
        "Cost:     2.60 dollars",
    ];
    same(&o, &dir, &want);
}

#[test]
fn status_says_1_minute_for_one_minute() {
    let mut value = base("Running", "");
    value["since"] = Value::String(ago(65));
    let dir = write_run(&value);
    let o = status(&dir);
    let want = [
        "Run:      run-1",
        "Status:   Running",
        "Step:     build",
        "Since:    1 minute",
    ];
    same(&o, &dir, &want);
}

#[test]
fn status_of_a_running_run_with_no_since_has_no_since_line() {
    let value = base("Running", "");
    let dir = write_run(&value);
    let o = status(&dir);
    has(&o, "Status:   Running");
    has(&o, "Step:     build");
    no_line(&o, "Since:");
    no_line(&o, "Workers:");
}

#[test]
fn status_of_a_stopped_run_keeps_the_lines_about_resume() {
    let note = "A person stopped the run at the step build.";
    let mut value = base("Stopped", note);
    value["total"] = total(2, 90.0, 1.75, 0);
    let dir = write_run(&value);
    let o = status(&dir);
    let note = format!("Note:     {note}");
    let resume = "Resume only after the person says yes. An AI must not give this approval.";
    let want = [
        "Run:      run-1",
        "Status:   Stopped",
        note.as_str(),
        "Workers:  2",
        "Time:     1.5 worker minutes",
        "Cost:     1.75 dollars",
        "To continue: codeumbrella resume run-1",
        resume,
    ];
    same(&o, &dir, &want);
}

#[test]
fn status_changes_no_file() {
    let mut value = base("Running", "");
    value["since"] = Value::String(ago(185));
    value["total"] = total(1, 462.0, 2.6, 0);
    let dir = write_run(&value);
    let run = dir.join(".codeumbrella/runs/run-1");
    let before = fs::read(run.join("state.json")).unwrap();
    let names = |d: &Path| fs::read_dir(d).unwrap().count();
    let count = names(&run);
    status(&dir);
    assert_eq!(fs::read(run.join("state.json")).unwrap(), before);
    assert_eq!(names(&run), count);
}

// The documents.

fn read(name: &str) -> String {
    fs::read_to_string(root().join(name)).unwrap()
}

/// The first line of `text` that starts with `start`.
fn row(text: &str, start: &str) -> String {
    let line = text.lines().find(|l| l.starts_with(start));
    line.unwrap_or_default().to_owned()
}

#[test]
fn the_documents_name_the_new_lines_and_fields() {
    let agents = read("AGENTS.md");
    let notebook = row(&agents, "| Notebook |");
    for field in ["total", "since"] {
        assert!(notebook.contains(field), "{notebook}");
    }
    let log = row(&agents, "| Log |");
    for field in ["worker_seconds", "dollars"] {
        assert!(log.contains(field), "{log}");
    }
    for word in [
        "Limits of the numbers",
        "tests/run_total.rs",
        "Workers:",
        "Time:",
        "Cost:",
    ] {
        assert!(agents.contains(word), "AGENTS.md: {word}");
    }
    let skill = read("skills/codeumbrella/SKILL.md");
    for word in ["Workers:", "Time:", "Cost:"] {
        assert!(skill.contains(word), "SKILL.md: {word}");
    }
}
