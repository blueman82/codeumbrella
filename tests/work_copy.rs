//! Tests of the work copy: each worker starts in `<run folder>/work`, and the
//! project folder stays as it was. They use a simulated `claude` and `codex`.

use codeumbrella::engine::Run;
use codeumbrella::graph::{Graph, Kind};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs, process};

static COUNT: AtomicU32 = AtomicU32::new(0);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn sheet() -> Graph {
    Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap()
}

/// A new empty folder, with its full real path.
fn scratch() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let dir = env::temp_dir().join(format!("cu-work-{}-{n}", process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {o:?}");
    String::from_utf8_lossy(&o.stdout).trim().to_owned()
}

/// A git project with the factory files and one commit, in a folder with the
/// given name. The simulated tools are in a different folder, outside the project.
fn project_named(name: &str) -> (PathBuf, PathBuf) {
    let top = scratch();
    let tools = top.join("tools");
    let dir = top.join(name);
    fs::create_dir_all(&tools).unwrap();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("README.md"), "a project\n").unwrap();
    git(&dir, &["init", "--quiet"]);
    assert!(cu(&dir, &tools, &["init"]).status.success());
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "--quiet", "-m", "first"]);
    (dir, tools)
}

fn project() -> (PathBuf, PathBuf) {
    project_named("project")
}

