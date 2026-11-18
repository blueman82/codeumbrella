//! Tests of the three new roles, the new steps of the default sheet, the words
//! of the documents and the sentences about `resume`. No worker is started.

use codeumbrella::graph::{Graph, Kind};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs, process};

const NEW_ROLES: [&str; 3] = ["reviewer", "security", "simplifier"];
const SKILL: &str = "skills/codeumbrella/SKILL.md";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn page(role: &str) -> String {
    text(&format!("roles/{role}.md"))
}

fn graph() -> Graph {
    Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap()
}

fn folder() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    env::temp_dir().join(format!("codeumbrella-newroles-{}-{n}", process::id()))
}

fn cu(dir: &Path, args: &[&str]) -> bool {
    Command::new(env!("CARGO_BIN_EXE_codeumbrella"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("start codeumbrella")
        .status
        .success()
}

#[test]
fn the_nine_new_role_files_exist_and_init_writes_them() {
    let dir = folder();
    fs::create_dir_all(&dir).unwrap();
    assert!(cu(&dir, &["init"]));
    for role in NEW_ROLES {
        for ext in ["md", "claude.json", "codex.txt"] {
            let rel = format!("roles/{role}.{ext}");
            assert!(!text(&rel).trim().is_empty(), "{rel} is empty");
            let written = dir.join(".codeumbrella").join(&rel);
            assert_eq!(fs::read_to_string(&written).unwrap(), text(&rel), "{rel}");
        }
    }
    // `init --update` puts a missing file back.
    let gone = dir.join(".codeumbrella/roles/simplifier.md");
    fs::remove_file(&gone).unwrap();
    assert!(cu(&dir, &["init", "--update"]));
    assert_eq!(fs::read_to_string(&gone).unwrap(), page("simplifier"));
}

#[test]
fn the_settings_files_are_the_same_text_as_the_files_of_their_model_role() {
    for (role, model) in [
        ("reviewer", "safety"),
        ("security", "safety"),
        ("simplifier", "builder"),
    ] {
        for ext in ["claude.json", "codex.txt"] {
            let new = text(&format!("roles/{role}.{ext}"));
            let old = text(&format!("roles/{model}.{ext}"));
            assert_eq!(new, old, "{role}.{ext} and {model}.{ext}");
        }
    }
    // The settings files of the new roles are valid JSON.
    for role in NEW_ROLES {
        let file = text(&format!("roles/{role}.claude.json"));
        assert!(serde_json::from_str::<Value>(&file).is_ok(), "{role}");
    }
}

/// The paragraphs of a page.
fn paragraphs(role: &str) -> Vec<String> {
    let page = page(role);
    page.trim().split("\n\n").map(str::to_owned).collect()
}

#[test]
fn each_new_page_has_numbered_rules_and_the_two_last_paragraphs_of_the_other_pages() {
    let safety = paragraphs("safety");
    for role in NEW_ROLES {
        let page = page(role);
        let numbers: Vec<usize> = page
            .lines()
            .filter_map(|l| l.split_once(". ")?.0.parse().ok())
            .collect();
        assert!(numbers.len() >= 3, "{role}: {numbers:?}");
        assert_eq!(numbers, (1..=numbers.len()).collect::<Vec<_>>(), "{role}");
        let parts = paragraphs(role);
        assert_eq!(parts.last(), safety.last(), "{role}: summary");
        let ok = &parts[parts.len() - 2];
        let both = ok.contains("`ok: true`") && ok.contains("`ok: false`");
        assert!(both, "{role}: {ok}");
        assert!(page.contains("100 words or fewer"), "{role}");
    }
}

#[test]
fn the_sentences_of_the_new_pages_have_20_words_or_fewer() {
    for role in NEW_ROLES {
        let t = page(role).replace('`', "");
        for line in t.lines().filter(|l| !l.starts_with('#')) {
            let line = line
                .split_once(". ")
                .filter(|(n, _)| n.parse::<u32>().is_ok())
                .map_or(line, |(_, rest)| rest);
            for sentence in line.split(['.', '?']) {
                let n = sentence.split_whitespace().count();
                assert!(n <= 20, "{role}: {n} words: {sentence}");
            }
        }
    }
}

#[test]
fn no_new_page_has_knowledge_of_one_programming_language() {
    let languages = [
        "rust",
        "python",
        "javascript",
        "typescript",
        "java",
        "ruby",
        "php",
        "kotlin",
        "cargo",
        "npm",
        "pytest",
        "gradle",
        "maven",
    ];
    for role in NEW_ROLES {
        let page = page(role).to_lowercase();
        let words = page.split(|c: char| !c.is_alphanumeric());
        for word in words {
            assert!(!languages.contains(&word), "{role} has the word {word}");
        }
    }
}

/// Checks that a page has each phrase, with no difference for capital letters.
fn has_all(role: &str, phrases: &[&str]) {
    let page = page(role).to_lowercase();
    for phrase in phrases {
        assert!(page.contains(&phrase.to_lowercase()), "{role}: {phrase}");
    }
}

#[test]
fn the_reviewer_page_names_its_five_problems() {
    has_all(
        "reviewer",
        &[
            "AGENTS.md",
            "CLAUDE.md",
            "spec",
            "incorrect result",
            "default value",
            "shows nothing",
            "does not agree",
            "breaks a rule",
            "ok: false",
            "file",
            "line",
            "summary",
            "no file",
        ],
    );
}

#[test]
fn the_security_page_names_its_five_problems() {
    has_all(
        "security",
        &[
            "AGENTS.md",
            "CLAUDE.md",
            "no check",
            "command",
            "query",
            "file path",
            "page",
            "who a person is",
            "what a person can do",
            "fixed value",
            "weak method",
            "value in a log",
            "new package",
            "safety setting",
            "do not run",
            "ok: false",
            "file",
            "line",
            "summary",
            "no file",
        ],
    );
}

#[test]
fn the_simplifier_page_names_its_six_rules_and_its_limits() {
    has_all(
        "simplifier",
        &[
            "KISS",
            "DRY",
            "YAGNI",
            "one source of truth",
            "plain direct code",
            "no boilerplate",
            "codeumbrella/<run id>",
            "changes nothing",
            "ok: true",
            "ok: false",
        ],
    );
    let page = page("simplifier").replace('`', "");
    let sentences: Vec<&str> = page.split(". ").collect();
    let tests = sentences
        .iter()
        .find(|s| s.to_lowercase().contains("no test file"))
        .expect("a sentence with the words no test file");
    assert!(!tests.to_lowercase().contains("paper"), "{tests}");
    let commit = sentences.iter().any(|s| {
        let s = s.to_lowercase();
        s.contains("test command") && s.contains("before") && s.contains("commit")
    });
    assert!(commit, "no commit sentence");
}

#[test]
fn the_default_sheet_has_the_step_simplify_between_build_and_check() {
    let graph = graph();
    let build = graph.step("build").unwrap();
    assert_eq!(build.next.as_deref(), Some("simplify"));
    let simplify = graph.step("simplify").unwrap();
    assert_eq!(simplify.kind, Kind::Worker);
    assert_eq!(simplify.role.as_deref(), Some("simplifier"));
    assert_eq!(simplify.reads, ["spec"]);
    assert_eq!(simplify.next.as_deref(), Some("check"));
    assert_eq!(simplify.on_bad, None);
    let repair = graph.step("repair").unwrap();
    assert_eq!(repair.next.as_deref(), Some("check"));
    // Only `build` goes to `simplify`. The repair does not go through it.
    for (name, step) in &graph.steps {
        let to = [&step.next, &step.on_bad].contains(&&Some("simplify".to_owned()));
        assert_eq!(to, name == "build", "{name}");
    }
}

#[test]
fn the_fan_of_the_check_has_four_parts() {
    let graph = graph();
    let check = graph.step("check").unwrap();
    assert_eq!(
        check.fan,
        [
            "check_tests",
            "check_safety",
            "check_review",
            "check_security"
        ]
    );
    assert_eq!(check.on_bad.as_deref(), Some("repair"));
    for (name, role) in [("check_review", "reviewer"), ("check_security", "security")] {
        let part = graph.step(name).unwrap();
        assert_eq!(part.role.as_deref(), Some(role), "{name}");
        assert_eq!(part.reads, ["spec"], "{name}");
        assert_eq!(part.writes, [] as [String; 0], "{name}");
    }
}

/// The comment lines right above the step `name` in the default sheet.
fn comment_above(name: &str) -> String {
    let sheet = text("graph.example.yaml");
    let lines: Vec<&str> = sheet.lines().collect();
    let head = format!("  {name}:");
    let at = lines.iter().position(|l| *l == head).expect(&head);
    let mut found = Vec::new();
    for line in lines[..at].iter().rev() {
        if !line.trim_start().starts_with('#') {
            break;
        }
        found.push(*line);
    }
    found.join("\n").to_lowercase()
}

#[test]
fn the_comment_of_each_new_step_tells_about_new_and_active_runs() {
    for name in ["simplify", "check_review", "check_security"] {
        let comment = comment_above(name);
        assert!(comment.contains("new run"), "{name}: {comment}");
        assert!(comment.contains("active"), "{name}: {comment}");
        let reads = comment.contains("a run reads the sheet at each command");
        assert!(reads, "{name}: {comment}");
        assert!(!comment.contains("keeps its sheet"), "{name}: {comment}");
    }
}

#[test]
fn no_file_has_the_words_the_tool_of_the_command() {
    let mut files = vec![
        "README.md".to_owned(),
        "AGENTS.md".to_owned(),
        "ARCHITECTURE.md".to_owned(),
        SKILL.to_owned(),
        "graph.example.yaml".to_owned(),
    ];
    for entry in fs::read_dir(root().join("src")).unwrap() {
        let name = entry.unwrap().file_name();
        files.push(format!("src/{}", name.to_string_lossy()));
    }
    for rel in files {
        let low = text(&rel).to_lowercase();
        assert!(!low.contains("the tool of the command"), "{rel}");
    }
    let sheet = text("graph.example.yaml");
    assert!(sheet.contains("the tool of the run"));
}

#[test]
fn a_codex_chat_adds_tool_codex_only_to_run() {
    for rel in [SKILL, "README.md"] {
        let t = text(rel);
        let line = t
            .lines()
            .find(|l| l.contains("--tool codex") && l.contains("only"))
            .unwrap_or_else(|| panic!("{rel}: no line with --tool codex and only"));
        assert!(line.contains("`run`"), "{rel}: {line}");
        assert!(!line.contains("each command"), "{rel}: {line}");
    }
}

/// The sentences of a document that tell the chat to run `resume`.
fn resume_sentences(rel: &str) -> Vec<String> {
    let t = text(rel);
    let mut found = Vec::new();
    for line in t.lines().filter(|l| !l.starts_with('|')) {
        for sentence in line.split(". ") {
            let tells = sentence.contains("run `codeumbrella resume")
                || sentence.contains("runs `resume`")
                || sentence.contains("run `resume`");
            if tells {
                found.push(sentence.to_owned());
            }
        }
    }
    found
}

#[test]
fn each_sentence_that_tells_the_chat_to_run_resume_waits_for_a_yes() {
    for (rel, least) in [(SKILL, 3), ("AGENTS.md", 1)] {
        let sentences = resume_sentences(rel);
        assert!(sentences.len() >= least, "{rel}: {sentences:?}");
        for sentence in sentences {
            let waits = sentence
                .to_lowercase()
                .contains("only after the person says yes");
            assert!(waits, "{rel}: {sentence}");
        }
    }
}

#[test]
fn the_documents_name_the_new_roles() {
    for rel in ["AGENTS.md", "ARCHITECTURE.md", "README.md"] {
        let low = text(rel).to_lowercase();
        for word in ["simplifier", "reviewer", "security"] {
            assert!(low.contains(word), "{rel}: {word}");
        }
    }
    let agents = text("AGENTS.md");
    for step in ["check_review", "check_security"] {
        assert!(agents.contains(step), "{step}");
    }
}

#[test]
fn the_source_files_are_the_same_seven_of_today() {
    let mut names: Vec<String> = fs::read_dir(root().join("src"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let expected = [
        "claude.rs",
        "codex.rs",
        "engine.rs",
        "graph.rs",
        "lib.rs",
        "log.rs",
        "main.rs",
    ];
    assert_eq!(names, expected);
}
