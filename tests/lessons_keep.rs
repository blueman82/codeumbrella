//! Tests of the lessons that stay until a merged PR repairs them. They run
//! the commands of the default sheet with a simulated `gh`. No worker and
//! no network.

use codeumbrella::graph::{Graph, Kind};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// The run of the step tests.
const ID: &str = "run-1795000000-0005";

/// The project file of the lessons.
const TODAY: &str = ".codeumbrella/lessons.md";

/// The file of the lessons at the step `copy`.
const BEFORE: &str = ".codeumbrella/runs/run-1795000000-0005/lessons.before.md";

/// The file of the paper `lessons`.
const PAPER: &str = ".codeumbrella/runs/run-1795000000-0005/papers/lessons/lessons.md";

const FIRST_ENTRY: &str = "- The first entry. (19 November 2026)";
const SECOND_ENTRY: &str = "- The second entry. (20 November 2026)";
const THIRD_ENTRY: &str = "- The third entry. (21 November 2026)";
const FIRST_DONE: &str = "- ~~The first entry.~~ (19 November 2026) Done: run-1795000000-0001";

/// A done entry of a run. The simulated `gh` has a merged PR for its repair.
const MERGED: &str = concat!(
    "- ~~Merged one.~~ (run-1794000000, 19 November 2026)",
    " Done: run-1795000000-0001"
);

/// A done entry of a person, with a merged PR.
const MERGED_PERSON: &str = "- ~~Merged two.~~ (19 November 2026) Done: run-1795000000-0002";

/// A done entry whose PR is not merged.
const OPEN: &str = "- ~~Open one.~~ (19 November 2026) Done: run-1795000000-0003";

/// A done entry for which the simulated `gh` gives an error.
const BROKEN: &str = "- ~~Broken one.~~ (19 November 2026) Done: run-1795000000-0009";

/// The simulated `gh` of the step tests. `BIN` is its folder.
const GH: &str = r#"echo "$*" >> "BIN/gh.txt"
case "$*" in
*"pr list --head codeumbrella/run-1795000000-0001 --state merged"*) echo 11 ;;
*"pr list --head codeumbrella/run-1795000000-0002 --state merged"*) echo 12 ;;
*"pr list --head codeumbrella/run-1795000000-0009 "*) echo "no network" >&2; exit 1 ;;
esac"#;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    let path = root().join(rel);
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn default_sheet() -> Graph {
    let path = root().join("graph.example.yaml");
    Graph::load(&path, &root().join("roles")).unwrap()
}

/// A new empty folder, with its full path.
fn folder() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let name = format!("cu-keep-{}-{n}", std::process::id());
    let dir = env::temp_dir().join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

/// Writes a simulated tool into the folder `bin` of `dir`.
fn tool(dir: &Path, name: &str, script: &str) {
    let bin = dir.join("bin");
    let script = script.replace("BIN", &bin.display().to_string());
    let path = bin.join(name);
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    let mode = fs::Permissions::from_mode(0o755);
    fs::set_permissions(&path, mode).unwrap();
}

/// A folder with the run folder, the paper folder and a simulated `gh`.
fn place() -> PathBuf {
    let dir = folder();
    let paper = dir.join(PAPER);
    fs::create_dir_all(paper.parent().unwrap()).unwrap();
    fs::create_dir_all(dir.join("bin")).unwrap();
    tool(&dir, "gh", GH);
    dir
}

fn put(dir: &Path, rel: &str, content: &str) {
    fs::write(dir.join(rel), content).unwrap();
}