/// Writes a simulated tool: a shell script in the tools folder.
fn tool(tools: &Path, name: &str, script: &str) {
    let path = tools.join(name);
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// Starts the real command in the project, with the simulated tools first on the PATH.
fn cu(dir: &Path, tools: &Path, args: &[&str]) -> Output {
    let path = format!("{}:{}", tools.display(), env::var("PATH").unwrap());
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
    let line = all
        .lines()
        .find(|l| l.starts_with("Run:      "))
        .expect("a run");
    line["Run:      ".len()..].to_owned()
}

/// A simulated `claude`: it records where it started and what it got, writes
/// a file in its current folder, and gives a good result.
fn claude(tools: &Path) -> String {
    let t = tools.display();
    format!(
        r#"pwd -P > "{t}/cwd.txt"
for a in "$@"; do echo "$a"; done > "{t}/args.txt"
p=""; for a in "$@"; do [ "$p" = "--settings" ] && [ ! -f "{t}/settings.json" ] && printf '%s' "$a" > "{t}/settings.json"; p="$a"; done
touch evil.txt
echo '{{"is_error":false,"result":"","structured_output":{{"ok":true,"summary":"done"}}}}'"#
    )
}

/// A simulated `codex`: it records where it started and what it got, and gives a good result.
fn codex(tools: &Path) -> String {
    let t = tools.display();
    format!(
        r#"pwd -P > "{t}/cwd.txt"
[ -f "{t}/args.txt" ] || for a in "$@"; do echo "$a"; done > "{t}/args.txt"
while [ "$1" != "--output-last-message" ]; do shift; done
echo '{{"ok":true,"summary":"done"}}' > "$2""#
    )
}

#[test]
fn a_new_run_has_an_empty_work_folder_next_to_the_papers() {
    let dir = scratch().join("run-1");
    let _run = Run::start(dir.clone(), "a task", &sheet()).unwrap();
    assert!(dir.join("papers").is_dir());
    assert!(dir.join("work").is_dir());
    assert_eq!(fs::read_dir(dir.join("work")).unwrap().count(), 0);
}

#[test]
fn a_claude_worker_starts_in_the_work_folder_and_the_project_stays_as_it_was() {
    let (dir, tools) = project();
    tool(&tools, "claude", &claude(&tools));
    let branch = git(&dir, &["branch", "--show-current"]);
    let head = git(&dir, &["rev-parse", "HEAD"]);
    let run = cu(&dir, &tools, &["run", "a task"]);
    assert!(out(&run).contains("Status:   Waiting"), "{}", out(&run));
    let id = id(&run);
    let work = dir.join(".codeumbrella/runs").join(&id).join("work");
    let cwd = fs::read_to_string(tools.join("cwd.txt")).unwrap();
    assert_eq!(PathBuf::from(cwd.trim()), work.canonicalize().unwrap());
    // The copy holds the project, and the worker wrote there.
    assert!(work.join(".git").exists());
    assert!(work.join("README.md").exists());
    assert!(work.join("evil.txt").exists());
    // The project folder and its branch did not change.
    assert!(!dir.join("evil.txt").exists());
    assert_eq!(git(&dir, &["status", "--porcelain"]), "");
    assert_eq!(git(&dir, &["branch", "--show-current"]), branch);
    assert_eq!(git(&dir, &["rev-parse", "HEAD"]), head);
}

#[test]
fn a_claude_worker_gets_full_paths_for_the_papers_and_the_project() {
    let (dir, tools) = project();
    tool(&tools, "claude", &claude(&tools));
    let run = cu(&dir, &tools, &["run", "a task"]);
    let id = id(&run);
    let args = fs::read_to_string(tools.join("args.txt")).unwrap();
    let papers = format!("{}/.codeumbrella/runs/{id}/papers", dir.display());
    assert!(
        args.contains(&format!("Each paper is a folder in {papers}.")),
        "{args}"
    );
    // In the settings, `{project}` is the project folder, not the work folder.
    let settings = fs::read_to_string(tools.join("settings.json")).unwrap();
    assert!(settings.contains(&format!(
        "{}/.codeumbrella/runs/{id}/papers/spec",
        dir.display()
    )));
    assert!(!settings.contains("{project}"), "{settings}");
    assert!(!settings.contains("/work/.codeumbrella"), "{settings}");
}

#[test]
fn a_codex_worker_starts_in_the_work_folder_with_the_full_project_path_in_its_settings() {
    let (dir, tools) = project();
    tool(&tools, "codex", &codex(&tools));
    let run = cu(&dir, &tools, &["--tool", "codex", "run", "a task"]);
    assert!(out(&run).contains("Status:   Waiting"), "{}", out(&run));
    let id = id(&run);
    let work = dir.join(".codeumbrella/runs").join(&id).join("work");
    let cwd = fs::read_to_string(tools.join("cwd.txt")).unwrap();
    assert_eq!(PathBuf::from(cwd.trim()), work.canonicalize().unwrap());
    let args = fs::read_to_string(tools.join("args.txt")).unwrap();
    let papers = format!("{}/.codeumbrella/runs/{id}/papers", dir.display());
    assert!(
        args.contains(&format!("\"{papers}/spec\"=\"write\"")),
        "{args}"
    );
    assert!(
        args.contains(&format!("\"{papers}/tests\"=\"write\"")),
        "{args}"
    );
    assert!(
        !args.contains("{project}") && !args.contains("{run}"),
        "{args}"
    );
    assert!(!dir.join("evil.txt").exists());
}

#[test]
fn a_codex_worker_does_not_start_in_a_project_path_with_a_bad_character() {
    for bad in ["a\"b", "a\\b", "a{b", "a}b", "a\nb"] {
        let (dir, tools) = project_named(bad);
        tool(
            &tools,
            "codex",
            &format!("touch \"{}/started\"", tools.display()),
        );
        let run = cu(&dir, &tools, &["--tool", "codex", "run", "a task"]);
        assert!(
            out(&run).contains("Status:   Stopped"),
            "{bad:?}: {}",
            out(&run)
        );
        assert!(
            !tools.join("started").exists(),
            "{bad:?}: the worker started"
        );
    }
}

#[test]
fn the_default_sheet_copies_the_project_first_and_gets_the_run_branch_from_the_work_copy() {
    let graph = sheet();
    assert_eq!(graph.start, "copy");
    let copy = graph.step("copy").unwrap();
    assert_eq!(copy.kind, Kind::Command);
    assert_eq!(copy.next.as_deref(), Some("plan"));
    assert_eq!(
        copy.run,
        [[
            "git",
            "clone",
            "--quiet",
            "--no-hardlinks",
            ".",
            ".codeumbrella/runs/{run}/work"
        ]]
    );
    let clean = ".codeumbrella/runs/{run}/clean";
    let name = "refs/heads/codeumbrella/{run}";
    let guard = &graph.step("no_locked_files").unwrap().run;
    assert_eq!(guard.len(), 6);
    assert_eq!(
        guard[4],
        [
            "git",
            "-C",
            clean,
            "fetch",
            "--quiet",
            "--recurse-submodules=no",
            "../work",
            &format!("+{name}:{name}")
        ]
    );
    // The other commands are as they were.
    assert_eq!(guard[0], ["git", "init", "--quiet", clean]);
    assert_eq!(guard[1].last().unwrap(), "../../../../../.git/config");
    assert_eq!(guard[3][3..], ["remote", "set-head", "origin", "--auto"]);
    let words = graph.steps.values().flat_map(|s| s.run.iter().flatten());
    assert!(words.into_iter().all(|w| w != "../../../.."));
}

#[test]
fn the_codex_planner_papers_are_full_paths_next_to_the_home_keys() {
    let file = text("roles/planner.codex.txt");
    for paper in ["spec", "tests"] {
        let key = format!("\"{{project}}/.codeumbrella/runs/{{run}}/papers/{paper}\"=\"write\"");
        assert!(file.contains(&key), "{file}");
    }
    assert!(!file.contains(":workspace_roots"), "{file}");
    assert!(file.contains("\"{home}/.ssh\"=\"deny\""), "{file}");
}

#[test]
fn the_builder_and_the_checker_can_edit_only_the_work_copy_of_their_run() {
    for role in ["builder", "checker"] {
        let file: serde_json::Value =
            serde_json::from_str(&text(&format!("roles/{role}.claude.json"))).unwrap();
        let allow: Vec<&str> = file["permissions"]["allow"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let rule = "Edit(/{project}/.codeumbrella/runs/{run}/work/**)";
        assert_eq!(allow, ["Read", "Glob", "Grep", rule, "Bash"], "{role}");
        // The locks stay.
        let deny = file["permissions"]["deny"].to_string();
        for lock in [
            "Edit(.codeumbrella/**)",
            "Edit(.git/config)",
            "Edit(.git/hooks/**)",
        ] {
            assert!(deny.contains(lock), "{role} {lock}");
        }
    }
}

#[test]
fn the_notes_do_not_say_that_a_worker_writes_in_the_project_folder() {
    let agents = text("AGENTS.md");
    assert!(
        !agents.contains("cannot write to a folder that is not in the project"),
        "AGENTS.md still has the old sentence"
    );
    assert!(agents.contains("work"));
}

#[test]
fn the_documents_tell_about_the_work_copy() {
    let agents = text("AGENTS.md");
    assert!(!agents.to_lowercase().contains("known limit"));
    assert!(agents.contains("git pull"));
    let skill = text("skills/codeumbrella/SKILL.md");
    assert!(!skill.contains("git switch"));
    assert!(skill.contains("git pull"));
    let readme = text("README.md");
    assert!(readme.contains("git pull"));
    assert!(readme.contains(".venv"));
    assert!(text("ARCHITECTURE.md").contains("work"));
}
