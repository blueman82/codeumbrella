//! Tests of the lines `tool`, `model` and `effort` of a step. They start the
//! real command with a simulated `claude` and `codex`. No worker, no network.

use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// A new project with the factory files, and a folder for simulated tools.
fn project() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let dir = env::temp_dir().join(format!("cu-steptools-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
    assert!(cu(&dir, &["init"]).status.success());
    for args in [
        &["init", "--quiet"][..],
        &["add", "-A"],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "--quiet",
            "-m",
            "x",
        ],
    ] {
        let status = Command::new("git").args(args).current_dir(&dir).status();
        assert!(status.unwrap().success());
    }
    dir
}

/// Writes a simulated tool: a shell script in the `bin` folder of the project.
fn tool(dir: &Path, name: &str, script: &str) {
    let path = dir.join("bin").join(name);
    let script = script.replace("$PWD/bin", &dir.join("bin").display().to_string());
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// Starts the real command with the simulated tools first on the PATH.
fn cu(dir: &Path, args: &[&str]) -> Output {
    let path = format!(
        "{}:{}",
        dir.join("bin").display(),
        env::var("PATH").unwrap()
    );
    Command::new(env!("CARGO_BIN_EXE_codeumbrella"))
        .args(args)
        .current_dir(dir)
        .env("PATH", path)
        .output()
        .expect("start codeumbrella")
}

fn out(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// A simulated `claude`. Each call writes its words, one on each line, to its own file.
const CLAUDE: &str = r#"for a in "$@"; do echo "$a"; done > "$PWD/bin/claude.$$"
echo '{"type":"result","is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

/// A simulated `codex`. Each call writes its words, one on each line, to its own file.
const CODEX: &str = r#"for a in "$@"; do echo "$a"; done > "$PWD/bin/codex.$$"
while [ "$1" != "--output-last-message" ]; do shift; done
echo '{"ok":true,"summary":"done"}' > "$2""#;

/// A sheet with two worker steps. `ONE` and `TWO` get the extra lines of each step.
const TWO_STEPS: &str = r#"start: copy
steps:
  copy:
    type: command
    run:
      - [git, clone, --quiet, --no-hardlinks, ., ".codeumbrella/runs/{run}/work"]
    next: one
  one:
    role: planner
    writes: [spec]
    ONE
    next: two
  two:
    role: builder
    TWO
"#;

/// A sheet with a fan of two worker steps. `ONE` and `TWO` get the extra lines of each part.
const A_FAN: &str = r#"start: copy
steps:
  copy:
    type: command
    run:
      - [git, clone, --quiet, --no-hardlinks, ., ".codeumbrella/runs/{run}/work"]
    next: one
  one:
    role: planner
    writes: [spec]
    next: check
  check:
    type: fan
    fan: [a, b]
  a:
    role: checker
    reads: [spec]
    ONE
  b:
    role: safety
    reads: [spec]
    TWO
"#;

/// Puts a sheet into the project. `one` and `two` are the extra lines of the two steps.
fn sheet(dir: &Path, base: &str, one: &str, two: &str) {
    let text = base.replace("ONE", one).replace("TWO", two);
    fs::write(dir.join(".codeumbrella/graph.yaml"), text).unwrap();
}

/// The words of each call of a simulated tool: one list for each call.
fn calls(dir: &Path, name: &str) -> Vec<Vec<String>> {
    let prefix = format!("{name}.");
    let mut all = Vec::new();
    for entry in fs::read_dir(dir.join("bin")).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let text = fs::read_to_string(entry.path()).unwrap();
            all.push(text.lines().map(str::to_owned).collect());
        }
    }
    all
}

/// True if the word `key` is directly before the word `value`.
fn pair(call: &[String], key: &str, value: &str) -> bool {
    call.windows(2).any(|w| w[0] == key && w[1] == value)
}

/// Splits the two calls of a run: first the call that has `key` and `value`, then the other.
fn split<'a>(all: &'a [Vec<String>], key: &str, value: &str) -> (&'a [String], &'a [String]) {
    assert_eq!(all.len(), 2);
    let at = all.iter().position(|c| pair(c, key, value)).expect(value);
    (&all[at], &all[1 - at])
}

