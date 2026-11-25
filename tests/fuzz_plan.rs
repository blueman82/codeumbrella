//! Tests of the fuzzer: its files, its seeds, the documents, the repairs of
//! a sum that is too large and the version script. No nightly Rust, no network.

use codeumbrella::engine::{Numbers, Total};
use serde_json::Value;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &str) -> String {
    let full = root().join(path);
    fs::read_to_string(&full).expect(path)
}

fn is_file(path: &str) {
    assert!(root().join(path).is_file(), "no file {path}");
}

const TARGETS: [&str; 3] = ["fuzz_sheet", "fuzz_claude_reply", "fuzz_codex_trace"];

/// The lines of code, counted as `AGENTS.md` says.
fn code_lines(path: &str) -> usize {
    let text = read(path);
    let code = |l: &&str| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with("//")
    };
    text.lines().filter(code).count()
}

#[test]
fn the_fuzz_folder_has_three_targets() {
    is_file("fuzz/Cargo.toml");
    is_file("fuzz/.gitignore");
    for target in TARGETS {
        is_file(&format!("fuzz/fuzz_targets/{target}.rs"));
        let cargo = read("fuzz/Cargo.toml");
        assert!(cargo.contains(target), "{target} is not in fuzz/Cargo.toml");
    }
    assert!(read("fuzz/Cargo.toml").contains("libfuzzer-sys"));
}

#[test]
fn the_fuzz_folder_is_its_own_workspace() {
    let cargo = read("fuzz/Cargo.toml");
    assert!(cargo.contains("[workspace]"), "{cargo}");
    assert!(cargo.contains("cargo-fuzz = true"), "{cargo}");
    assert!(cargo.contains("codeumbrella"), "{cargo}");
}

#[test]
fn the_fuzz_gitignore_keeps_corpus_and_artifacts_out_of_git() {
    let text = read("fuzz/.gitignore");
    assert!(text.contains("corpus"), "{text}");
    assert!(text.contains("artifacts"), "{text}");
    let target = text.lines().any(|l| l.trim_matches('/') == "target");
    assert!(target, "{text}");
    is_file("fuzz/Cargo.lock");
}

#[test]
fn the_fuzz_deny_file_permits_ncsa_and_the_root_file_does_not() {
    let fuzz = read("fuzz/deny.toml");
    assert!(fuzz.contains("NCSA"), "{fuzz}");
    for license in ["MIT", "Apache-2.0", "Unicode-3.0"] {
        assert!(fuzz.contains(license), "{license} is not in fuzz/deny.toml");
    }
    let root = read("deny.toml");
    assert!(!root.contains("NCSA"), "the root deny.toml names NCSA");
}

#[test]
fn the_root_cargo_file_has_no_fuzz_package() {
    let cargo = read("Cargo.toml");
    assert!(!cargo.contains("libfuzzer"), "{cargo}");
    assert!(!cargo.contains("[workspace]"), "{cargo}");
    assert!(!cargo.contains("[profile.fuzz]"), "{cargo}");
    let toolchain = read("rust-toolchain.toml");
    assert!(!toolchain.contains("nightly"), "{toolchain}");
}

#[test]
fn each_target_has_good_seeds() {
    for target in TARGETS {
        let dir = root().join("fuzz/seeds").join(target);
        let count = fs::read_dir(&dir).unwrap().count();
        assert!(count >= 1, "no seed in {}", dir.display());
    }
    is_file("fuzz/seeds/fuzz_sheet/graph.example.yaml");
    let sheet = read("fuzz/seeds/fuzz_sheet/graph.example.yaml");
    assert_eq!(sheet, read("graph.example.yaml"));
}

/// The text of each seed file of a target.
fn seeds(target: &str) -> Vec<String> {
    let dir = root().join("fuzz/seeds").join(target);
    let mut all = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        all.push(fs::read_to_string(entry.unwrap().path()).unwrap());
    }
    all
}

