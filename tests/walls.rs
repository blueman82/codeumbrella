//! Tests of the walls: each one becomes bad if a safety setting or a safety
//! check goes. They read the settings as data, and they start the real command
//! with a simulated `claude` and `codex`. No worker, no network.

use codeumbrella::graph::{Graph, Kind};
use serde_json::Value;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn json(rel: &str) -> Value {
    serde_json::from_str(&text(rel)).unwrap_or_else(|e| panic!("{rel} is not JSON: {e}"))
}

fn list(value: &Value) -> Vec<&str> {
    let items = value.as_array().expect("a list");
    items.iter().map(|v| v.as_str().expect("text")).collect()
}

#[test]
fn the_builder_and_the_checker_run_in_the_claude_sandbox_with_the_locks() {
    for role in ["builder", "checker"] {
        let file = json(&format!("roles/{role}.claude.json"));
        let sandbox = &file["sandbox"];
        assert_eq!(sandbox["enabled"], true, "{role}");
        assert_eq!(sandbox["allowUnsandboxedCommands"], false, "{role}");
        assert_eq!(sandbox["failIfUnavailable"], true, "{role}");
        let network = sandbox["network"].as_object().expect("network");
        assert_eq!(network.keys().collect::<Vec<_>>(), ["allowLocalBinding"]);
        let deny = list(&file["permissions"]["deny"]);
        for lock in [".codeumbrella/**", ".claude/**", ".github/**"] {
            assert!(deny.contains(&format!("Edit({lock})").as_str()), "{role}");
        }
        for lock in ["hooks/**", "config", "commondir", "config.worktree"] {
            assert!(
                deny.contains(&format!("Edit(.git/{lock})").as_str()),
                "{role}"
            );
        }
        assert_eq!(list(&sandbox["filesystem"]["denyRead"]), KEYS, "{role}");
    }
}

/// The folders and files that hold keys of the person.
const KEYS: [&str; 5] = [
    "~/.ssh",
    "~/.config/gh",
    "~/.netrc",
    "~/.git-credentials",
    "~/Library/Keychains",
];

#[test]
fn no_claude_role_runs_hooks_uses_the_web_or_reads_the_keys_of_the_person() {
    for role in ["planner", "builder", "checker", "safety", "attacker"] {
        let file = json(&format!("roles/{role}.claude.json"));
        assert_eq!(file["disableAllHooks"], true, "{role}");
        let deny = list(&file["permissions"]["deny"]);
        for rule in ["WebFetch", "WebSearch"] {
            assert!(deny.contains(&rule), "{role} {rule}");
        }
        for key in KEYS {
            let folder = deny.contains(&format!("Read({key}/**)").as_str());
            let file = deny.contains(&format!("Read({key})").as_str());
            assert!(folder || file, "{role} {key}");
        }
    }
    let planner = json("roles/planner.claude.json");
    assert!(list(&planner["permissions"]["deny"]).contains(&"Bash"));
}

#[test]
fn the_claude_planner_has_no_shell_and_writes_only_the_papers_of_its_run() {
    let file = json("roles/planner.claude.json");
    let allow = list(&file["permissions"]["allow"]);
    let papers = "Edit(/{project}/.codeumbrella/runs/{run}/papers";
    assert_eq!(
        allow,
        [
            "Read",
            "Glob",
            "Grep",
            &format!("{papers}/spec/**)"),
            &format!("{papers}/tests/**)")
        ]
    );
}

#[test]
fn each_codex_role_has_its_sandbox_and_its_locks() {
    let planner = text("roles/planner.codex.txt");
    assert!(planner.starts_with("{extends=\":read-only\""), "{planner}");
    assert!(planner.contains("runs/{run}/papers/spec\"=\"write\""));
    for role in ["builder", "checker"] {
        let file = text(&format!("roles/{role}.codex.txt"));
        assert!(file.starts_with("{extends=\":workspace\""), "{file}");
        assert!(file.contains("mode=\"limited\""), "{file}");
        for lock in [
            ".codeumbrella",
            ".claude",
            ".github",
            ".git/hooks",
            ".git/config",
            ".git/commondir",
            ".git/config.worktree",
        ] {
            assert!(
                file.contains(&format!("\"{lock}\"=\"read\"")),
                "{role} {lock}"
            );
        }
    }
    for role in ["planner", "builder", "checker", "safety", "attacker"] {
        let file = text(&format!("roles/{role}.codex.txt"));
        assert!(!file.contains("danger"));
        // No Codex role can read the folders that hold keys of the person.
        for key in KEYS {
            let lock = format!("\"{}\"=\"deny\"", key.replace('~', "{home}"));
            assert!(file.contains(&lock), "{role} {lock}");
        }
    }
}