fn get(dir: &Path, rel: &str) -> String {
    let read = fs::read_to_string(dir.join(rel));
    read.unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// The `PATH` with the folder `bin` of `dir` first.
fn path_with_bin(dir: &Path) -> String {
    let bin = dir.join("bin");
    format!("{}:{}", bin.display(), env::var("PATH").unwrap())
}

/// Runs the commands of one step of the default sheet in `dir`, as the
/// supervisor does. `false` means a bad result.
fn step(dir: &Path, name: &str) -> bool {
    let graph = default_sheet();
    for words in &graph.step(name).unwrap().run {
        let words: Vec<String> = words.iter().map(|w| w.replace("{run}", ID)).collect();
        let mut command = Command::new(&words[0]);
        command.args(&words[1..]).current_dir(dir);
        command.env("PATH", path_with_bin(dir));
        let out = command.output().unwrap();
        if !out.status.success() {
            return false;
        }
    }
    true
}

/// A lessons file: the line of the read runs, then the entries.
fn file(read: &str, entries: &[&str]) -> String {
    let list = entries.join("\n");
    format!("Runs read to: {read}\n\n{list}\n")
}

/// How many lines of the text are this line.
fn count(content: &str, line: &str) -> usize {
    content.lines().filter(|l| *l == line).count()
}

/// The lines `Runs read to:` of a text.
fn read_lines(content: &str) -> Vec<&str> {
    let lines = content.lines();
    lines.filter(|l| l.starts_with("Runs read to:")).collect()
}

// ---------------------------------------------------------------------
// The sheet
// ---------------------------------------------------------------------

#[test]
fn tidy_lessons_is_a_command_step_between_save_lessons_and_clean_up() {
    let graph = default_sheet();
    let save = graph.step("save_lessons").unwrap();
    assert_eq!(save.kind, Kind::Command);
    assert_eq!(save.next.as_deref(), Some("tidy_lessons"));
    let tidy = graph.step("tidy_lessons").unwrap();
    assert_eq!(tidy.kind, Kind::Command);
    assert_eq!(tidy.next.as_deref(), Some("clean_up"));
    assert!(tidy.on_bad.is_none(), "{:?}", tidy.on_bad);
    assert!(tidy.max_tries.is_none(), "{:?}", tidy.max_tries);
    assert!(tidy.reads.is_empty() && tidy.writes.is_empty());
    assert!(tidy.role.is_none() && tidy.fan.is_empty());
}

#[test]
fn save_lessons_uses_the_union_merge_of_git() {
    let graph = default_sheet();
    let save = graph.step("save_lessons").unwrap();
    let words: Vec<String> = save.run.iter().map(|c| c.join(" ")).collect();
    let all = words.join("\n");
    assert!(all.contains("merge-file"), "{all}");
    assert!(all.contains("--union"), "{all}");
}

#[test]
fn only_the_three_lessons_steps_name_the_lessons_file_of_the_project() {
    let graph = default_sheet();
    let want = ["copy", "save_lessons", "tidy_lessons"];
    for (name, step) in &graph.steps {
        let mut words = step.run.iter().flatten();
        let names = words.any(|w| w.contains(TODAY));
        assert_eq!(names, want.contains(&name.as_str()), "{name}");
    }
}

#[test]
fn the_gh_command_of_tidy_lessons_reads_the_clean_copy() {
    let graph = default_sheet();
    let tidy = graph.step("tidy_lessons").unwrap();
    let mut found = 0;
    for words in &tidy.run {
        if words.iter().any(|w| w.contains("gh pr list")) {
            found += 1;
            assert_eq!(words[0], "env");
            assert_eq!(words[1], "GIT_DIR=.codeumbrella/runs/{run}/clean/.git");
        }
    }
    assert!(found > 0, "no command with gh: {:?}", tidy.run);
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
fn the_comments_of_the_sheet_tell_about_the_new_work() {
    let copy = comment_above("copy");
    assert!(copy.contains("lessons.before.md"), "{copy}");
    let tidy = comment_above("tidy_lessons");
    for word in ["strikethrough", "merged", "gh"] {
        assert!(tidy.contains(word), "{word}: {tidy}");
    }
    let clean = comment_above("clean_up");
    assert!(clean.contains("tidy_lessons"), "{clean}");
}

// ---------------------------------------------------------------------
// The step `copy`
// ---------------------------------------------------------------------

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

/// A git project with one commit and the run folder of `ID`.
fn repo() -> PathBuf {
    let dir = place();
    put(&dir, "keep.txt", "x");
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "keep.txt"]);
    git(&dir, &["commit", "--quiet", "-m", "first"]);
    dir
}

