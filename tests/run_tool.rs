//! Tests of the stop message and of the tool that a run remembers. They start
//! the real command with a simulated `claude` and `codex`. No worker, no network.

use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// A new project with the factory files, and a folder for simulated tools.
/// Each test that starts a run uses its own project: two runs in one project
/// in the same second would get the same run name.
fn project() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let dir = env::temp_dir().join(format!("cu-runtool-{}-{n}", std::process::id()));
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

/// The id of the run that an output names.
fn id(o: &Output) -> String {
    let all = out(o);
    let line = all.lines().find(|l| l.starts_with("Run: ")).expect("a run");
    line["Run: ".len()..].to_owned()
}

/// A simulated `claude`. Each call writes a file of its own.
const CLAUDE: &str = r#"echo called > "$PWD/bin/claude.$$"
echo '{"is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

/// A simulated `codex`. Each call writes a file of its own.
const CODEX: &str = r#"echo called > "$PWD/bin/codex.$$"
while [ "$1" != "--output-last-message" ]; do shift; done
echo '{"ok":true,"summary":"done"}' > "$2""#;

/// A simulated `codex` that ends with an error code until the file `ok` is there.
const CODEX_FAILS: &str = r#"echo called > "$PWD/bin/codex.$$"
[ -f "$PWD/bin/ok" ] || exit 3
while [ "$1" != "--output-last-message" ]; do shift; done
echo '{"ok":true,"summary":"done"}' > "$2""#;

/// A sheet with a worker step, a question, a worker step, a question and a
/// last worker step. `EXTRA` gets the extra lines of the last step.
const THREE: &str = r#"start: copy
steps:
  copy:
    type: command
    run:
      - [git, clone, --quiet, --no-hardlinks, ., ".codeumbrella/runs/{run}/work"]
    next: one
  one:
    role: planner
    writes: [spec]
    next: ask1
  ask1:
    type: person
    next: two
  two:
    role: builder
    next: ask2
  ask2:
    type: person
    next: three
  three:
    role: builder
    EXTRA
"#;

fn sheet(dir: &Path, extra: &str) {
    let text = THREE.replace("EXTRA", extra);
    fs::write(dir.join(".codeumbrella/graph.yaml"), text).unwrap();
}

/// A project with the two simulated tools and the sheet `THREE`.
fn setup(codex: &str) -> PathBuf {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    tool(&dir, "codex", codex);
    sheet(&dir, "");
    dir
}

/// How many times a simulated tool started.
fn count(dir: &Path, name: &str) -> usize {
    let prefix = format!("{name}.");
    let files = fs::read_dir(dir.join("bin")).unwrap();
    files
        .filter(|e| {
            let file = e.as_ref().unwrap().file_name();
            file.to_string_lossy().starts_with(&prefix)
        })
        .count()
}

fn notebook_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(".codeumbrella/runs").join(id).join("state.json")
}

/// The bytes of the notebook.
fn bytes(dir: &Path, id: &str) -> Vec<u8> {
    fs::read(notebook_path(dir, id)).unwrap()
}

/// The tool in the notebook, or `None` if the notebook has no tool.
fn saved_tool(dir: &Path, id: &str) -> Option<String> {
    let value: Value = serde_json::from_slice(&bytes(dir, id)).unwrap();
    value.get("tool")?.as_str().map(str::to_owned)
}

/// Makes the notebook look like one of an older version: no tool.
fn forget_tool(dir: &Path, id: &str) {
    let mut value: Value = serde_json::from_slice(&bytes(dir, id)).unwrap();
    value.as_object_mut().unwrap().remove("tool");
    let text = serde_json::to_vec_pretty(&value).unwrap();
    fs::write(notebook_path(dir, id), text).unwrap();
    assert_eq!(saved_tool(dir, id), None);
}

/// The output of the command must say that `needle` is in it.
fn says(o: &Output, needle: &str) {
    assert!(out(o).contains(needle), "{}", out(o));
}

/// The line after the line that says how to continue.
const NOTE: &str = "Resume only after the person says yes. An AI must not give this approval.";

/// The number of the first line that is exactly `line`.
fn at(lines: &[&str], line: &str) -> usize {
    lines.iter().position(|l| *l == line).expect(line)
}