#[test]
fn only_the_approval_of_the_person_goes_to_the_merge() {
    let graph = Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap();
    let merge = graph.step("merge").unwrap();
    assert_eq!(
        merge.run,
        [[
            "env",
            "GIT_DIR=.codeumbrella/runs/{run}/clean/.git",
            "gh",
            "pr",
            "merge",
            "codeumbrella/{run}",
            "--squash"
        ]]
    );
    for (name, step) in &graph.steps {
        let to_merge = [&step.next, &step.on_bad].contains(&&Some("merge".to_owned()));
        assert_eq!(to_merge, name == "approval", "{name}");
    }
    assert_eq!(graph.step("approval").unwrap().kind, Kind::Person);
    assert_ne!(graph.start, "merge");
}

#[test]
fn the_push_sends_only_the_checked_commit_to_the_branch_of_the_run() {
    let graph = Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap();
    let clean = ".codeumbrella/runs/{run}/clean";
    let name = "refs/heads/codeumbrella/{run}";
    let both = format!("{name}:{name}");
    let push = &graph.step("push").unwrap().run;
    assert_eq!(
        push,
        &[[
            "git",
            "-C",
            clean,
            "push",
            "--quiet",
            "--no-follow-tags",
            "origin",
            &both
        ]]
    );
    // The step before gets GitHub and the run branch into the clean copy, then compares.
    let guard = &graph.step("no_locked_files").unwrap().run;
    assert_eq!(guard.len(), 6);
    assert!(guard[1..].iter().all(|c| c[..3] == ["git", "-C", clean]));
    assert_eq!(
        guard[2][3..],
        ["fetch", "--quiet", "--recurse-submodules=no", "origin"]
    );
    assert_eq!(guard[4].last().unwrap(), &format!("+{both}"));
    assert_eq!(
        guard[5][3..].join(" "),
        format!(
            "--no-replace-objects diff --quiet --no-ext-diff --no-textconv \
             refs/remotes/origin/HEAD...{name} -- \
             :(icase).github :(icase).claude :(icase).codeumbrella"
        )
    );
    // No command of the sheet uses a word that goes around a check.
    let words = graph.steps.values().flat_map(|s| s.run.iter().flatten());
    for word in words {
        assert!(!["--force", "-f", "--admin", "--no-verify"].contains(&word.as_str()));
    }
}

static COUNT: AtomicU32 = AtomicU32::new(0);