#[test]
fn copy_keeps_the_lessons_file_of_that_moment() {
    let dir = repo();
    let today = file("run-1795000000-0001", &[FIRST_ENTRY]);
    put(&dir, TODAY, &today);
    assert!(step(&dir, "copy"));
    assert_eq!(get(&dir, BEFORE), today);
    assert_eq!(get(&dir, TODAY), today);
}

#[test]
fn copy_makes_an_empty_file_with_no_lessons_file() {
    let dir = repo();
    assert!(step(&dir, "copy"));
    assert_eq!(get(&dir, BEFORE), "");
}

// ---------------------------------------------------------------------
// The step `save_lessons`
// ---------------------------------------------------------------------

#[test]
fn save_lessons_keeps_the_new_entries_of_two_runs() {
    let dir = place();
    let before = file("run-1795000000-0001", &[FIRST_ENTRY]);
    let today = file("run-1795000000-0003", &[FIRST_ENTRY, SECOND_ENTRY]);
    let paper = file("run-1795000000-0002", &[FIRST_ENTRY, THIRD_ENTRY]);
    put(&dir, BEFORE, &before);
    put(&dir, TODAY, &today);
    put(&dir, PAPER, &paper);
    assert!(step(&dir, "save_lessons"));
    let got = get(&dir, TODAY);
    for entry in [FIRST_ENTRY, SECOND_ENTRY, THIRD_ENTRY] {
        assert_eq!(count(&got, entry), 1, "{entry}: {got}");
    }
    let newest = "Runs read to: run-1795000000-0003";
    assert_eq!(read_lines(&got), [newest], "{got}");
    assert_eq!(got.lines().next(), Some(newest), "{got}");
    assert!(!got.contains("<<<<<<<"), "{got}");
    // A change of a paper stops the run at the next step.
    assert_eq!(get(&dir, PAPER), paper);
}

#[test]
fn save_lessons_keeps_the_newest_line_of_the_read_runs() {
    let pairs = [
        ("run-1794999999-9999", "run-1795000000-0001"),
        ("run-1795000000", "run-1795000000-0002"),
        ("run-1795000000-0009", "run-1795000000-0010"),
    ];
    for (older, newer) in pairs {
        for (today, paper) in [(older, newer), (newer, older)] {
            let dir = place();
            put(&dir, BEFORE, &file("run-1", &[FIRST_ENTRY]));
            put(&dir, TODAY, &file(today, &[FIRST_ENTRY]));
            put(&dir, PAPER, &file(paper, &[FIRST_ENTRY]));
            assert!(step(&dir, "save_lessons"));
            let got = get(&dir, TODAY);
            let want = format!("Runs read to: {newer}");
            assert_eq!(read_lines(&got), [want.as_str()], "{got}");
            assert_eq!(got.lines().next(), Some(want.as_str()), "{got}");
            assert_eq!(count(&got, FIRST_ENTRY), 1, "{got}");
        }
    }
}

/// Runs `save_lessons` with these entries in the three files. Gives the
/// project file at the end.
fn save(before: &[&str], today: &[&str], paper: &[&str]) -> String {
    let dir = place();
    let read = "run-1795000000-0001";
    put(&dir, BEFORE, &file(read, before));
    put(&dir, TODAY, &file(read, today));
    put(&dir, PAPER, &file(read, paper));
    assert!(step(&dir, "save_lessons"));
    get(&dir, TODAY)
}

