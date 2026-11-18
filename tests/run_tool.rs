//! Tests: the stop message and the tool that a run remembers. They start the
//! real command with a simulated `claude` and `codex`. No worker, no network.

use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// A new project with the factory files, and a folder for simulated tools.
/// Each test that needs a second run takes a second project: two runs in one
/// project in the same second get the same run name.
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

/// A simulated `claude`. Each call writes one file in `bin`, then gives a good result.
const CLAUDE: &str = r#"for a in "$@"; do echo "$a"; done > "$PWD/bin/claude.$$"
echo '{"is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

/// A simulated `codex`. With a file `bin/fail`, it gives no result.
const CODEX: &str = r#"for a in "$@"; do echo "$a"; done > "$PWD/bin/codex.$$"
[ -f "$PWD/bin/fail" ] && exit 0
while [ "$1" != "--output-last-message" ]; do shift; done
echo '{"ok":true,"summary":"done"}' > "$2""#;

/// A planner step, a question for the person, and a builder step.
const SHEET: &str = r#"start: copy
steps:
  copy:
    type: command
    run:
      - [git, clone, --quiet, --no-hardlinks, ., ".codeumbrella/runs/{run}/work"]
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
"#;

/// A new project with both simulated tools and the sheet above.
fn ready() -> PathBuf {
    let dir = project();
    tool(&dir, "claude", CLAUDE);
    tool(&dir, "codex", CODEX);
    fs::write(dir.join(".codeumbrella/graph.yaml"), SHEET).unwrap();
    dir
}

/// How many times the simulated tool `name` started.
fn started(dir: &Path, name: &str) -> usize {
    let prefix = format!("{name}.");
    let files = fs::read_dir(dir.join("bin")).unwrap();
    files
        .filter(|e| {
            let name = e.as_ref().unwrap().file_name();
            name.to_string_lossy().starts_with(&prefix)
        })
        .count()
}

fn notebook(dir: &Path, id: &str) -> PathBuf {
    dir.join(".codeumbrella/runs").join(id).join("state.json")
}

fn read_state(dir: &Path, id: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(notebook(dir, id)).unwrap()).unwrap()
}

/// Changes the notebook of a run.
fn edit_state(dir: &Path, id: &str, change: impl FnOnce(&mut Value)) {
    let mut state = read_state(dir, id);
    change(&mut state);
    fs::write(notebook(dir, id), state.to_string()).unwrap();
}

/// The two lines that a stopped run prints after the note.
fn stop_lines(id: &str) -> [String; 2] {
    [
        format!("To continue: codeumbrella resume {id}"),
        "Resume only after the person says yes. An AI must not give this approval.".to_owned(),
    ]
}

/// The lines of the output, in sequence.
fn lines(o: &Output) -> Vec<String> {
    out(o).lines().map(str::to_owned).collect()
}

#[test]
fn a_stopped_run_prints_the_two_lines_with_its_own_run_id() {
    // Each project has its own run, and the run id is in the line.
    for _ in 0..2 {
        let dir = ready();
        fs::remove_file(dir.join(".codeumbrella/roles/planner.claude.json")).unwrap();
        let run = cu(&dir, &["run", "a task"]);
        assert!(out(&run).contains("Status: Stopped"), "{}", out(&run));
        let all = lines(&run);
        let status = all.iter().position(|l| l == "Status: Stopped").unwrap();
        let [first, second] = stop_lines(&id(&run));
        let at_first = all.iter().position(|l| *l == first).expect(&first);
        let at_second = all.iter().position(|l| *l == second).expect(&second);
        assert!(status < at_first && at_first < at_second, "{all:?}");
    }
}

#[test]
fn the_two_lines_are_not_cut_by_the_limit_for_the_note() {
    let dir = ready();
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    // A note of 400 characters. The command shows only the first 300 of them.
    let note = format!("{}END", "x".repeat(400));
    edit_state(&dir, &id, |s| {
        s["status"] = "Stopped".into();
        s["note"] = note.into();
    });
    let with_id = ["status", id.as_str()];
    for args in [&with_id[..], &["status"][..]] {
        let shown = cu(&dir, args);
        assert!(shown.status.success(), "{}", out(&shown));
        let all = lines(&shown);
        for line in stop_lines(&id) {
            assert!(all.contains(&line), "{line}: {all:?}");
        }
        assert!(!out(&shown).contains("END"), "the note was not cut");
        let longest = all.iter().map(|l| l.matches('x').count()).max().unwrap();
        assert!((1..=300).contains(&longest), "{longest}");
    }
}

#[test]
fn a_run_that_is_not_stopped_prints_no_stop_lines() {
    let dir = ready();
    let run = cu(&dir, &["run", "a task"]);
    assert!(out(&run).contains("Status: Waiting"), "{}", out(&run));
    let id = id(&run);
    for status in ["Waiting", "Running", "Done"] {
        edit_state(&dir, &id, |s| s["status"] = status.into());
        let shown = cu(&dir, &["status", &id]);
        assert!(out(&shown).contains(&format!("Status: {status}")));
        let all = out(&shown);
        assert!(!all.contains("To continue"), "{status}: {all}");
        assert!(!all.contains("Resume only"), "{status}: {all}");
    }
}

#[test]
fn run_writes_its_tool_into_the_notebook() {
    let dir = ready();
    let run = cu(&dir, &["run", "a task"]);
    assert_eq!(read_state(&dir, &id(&run))["tool"], "claude");
    let dir = ready();
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    assert!(out(&run).contains("Status: Waiting"), "{}", out(&run));
    assert_eq!(read_state(&dir, &id(&run))["tool"], "codex");
}