#[test]
fn a_step_with_model_and_effort_gives_them_to_claude() {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    sheet(&dir, TWO_STEPS, "model: opus\n    effort: high", "");
    let run = cu(&dir, &["run", "a task"]);
    assert!(out(&run).contains("Status:   Done"), "{}", out(&run));
    let all = calls(&dir, "claude");
    let (one, other) = split(&all, "--model", "opus");
    assert!(pair(one, "--effort", "high"), "{one:?}");
    // The step with no lines adds no option.
    assert!(!other.iter().any(|w| w == "--model" || w == "--effort"));
}

#[test]
fn a_step_with_only_one_of_the_two_lines_gives_only_that_option() {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    sheet(&dir, TWO_STEPS, "effort: xhigh", "model: haiku");
    let run = cu(&dir, &["run", "a task"]);
    assert!(out(&run).contains("Status:   Done"), "{}", out(&run));
    let all = calls(&dir, "claude");
    let (effort, model) = split(&all, "--effort", "xhigh");
    assert!(!effort.iter().any(|w| w == "--model"), "{effort:?}");
    assert!(pair(model, "--model", "haiku"), "{model:?}");
    assert!(!model.iter().any(|w| w == "--effort"), "{model:?}");
}

#[test]
fn a_step_with_model_and_effort_gives_them_to_codex() {
    let dir = project();
    tool(&dir, "codex", CODEX);
    let lines = "model: gpt-6.2-sol\n    effort: medium";
    sheet(&dir, TWO_STEPS, lines, "");
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    assert!(out(&run).contains("Status:   Done"), "{}", out(&run));
    let all = calls(&dir, "codex");
    let (one, other) = split(&all, "-m", "gpt-6.2-sol");
    let effort = "model_reasoning_effort=\"medium\"";
    assert!(pair(one, "-c", effort), "{one:?}");
    // The step with no lines adds no option.
    assert!(!other.iter().any(|w| w == "-m"), "{other:?}");
    let any = other.iter().any(|w| w.contains("model_reasoning_effort"));
    assert!(!any, "{other:?}");
}

#[test]
fn a_run_with_the_claude_tool_starts_codex_for_a_step_with_tool_codex() {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    tool(&dir, "codex", CODEX);
    sheet(&dir, TWO_STEPS, "tool: codex\n    model: gpt-6-luna", "");
    let run = cu(&dir, &["run", "a task"]);
    assert!(out(&run).contains("Status:   Done"), "{}", out(&run));
    let claude = calls(&dir, "claude");
    let codex = calls(&dir, "codex");
    assert_eq!((claude.len(), codex.len()), (1, 1));
    assert!(pair(&codex[0], "-m", "gpt-6-luna"), "{:?}", codex[0]);
    // The Claude call is for the second step. It has no option of the first.
    assert!(!claude[0].iter().any(|w| w == "--model"));
}

#[test]
fn a_run_with_the_codex_tool_starts_claude_for_a_step_with_tool_claude() {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    tool(&dir, "codex", CODEX);
    sheet(&dir, TWO_STEPS, "", "tool: claude\n    model: sonnet");
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    assert!(out(&run).contains("Status:   Done"), "{}", out(&run));
    let claude = calls(&dir, "claude");
    let codex = calls(&dir, "codex");
    assert_eq!((claude.len(), codex.len()), (1, 1));
    assert!(pair(&claude[0], "--model", "sonnet"), "{:?}", claude[0]);
}

#[test]
fn the_role_of_a_step_needs_the_settings_file_of_the_tool_that_the_step_uses() {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    tool(&dir, "codex", CODEX);
    fs::remove_file(dir.join(".codeumbrella/roles/builder.codex.txt")).unwrap();
    sheet(&dir, TWO_STEPS, "", "tool: codex");
    let run = cu(&dir, &["run", "a task"]);
    assert!(out(&run).contains("Status:   Stopped"), "{}", out(&run));
    assert!(out(&run).contains("cannot read the settings"));
    assert!(calls(&dir, "codex").is_empty(), "codex started");
    // With the Claude tool for that step, the same role works. A new project
    // gives the run its own id: two runs in one second would share an id.
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    tool(&dir, "codex", CODEX);
    fs::remove_file(dir.join(".codeumbrella/roles/builder.codex.txt")).unwrap();
    sheet(&dir, TWO_STEPS, "", "tool: claude");
    let run = cu(&dir, &["run", "a task"]);
    assert!(out(&run).contains("Status:   Done"), "{}", out(&run));
}