#[test]
fn save_lessons_gets_the_mark_and_keeps_the_entry_of_another_run() {
    let got = save(
        &[FIRST_ENTRY, SECOND_ENTRY],
        &[FIRST_ENTRY, SECOND_ENTRY, THIRD_ENTRY],
        &[FIRST_DONE, SECOND_ENTRY],
    );
    assert_eq!(count(&got, FIRST_DONE), 1, "{got}");
    assert_eq!(count(&got, FIRST_ENTRY), 0, "{got}");
    assert_eq!(count(&got, SECOND_ENTRY), 1, "{got}");
    assert_eq!(count(&got, THIRD_ENTRY), 1, "{got}");
}

#[test]
fn save_lessons_marks_an_entry_that_another_run_added() {
    // The same text as `SECOND_ENTRY`, with other brackets. It stays.
    let other = "- The second entry. (22 November 2026)";
    let done = concat!(
        "- ~~The second entry.~~ (20 November 2026)",
        " Done: run-1795000000-0002"
    );
    let got = save(
        &[FIRST_ENTRY],
        &[FIRST_ENTRY, SECOND_ENTRY, other],
        &[FIRST_ENTRY, done, THIRD_ENTRY],
    );
    assert_eq!(count(&got, SECOND_ENTRY), 0, "{got}");
    assert_eq!(count(&got, done), 1, "{got}");
    assert_eq!(count(&got, other), 1, "{got}");
    assert_eq!(count(&got, FIRST_ENTRY), 1, "{got}");
    assert_eq!(count(&got, THIRD_ENTRY), 1, "{got}");
}

#[test]
fn save_lessons_leaves_no_unmarked_line_next_to_a_new_entry() {
    let got = save(&[FIRST_ENTRY], &[FIRST_ENTRY, SECOND_ENTRY], &[FIRST_DONE]);
    assert_eq!(count(&got, FIRST_DONE), 1, "{got}");
    assert_eq!(count(&got, FIRST_ENTRY), 0, "{got}");
    assert_eq!(count(&got, SECOND_ENTRY), 1, "{got}");
}

#[test]
fn save_lessons_keeps_one_line_of_an_entry_at_two_places() {
    let got = save(
        &[FIRST_ENTRY],
        &[SECOND_ENTRY, FIRST_ENTRY, THIRD_ENTRY],
        &[FIRST_ENTRY, SECOND_ENTRY],
    );
    for entry in [FIRST_ENTRY, SECOND_ENTRY, THIRD_ENTRY] {
        assert_eq!(count(&got, entry), 1, "{entry}: {got}");
    }
}

/// An entry with characters that a script can change by error.
const SLASHES: &str = r"- Zeta  keeps \n, \\, \1, %s, & and $HOME. (19 November 2026)";

/// `SLASHES` with the mark of a done entry.
const SLASHES_DONE: &str = concat!(
    r"- ~~Zeta  keeps \n, \\, \1, %s, & and $HOME.~~",
    " (19 November 2026) Done: run-1795000000-0001"
);

/// A lessons file with headings, other lines and entries in no sequence.
const RICH: [&str; 14] = [
    "Runs read to: run-1795000000-0001",
    "",
    "# Lessons",
    "",
    SLASHES,
    "- Alpha. (18 November 2026)",
    "",
    "## For the factory",
    "",
    "A note\twith a tab and two spaces at the end.  ",
    "  - An indented line.",
    "  - An indented line.",
    "",
    "- Mu. (17 November 2026)",
];

#[test]
fn save_lessons_keeps_the_headings_the_sequence_and_each_character() {
    let before = format!("{}\n", RICH.join("\n"));
    let paper = before
        .replace(SLASHES, SLASHES_DONE)
        .replace("run-1795000000-0001\n\n#", "run-1795000000-0002\n\n#")
        + "- Beta. (21 November 2026)\n";
    // First no other run changes the project file. Then a second run adds a
    // line far from the changes of the paper.
    let head = "## For the factory\n\n";
    let added = format!("{head}- Other run. (22 November 2026)\n");
    for (old, new) in [(head, head), (head, added.as_str())] {
        let dir = place();
        put(&dir, BEFORE, &before);
        put(&dir, TODAY, &before.replace(old, new));
        put(&dir, PAPER, &paper);
        assert!(step(&dir, "save_lessons"));
        assert_eq!(get(&dir, TODAY), paper.replace(old, new));
    }
}