#[test]
fn an_answer_with_no_tool_option_uses_the_tool_of_the_run() {
    let dir = ready();
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    assert_eq!((started(&dir, "codex"), started(&dir, "claude")), (1, 0));
    let done = cu(&dir, &["answer", &id(&run), "ask", "yes"]);
    assert!(out(&done).contains("Status: Done"), "{}", out(&done));
    // The step after the answer used the simulated codex, and no step used claude.
    assert_eq!((started(&dir, "codex"), started(&dir, "claude")), (2, 0));
}

#[test]
fn a_resume_with_no_tool_option_uses_the_tool_of_the_run() {
    let dir = ready();
    fs::write(dir.join("bin/fail"), "").unwrap();
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    assert!(out(&run).contains("Status: Stopped"), "{}", out(&run));
    fs::remove_file(dir.join("bin/fail")).unwrap();
    let again = cu(&dir, &["resume", &id(&run)]);
    assert!(out(&again).contains("Status: Waiting"), "{}", out(&again));
    assert_eq!((started(&dir, "codex"), started(&dir, "claude")), (2, 0));
}

#[test]
fn an_explicit_tool_option_that_agrees_with_the_run_works() {
    let dir = ready();
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    let id = id(&run);
    let done = cu(&dir, &["--tool", "codex", "answer", &id, "ask", "yes"]);
    assert!(out(&done).contains("Status: Done"), "{}", out(&done));
}

#[test]
fn an_answer_with_a_different_tool_is_an_error_and_starts_no_tool() {
    let dir = ready();
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    let first = id(&run);
    let bad = cu(&dir, &["--tool", "claude", "answer", &first, "ask", "yes"]);
    assert!(!bad.status.success(), "{}", out(&bad));
    assert!(out(&bad).contains("codex"), "{}", out(&bad));
    assert_eq!((started(&dir, "codex"), started(&dir, "claude")), (1, 0));
    // The run still waits for its answer.
    let shown = cu(&dir, &["status", &first]);
    assert!(out(&shown).contains("Status: Waiting"), "{}", out(&shown));

    // A run with the Claude tool refuses `--tool codex`, and the error names Claude.
    let dir = ready();
    let run = cu(&dir, &["run", "a task"]);
    let bad = cu(
        &dir,
        &["--tool", "codex", "answer", &id(&run), "ask", "yes"],
    );
    assert!(!bad.status.success(), "{}", out(&bad));
    assert!(out(&bad).contains("claude"), "{}", out(&bad));
    assert_eq!((started(&dir, "codex"), started(&dir, "claude")), (0, 1));
}

#[test]
fn a_resume_with_a_different_tool_is_an_error_and_starts_no_tool() {
    let dir = ready();
    fs::write(dir.join("bin/fail"), "").unwrap();
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    assert!(out(&run).contains("Status: Stopped"), "{}", out(&run));
    let id = id(&run);
    let bad = cu(&dir, &["--tool", "claude", "resume", &id]);
    assert!(!bad.status.success(), "{}", out(&bad));
    assert!(out(&bad).contains("codex"), "{}", out(&bad));
    assert_eq!((started(&dir, "codex"), started(&dir, "claude")), (1, 0));
    let shown = cu(&dir, &["status", &id]);
    assert!(out(&shown).contains("Status: Stopped"), "{}", out(&shown));
}

#[test]
fn a_notebook_with_no_tool_loads_and_the_command_uses_its_own_tool() {
    // No `--tool` option: the command uses Claude, as before.
    let dir = ready();
    let run = cu(&dir, &["run", "a task"]);
    let old = id(&run);
    edit_state(&dir, &old, |s| {
        s.as_object_mut().unwrap().remove("tool");
    });
    assert!(read_state(&dir, &old).get("tool").is_none());
    let shown = cu(&dir, &["status", &old]);
    assert!(shown.status.success(), "{}", out(&shown));
    let done = cu(&dir, &["answer", &old, "ask", "yes"]);
    assert!(out(&done).contains("Status: Done"), "{}", out(&done));
    assert_eq!((started(&dir, "codex"), started(&dir, "claude")), (0, 2));

    // With `--tool codex`, the command uses Codex for the next step.
    let dir = ready();
    let run = cu(&dir, &["run", "a task"]);
    let old = id(&run);
    edit_state(&dir, &old, |s| {
        s.as_object_mut().unwrap().remove("tool");
    });
    let args = ["--tool", "codex", "answer", &old, "ask", "yes"];
    let done = cu(&dir, &args);
    assert!(out(&done).contains("Status: Done"), "{}", out(&done));
    assert_eq!((started(&dir, "codex"), started(&dir, "claude")), (1, 1));
}

#[test]
fn a_step_with_its_own_tool_line_keeps_that_tool() {
    let dir = ready();
    let own = "  two:\n    role: builder\n    tool: claude";
    let sheet = SHEET.replace("  two:\n    role: builder", own);
    fs::write(dir.join(".codeumbrella/graph.yaml"), sheet).unwrap();
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    let done = cu(&dir, &["answer", &id(&run), "ask", "yes"]);
    assert!(out(&done).contains("Status: Done"), "{}", out(&done));
    assert_eq!((started(&dir, "codex"), started(&dir, "claude")), (1, 1));
}

#[test]
fn the_chat_page_gives_the_resume_command_to_the_person_and_waits_for_a_yes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let page = fs::read_to_string(root.join("skills/codeumbrella/SKILL.md")).unwrap();
    let line = page
        .lines()
        .find(|l| l.contains("`Stopped`"))
        .expect("a line about the status Stopped");
    assert!(line.contains("note"), "{line}");
    assert!(line.contains("codeumbrella resume <run id>"), "{line}");
    assert!(line.to_lowercase().contains("yes"), "{line}");
}