/// A new project with the factory files, and a folder for simulated tools.
fn project() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let dir = env::temp_dir().join(format!("cu-walls-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
    assert!(cu(&dir, &["init"]).status.success());
    // The first step `copy` clones the project, so it must be a git repo.
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
    // Workers start in the work folder, so the script needs the full path.
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
        .env("GH_TOKEN", "a-secret")
        .env("GITHUB_TOKEN", "a-secret")
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

/// A simulated `claude` that records what it got and gives a good result.
const GOOD_CLAUDE: &str = r#"env > "$PWD/bin/env.txt"
for a in "$@"; do echo "$a"; done > "$PWD/bin/args.txt"
p=""; for a in "$@"; do [ "$p" = "--settings" ] && [ ! -f "$PWD/bin/settings.json" ] && printf '%s' "$a" > "$PWD/bin/settings.json"; p="$a"; done
echo '{"is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

#[test]
fn a_claude_worker_gets_its_settings_and_no_github_token() {
    let dir = project();
    tool(&dir, "claude", GOOD_CLAUDE);
    let run = cu(&dir, &["run", "a task"]);
    assert!(out(&run).contains("Status: Waiting"), "{}", out(&run));
    let args = fs::read_to_string(dir.join("bin/args.txt")).unwrap();
    let lines: Vec<&str> = args.lines().collect();
    let at = |word: &str| lines.iter().position(|l| *l == word).expect(word);
    assert_eq!(lines[at("--permission-mode") + 1], "dontAsk");
    assert_eq!(
        lines[at("--setting-sources") + 1],
        "",
        "project settings load"
    );
    at("--strict-mcp-config");
    let settings = fs::read_to_string(dir.join("bin/settings.json")).unwrap();
    let settings: Value = serde_json::from_str(&settings).unwrap();
    let allow = list(&settings["permissions"]["allow"]);
    assert!(allow[3].contains(&format!("runs/{}/papers/spec", id(&run))));
    let env = fs::read_to_string(dir.join("bin/env.txt")).unwrap();
    assert!(!env.contains("a-secret"), "the worker got a GitHub token");
}

#[test]
fn a_claude_role_with_no_settings_file_does_not_start() {
    let dir = project();
    tool(&dir, "claude", GOOD_CLAUDE);
    fs::remove_file(dir.join(".codeumbrella/roles/planner.claude.json")).unwrap();
    let run = cu(&dir, &["run", "a task"]);
    assert!(out(&run).contains("Status: Stopped"), "{}", out(&run));
    assert!(out(&run).contains("cannot read the settings"));
    assert!(!dir.join("bin/args.txt").exists(), "the worker started");
}

#[test]
fn a_worker_that_ends_with_an_error_code_is_not_good() {
    let dir = project();
    tool(&dir, "claude", &format!("{GOOD_CLAUDE}\nexit 3"));
    let run = cu(&dir, &["run", "a task"]);
    assert!(out(&run).contains("Status: Stopped"), "{}", out(&run));
    // This simulated codex writes a good result, and then ends with an error code.
    let codex = r#"while [ "$1" != "--output-last-message" ]; do shift; done
echo '{"ok":true,"summary":"done"}' > "$2"
exit 3"#;
    tool(&dir, "codex", codex);
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    assert!(out(&run).contains("Status: Stopped"), "{}", out(&run));
}

#[test]
fn a_codex_try_cannot_use_the_result_of_a_try_before() {
    let dir = project();
    // The first two calls (plan, attack) write a result. Each call after that writes nothing.
    tool(
        &dir,
        "codex",
        r#"env > "$PWD/bin/env.txt"
for a in "$@"; do echo "$a"; done > "$PWD/bin/args.txt"
[ -f "$PWD/bin/called2" ] && exit 0
if [ -f "$PWD/bin/called" ]; then touch "$PWD/bin/called2"; else touch "$PWD/bin/called"; fi
while [ "$1" != "--output-last-message" ]; do shift; done
echo '{"ok":true,"summary":"try 1"}' > "$2""#,
    );
    let run = cu(&dir, &["--tool", "codex", "run", "a task"]);
    assert!(out(&run).contains("Status: Waiting"), "{}", out(&run));
    let args = fs::read_to_string(dir.join("bin/args.txt")).unwrap();
    assert!(args.contains("--enable\nnetwork_proxy\n"), "{args}");
    assert!(
        args.contains("permissions.role={extends=\":read-only\""),
        "{args}"
    );
    assert!(
        !args.contains("{home}") && !args.contains("{run}"),
        "{args}"
    );
    assert!(
        !fs::read_to_string(dir.join("bin/env.txt"))
            .unwrap()
            .contains("a-secret")
    );
    let no = cu(
        &dir,
        &[
            "--tool",
            "codex",
            "answer",
            &id(&run),
            "look_at_tests",
            "no",
        ],
    );
    assert!(out(&no).contains("Status: Stopped"), "{}", out(&no));
    assert!(out(&no).contains("codex gave no result"), "{}", out(&no));
}

#[test]
fn an_answer_is_only_for_the_step_that_it_names() {
    let dir = project();
    tool(&dir, "claude", GOOD_CLAUDE);
    let run = cu(&dir, &["run", "a task"]);
    let id = id(&run);
    // A yes for the PR does not answer the question about the tests.
    let yes = cu(&dir, &["answer", &id, "approval", "yes"]);
    assert!(!yes.status.success());
    assert!(
        out(&yes).contains("the run is at look_at_tests"),
        "{}",
        out(&yes)
    );
    let status = cu(&dir, &["status", &id]);
    assert!(out(&status).contains("Status: Waiting"), "{}", out(&status));
}

#[test]
fn a_second_command_cannot_work_on_a_run_that_has_a_lock() {
    let dir = project();
    tool(&dir, "claude", GOOD_CLAUDE);
    let run = cu(&dir, &["run", "a task"]);
    let folder = dir.join(".codeumbrella/runs").join(id(&run));
    assert!(
        !folder.join("lock").exists(),
        "the lock stayed after the run"
    );
    fs::write(folder.join("lock"), "").unwrap();
    fs::remove_file(dir.join("bin/args.txt")).unwrap();
    let again = cu(&dir, &["answer", &id(&run), "look_at_tests", "yes"]);
    assert!(!again.status.success());
    assert!(out(&again).contains("a different command works on this run"));
    // The refused command did not write its answer into the notebook.
    let notebook = fs::read_to_string(folder.join("state.json")).unwrap();
    let notebook: Value = serde_json::from_str(&notebook).unwrap();
    assert!(notebook["answer"].is_null(), "{notebook}");
    assert!(!dir.join("bin/args.txt").exists(), "a worker started");
}

/// A sheet with one worker step, and `lines` added to that step.
fn sheet_error(lines: &str) -> String {
    let text = format!("start: a\nsteps:\n  a:\n    role: planner\n{lines}");
    let graph: Graph = yaml_serde::from_str(&text).unwrap();
    let error = graph.check(&root().join("roles")).unwrap_err();
    format!("{error:#}")
}

#[test]
fn the_sheet_check_refuses_a_maximum_of_0_and_a_name_that_is_a_path() {
    assert!(sheet_error("    max_tries: 0\n").contains("max_tries must be 1 or more"));
    for name in ["../../outside", "/tmp/outside", ".hidden", "a/b"] {
        let error = sheet_error(&format!("    writes: [\"{name}\"]\n"));
        assert!(error.contains("must not have a `/`"), "{name}: {error}");
        let error = sheet_error(&format!("    reads: [\"{name}\"]\n"));
        assert!(error.contains("must not have a `/`"), "{name}: {error}");
    }
    let text = "start: a\nsteps:\n  a:\n    type: command\n    run: [[]]\n";
    let graph: Graph = yaml_serde::from_str(text).unwrap();
    assert!(graph.check(&root().join("roles")).is_err());
}

#[test]
fn a_codex_worker_does_not_start_with_no_home_folder() {
    let dir = project();
    tool(&dir, "codex", "touch \"$PWD/bin/started\"");
    // With no home folder, the read locks for the keys point at no folder.
    let path = format!(
        "{}:{}",
        dir.join("bin").display(),
        env::var("PATH").unwrap()
    );
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(["--tool", "codex", "run", "a task"]);
    command
        .current_dir(&dir)
        .env("PATH", path)
        .env_remove("HOME");
    let run = command.output().unwrap();
    assert!(out(&run).contains("HOME is not set"), "{}", out(&run));
    assert!(!dir.join("bin/started").exists(), "the worker started");
}

#[test]
fn the_safety_checker_cannot_write_a_file() {
    let file = json("roles/safety.claude.json");
    let allow = list(&file["permissions"]["allow"]);
    assert_eq!(allow, ["Read", "Glob", "Grep", "Bash"]);
    let deny = list(&file["permissions"]["deny"]);
    assert!(deny.contains(&"Edit(/{project}/**)"), "{deny:?}");
    assert_eq!(file["sandbox"]["enabled"], true);
    assert_eq!(file["sandbox"]["allowUnsandboxedCommands"], false);
    assert_eq!(file["sandbox"]["failIfUnavailable"], true);
    let network = file["sandbox"]["network"].as_object().expect("network");
    assert_eq!(network.keys().collect::<Vec<_>>(), ["allowLocalBinding"]);
    assert_eq!(list(&file["sandbox"]["filesystem"]["denyRead"]), KEYS);
    let codex = text("roles/safety.codex.txt");
    assert!(codex.starts_with("{extends=\":read-only\""), "{codex}");
    assert!(!codex.contains("\"write\""), "{codex}");
}

#[test]
fn the_check_of_the_default_sheet_is_a_fan_with_the_tests_and_the_safety_checker() {
    let graph = Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap();
    let check = graph.step("check").unwrap();
    assert_eq!(check.kind, Kind::Fan);
    assert_eq!(check.fan, ["check_tests", "check_safety"]);
    assert_eq!(check.on_bad.as_deref(), Some("repair"));
    let role = |name| graph.step(name).unwrap().role.clone().unwrap();
    assert_eq!(role("check_tests"), "checker");
    assert_eq!(role("check_safety"), "safety");
}

/// True if the sheet text is refused: by the reader or by the check.
fn refused(text: &str) -> bool {
    match yaml_serde::from_str::<Graph>(text) {
        Err(_) => true,
        Ok(graph) => graph.check(&root().join("roles")).is_err(),
    }
}

/// A sheet with one worker step that has the line `line`.
fn worker(line: &str) -> String {
    format!("start: a\nsteps:\n  a:\n    role: planner\n    {line}\n")
}

/// A sheet with a fan step and one part. `ONE` and `TWO` get the lines of each step.
const FAN: &str = r"start: a
steps:
  a:
    type: fan
    fan: [b]
    ONE
  b:
    role: planner
    TWO
";

/// The characters that `model` and `effort` must not have.
const BAD: &str = " \"}{'\\/;$=,\t\n:#é*";

#[test]
fn a_model_or_effort_with_a_bad_character_is_an_error() {
    for good in ["opus", "gpt-6.2-sol", "a_b.c-1", "xhigh", "A9"] {
        for line in ["model", "effort"] {
            let text = worker(&format!("{line}: '{good}'"));
            assert!(!refused(&text), "{line} {good} is refused");
        }
    }
    for line in ["model", "effort"] {
        let empty = worker(&format!("{line}: \"\""));
        assert!(refused(&empty), "{line} is empty");
        for c in BAD.chars() {
            // A double-quoted YAML text can hold each of these characters.
            let value = format!("a{c}b").replace('\\', "\\\\").replace('"', "\\\"");
            let value = value.replace('\n', "\\n").replace('\t', "\\t");
            let text = worker(&format!("{line}: \"{value}\""));
            assert!(refused(&text), "{line} with {c:?} is not refused");
        }
        // A value that starts with `-` would look like an option to the tool.
        for bad in ["-x", "--dangerous", "-", "-1"] {
            let text = worker(&format!("{line}: \"{bad}\""));
            assert!(refused(&text), "{line} {bad:?} is not refused");
        }
        // A `-` inside the value is good.
        assert!(!refused(&worker(&format!("{line}: \"a-b\""))));
    }
}

#[test]
fn a_tool_that_is_not_claude_or_codex_is_an_error() {
    for good in ["claude", "codex"] {
        assert!(!refused(&worker(&format!("tool: {good}"))), "{good}");
    }
    for bad in [
        "gemini", "Claude", "CODEX", "claude ", "", " codex", "codex}",
    ] {
        let text = worker(&format!("tool: \"{bad}\""));
        assert!(refused(&text), "{bad:?} is not refused");
    }
}

#[test]
fn only_a_worker_step_can_have_tool_model_or_effort() {
    let person = "start: a\nsteps:\n  a:\n    type: person\n";
    let command = "start: a\nsteps:\n  a:\n    type: command\n    run: [[true]]\n";
    for (base, name) in [(person, "person"), (command, "command")] {
        assert!(!refused(base), "{name} base");
        for line in ["tool: claude", "model: opus", "effort: high"] {
            let text = format!("{base}    {line}\n");
            assert!(refused(&text), "a {name} step with {line}");
        }
    }
    assert!(!refused(&FAN.replace("ONE", "").replace("TWO", "")));
    for line in ["tool: claude", "model: opus", "effort: high"] {
        let text = FAN.replace("ONE", line).replace("TWO", "");
        assert!(refused(&text), "a fan step with {line}");
    }
}

#[test]
fn a_part_of_a_fan_can_have_tool_model_and_effort() {
    let lines = "tool: codex\n    model: gpt-6.2-sol\n    effort: high";
    assert!(!refused(&FAN.replace("ONE", "").replace("TWO", lines)));
}
