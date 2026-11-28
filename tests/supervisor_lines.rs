//! The limit of lines of code of the supervisor. No worker, no network.

use std::fs;
use std::path::PathBuf;

const SUPERVISOR: [&str; 5] = ["graph", "engine", "main", "log", "lib"];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &str) -> String {
    fs::read_to_string(root().join(path)).expect(path)
}

/// Counts as `grep -v -E '^\s*(//|$)' <file> | wc -l` does.
fn code_lines(path: &str) -> usize {
    let text = read(path);
    let lines = text.lines().map(str::trim_start);
    lines
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .count()
}

/// The line of `AGENTS.md` with the limit of the supervisor.
fn limit_line() -> String {
    let page = read("AGENTS.md");
    let line = page.lines().find(|l| l.starts_with("- The supervisor ("));
    line.expect("no limit line").to_owned()
}

#[test]
fn the_supervisor_is_in_the_limit_of_agents_md() {
    let line = limit_line();
    let after = line.split("): ").nth(1).expect("no number");
    let number = after.split(' ').next().expect("no number");
    let limit: usize = number.parse().expect("not a number");
    let mut sum = 0;
    for name in SUPERVISOR {
        let path = format!("src/{name}.rs");
        assert!(line.contains(&path), "the limit line has no {path}");
        sum += code_lines(&path);
    }
    assert!(sum <= limit, "{sum} lines, the limit is {limit}");
    // Code in a new file must not get out of the count.
    for entry in fs::read_dir(root().join("src")).expect("src") {
        let name = entry.expect("src").file_name();
        let name = name.to_string_lossy();
        let stem = name.trim_end_matches(".rs");
        let tool = stem == "claude" || stem == "codex";
        assert!(tool || SUPERVISOR.contains(&stem), "{name} is not counted");
    }
}
