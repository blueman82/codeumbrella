//! Tests of the free space check, of `codeumbrella clean`, of the copy of
//! the learner and of the documents. No Claude, no Codex and no network.

use codeumbrella::graph::Graph;
use serde_json::json;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// A sheet with one step for the person.
const SHEET: &str = "start: ask\nsteps:\n  ask:\n    type: person\n";

/// The file that names the run of a temp folder.
const MARK: &str = "codeumbrella-run";

/// The free blocks of a disk with much free space.
const PLENTY: u64 = 900_000_000;

/// The files of a run folder that stay.
const RECORD: [&str; 5] = [
    "state.json",
    "log.jsonl",
    "papers/spec/spec.md",
    "new-work.bundle",
    "plan.1.trace.jsonl",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let path = root().join(rel);
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// A new empty folder, with its full path.
fn folder() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-disk-{}-{n}", std::process::id());
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

/// A run id that no other test and no run of the computer has.
fn id(name: &str) -> String {
    format!("run-{name}-{}", std::process::id())
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A file with a length of `bytes`. It uses almost no disk space.
fn large(path: &Path, bytes: u64) {
    write(path, "");
    let mut options = fs::File::options();
    let file = options.write(true).open(path).unwrap();
    file.set_len(bytes).unwrap();
}

fn set_mode(path: &Path, mode: u32) {
    let mode = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, mode).unwrap();
}

/// The built command in `dir`. Its temp folder is `dir/tmp`.
fn command(dir: &Path, args: &[&str]) -> Command {
    let temp = dir.join("tmp");
    fs::create_dir_all(&temp).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(args).current_dir(dir).env("TMPDIR", temp);
    command.env_remove("CLAUDE_CODE_TMPDIR");
    command
}

fn cu(dir: &Path, args: &[&str]) -> Output {
    command(dir, args).output().unwrap()
}

fn all(out: &Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    format!("{stdout}{stderr}")
}

/// The output of a command that must succeed.
fn good(out: &Output) -> String {
    assert!(out.status.success(), "{}", all(out));
    all(out)
}

/// The output of a command that must give an error.
fn bad(out: &Output) -> String {
    assert!(!out.status.success(), "{}", all(out));
    all(out)
}

fn has_line(printed: &str, line: &str) -> bool {
    printed.lines().any(|l| l == line)
}

// ---------------------------------------------------------------------
// The free space check
// ---------------------------------------------------------------------

/// A project whose run stops at once at a step for the person.
fn project() -> PathBuf {
    let dir = folder();
    good(&cu(&dir, &["init"]));
    write(&dir.join(".codeumbrella/graph.yaml"), SHEET);
    dir
}

/// Starts the command with a simulated `df` in its `PATH`.
fn with_df(dir: &Path, script: &str, args: &[&str]) -> Output {
    let tool = dir.join("bin/df");
    write(&tool, &format!("#!/bin/sh\n{script}\n"));
    set_mode(&tool, 0o755);
    let before = env::var("PATH").unwrap();
    let path = format!("{}:{before}", dir.join("bin").display());
    command(dir, args).env("PATH", path).output().unwrap()
}

/// The script of a `df` with `blocks` free blocks of 1024 bytes. The name
/// of the disk has spaces.
fn free(blocks: u64) -> String {
    let head = "Filesystem 1024-blocks Used Available Capacity Mounted";
    let line = format!("map auto home 900 800 {blocks} 50% /a b");
    format!("echo '{head}'\necho '{line}'")
}

fn run_count(dir: &Path) -> usize {
    let runs = dir.join(".codeumbrella/runs");
    fs::read_dir(runs).map_or(0, Iterator::count)
}

#[test]
fn run_starts_no_run_below_the_limit_and_names_the_two_numbers() {
    let dir = project();
    let out = with_df(&dir, &free(3_000_000), &["run", "a task"]);
    let printed = bad(&out);
    let words = ["3.0 GB free", "limit is 10 GB", "codeumbrella clean"];
    for word in words {
        assert!(printed.contains(word), "{word}: {printed}");
    }
    assert_eq!(run_count(&dir), 0);
}

#[test]
fn run_next_below_the_limit_keeps_the_task_in_the_queue() {
    let dir = project();
    let task = dir.join(".codeumbrella/queue/01-one.txt");
    write(&task, "a task");
    let out = with_df(&dir, &free(3_000_000), &["run", "--next"]);
    let printed = bad(&out);
    assert!(printed.contains("limit is 10 GB"), "{printed}");
    assert!(task.is_file(), "the task is gone");
    assert_eq!(run_count(&dir), 0);
}

#[test]
fn the_limit_is_10_gb_and_a_disk_with_that_space_is_good() {
    let dir = project();
    let out = with_df(&dir, &free(9_765_624), &["run", "a task"]);
    let printed = bad(&out);
    assert!(printed.contains("9.9 GB free"), "{printed}");
    assert_eq!(run_count(&dir), 0);
    let out = with_df(&dir, &free(9_765_625), &["run", "a task"]);
    let printed = good(&out);
    assert!(printed.contains("Status:   Waiting"), "{printed}");
    assert_eq!(run_count(&dir), 1);
}

#[test]
fn the_option_min_free_gb_sets_a_different_limit() {
    let dir = project();
    let script = free(3_000_000);
    let args = ["--min-free-gb", "4", "run", "a task"];
    let printed = bad(&with_df(&dir, &script, &args));
    assert!(printed.contains("limit is 4 GB"), "{printed}");
    assert_eq!(run_count(&dir), 0);
    let args = ["--min-free-gb", "2", "run", "a task"];
    good(&with_df(&dir, &script, &args));
    assert_eq!(run_count(&dir), 1);
}

#[test]
fn with_a_limit_of_0_the_command_does_not_use_df() {
    let dir = project();
    let args = ["--min-free-gb", "0", "run", "a task"];
    good(&with_df(&dir, "touch used\nexit 1", &args));
    assert!(!dir.join("used").exists(), "df ran");
    assert_eq!(run_count(&dir), 1);
}

#[test]
fn a_df_with_an_error_or_no_number_starts_no_run() {
    let dir = project();
    let error = format!("{}\nexit 1", free(PLENTY));
    for script in [error.as_str(), "echo nothing", "true"] {
        let out = with_df(&dir, script, &["run", "a task"]);
        let printed = bad(&out);
        assert!(printed.contains("df"), "{script}: {printed}");
        assert_eq!(run_count(&dir), 0, "{script}");
    }
}

#[test]
fn df_gets_the_options_and_the_full_path_of_the_project() {
    let dir = project();
    let script = format!("echo \"$@\" > words\n{}", free(PLENTY));
    good(&with_df(&dir, &script, &["run", "a task"]));
    let words = fs::read_to_string(dir.join("words")).unwrap();
    assert_eq!(words.trim(), format!("-Pk {}", dir.display()));
}

#[test]
fn answer_does_not_check_the_free_space() {
    let dir = project();
    let out = with_df(&dir, &free(PLENTY), &["run", "a task"]);
    let printed = good(&out);
    let line = printed.lines().find(|l| l.starts_with("Run: "));
    let run = line.unwrap()[10..].to_owned();
    let args = ["answer", run.as_str(), "ask", "yes"];
    let printed = good(&with_df(&dir, &free(1), &args));
    assert!(printed.contains("Status:   Done"), "{printed}");
}

// ---------------------------------------------------------------------
// The command `clean`
// ---------------------------------------------------------------------

/// A run folder with a notebook, the two copies and the record. The copies
/// have 3,000,000 bytes.
fn record(project: &Path, id: &str, status: &str) -> PathBuf {
    let run = project.join(".codeumbrella/runs").join(id);
    let book = json!({
        "task": "t", "status": status, "at": null, "note": "",
        "answer": null, "tries": {}, "results": {}, "prints": {},
        "last": null,
    });
    for name in RECORD {
        write(&run.join(name), "x");
    }
    write(&run.join("state.json"), &book.to_string());
    large(&run.join("work/file"), 2_000_000);
    large(&run.join("clean/file"), 1_000_000);
    run
}

fn gone(run: &Path) -> bool {
    !run.join("work").exists() && !run.join("clean").exists()
}

fn kept(run: &Path) -> bool {
    let work = run.join("work/file").is_file();
    work && run.join("clean/file").is_file()
}

#[test]
fn clean_removes_the_copies_of_done_and_stopped_and_keeps_the_record() {
    let dir = folder();
    let done = record(&dir, &id("done"), "Done");
    let stopped = record(&dir, &id("stopped"), "Stopped");
    let printed = good(&cu(&dir, &["clean"]));
    assert!(has_line(&printed, "Freed:    6 MB"), "{printed}");
    for run in [done, stopped] {
        assert!(gone(&run), "{}", run.display());
        for name in RECORD {
            assert!(run.join(name).is_file(), "{name}");
        }
        assert!(!run.join("lock").exists(), "a lock stays");
    }
}

#[test]
fn clean_does_not_touch_a_running_or_a_waiting_run() {
    let dir = folder();
    for status in ["Running", "Waiting"] {
        let name = id(status);
        let run = record(&dir, &name, status);
        let scratch = dir.join("tmp").join(format!("scratch.{status}"));
        marked(&scratch, &name);
        let before = fs::read(run.join("state.json")).unwrap();
        let printed = good(&cu(&dir, &["clean"]));
        assert!(has_line(&printed, "Freed:    0 MB"), "{printed}");
        assert!(kept(&run), "{status}");
        assert!(scratch.join(MARK).is_file(), "{status}");
        let after = fs::read(run.join("state.json")).unwrap();
        assert_eq!(before, after, "{status}");
        assert!(!run.join("lock").exists(), "a lock stays");
    }
}

#[test]
fn clean_skips_a_run_with_a_lock_and_names_it() {
    let dir = folder();
    let name = id("held");
    let run = record(&dir, &name, "Stopped");
    write(&run.join("lock"), "1");
    let printed = good(&cu(&dir, &["clean"]));
    let line = printed.lines().find(|l| l.starts_with("Not cleaned:"));
    let line = line.unwrap_or_else(|| panic!("{printed}"));
    assert!(line.contains(&name), "{line}");
    assert!(line.contains("lock"), "{line}");
    assert!(kept(&run));
    assert!(run.join("lock").is_file(), "the lock is gone");
}

#[test]
fn clean_with_a_run_id_cleans_only_that_run() {
    let dir = folder();
    let name = id("one");
    let chosen = record(&dir, &name, "Stopped");
    let other = record(&dir, &id("other"), "Stopped");
    let scratch = dir.join("tmp/scratch.other");
    marked(&scratch, &id("other"));
    let printed = good(&cu(&dir, &["clean", &name]));
    assert!(has_line(&printed, "Freed:    3 MB"), "{printed}");
    assert!(gone(&chosen));
    assert!(kept(&other));
    assert!(scratch.is_dir(), "the other scratch folder is gone");
}

#[test]
fn clean_with_the_id_of_an_active_run_gives_an_error() {
    let dir = folder();
    for status in ["Running", "Waiting"] {
        let name = id(status);
        let run = record(&dir, &name, status);
        let printed = bad(&cu(&dir, &["clean", &name]));
        assert!(printed.contains(status), "{printed}");
        assert!(kept(&run), "{status}");
    }
}

#[test]
fn clean_with_an_unknown_id_or_a_path_removes_nothing() {
    let dir = folder();
    let run = record(&dir, &id("safe"), "Done");
    let name = id("unknown");
    let printed = bad(&cu(&dir, &["clean", &name]));
    assert!(printed.contains(&name), "{printed}");
    // A notebook and a copy right above the folder `runs`.
    let above = dir.join(".codeumbrella");
    let book = fs::read_to_string(run.join("state.json")).unwrap();
    write(&above.join("state.json"), &book);
    write(&above.join("work/file"), "x");
    bad(&cu(&dir, &["clean", ".."]));
    assert!(above.join("work/file").is_file());
    // An id that starts at a run folder and goes up.
    let up = format!("{}/../..", id("safe"));
    bad(&cu(&dir, &["clean", &up]));
    assert!(above.join("work/file").is_file(), "{up}");
    assert!(kept(&run));
    // The full path of a run of a different project.
    let far = record(&folder(), &id("far-path"), "Done");
    bad(&cu(&dir, &["clean", far.to_str().unwrap()]));
    assert!(kept(&far), "an absolute path");
}

#[test]
fn clean_removes_nothing_of_a_run_whose_lock_it_cannot_make() {
    let dir = folder();
    let name = id("no-lock");
    let run = record(&dir, &name, "Stopped");
    // The run folder gets no new file. The files in the copies can go.
    set_mode(&run, 0o500);
    let out = cu(&dir, &["clean"]);
    set_mode(&run, 0o700);
    let printed = all(&out);
    assert!(kept(&run), "a file is gone: {printed}");
    let named = |l: &str| l.starts_with("Not cleaned:") && l.contains(&name);
    assert!(printed.lines().any(named), "{printed}");
}

#[test]
fn clean_with_no_runs_frees_nothing() {
    let dir = folder();
    let printed = good(&cu(&dir, &["clean"]));
    assert!(has_line(&printed, "Freed:    0 MB"), "{printed}");
}

#[test]
fn clean_keeps_a_folder_with_no_notebook() {
    let dir = folder();
    let run = dir.join(".codeumbrella/runs").join(id("bare"));
    write(&run.join("work/file"), "x");
    good(&cu(&dir, &["clean"]));
    assert!(run.join("work/file").is_file());
}

#[test]
fn clean_gives_a_large_space_in_gb() {
    let dir = folder();
    let run = record(&dir, &id("large"), "Done");
    large(&run.join("work/file"), 1_590_000_000);
    let printed = good(&cu(&dir, &["clean"]));
    assert!(has_line(&printed, "Freed:    1.5 GB"), "{printed}");
}

#[test]
fn clean_follows_no_symbolic_link() {
    let dir = folder();
    let elsewhere = folder();
    write(&elsewhere.join("file"), "x");
    let first = record(&dir, &id("link"), "Done");
    fs::remove_dir_all(first.join("work")).unwrap();
    symlink(&elsewhere, first.join("work")).unwrap();
    let second = record(&dir, &id("inner"), "Done");
    symlink(&elsewhere, second.join("work/link")).unwrap();
    // A run folder that is a link.
    let foreign = folder();
    let source = record(&foreign, &id("far"), "Done");
    let linked = dir.join(".codeumbrella/runs").join(id("far"));
    symlink(&source, linked).unwrap();
    let scratch = dir.join("tmp/scratch.far");
    marked(&scratch, &id("far"));
    cu(&dir, &["clean"]);
    assert!(elsewhere.join("file").is_file());
    assert!(gone(&second));
    assert!(kept(&source));
    assert!(scratch.is_dir(), "the scratch folder of the link");
    // The same with the id of the link.
    cu(&dir, &["clean", &id("far")]);
    assert!(kept(&source), "clean with the id of a link");
    assert!(scratch.is_dir(), "the scratch folder of the link");
}

#[test]
fn clean_goes_on_after_a_folder_that_it_cannot_remove() {
    let dir = folder();
    let first = record(&dir, &id("a-shut"), "Done");
    let second = record(&dir, &id("b-open"), "Done");
    let shut = first.join("work/shut");
    write(&shut.join("file"), "x");
    set_mode(&shut, 0o500);
    let out = cu(&dir, &["clean"]);
    set_mode(&shut, 0o700);
    let printed = bad(&out);
    assert!(printed.contains("Not cleaned:"), "{printed}");
    assert!(gone(&second));
}

// ---------------------------------------------------------------------
// The temp folders
// ---------------------------------------------------------------------

/// A temp folder of a worker with the marker of the run `id`.
fn marked(scratch: &Path, id: &str) {
    write(&scratch.join(MARK), &format!("{id}\n"));
    write(&scratch.join("repo/file"), "x");
}

#[test]
fn clean_removes_the_marked_temp_folders_of_a_cleaned_run() {
    let dir = folder();
    let name = id("temp");
    record(&dir, &name, "Stopped");
    let more = folder();
    let found = [
        dir.join("tmp/scratch.AbC123"),
        dir.join("tmp/lesson.dEf456"),
        dir.join("tmp/claude-501/scratch.gHi789"),
        more.join("scratch.jKl012"),
        more.join("claude-7/lesson.mNo345"),
    ];
    for scratch in &found {
        marked(scratch, &name);
    }
    large(&found[0].join("large"), 2_000_000);
    let mut clean = command(&dir, &["clean"]);
    clean.env("CLAUDE_CODE_TMPDIR", &more);
    let printed = good(&clean.output().unwrap());
    assert!(has_line(&printed, "Freed:    5 MB"), "{printed}");
    for scratch in &found {
        assert!(!scratch.exists(), "{}", scratch.display());
    }
}

#[test]
fn clean_removes_a_temp_folder_of_a_run_that_has_no_copies() {
    let dir = folder();
    let name = id("empty");
    let run = record(&dir, &name, "Done");
    fs::remove_dir_all(run.join("work")).unwrap();
    fs::remove_dir_all(run.join("clean")).unwrap();
    let scratch = dir.join("tmp/scratch.pQr678");
    marked(&scratch, &name);
    good(&cu(&dir, &["clean", &name]));
    assert!(!scratch.exists(), "the scratch folder stays");
}

#[test]
fn clean_keeps_each_temp_folder_that_is_not_of_a_cleaned_run() {
    let dir = folder();
    let name = id("keep");
    record(&dir, &name, "Done");
    let temp = dir.join("tmp");
    let different = temp.join("scratch.different");
    marked(&different, "run-of-nobody");
    let longer = temp.join("scratch.longer");
    marked(&longer, &format!("{name}-1"));
    let unmarked = temp.join("scratch.unmarked");
    write(&unmarked.join("file"), "x");
    let unnamed = temp.join("other.folder");
    marked(&unnamed, &name);
    let deep = temp.join("project/scratch.deep");
    marked(&deep, &name);
    let linked = temp.join("scratch.linkmark");
    let source = dir.join("elsewhere/mark");
    write(&source, &name);
    write(&linked.join("file"), "x");
    symlink(source, linked.join(MARK)).unwrap();
    let target = dir.join("elsewhere/target");
    marked(&target, &name);
    symlink(&target, temp.join("scratch.link")).unwrap();
    // A `claude` folder in a base that is a link.
    let hidden = dir.join("elsewhere/claude");
    let behind = hidden.join("scratch.behind");
    marked(&behind, &name);
    symlink(&hidden, temp.join("claude-link")).unwrap();
    good(&cu(&dir, &["clean"]));
    let stay = [different, longer, unmarked, unnamed, deep, linked];
    for scratch in stay {
        assert!(scratch.is_dir(), "{}", scratch.display());
    }
    assert!(target.join("repo/file").is_file(), "the link");
    assert!(behind.is_dir(), "the claude link");
}

#[test]
fn clean_follows_a_base_folder_that_is_a_link() {
    let dir = folder();
    let name = id("base-link");
    record(&dir, &name, "Done");
    let base = folder();
    let scratch = base.join("scratch.sTu901");
    marked(&scratch, &name);
    let link = dir.join("base");
    symlink(&base, &link).unwrap();
    let mut clean = command(&dir, &["clean"]);
    clean.env("CLAUDE_CODE_TMPDIR", &link);
    good(&clean.output().unwrap());
    assert!(!scratch.exists(), "the scratch folder stays");
}

#[test]
fn clean_handles_a_temp_folder_of_two_bases_once() {
    let dir = folder();
    let name = id("twice");
    record(&dir, &name, "Done");
    let scratch = dir.join("tmp/scratch.vWx234");
    marked(&scratch, &name);
    large(&scratch.join("large"), 2_000_000);
    // A second name of the temp folder of the command.
    let link = dir.join("again");
    symlink(dir.join("tmp"), &link).unwrap();
    for second in [dir.join("tmp"), link] {
        let mut clean = command(&dir, &["clean"]);
        clean.env("CLAUDE_CODE_TMPDIR", &second);
        let printed = good(&clean.output().unwrap());
        assert!(!printed.contains("Not cleaned:"), "{printed}");
        assert!(!scratch.exists(), "the scratch folder stays");
        assert!(has_line(&printed, "Freed:    5 MB"), "{printed}");
        // The same state again, for the next second name.
        record(&dir, &name, "Done");
        marked(&scratch, &name);
        large(&scratch.join("large"), 2_000_000);
    }
}

/// True for a folder `claude*` directly in `/tmp`.
fn is_claude_in_tmp(path: &Path) -> bool {
    let tops = [Path::new("/tmp"), Path::new("/private/tmp")];
    let top = path.parent().is_some_and(|p| tops.contains(&p));
    let name = path.file_name().and_then(|n| n.to_str());
    top && name.is_some_and(|n| n.starts_with("claude"))
}

/// A new folder `scratch.*` in `/tmp`, or in a `claude` folder of `/tmp`.
/// A sandbox lets a test write only in some of these folders.
fn scratch_in_tmp() -> PathBuf {
    let name = format!("scratch.cu-tmp-{}", std::process::id());
    let claude = Some(PathBuf::from("/tmp/claude"));
    let own = Some(env::temp_dir()).filter(|p| is_claude_in_tmp(p));
    let top = Some(PathBuf::from("/tmp"));
    for base in [claude, own, top].into_iter().flatten() {
        let scratch = base.join(&name);
        if fs::create_dir_all(&scratch).is_ok() {
            return scratch;
        }
    }
    panic!("this test can write no folder in /tmp");
}

/// On a Mac, `/tmp` is a link. The temp folder of the command is not `/tmp`
/// here, and `CLAUDE_CODE_TMPDIR` is not set.
#[test]
fn clean_finds_a_marked_temp_folder_through_slash_tmp() {
    let dir = folder();
    let name = id("slash-tmp");
    record(&dir, &name, "Done");
    let scratch = scratch_in_tmp();
    marked(&scratch, &name);
    let out = cu(&dir, &["clean", &name]);
    let stays = scratch.exists();
    let _ = fs::remove_dir_all(&scratch);
    good(&out);
    assert!(!stays, "{}", scratch.display());
}

// ---------------------------------------------------------------------
// The step `clean_up`
// ---------------------------------------------------------------------

/// Runs the commands of `clean_up` of the default sheet for the run `id`.
/// `false` means a bad result.
fn clean_up(dir: &Path, id: &str) -> bool {
    let sheet = root().join("graph.example.yaml");
    let graph = Graph::load(&sheet, &root().join("roles")).unwrap();
    for words in &graph.step("clean_up").unwrap().run {
        let words = words.iter().map(|w| w.replace("{run}", id));
        let words: Vec<String> = words.collect();
        let mut step = Command::new(&words[0]);
        step.args(&words[1..]).current_dir(dir);
        let out = step.output().unwrap();
        if !out.status.success() {
            return false;
        }
    }
    true
}

/// A line of the log of a run, with the fields of a real line. In the
/// text, `time` and `worker_seconds` come after `summary`.
fn line(step: &str, summary: &str) -> String {
    let line = json!({
        "time": "2026-11-27T10:00:00Z", "step": step, "ok": true,
        "seconds": 12, "summary": summary, "worker_seconds": 11,
    });
    format!("{line}\n")
}

/// The summary line that names a copy, with no newline around it.
fn copy(path: &Path) -> String {
    format!("Copy: {}", path.display())
}

#[test]
fn clean_up_removes_each_copy_of_the_learner_by_its_path_in_the_log() {
    let dir = folder();
    let name = id("learn");
    let run = record(&dir, &name, "Running");
    let temp = folder();
    let first = temp.join("lesson.AbC123");
    let second = temp.join("lesson.dEf456");
    let third = temp.join("lesson.gHi789");
    let middle = temp.join("lesson.jKl012");
    for path in [&first, &second, &third, &middle] {
        marked(path, &name);
    }
    // `Copy:` is the first and the last text of a summary.
    let one = copy(&first);
    let two = format!("{one}\nI added one test.\n{}", copy(&second));
    // A summary that is only the line, and a line between other lines.
    let inner = format!("I added one test.\n{}\nDone.", copy(&middle));
    let mut log = line("learn", &two);
    log += &line("learn", &copy(&third));
    log += &line("learn", &inner);
    write(&run.join("log.jsonl"), &log);
    assert!(clean_up(&dir, &name), "the step is bad");
    for path in [first, second, third, middle] {
        assert!(!path.exists(), "{}", path.display());
    }
    assert!(gone(&run));
    assert!(run.join("log.jsonl").is_file());
}

#[test]
fn clean_up_keeps_each_path_that_it_cannot_trust() {
    let dir = folder();
    let name = id("trust");
    let run = record(&dir, &name, "Running");
    let temp = folder();
    let unmarked = temp.join("lesson.unmarked");
    write(&unmarked.join("file"), "x");
    let different = temp.join("lesson.different");
    marked(&different, "run-of-nobody");
    let unnamed = temp.join("work.folder");
    marked(&unnamed, &name);
    let built = temp.join("lesson.built");
    marked(&built, &name);
    let target = temp.join("target");
    marked(&target, &name);
    let link = temp.join("lesson.link");
    symlink(&target, &link).unwrap();
    let relative = dir.join("lesson.relative");
    marked(&relative, &name);
    let upper = temp.join("lesson.upper");
    marked(&upper, &name);
    fs::create_dir_all(temp.join("below")).unwrap();
    let dots = temp.join("below/../lesson.upper");
    let hostile = "Copy: /tmp/$(touch hacked)/lesson.abc\n";
    let mut log = line("build", &copy(&built));
    for path in [&unmarked, &different, &unnamed, &link, &dots] {
        log += &line("learn", &copy(path));
    }
    log += &line("learn", "Copy: lesson.relative\n");
    log += &line("learn", hostile);
    write(&run.join("log.jsonl"), &log);
    assert!(clean_up(&dir, &name), "the step is bad");
    let stay = [unmarked, different, unnamed, built, target];
    for path in stay {
        assert!(path.is_dir(), "{}", path.display());
    }
    assert!(link.is_symlink(), "the link is gone");
    assert!(relative.is_dir(), "relative");
    assert!(upper.is_dir(), "a path with two points");
    assert!(!dir.join("hacked").exists(), "the log ran");
}

#[test]
fn clean_up_is_good_with_no_line_of_the_learner() {
    let dir = folder();
    let name = id("quiet");
    let run = record(&dir, &name, "Running");
    write(&run.join("log.jsonl"), &line("build", "Done."));
    assert!(clean_up(&dir, &name), "the step is bad");
    assert!(gone(&run));
}

#[test]
fn the_comment_of_clean_up_names_the_copy_of_the_learner() {
    let sheet = text("graph.example.yaml");
    let end = sheet.find("\n  clean_up:\n").unwrap();
    let start = sheet[..end].rfind("\n\n").unwrap();
    let comment = &sheet[start..end];
    let words = ["learner", "codeumbrella-run", "codeumbrella clean"];
    for word in words {
        assert!(comment.contains(word), "{word}: {comment}");
    }
}

// ---------------------------------------------------------------------
// The rules pages, the chat page and the documents
// ---------------------------------------------------------------------

fn has_each(rel: &str, sentences: &[&str]) {
    let page = text(rel);
    for sentence in sentences {
        assert!(page.contains(sentence), "{rel}: {sentence}");
    }
}

/// The lines of the part `## title` of a document.
fn section(rel: &str, title: &str) -> String {
    let page = text(rel);
    let head = format!("## {title}");
    let mut found = Vec::new();
    let mut inside = false;
    for row in page.lines() {
        if row.starts_with("## ") {
            inside = row == head;
        } else if inside {
            found.push(row);
        }
    }
    assert!(!found.is_empty(), "{rel}: no part {title}");
    found.join("\n")
}

const FOLDER_MARK: [&str; 2] = [
    "Then write the run id into the file `codeumbrella-run` at the top of that folder.",
    "With that file, `codeumbrella clean` finds the folder after the time limit ends you.",
];

#[test]
fn the_attacker_and_the_user_checker_mark_their_scratch_folder() {
    has_each("roles/attacker.md", &FOLDER_MARK);
    has_each("roles/user.md", &FOLDER_MARK);
    let clone = "Clone into a new folder inside the scratch folder.";
    has_each("roles/user.md", &[clone]);
}

#[test]
fn the_learner_marks_its_copy_and_names_it_in_its_summary() {
    let sentences = [
        "Then write the run id into the file `codeumbrella-run` at the top of that copy.",
        "Give one line `Copy: <full path>` in your summary for each copy.",
        "After your step, `clean_up` or `codeumbrella clean` cleans up that copy for the person.",
        "Remove no file and no folder.",
    ];
    has_each("roles/learner.md", &sentences);
}

#[test]
fn the_chat_page_has_the_part_clean_up() {
    let part = section("skills/codeumbrella/SKILL.md", "Clean up");
    let sentences = [
        "After status `Done`, run `codeumbrella clean <run id>`.",
        "After status `Stopped`, run it only when the person says that the run ends.",
        "Tell the person the freed space in one line, from the line `Freed:`.",
        "A cleaned run cannot continue with `resume`.",
        "Run `codeumbrella clean` with no run id only when the person tells you to.",
        "If `run` gives the error for free space, tell the person that error.",
        "Then offer to run `codeumbrella clean`.",
        "Run `clean` with no sandbox if your sandbox stops it.",
    ];
    for sentence in sentences {
        assert!(part.contains(sentence), "{sentence}: {part}");
    }
}

#[test]
fn the_readme_tells_the_limit_and_the_command() {
    let part = section("README.md", "Free disk space");
    let sentences = [
        "`codeumbrella run` and `run --next` give an error and start no run with less than 10 GB free.",
        "The option `--min-free-gb <number>` sets a different limit, and 0 turns the check off.",
        "The command `codeumbrella clean` removes the copies of each run with status `Done` or `Stopped`.",
        "A cleaned run cannot continue with `resume`.",
    ];
    for sentence in sentences {
        assert!(part.contains(sentence), "{sentence}: {part}");
    }
}

#[test]
fn agents_md_tells_the_command_the_limit_and_the_rule() {
    let commands = section("AGENTS.md", "Commands");
    let mut rows = commands.lines();
    assert!(rows.any(|l| l.starts_with("codeumbrella clean")));
    let mut rows = commands.lines();
    let limit = |l: &str| l.contains("--min-free-gb") && l.contains("10 GB");
    assert!(rows.any(limit), "{commands}");
    let rules = section("AGENTS.md", "Safety rules");
    let rule = |l: &str| {
        let named = l.contains("`codeumbrella clean`");
        named && l.contains("27 November 2026")
    };
    assert!(rules.lines().any(rule), "{rules}");
    let before = "removes no file. The person cleans it up.";
    assert!(!rules.contains(before), "{before}");
    let table = section("AGENTS.md", "Structure");
    assert!(table.contains("| `tests/disk_space.rs` |"), "{table}");
}

#[test]
fn the_architecture_tells_how_long_a_stopped_run_keeps_its_copies() {
    let sentences = [
        "A run that stops before the merge keeps its copies until `codeumbrella clean` removes them.",
        "`resume` uses them.",
    ];
    has_each("ARCHITECTURE.md", &sentences);
    let page = text("ARCHITECTURE.md");
    let before = "keeps its copies, because";
    assert!(!page.contains(before), "{before}");
}