#[test]
fn a_part_of_a_fan_gets_its_own_model_and_effort() {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    sheet(&dir, A_FAN, "model: opus\n    effort: high", "");
    let run = cu(&dir, &["run", "a task"]);
    assert!(out(&run).contains("Status:   Done"), "{}", out(&run));
    let all = calls(&dir, "claude");
    // One call for `one`, and one for each part of the fan.
    assert_eq!(all.len(), 3);
    let with = all.iter().filter(|c| pair(c, "--effort", "high"));
    assert_eq!(with.count(), 1);
    let model = all.iter().filter(|c| pair(c, "--model", "opus"));
    assert_eq!(model.count(), 1);
}

#[test]
fn a_sheet_with_a_bad_tool_model_or_effort_is_an_error_before_the_run_starts() {
    for lines in [
        "tool: gemini",
        "tool: Claude",
        "model: \"a b\"",
        "model: 'a\"b'",
        "effort: 'a}b'",
        "effort: \"high; rm\"",
        "model: \"--dangerous\"",
        "effort: \"-x\"",
    ] {
        let dir = project();
        tool(&dir, "claude", CLAUDE);
        tool(&dir, "codex", CODEX);
        sheet(&dir, TWO_STEPS, "", lines);
        let run = cu(&dir, &["run", "a task"]);
        assert!(!run.status.success(), "{lines}: {}", out(&run));
        assert!(calls(&dir, "claude").is_empty(), "{lines}: claude started");
        assert!(calls(&dir, "codex").is_empty(), "{lines}: codex started");
        // The run did not start, so the project has no run folder with a notebook.
        let runs = dir.join(".codeumbrella/runs");
        let started = fs::read_dir(runs).map_or(0, Iterator::count);
        assert_eq!(started, 0, "{lines}: a run started");
    }
}

#[test]
fn the_default_sheet_sets_none_of_the_three_lines() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let text = fs::read_to_string(root.join("graph.example.yaml")).unwrap();
    // A line of a comment does not count. A set line starts with its name and a colon.
    for line in text.lines().map(str::trim_start) {
        for name in ["tool:", "model:", "effort:"] {
            assert!(!line.starts_with(name), "the default sheet sets {line}");
        }
    }
}

// Which line of the reply of `claude` is the result.

/// A good result line, with its numbers. Its `type` is not the first field.
const GOOD: &str = concat!(
    r#"{"is_error":false,"result":"","type":"result","#,
    r#""structured_output":{"ok":true,"summary":"done"},"#,
    r#""total_cost_usd":1.25,"num_turns":7,"#,
    r#""modelUsage":{"model-b":{}},"#,
    r#""usage":{"input_tokens":10,"#,
    r#""cache_creation_input_tokens":200,"#,
    r#""cache_read_input_tokens":3000,"output_tokens":40}}"#,
);

/// A result line with a bad result and different numbers.
const OTHER: &str = concat!(
    r#"{"type":"result","is_error":false,"result":"","#,
    r#""structured_output":{"ok":false,"summary":"other"},"#,
    r#""total_cost_usd":0.15,"num_turns":3,"#,
    r#""modelUsage":{"model-a":{}},"#,
    r#""usage":{"input_tokens":1,"output_tokens":2}}"#,
);

/// The line that `claude` writes after its result when the disk is full.
const DISK_FULL: &str = concat!(
    r#"{"type":"system","subtype":"session_writes_lost","#,
    r#""error_code":"ENOSPC"}"#,
);

/// A line with all fields of a result, but with a different `type`.
const NOT_A_RESULT: &str = concat!(
    r#"{"type":"system","is_error":false,"result":"","#,
    r#""structured_output":{"ok":true,"summary":"done"},"#,
    r#""content":[{"type":"result"}]}"#,
);

/// Runs the two steps with a `claude` that prints `lines` and ends with `code`.
/// Gives the project folder and the output of the command.
fn reply(lines: &[&str], code: i32) -> (PathBuf, String) {
    let dir = project();
    let words = lines.join("' '");
    let script = format!("printf '%s\\n' '{words}'\nexit {code}");
    tool(&dir, "claude", &script);
    sheet(&dir, TWO_STEPS, "", "");
    let text = out(&cu(&dir, &["run", "a task"]));
    (dir, text)
}

/// The line of the step `one` in the log of the only run of the project.
fn log_of_one(dir: &Path) -> Value {
    let mut runs = fs::read_dir(dir.join(".codeumbrella/runs")).unwrap();
    let run = runs.next().unwrap().unwrap().path();
    let text = fs::read_to_string(run.join("log.jsonl")).unwrap();
    let parse = |l: &str| serde_json::from_str::<Value>(l).unwrap();
    let mut all = text.lines().map(parse);
    all.find(|v| v["step"] == "one").expect("a line of one")
}