#[test]
fn the_claude_seeds_are_one_line_of_json_with_a_result() {
    let all = seeds("fuzz_claude_reply");
    assert!(all.len() >= 2, "the two lines of the design are needed");
    let mut with_usage = false;
    for text in &all {
        let line = text.trim_end_matches('\n');
        let value: Value = serde_json::from_str(line).unwrap();
        assert_eq!(value["structured_output"]["ok"], true, "{line}");
        with_usage |= value["usage"]["cache_read_input_tokens"] == 3000;
    }
    assert!(with_usage, "no seed with the full numbers");
}

#[test]
fn the_codex_seed_has_two_turn_lines() {
    let all = seeds("fuzz_codex_trace");
    let two = all.iter().any(|t| t.lines().count() == 2);
    assert!(two, "no seed with two lines");
    for line in all.iter().flat_map(|t| t.lines()) {
        let value: Value = serde_json::from_str(line).unwrap();
        assert_eq!(value["type"], "turn.completed", "{line}");
    }
}

#[test]
fn the_crash_folder_and_its_test_are_there() {
    is_file("tests/fuzz_crashes.rs");
    for target in TARGETS {
        let dir = root().join("tests/fuzz_crashes").join(target);
        assert!(dir.is_dir(), "no folder {}", dir.display());
    }
    let text = read("tests/fuzz_crashes.rs");
    for target in TARGETS {
        assert!(text.contains(target), "{target} is not in fuzz_crashes.rs");
    }
}

#[test]
fn the_targets_and_the_crash_test_call_the_readers() {
    let claude = "codeumbrella::claude::";
    let codex = "codeumbrella::codex::";
    for (path, reader) in [
        ("fuzz/fuzz_targets/fuzz_claude_reply.rs", claude),
        ("fuzz/fuzz_targets/fuzz_codex_trace.rs", codex),
        ("tests/fuzz_crashes.rs", claude),
        ("tests/fuzz_crashes.rs", codex),
    ] {
        let text = read(path);
        assert!(text.contains(reader), "{path} has no {reader}");
    }
}

#[test]
fn the_targets_do_not_write_or_print() {
    for target in TARGETS {
        let text = read(&format!("fuzz/fuzz_targets/{target}.rs"));
        assert!(text.contains("fuzz_target!"), "{target}");
        for bad in ["println!", "eprintln!", "fs::write", "File::create"] {
            assert!(!text.contains(bad), "{target} has {bad}");
        }
    }
}

#[test]
fn the_sheet_target_gives_the_roles_folder_of_the_repository() {
    let text = read("fuzz/fuzz_targets/fuzz_sheet.rs");
    assert!(text.contains("roles"), "{text}");
    assert!(text.contains("check"), "{text}");
}

#[test]
fn the_new_files_are_there() {
    is_file("scripts/fuzz-repair");
    is_file("scripts/fuzz-repair.plist");
    let mode = fs::metadata(root().join("scripts/fuzz-repair"))
        .unwrap()
        .permissions()
        .mode();
    assert!(mode & 0o111 != 0, "scripts/fuzz-repair is not executable");
}