#[test]
fn save_lessons_with_no_file_of_before_keeps_the_two_sides() {
    let dir = place();
    put(&dir, TODAY, &file("run-1795000000-0001", &[FIRST_ENTRY]));
    put(&dir, PAPER, &file("run-1795000000-0002", &[SECOND_ENTRY]));
    assert!(step(&dir, "save_lessons"));
    let got = get(&dir, TODAY);
    assert_eq!(count(&got, FIRST_ENTRY), 1, "{got}");
    assert_eq!(count(&got, SECOND_ENTRY), 1, "{got}");
    let newest = "Runs read to: run-1795000000-0002";
    assert_eq!(read_lines(&got), [newest], "{got}");
}

#[test]
fn save_lessons_with_no_lessons_file_gives_the_paper() {
    let dir = place();
    let paper = file("run-1795000000-0002", &[FIRST_ENTRY]);
    put(&dir, BEFORE, "");
    put(&dir, PAPER, &paper);
    assert!(step(&dir, "save_lessons"));
    assert_eq!(get(&dir, TODAY), paper);
}

#[test]
fn save_lessons_with_no_paper_is_bad_and_changes_nothing() {
    let dir = place();
    let today = file("run-1795000000-0001", &[FIRST_ENTRY]);
    put(&dir, BEFORE, &today);
    put(&dir, TODAY, &today);
    assert!(!step(&dir, "save_lessons"));
    assert_eq!(get(&dir, TODAY), today);
}

/// Each file and link below `dir`, with its content or its target.
fn snapshot(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let kind = fs::symlink_metadata(&path).unwrap().file_type();
        if kind.is_dir() {
            found.extend(snapshot(&path));
        } else if kind.is_symlink() {
            let target = fs::read_link(&path).unwrap();
            found.push((path, format!("link to {}", target.display())));
        } else {
            let content = fs::read_to_string(&path).unwrap();
            found.push((path, content));
        }
    }
    found.sort();
    found
}

/// A folder with a project file and a file of before. The file
/// `outside.md` is not a part of the paper.
fn place_with_outside_file() -> PathBuf {
    let dir = place();
    let today = file("run-1795000000-0001", &[FIRST_ENTRY]);
    put(&dir, BEFORE, &today);
    put(&dir, TODAY, &today);
    let outside = file("run-1795000000-0002", &[SECOND_ENTRY]);
    put(&dir, "outside.md", &outside);
    dir
}

/// `save_lessons` gives a bad result and changes no file.
fn assert_refused(dir: &Path) {
    let before = snapshot(dir);
    assert!(!step(dir, "save_lessons"), "the paper must be refused");
    assert_eq!(snapshot(dir), before);
}

#[test]
fn save_lessons_refuses_a_paper_that_is_a_symbolic_link() {
    let dir = place_with_outside_file();
    symlink(dir.join("outside.md"), dir.join(PAPER)).unwrap();
    assert_refused(&dir);
}

#[test]
fn save_lessons_refuses_a_paper_with_a_second_hard_link() {
    let dir = place_with_outside_file();
    fs::hard_link(dir.join("outside.md"), dir.join(PAPER)).unwrap();
    assert_refused(&dir);
}

#[test]
fn save_lessons_refuses_a_paper_folder_that_is_a_symbolic_link() {
    let dir = place_with_outside_file();
    let paper = dir.join(PAPER);
    let paper_dir = paper.parent().unwrap();
    let other = dir.join("other");
    fs::rename(paper_dir, &other).unwrap();
    fs::copy(dir.join("outside.md"), other.join("lessons.md")).unwrap();
    symlink(&other, paper_dir).unwrap();
    assert_refused(&dir);
}