/// Checks that the log line of the step `one` has the numbers of `GOOD`.
fn has_the_numbers_of_good(dir: &Path) {
    let line = log_of_one(dir);
    assert_eq!(line["dollars"], 1.25, "{line}");
    assert_eq!(line["turns"], 7, "{line}");
    assert_eq!(line["model"], "model-b", "{line}");
    assert_eq!(line["tokens_in"], 3210, "{line}");
    assert_eq!(line["tokens_out"], 40, "{line}");
}

/// Checks that the run stopped with the error for a reply with no result.
fn has_no_result(text: &str) {
    assert!(text.contains("Status:   Stopped"), "{text}");
    assert!(text.contains("claude gave no result; see "), "{text}");
    assert!(text.contains("one.1.err.txt"), "{text}");
}

#[test]
fn a_line_after_the_result_does_not_remove_the_result() {
    let (dir, text) = reply(&[GOOD, DISK_FULL], 0);
    assert!(text.contains("Status:   Done"), "{text}");
    has_the_numbers_of_good(&dir);
}

#[test]
fn a_line_that_is_not_json_after_the_result_does_not_count() {
    let half = r#"{"type":"system","subty"#;
    let (dir, text) = reply(&[GOOD, half, "plain words", ""], 0);
    assert!(text.contains("Status:   Done"), "{text}");
    has_the_numbers_of_good(&dir);
}

#[test]
fn of_two_result_lines_the_step_uses_the_last_one() {
    let (dir, text) = reply(&[OTHER, GOOD, DISK_FULL], 0);
    assert!(text.contains("Status:   Done"), "{text}");
    has_the_numbers_of_good(&dir);
}

#[test]
fn a_reply_with_no_result_line_gives_no_result() {
    let (_, text) = reply(&[DISK_FULL, NOT_A_RESULT], 0);
    has_no_result(&text);
}

#[test]
fn a_line_with_no_type_is_not_a_result_line() {
    let untyped = concat!(
        r#"{"is_error":false,"result":"","#,
        r#""structured_output":{"ok":true,"summary":"done"}}"#,
    );
    let (_, text) = reply(&[DISK_FULL, untyped], 0);
    has_no_result(&text);
}

#[test]
fn no_result_line_comes_before_the_exit_status() {
    let (_, text) = reply(&[NOT_A_RESULT], 1);
    has_no_result(&text);
}

#[test]
fn a_last_result_line_that_cannot_be_read_gives_no_result() {
    let bare = r#"{"type":"result"}"#;
    let (_, text) = reply(&[GOOD, bare, DISK_FULL], 0);
    has_no_result(&text);
}

#[test]
fn a_result_line_with_an_error_gives_that_error() {
    let bad = r#"{"type":"result","is_error":true,"result":"boom"}"#;
    let (_, text) = reply(&[bad, DISK_FULL], 1);
    assert!(text.contains("Status:   Stopped"), "{text}");
    assert!(text.contains("claude reported an error: boom"), "{text}");
}

#[test]
fn a_good_result_with_exit_status_1_stays_an_error() {
    let (_, text) = reply(&[GOOD, DISK_FULL], 1);
    assert!(text.contains("Status:   Stopped"), "{text}");
    assert!(text.contains("claude ended with an error"), "{text}");
}

#[test]
fn each_claude_seed_of_the_fuzzer_is_a_result_line() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let seeds = root.join("fuzz/seeds/fuzz_claude_reply");
    for entry in fs::read_dir(seeds).unwrap() {
        let text = fs::read_to_string(entry.unwrap().path()).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["type"], "result", "{text}");
        let mut numbers = codeumbrella::engine::Numbers::default();
        let read = codeumbrella::claude::read_reply(&text, &mut numbers);
        assert!(read.is_ok(), "{text}");
    }
}

#[test]
fn the_safety_rules_tell_the_result_line_its_cause_and_its_date() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let text = fs::read_to_string(root.join("AGENTS.md")).unwrap();
    let from = text.split("\n## Safety rules\n").nth(1).unwrap();
    let rules = from.split("\n## ").next().unwrap();
    for word in [
        r#"`"type":"result"`"#,
        "`session_writes_lost`",
        "27 November 2026",
    ] {
        assert!(rules.contains(word), "no {word} in the safety rules");
    }
}