#[test]
fn the_script_has_valid_shell_syntax() {
    let path = root().join("scripts/fuzz-repair");
    let first = read("scripts/fuzz-repair");
    let shell = if first.contains("bash") { "bash" } else { "sh" };
    let o = Command::new(shell).arg("-n").arg(path).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn agents_md_has_the_lines_of_design_step_six() {
    let text = read("AGENTS.md");
    assert!(text.contains("`libfuzzer-sys`"), "no package line");
    for path in [
        "fuzz/deny.toml",
        "fuzz/seeds/",
        "tests/fuzz_crashes/",
        "tests/fuzz_crashes.rs",
        "scripts/fuzz-repair",
        "scripts/fuzz-repair.plist",
        ".github/workflows/fuzz.yml",
    ] {
        assert!(text.contains(path), "AGENTS.md has no {path}");
    }
    assert!(text.contains("cargo +nightly fuzz run <target> --sanitizer none"));
    assert!(
        text.contains("19 November 2026"),
        "no approval of the person"
    );
    assert!(text.contains("fuzz/"), "no line for fuzz/");
}

#[test]
fn the_limits_for_lines_of_code_hold() {
    assert!(code_lines("src/claude.rs") <= 200);
    assert!(code_lines("src/codex.rs") <= 200);
    let files = ["graph", "engine", "main", "log", "lib"];
    let sum: usize = files
        .iter()
        .map(|f| code_lines(&format!("src/{f}.rs")))
        .sum();
    // The person set this limit.
    assert!(sum <= 2000, "the supervisor has {sum} lines");
}

#[test]
fn agents_md_names_the_supervisor_limit_of_2000_lines() {
    let text = read("AGENTS.md");
    assert!(text.contains("`src/lib.rs`): 2000 lines of code."));
    assert!(!text.contains("`src/lib.rs`): 1100 lines of code."));
    assert!(text.contains("Each tool part: 200 lines of code."));
}

// A simulated `claude` and `codex` for a run with the real command.

fn folder(name: &str) -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let dir = env::temp_dir().join(format!("cu-fuzz-{name}-{pid}-{n}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("bin")).unwrap();
    fs::canonicalize(&dir).unwrap()
}

fn cu(dir: &Path, args: &[&str]) -> Output {
    let bin = dir.join("bin").display().to_string();
    let old = env::var("PATH").unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(args).current_dir(dir);
    command.env("PATH", format!("{bin}:{old}"));
    command.output().unwrap()
}

fn git(dir: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    let o = command.args(args).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "git {args:?}");
}

fn commit(dir: &Path) {
    git(dir, &["add", "-A"]);
    let who = ["-c", "user.name=t", "-c", "user.email=t@t"];
    let commit = ["commit", "-qm", "x"];
    git(dir, &[who.as_slice(), commit.as_slice()].concat());
}

fn project(tool: &str, script: &str) -> PathBuf {
    let dir = folder("run");
    assert!(cu(&dir, &["init"]).status.success());
    git(&dir, &["init", "--quiet"]);
    commit(&dir);
    let path = dir.join("bin").join(tool);
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let sheet = "start: copy\nsteps:\n  copy:\n    type: command\n    run:\n      - [git, clone, --quiet, --no-hardlinks, ., '.codeumbrella/runs/{run}/work']\n    next: one\n  one:\n    role: planner\n    writes: [spec]\n";
    fs::write(dir.join(".codeumbrella/graph.yaml"), sheet).unwrap();
    dir
}

fn no_panic(o: &Output) {
    let text = String::from_utf8_lossy(&o.stderr);
    assert!(!text.contains("panicked"), "{text}");
    assert_ne!(o.status.code(), Some(101), "{text}");
}

#[test]
fn a_claude_sum_that_is_too_large_gives_no_panic() {
    let max = u64::MAX;
    let line = format!(
        "{{\"is_error\":false,\"result\":\"\",\"structured_output\":{{\"ok\":true,\"summary\":\"done\"}},\"usage\":{{\"input_tokens\":{max},\"cache_creation_input_tokens\":1,\"cache_read_input_tokens\":1,\"output_tokens\":1}}}}"
    );
    let script = format!("echo '{{\"type\":\"system\"}}'\necho '{line}'");
    let dir = project("claude", &script);
    no_panic(&cu(&dir, &["run", "a task"]));
}

#[test]
fn a_codex_sum_that_is_too_large_gives_no_panic() {
    let max = u64::MAX;
    let turn = format!(
        "{{\"type\":\"turn.completed\",\"usage\":{{\"input_tokens\":{max},\"output_tokens\":{max}}}}}"
    );
    let end = "while [ \"$1\" != \"--output-last-message\" ]; do shift; done\necho '{\"ok\":true,\"summary\":\"done\"}' > \"$2\"";
    let script = format!("echo '{turn}'\necho '{turn}'\n{end}");
    let dir = project("codex", &script);
    no_panic(&cu(&dir, &["--tool", "codex", "run", "a task"]));
}

#[test]
fn the_total_of_a_run_stays_at_the_maximum() {
    let mut total = Total::default();
    let numbers = Numbers {
        tokens_in: Some(u64::MAX),
        tokens_out: Some(u64::MAX),
        ..Numbers::default()
    };
    total.add(&numbers);
    total.add(&numbers);
    assert_eq!(total.tokens_in, u64::MAX);
    assert_eq!(total.tokens_out, u64::MAX);
}

// The version script of this project, with a lock file in `fuzz/`.

const OWN: &str = "[[package]]\nname = \"codeumbrella\"\nversion = \"0.7.3\"\n";
const FUZZ: &str = "[[package]]\nname = \"codeumbrella-fuzz\"\nversion = \"0.0.0\"\n";

/// A folder with a bare `origin`, and a work copy with `fuzz/Cargo.lock`.
fn versioned() -> (PathBuf, PathBuf) {
    let base = folder("version");
    let origin = base.join("origin.git");
    let work = base.join("work");
    let url = origin.display().to_string();
    let start = ["init", "--bare", "--initial-branch=master", &url];
    git(&base, &start);
    let name = work.display().to_string();
    git(&base, &["clone", "--quiet", &url, &name]);
    for sub in [".claude-plugin", ".codex-plugin", "fuzz"] {
        fs::create_dir_all(work.join(sub)).unwrap();
    }
    let toml = "[package]\nname = \"codeumbrella\"\nversion = \"0.7.3\"\n";
    let plugin = "{\n  \"version\": \"0.7.3\"\n}\n";
    let lock = format!("{OWN}\n{FUZZ}");
    let files = [
        ("Cargo.toml", toml),
        ("Cargo.lock", OWN),
        ("fuzz/Cargo.lock", lock.as_str()),
        (".claude-plugin/plugin.json", plugin),
        (".codex-plugin/plugin.json", plugin),
    ];
    for (path, text) in files {
        fs::write(work.join(path), text).unwrap();
    }
    commit(&work);
    let push = ["push", "--quiet", "origin", "HEAD:refs/heads/master"];
    git(&work, &push);
    (base, work)
}

fn set_version(base: &Path, work: &Path) -> Output {
    let mut command = Command::new("sh");
    command.arg(root().join("scripts/set-version")).arg(work);
    for who in ["AUTHOR", "COMMITTER"] {
        command.env(format!("GIT_{who}_NAME"), "t");
        command.env(format!("GIT_{who}_EMAIL"), "t@t");
    }
    command.current_dir(base).output().unwrap()
}

#[test]
fn the_version_script_also_sets_the_fuzz_lock_file() {
    let (base, work) = versioned();
    let o = set_version(&base, &work);
    assert!(o.status.success(), "{o:?}");
    let new = OWN.replace("0.7.3", "0.7.4");
    let want = format!("{new}\n{FUZZ}");
    let lock = fs::read_to_string(work.join("fuzz/Cargo.lock")).unwrap();
    assert_eq!(lock, want);
    let mut command = Command::new("git");
    command.current_dir(&work);
    command.args(["diff", "--quiet", "HEAD"]);
    assert!(command.status().unwrap().success(), "not committed");
}

#[test]
fn the_version_script_refuses_a_linked_fuzz_file() {
    for linked in ["fuzz/Cargo.lock", "fuzz"] {
        let (base, work) = versioned();
        let moved = base.join("moved");
        fs::rename(work.join(linked), &moved).unwrap();
        symlink(&moved, work.join(linked)).unwrap();
        let o = set_version(&base, &work);
        assert!(!o.status.success(), "{linked}: {o:?}");
        let file = if moved.is_dir() {
            moved.join("Cargo.lock")
        } else {
            moved
        };
        let text = fs::read_to_string(file).unwrap();
        assert!(text.contains("0.7.3"), "{linked}: {text}");
    }
}

#[test]
fn a_claude_worker_still_gives_the_numbers_of_today() {
    let line = concat!(
        r#"{"is_error":false,"result":"","#,
        r#""structured_output":{"ok":true,"summary":"done"},"#,
        r#""total_cost_usd":1.25,"num_turns":7,"#,
        r#""usage":{"input_tokens":10,"#,
        r#""cache_creation_input_tokens":200,"#,
        r#""cache_read_input_tokens":3000,"output_tokens":40}}"#,
    );
    let dir = project("claude", &format!("echo '{line}'"));
    let o = cu(&dir, &["run", "a task"]);
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(text.contains("Status:   Done"), "{text}");
    assert!(text.contains("Cost:     1.25 dollars"), "{text}");
}