// ---------------------------------------------------------------------
// The step `tidy_lessons`
// ---------------------------------------------------------------------

#[test]
fn tidy_lessons_removes_only_the_done_entries_with_a_merged_pr() {
    let dir = place();
    let odd = r"- Text with \n, %s, $HOME and `*` stays.   (19 November 2026)";
    let all = [FIRST_ENTRY, MERGED, odd, OPEN, "", MERGED_PERSON];
    let mut today = file("run-1795000000-0004", &all);
    today.push_str("\n## For the factory\n- Last entry. (19 November 2026)\n");
    put(&dir, TODAY, &today);
    assert!(step(&dir, "tidy_lessons"));
    let gone = [format!("{MERGED}\n"), format!("{MERGED_PERSON}\n")];
    let mut want = today.clone();
    for line in &gone {
        want = want.replacen(line.as_str(), "", 1);
    }
    assert_eq!(get(&dir, TODAY), want);
    let calls = get(&dir, "bin/gh.txt");
    let head = "pr list --head codeumbrella/run-1795000000-0003 --state merged";
    assert!(calls.contains(head), "{calls}");
}

#[test]
fn tidy_lessons_keeps_an_entry_with_only_one_of_the_two_marks() {
    let dir = place();
    let kept = [
        "- No strikethrough. (19 November 2026) Done: run-1795000000-0001",
        "- ~~No done mark.~~ (19 November 2026)",
        "- ~~One part.~~ The rest. (19 November 2026) Done: run-1795000000-0001",
        "- A text with ~~marks~~ in it. (19 November 2026) Done: run-1795000000-0001",
    ];
    let today = file("run-1795000000-0004", &kept);
    put(&dir, TODAY, &today);
    assert!(step(&dir, "tidy_lessons"));
    assert_eq!(get(&dir, TODAY), today);
}

#[test]
fn tidy_lessons_keeps_a_last_line_with_no_line_end() {
    let dir = place();
    let today = format!("Runs read to: run-1\n\n{MERGED}\n{FIRST_ENTRY}");
    put(&dir, TODAY, &today);
    assert!(step(&dir, "tidy_lessons"));
    let want = format!("Runs read to: run-1\n\n{FIRST_ENTRY}");
    assert_eq!(get(&dir, TODAY).trim_end(), want);
}

#[test]
fn tidy_lessons_removes_nothing_if_gh_gives_an_error() {
    for list in [[MERGED, BROKEN], [BROKEN, MERGED]] {
        let dir = place();
        let today = file("run-1795000000-0004", &list);
        put(&dir, TODAY, &today);
        assert!(step(&dir, "tidy_lessons"), "the run must continue");
        assert_eq!(get(&dir, TODAY), today);
    }
}

#[test]
fn tidy_lessons_sends_only_a_run_id_to_gh() {
    let dir = place();
    let bad = [
        "- ~~One.~~ (19 November 2026) Done: run-1;touch pwned",
        "- ~~Two.~~ (19 November 2026) Done: $(touch pwned)",
        "- ~~Three.~~ (19 November 2026) Done: `touch pwned`",
        "- ~~Four.~~ (19 November 2026) Done: run-1795000000-0001 --repo a/b",
        "- ~~Five.~~ (19 November 2026) Done: master",
    ];
    let today = file("run-1795000000-0004", &bad);
    put(&dir, TODAY, &today);
    assert!(step(&dir, "tidy_lessons"));
    assert_eq!(get(&dir, TODAY), today);
    assert!(!dir.join("pwned").exists(), "a command of an entry ran");
    assert!(!dir.join("bin/gh.txt").exists(), "gh got a bad id");
}

#[test]
fn tidy_lessons_with_no_lessons_file_is_good_and_makes_none() {
    let dir = place();
    assert!(step(&dir, "tidy_lessons"));
    assert!(!dir.join(TODAY).exists(), "a new lessons file");
}