/// A sheet with one command step that has a long name and a long bad command.
fn long_stop_sheet() -> String {
    let word = format!("no-such-command-{}", "x".repeat(400));
    let text = "start: s\nsteps:\n  s:\n    type: command\n    run:\n      - [WORD]\n";
    text.replace("WORD", &word)
}

#[test]
fn a_stopped_run_prints_the_two_lines_in_full_also_after_a_long_note() {
    let dir = project();
    fs::write(dir.join(".codeumbrella/graph.yaml"), long_stop_sheet()).unwrap();
    let run = cu(&dir, &["run", "a task"]);
    let text = out(&run);
    assert!(text.contains("Status: Stopped"), "{text}");
    let lines: Vec<&str> = text.lines().collect();
    let first = format!("To continue: codeumbrella resume {}", id(&run));
    let from = at(&lines, &first);
    assert_eq!(lines[from + 1], NOTE);
    let status = at(&lines, "Status: Stopped");
    assert!(status < from, "{text}");
}

#[test]
fn a_stopped_run_with_a_short_note_prints_the_two_lines() {
    let dir = project();
    let text = "start: a\nsteps:\n  a:\n    type: command\n    run: [[false]]\n";
    fs::write(dir.join(".codeumbrella/graph.yaml"), text).unwrap();
    let run = cu(&dir, &["run", "a task"]);
    let text = out(&run);
    assert!(text.contains("Status: Stopped"), "{text}");
    let first = format!("To continue: codeumbrella resume {}\n", id(&run));
    assert!(text.contains(&first), "{text}");
    assert!(text.contains(NOTE), "{text}");
}

/// The output must have the two stop lines, in full, with the id of the run.
fn has_stop_lines(o: &Output, id: &str) {
    let text = out(o);
    let lines: Vec<&str> = text.lines().collect();
    let from = at(&lines, &format!("To continue: codeumbrella resume {id}"));
    assert_eq!(lines[from + 1], NOTE, "{text}");
}

#[test]
fn a_stopped_run_prints_the_two_lines_also_in_status_and_after_resume() {
    let dir = setup(CODEX_FAILS);
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    says(&run, "Status: Stopped");
    let id = id(&run);
    has_stop_lines(&run, &id);
    // The status command shows them too.
    let status = cu(&dir, &["status", &id]);
    says(&status, "Status: Stopped");
    has_stop_lines(&status, &id);
    // A resume that stops again shows them again.
    let resume = cu(&dir, &["resume", &id]);
    says(&resume, "Status: Stopped");
    has_stop_lines(&resume, &id);
    // A resume that goes on does not.
    fs::write(dir.join("bin/ok"), "").unwrap();
    let resume = cu(&dir, &["resume", &id]);
    says(&resume, "Status: Waiting");
    assert!(!out(&resume).contains("To continue:"), "{}", out(&resume));
    assert!(!out(&resume).contains("Resume only after"));
}

#[test]
fn a_run_that_is_not_stopped_prints_neither_line() {
    // Waiting.
    let dir = setup(CODEX);
    let run = cu(&dir, &["run", "a task"]);
    let waiting = out(&run);
    assert!(waiting.contains("Status: Waiting"), "{waiting}");
    // The status command shows the same, with no stop lines.
    let status = out(&cu(&dir, &["status", &id(&run)]));
    // Done.
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    let text = "start: a\nsteps:\n  a:\n    type: command\n    run: [[true]]\n";
    fs::write(dir.join(".codeumbrella/graph.yaml"), text).unwrap();
    let done = out(&cu(&dir, &["run", "a task"]));
    assert!(done.contains("Status: Done"), "{done}");
    for text in [waiting, status, done] {
        assert!(!text.contains("To continue:"), "{text}");
        assert!(!text.contains("Resume only after"), "{text}");
    }
}

#[test]
fn run_writes_its_tool_into_the_notebook() {
    let dir = setup(CODEX);
    let run = cu(&dir, &["run", "a task"]);
    assert_eq!(saved_tool(&dir, &id(&run)).as_deref(), Some("claude"));
    let dir = setup(CODEX);
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    assert_eq!(saved_tool(&dir, &id(&run)).as_deref(), Some("codex"));
}

