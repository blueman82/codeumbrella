//! Gives each stored crash input to the same function as its fuzz target.
//! A crash input is a file in `tests/fuzz_crashes/<target>/`.

use codeumbrella::engine::Numbers;
use codeumbrella::graph::Graph;
use std::fs;
use std::path::Path;

/// Calls `read` with each input of the target that is text, as the target does.
fn each_input(target: &str, read: impl Fn(&str)) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("tests/fuzz_crashes").join(target);
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let data = fs::read(&path).unwrap();
        if let Ok(text) = std::str::from_utf8(&data) {
            // A panic shows this line, so the person knows the input.
            println!("input: {}", path.display());
            read(text);
        }
    }
}

#[test]
fn fuzz_sheet() {
    let roles = Path::new(env!("CARGO_MANIFEST_DIR")).join("roles");
    each_input("fuzz_sheet", |text| {
        if let Ok(graph) = yaml_serde::from_str::<Graph>(text) {
            let _ = graph.check(&roles);
        }
    });
}

#[test]
fn fuzz_claude_reply() {
    each_input("fuzz_claude_reply", |text| {
        let _ = codeumbrella::claude::read_reply(text, &mut Numbers::default());
    });
}

#[test]
fn fuzz_codex_trace() {
    each_input("fuzz_codex_trace", |text| {
        codeumbrella::codex::count_tokens(text, &mut Numbers::default());
    });
}