// ---------------------------------------------------------------------
// Runs with the built command
// ---------------------------------------------------------------------

/// A sheet with the steps from `copy` to `tidy_lessons`.
const SHEET: &str = r"start: copy
steps:
  copy:
    type: command
    run: COPY
    next: learn
  learn:
    role: learner
    writes: [lessons]
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
    next: tidy_lessons
  tidy_lessons:
    type: command
    run: TIDY
";

/// A simulated `claude`. It writes the paper `lessons`.
const CLAUDE: &str = r#"cp "BIN/lessons.md" ../papers/lessons/lessons.md
echo '{"type":"result","is_error":false,"result":"","structured_output":{"ok":true,"summary":"done"}}'"#;

/// The commands of a step of the default sheet, as a list for a sheet.
fn commands(name: &str) -> String {
    let graph = default_sheet();
    let step = graph.step(name).unwrap();
    serde_json::to_string(&step.run).unwrap()
}

fn cu(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeumbrella"));
    command.args(args).current_dir(dir);
    command.env("PATH", path_with_bin(dir));
    command.output().unwrap()
}

fn all(out: &Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    format!("{stdout}{stderr}")
}

/// A run that ends after a yes. The paper has one done entry. `gh` is the
/// simulated tool. Gives the lessons file of the project at the end.
fn full_run(gh: &str) -> String {
    let dir = folder();
    fs::create_dir_all(dir.join("bin")).unwrap();
    let init = cu(&dir, &["init"]);
    assert!(init.status.success(), "{}", all(&init));
    git(&dir, &["init", "--quiet"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "--quiet", "-m", "first"]);
    tool(&dir, "claude", CLAUDE);
    tool(&dir, "gh", gh);
    let paper = file("run-1795000000-0004", &[MERGED, SECOND_ENTRY]);
    put(&dir, "bin/lessons.md", &paper);
    let sheet = SHEET.replace("COPY", &commands("copy"));
    let sheet = sheet.replace("MERGE", &commands("merge"));
    let sheet = sheet.replace("SAVE", &commands("save_lessons"));
    let sheet = sheet.replace("TIDY", &commands("tidy_lessons"));
    put(&dir, ".codeumbrella/graph.yaml", &sheet);
    let run = cu(&dir, &["run", "a task"]);
    let printed = all(&run);
    assert!(printed.contains("Status:   Waiting"), "{printed}");
    let line = printed.lines().find(|l| l.starts_with("Run:      "));
    let id = &line.expect("a run")["Run:      ".len()..];
    let yes = cu(&dir, &["answer", id, "approval", "yes"]);
    let printed = all(&yes);
    assert!(printed.contains("Status:   Done"), "{printed}");
    get(&dir, TODAY)
}