#[test]
fn answer_with_no_tool_uses_the_tool_of_the_run() {
    let dir = setup(CODEX);
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    let id = id(&run);
    assert_eq!((count(&dir, "codex"), count(&dir, "claude")), (1, 0));
    let answer = cu(&dir, &["answer", &id, "ask1", "yes"]);
    says(&answer, "Status: Waiting");
    assert_eq!((count(&dir, "codex"), count(&dir, "claude")), (2, 0));
    let answer = cu(&dir, &["answer", &id, "ask2", "yes"]);
    says(&answer, "Status: Done");
    assert_eq!((count(&dir, "codex"), count(&dir, "claude")), (3, 0));
}

#[test]
fn resume_with_no_tool_uses_the_tool_of_the_run() {
    let dir = setup(CODEX_FAILS);
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    says(&run, "Status: Stopped");
    let id = id(&run);
    let before = count(&dir, "codex");
    assert!(before >= 1);
    fs::write(dir.join("bin/ok"), "").unwrap();
    let resume = cu(&dir, &["resume", &id]);
    says(&resume, "Status: Waiting");
    assert!(count(&dir, "codex") > before);
    assert_eq!(count(&dir, "claude"), 0, "claude started");
}

#[test]
fn answer_with_a_different_tool_is_an_error_and_changes_nothing() {
    let dir = setup(CODEX);
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    let id = id(&run);
    let before = bytes(&dir, &id);
    let calls = (count(&dir, "codex"), count(&dir, "claude"));
    let wrong = cu(&dir, &["--tool", "claude", "answer", &id, "ask1", "yes"]);
    assert!(!wrong.status.success(), "{}", out(&wrong));
    says(
        &wrong,
        "this run uses the tool codex. Send the command again with no --tool.",
    );
    assert_eq!(bytes(&dir, &id), before, "the notebook changed");
    assert_eq!((count(&dir, "codex"), count(&dir, "claude")), calls);
    // The same tool, said out loud, is good.
    let same = cu(&dir, &["--tool", "codex", "answer", &id, "ask1", "yes"]);
    says(&same, "Status: Waiting");
    assert_eq!(count(&dir, "codex"), calls.0 + 1);
}

#[test]
fn resume_with_a_different_tool_is_an_error_and_changes_nothing() {
    let dir = setup(CODEX_FAILS);
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    says(&run, "Status: Stopped");
    let id = id(&run);
    fs::write(dir.join("bin/ok"), "").unwrap();
    let before = bytes(&dir, &id);
    let calls = count(&dir, "codex");
    let wrong = cu(&dir, &["--tool", "claude", "resume", &id]);
    assert!(!wrong.status.success(), "{}", out(&wrong));
    says(
        &wrong,
        "this run uses the tool codex. Send the command again with no --tool.",
    );
    assert_eq!(bytes(&dir, &id), before, "the notebook changed");
    assert_eq!(count(&dir, "codex"), calls, "codex started");
    assert_eq!(count(&dir, "claude"), 0, "claude started");
}

/// A simulated `claude` that ends with an error code until the file `ok` is there.
const CLAUDE_FAILS: &str = r#"echo called > "$PWD/bin/claude.$$"
[ -f "$PWD/bin/ok" ] || exit 3
echo '{"is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

#[test]
fn a_run_of_the_claude_tool_gives_the_error_for_tool_codex() {
    let message = "this run uses the tool claude. Send the command again with no --tool.";
    // `answer`, on a run that waits.
    let dir = setup(CODEX);
    let run = cu(&dir, &["run", "a task"]);
    let first = id(&run);
    let before = bytes(&dir, &first);
    let calls = (count(&dir, "claude"), count(&dir, "codex"));
    let wrong = cu(&dir, &["--tool", "codex", "answer", &first, "ask1", "yes"]);
    assert!(!wrong.status.success(), "{}", out(&wrong));
    says(&wrong, message);
    assert_eq!(bytes(&dir, &first), before, "the notebook changed");
    assert_eq!((count(&dir, "claude"), count(&dir, "codex")), calls);
    // `resume`, on a run that stopped.
    let dir = project();
    tool(&dir, "claude", CLAUDE_FAILS);
    tool(&dir, "codex", CODEX);
    sheet(&dir, "");
    let run = cu(&dir, &["run", "a task"]);
    says(&run, "Status: Stopped");
    let id = id(&run);
    fs::write(dir.join("bin/ok"), "").unwrap();
    let before = bytes(&dir, &id);
    let calls = (count(&dir, "claude"), count(&dir, "codex"));
    let wrong = cu(&dir, &["--tool", "codex", "resume", &id]);
    assert!(!wrong.status.success(), "{}", out(&wrong));
    says(&wrong, message);
    assert_eq!(bytes(&dir, &id), before, "the notebook changed");
    assert_eq!((count(&dir, "claude"), count(&dir, "codex")), calls);
}

