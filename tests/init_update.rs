//! Tests of `codeumbrella init` and `codeumbrella init --update`.
//! They start the real command in a new temp folder. No Claude, no Codex, no network.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs};

static COUNT: AtomicU32 = AtomicU32::new(0);

/// The files that `init` writes into `.codeumbrella/`, as paths from that folder.
const FILES: &[&str] = &[
    ".gitignore",
    "graph.yaml",
    "roles/planner.md",
    "roles/planner.claude.json",
    "roles/planner.codex.txt",
    "roles/builder.md",
    "roles/builder.claude.json",
    "roles/builder.codex.txt",
    "roles/checker.md",
    "roles/checker.claude.json",
    "roles/checker.codex.txt",
    "roles/safety.md",
    "roles/safety.claude.json",
    "roles/safety.codex.txt",
];

/// A new empty project folder.
fn project() -> PathBuf {
    let n = COUNT.fetch_add(1, Ordering::SeqCst);
    let dir = env::temp_dir().join(format!("cu-init-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("make temp folder");
    dir
}

fn cu(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeumbrella"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("start codeumbrella")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn home(dir: &Path, rel: &str) -> PathBuf {
    dir.join(".codeumbrella").join(rel)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The lines of the output that name a file of the factory.
fn lines_for(output: &str, rel: &str) -> usize {
    output.lines().filter(|l| l.contains(rel)).count()
}

fn after_init() -> PathBuf {
    let dir = project();
    assert!(cu(&dir, &["init"]).status.success());
    dir
}

#[test]
fn init_writes_all_default_files() {
    let dir = after_init();
    for f in FILES {
        assert!(home(&dir, f).is_file(), "missing {f}");
    }
}

#[test]
fn init_without_flag_does_not_replace_a_file_and_makes_no_before() {
    let dir = after_init();
    fs::write(home(&dir, "graph.yaml"), "mine").unwrap();
    fs::write(home(&dir, "roles/builder.md"), "mine too").unwrap();
    assert!(cu(&dir, &["init"]).status.success());
    assert_eq!(read(&home(&dir, "graph.yaml")), "mine");
    assert_eq!(read(&home(&dir, "roles/builder.md")), "mine too");
    assert!(!home(&dir, "graph.yaml.before").exists());
    assert!(!home(&dir, "roles/builder.md.before").exists());
}

#[test]
fn update_replaces_a_changed_file_and_keeps_the_old_one_as_before() {
    let dir = after_init();
    let default = read(&home(&dir, "graph.yaml"));
    fs::write(home(&dir, "graph.yaml"), "old sheet").unwrap();
    let o = cu(&dir, &["init", "--update"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(read(&home(&dir, "graph.yaml")), default);
    assert_eq!(read(&home(&dir, "graph.yaml.before")), "old sheet");
}

#[test]
fn update_covers_every_kind_of_default_file() {
    let dir = after_init();
    let defaults: Vec<String> = FILES.iter().map(|f| read(&home(&dir, f))).collect();
    for f in FILES {
        fs::write(home(&dir, f), format!("changed {f}")).unwrap();
    }
    assert!(cu(&dir, &["init", "--update"]).status.success());
    for (f, default) in FILES.iter().zip(&defaults) {
        assert_eq!(&read(&home(&dir, f)), default, "{f} not updated");
        assert_eq!(
            read(&home(&dir, &format!("{f}.before"))),
            format!("changed {f}"),
            "{f}.before wrong"
        );
    }
}

#[test]
fn update_prints_one_line_for_each_changed_file_only() {
    let dir = after_init();
    fs::write(home(&dir, "graph.yaml"), "x").unwrap();
    fs::write(home(&dir, "roles/checker.codex.txt"), "y").unwrap();
    let o = out(&cu(&dir, &["init", "--update"]));
    assert_eq!(lines_for(&o, "graph.yaml"), 1, "{o}");
    assert_eq!(lines_for(&o, "roles/checker.codex.txt"), 1, "{o}");
    for f in ["roles/planner.md", "roles/builder.claude.json"] {
        assert_eq!(lines_for(&o, f), 0, "line for unchanged {f}: {o}");
    }
}

#[test]
fn update_with_nothing_changed_prints_no_file_line_and_makes_no_before() {
    let dir = after_init();
    let o = cu(&dir, &["init", "--update"]);
    assert!(o.status.success());
    let text = out(&o);
    for f in FILES {
        assert_eq!(lines_for(&text, f), 0, "line for {f}: {text}");
        assert!(!home(&dir, &format!("{f}.before")).exists(), "{f}.before");
    }
}

#[test]
fn update_writes_a_missing_file_without_a_before_file() {
    let dir = after_init();
    let default = read(&home(&dir, "roles/planner.md"));
    fs::remove_file(home(&dir, "roles/planner.md")).unwrap();
    let o = out(&cu(&dir, &["init", "--update"]));
    assert_eq!(read(&home(&dir, "roles/planner.md")), default);
    assert!(!home(&dir, "roles/planner.md.before").exists());
    assert_eq!(lines_for(&o, "roles/planner.md"), 1, "{o}");
}

#[test]
fn update_replaces_an_old_before_file() {
    let dir = after_init();
    fs::write(home(&dir, "graph.yaml.before"), "very old").unwrap();
    fs::write(home(&dir, "graph.yaml"), "newer").unwrap();
    assert!(cu(&dir, &["init", "--update"]).status.success());
    assert_eq!(read(&home(&dir, "graph.yaml.before")), "newer");
}

#[test]
fn update_does_not_touch_the_runs_folder() {
    let dir = after_init();
    let run = home(&dir, "runs/run-1/papers/spec");
    fs::create_dir_all(&run).unwrap();
    fs::write(run.join("spec.md"), "keep").unwrap();
    fs::write(home(&dir, "runs/run-1/state.json"), "{}").unwrap();
    fs::write(home(&dir, "graph.yaml"), "x").unwrap();
    assert!(cu(&dir, &["init", "--update"]).status.success());
    assert_eq!(read(&run.join("spec.md")), "keep");
    assert_eq!(read(&home(&dir, "runs/run-1/state.json")), "{}");
    let names: Vec<_> = fs::read_dir(home(&dir, "runs/run-1"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.ends_with(".before")), "{names:?}");
}

#[test]
fn update_does_not_touch_the_gitignore_of_the_project() {
    let dir = project();
    // A project with its own ignore file.
    fs::write(dir.join(".gitignore"), "my rules\n").unwrap();
    assert!(cu(&dir, &["init"]).status.success());
    assert_eq!(read(&dir.join(".gitignore")), "my rules\n");
    assert!(cu(&dir, &["init", "--update"]).status.success());
    assert_eq!(read(&dir.join(".gitignore")), "my rules\n");
    assert!(!dir.join(".gitignore.before").exists());
}

#[test]
fn update_does_not_create_the_gitignore_of_the_project() {
    let dir = after_init();
    fs::remove_file(dir.join(".gitignore")).unwrap();
    assert!(cu(&dir, &["init", "--update"]).status.success());
    assert!(!dir.join(".gitignore").exists());
}

#[test]
fn update_leaves_a_file_that_the_person_added() {
    let dir = after_init();
    fs::write(home(&dir, "roles/custom.md"), "my role").unwrap();
    assert!(cu(&dir, &["init", "--update"]).status.success());
    assert_eq!(read(&home(&dir, "roles/custom.md")), "my role");
    assert!(!home(&dir, "roles/custom.md.before").exists());
}

#[test]
fn update_works_in_a_project_with_no_factory_folder() {
    let dir = project();
    let o = cu(&dir, &["init", "--update"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    for f in FILES {
        assert!(home(&dir, f).is_file(), "missing {f}");
    }
}

#[test]
fn docs_tell_about_the_update_flag() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for f in ["AGENTS.md", "README.md", "graph.example.yaml"] {
        let t = read(&root.join(f));
        assert!(t.contains("init --update"), "{f} lacks `init --update`");
        assert!(t.contains(".before"), "{f} lacks `.before`");
    }
}