#[test]
fn a_run_ends_with_no_done_entry_of_a_merged_pr() {
    let got = full_run(r#"case "$*" in *"pr list"*) echo 7 ;; esac"#);
    assert_eq!(count(&got, MERGED), 0, "{got}");
    assert_eq!(count(&got, SECOND_ENTRY), 1, "{got}");
}

#[test]
fn a_run_continues_and_keeps_the_entry_if_gh_cannot_list() {
    let got = full_run(r#"case "$*" in *"pr list"*) exit 1 ;; esac"#);
    assert_eq!(count(&got, MERGED), 1, "{got}");
    assert_eq!(count(&got, SECOND_ENTRY), 1, "{got}");
}

// ---------------------------------------------------------------------
// The rules pages and the documents
// ---------------------------------------------------------------------

const ROLES: [&str; 11] = [
    "attacker",
    "builder",
    "checker",
    "learner",
    "planner",
    "reviewer",
    "safety",
    "security",
    "simplifier",
    "user",
    "writer",
];

#[test]
fn each_rules_page_tells_to_skip_an_entry_with_a_strikethrough() {
    let first = "If the project has `AGENTS.md` or `CLAUDE.md`, read them first.";
    for role in ROLES {
        let page = text(&format!("roles/{role}.md"));
        let line = page.lines().find(|l| l.contains(first));
        let line = line.unwrap_or_else(|| panic!("{role}: no line"));
        let lower = line.to_lowercase();
        assert!(lower.contains("skip"), "{role}: {line}");
        assert!(lower.contains("strikethrough"), "{role}: {line}");
    }
}

#[test]
fn the_learner_page_marks_an_entry_and_removes_none() {
    let page = text("roles/learner.md");
    let shapes = [
        concat!(
            "- ~~<the text of the entry>~~ (<run id of the entry>, <date>)",
            " Done: <run id of the repair>"
        ),
        "- ~~<the text of the entry>~~ (<date>) Done: <run id of the repair>",
        "Remove no entry.",
    ];
    for shape in shapes {
        assert!(page.contains(shape), "{shape}");
    }
    let lower = page.to_lowercase();
    let gone = [
        "20 entries",
        "earliest date",
        "remove an entry",
        "if you find no new lesson, copy",
    ];
    for words in gone {
        assert!(!lower.contains(words), "{words}");
    }
}

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

#[test]
fn agents_md_tells_who_writes_the_file_and_when_an_entry_goes() {
    let rules = section("AGENTS.md", "Safety rules");
    let gone = "It copies the paper `lessons` after the merge.";
    assert!(!rules.contains(gone), "{gone}");
    let dated = rules.lines().any(|l| {
        let step = l.contains("tidy_lessons");
        step && l.contains("25 November 2026")
    });
    assert!(dated, "no line for tidy_lessons with its date");
    assert!(rules.contains("merged"), "merged");
    let words = section("AGENTS.md", "Words");
    let row = "| `.codeumbrella/lessons.md` |";
    let line = words.lines().find(|l| l.starts_with(row));
    let line = line.expect("the row of the lessons file");
    assert!(line.contains("tidy_lessons"), "{line}");
    let structure = section("AGENTS.md", "Structure");
    assert!(structure.contains("`tests/lessons_keep.rs`"));
}

#[test]
fn the_rules_and_the_sheet_tell_about_the_refused_paper() {
    let rules = section("AGENTS.md", "Safety rules");
    let told = rules.lines().any(|l| {
        let step = l.contains("save_lessons");
        step && l.contains("hard link")
    });
    assert!(told, "no line for save_lessons and its links");
    let save = comment_above("save_lessons");
    assert!(save.contains("link"), "{save}");
}

#[test]
fn agents_md_tells_when_fuzz_repair_starts_no_new_run() {
    let structure = section("AGENTS.md", "Structure");
    let row = "| `scripts/fuzz-repair` |";
    let line = structure.lines().find(|l| l.starts_with(row));
    let line = line.expect("the row of scripts/fuzz-repair");
    assert!(line.contains("not Done"), "{line}");
    assert!(line.contains("no new run"), "{line}");
}

#[test]
fn the_architecture_and_the_readme_show_tidy_lessons() {
    let path = section("ARCHITECTURE.md", "The path of a task");
    let lines: Vec<&str> = path.lines().collect();
    let at = |start: &str| {
        let found = lines.iter().position(|l| l.starts_with(start));
        found.unwrap_or_else(|| panic!("no line {start}"))
    };
    assert!(at("save_lessons") < at("tidy_lessons"), "{path}");
    assert!(at("tidy_lessons") < at("clean_up"), "{path}");
    let page = text("ARCHITECTURE.md");
    let gone = "`save_lessons` copies that paper";
    assert!(!page.contains(gone), "{gone}");
    let readme = text("README.md");
    assert!(readme.contains("tidy_lessons"), "README.md");
    let gone = "`save_lessons` copies the entries";
    assert!(!readme.contains(gone), "{gone}");
}