#[test]
fn resume_with_a_tool_writes_it_into_a_notebook_with_no_tool() {
    let dir = setup(CODEX_FAILS);
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    says(&run, "Status: Stopped");
    let id = id(&run);
    forget_tool(&dir, &id);
    fs::write(dir.join("bin/ok"), "").unwrap();
    let resume = cu(&dir, &["--tool", "codex", "resume", &id]);
    says(&resume, "Status: Waiting");
    assert_eq!(saved_tool(&dir, &id).as_deref(), Some("codex"));
    assert_eq!(count(&dir, "claude"), 0, "claude started");
    // A command after that, with no `--tool`, uses the tool in the notebook.
    let last = cu(&dir, &["answer", &id, "ask1", "yes"]);
    says(&last, "Status: Waiting");
    assert_eq!(count(&dir, "claude"), 0, "claude started");
}

#[test]
fn a_notebook_with_no_tool_loads_and_a_command_with_no_tool_writes_none() {
    let dir = setup(CODEX);
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    forget_tool(&dir, &id);
    assert_eq!(count(&dir, "claude"), 1);
    // No `--tool`: Claude, for this command only.
    let answer = cu(&dir, &["answer", &id, "ask1", "yes"]);
    says(&answer, "Status: Waiting");
    assert_eq!((count(&dir, "claude"), count(&dir, "codex")), (2, 0));
    assert_eq!(saved_tool(&dir, &id), None, "a tool was written");
    // A command with `--tool codex` then works.
    let last = cu(&dir, &["--tool", "codex", "answer", &id, "ask2", "yes"]);
    says(&last, "Status: Done");
    assert_eq!((count(&dir, "claude"), count(&dir, "codex")), (2, 1));
}

#[test]
fn a_command_with_a_tool_writes_it_into_a_notebook_with_no_tool() {
    let dir = setup(CODEX);
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    forget_tool(&dir, &id);
    let answer = cu(&dir, &["--tool", "codex", "answer", &id, "ask1", "yes"]);
    says(&answer, "Status: Waiting");
    assert_eq!(saved_tool(&dir, &id).as_deref(), Some("codex"));
    assert_eq!((count(&dir, "claude"), count(&dir, "codex")), (1, 1));
    // A command after that, with no `--tool`, starts codex.
    let last = cu(&dir, &["answer", &id, "ask2", "yes"]);
    says(&last, "Status: Done");
    assert_eq!((count(&dir, "claude"), count(&dir, "codex")), (1, 2));
}

#[test]
fn status_reads_the_notebook_and_does_not_change_it() {
    let dir = setup(CODEX);
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    let id = id(&run);
    let before = bytes(&dir, &id);
    let all: [&[&str]; 3] = [
        &["status", &id],
        &["--tool", "claude", "status", &id],
        &["--tool", "codex", "status", &id],
    ];
    for args in all {
        let status = cu(&dir, args);
        assert!(status.status.success(), "{}", out(&status));
        says(&status, "Status: Waiting");
        assert_eq!(bytes(&dir, &id), before, "{args:?} changed the notebook");
    }
    // A notebook with no tool stays as it is, also with `--tool`.
    forget_tool(&dir, &id);
    let before = bytes(&dir, &id);
    let status = cu(&dir, &["--tool", "codex", "status", &id]);
    assert!(status.status.success(), "{}", out(&status));
    assert_eq!(bytes(&dir, &id), before);
    assert_eq!(saved_tool(&dir, &id), None);
}

#[test]
fn a_step_with_its_own_tool_keeps_it_in_a_run_of_the_claude_tool() {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    tool(&dir, "codex", CODEX);
    sheet(&dir, "tool: codex");
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    assert_eq!((count(&dir, "claude"), count(&dir, "codex")), (1, 0));
    cu(&dir, &["answer", &id, "ask1", "yes"]);
    let last = cu(&dir, &["answer", &id, "ask2", "yes"]);
    says(&last, "Status: Done");
    // Steps `one` and `two` used claude. Step `three` used codex.
    assert_eq!((count(&dir, "claude"), count(&dir, "codex")), (2, 1));
    assert_eq!(saved_tool(&dir, &id).as_deref(), Some("claude"));
}
