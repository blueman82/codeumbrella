//! Tests of the plugin files and the README. They read files only.
//! They do not start Claude, Codex or the network.

use serde_json::Value;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn json(rel: &str) -> Value {
    serde_json::from_str(&text(rel)).unwrap_or_else(|e| panic!("{rel} is not JSON: {e}"))
}

/// The path of a marketplace entry: a string, or an object with `path`.
fn source_path(plugin: &Value) -> String {
    let s = &plugin["source"];
    s.as_str()
        .or_else(|| s["path"].as_str())
        .unwrap_or_else(|| panic!("plugin has no source path: {plugin}"))
        .to_string()
}

fn entries(marketplace: &str) -> Vec<Value> {
    json(marketplace)["plugins"]
        .as_array()
        .expect("plugins is a list")
        .clone()
}

fn plugin_dir(marketplace: &str) -> PathBuf {
    let list = entries(marketplace);
    let p = list
        .iter()
        .find(|p| p["name"] == "codeumbrella")
        .expect("marketplace lists the plugin codeumbrella");
    root().join(source_path(p))
}

#[test]
fn claude_manifest_names_the_plugin() {
    let m = json(".claude-plugin/plugin.json");
    assert_eq!(m["name"], "codeumbrella");
    assert!(m["description"].as_str().is_some_and(|d| !d.is_empty()));
}

#[test]
fn codex_manifest_names_the_plugin() {
    let m = json(".codex-plugin/plugin.json");
    assert_eq!(m["name"], "codeumbrella");
    assert!(m["description"].as_str().is_some_and(|d| !d.is_empty()));
}

#[test]
fn both_manifests_have_the_same_version_as_cargo() {
    let cargo = text("Cargo.toml");
    let v = cargo
        .lines()
        .find_map(|l| l.strip_prefix("version = \""))
        .and_then(|l| l.strip_suffix('"'))
        .expect("Cargo.toml has a version");
    for f in [".claude-plugin/plugin.json", ".codex-plugin/plugin.json"] {
        assert_eq!(json(f)["version"], v, "{f}");
    }
}

#[test]
fn codex_manifest_points_to_the_skills_folder() {
    let m = json(".codex-plugin/plugin.json");
    let rel = m["skills"].as_str().expect("`skills` is a path");
    assert!(
        root().join(rel).join("codeumbrella/SKILL.md").is_file(),
        "`skills` = {rel} does not lead to codeumbrella/SKILL.md"
    );
}

#[test]
fn claude_marketplace_leads_to_a_plugin_folder() {
    let m = json(".claude-plugin/marketplace.json");
    assert!(m["name"].as_str().is_some_and(|n| !n.is_empty()));
    let dir = plugin_dir(".claude-plugin/marketplace.json");
    assert!(dir.join(".claude-plugin/plugin.json").is_file());
    // Claude finds the skills in `skills/` of the plugin folder.
    assert!(dir.join("skills/codeumbrella/SKILL.md").is_file());
}

#[test]
fn codex_marketplace_leads_to_a_plugin_folder() {
    let m = json(".agents/plugins/marketplace.json");
    assert!(m["name"].as_str().is_some_and(|n| !n.is_empty()));
    let dir = plugin_dir(".agents/plugins/marketplace.json");
    assert!(dir.join(".codex-plugin/plugin.json").is_file());
}

#[test]
fn claude_marketplace_has_an_owner() {
    let m = json(".claude-plugin/marketplace.json");
    assert!(
        m["owner"]["name"].as_str().is_some_and(|n| !n.is_empty()),
        "owner.name must be a non-empty text"
    );
}

#[test]
fn marketplace_entry_names_equal_the_plugin_json_names() {
    for (market, manifest) in [
        (
            ".claude-plugin/marketplace.json",
            ".claude-plugin/plugin.json",
        ),
        (
            ".agents/plugins/marketplace.json",
            ".codex-plugin/plugin.json",
        ),
    ] {
        let list = entries(market);
        assert!(!list.is_empty(), "{market} has no entries");
        for p in &list {
            let file = root().join(source_path(p)).join(manifest);
            let content = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));
            let m: Value = serde_json::from_str(&content)
                .unwrap_or_else(|e| panic!("{} is not JSON: {e}", file.display()));
            assert_eq!(p["name"], m["name"], "{market}");
        }
    }
}

#[test]
fn codex_marketplace_entry_has_policy_and_category() {
    let list = entries(".agents/plugins/marketplace.json");
    assert!(!list.is_empty(), "marketplace has no entries");
    for p in &list {
        assert_eq!(p["policy"]["installation"], "AVAILABLE", "{p}");
        assert_eq!(p["policy"]["authentication"], "ON_INSTALL", "{p}");
        assert_eq!(p["category"], "Productivity", "{p}");
    }
}

fn find_skill_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        if name == "target" || name == ".git" || name == ".codeumbrella" {
            continue;
        }
        if p.is_dir() {
            find_skill_files(&p, out);
        } else if name == "SKILL.md" {
            out.push(p);
        }
    }
}

#[test]
fn skill_md_has_one_source_only() {
    let mut found = Vec::new();
    find_skill_files(&root(), &mut found);
    assert_eq!(
        found,
        vec![root().join("skills/codeumbrella/SKILL.md")],
        "SKILL.md must exist in one place only (no copy, no link in the plugin files)"
    );
}

#[test]
fn skill_keeps_its_name() {
    let s = text("skills/codeumbrella/SKILL.md");
    assert!(s.starts_with("---\nname: codeumbrella\n"));
}

#[test]
fn install_script_still_installs_the_command() {
    let s = text("install.sh");
    assert!(s.contains("cargo install --path . --locked"));
}

#[test]
fn readme_has_the_four_parts_for_a_new_person() {
    let r = text("README.md").to_lowercase();
    // What it is, what a computer must have, how to install, first run.
    for needle in [
        "codeumbrella",
        "software factory",
        "requirements",
        "install",
        "first run",
    ] {
        assert!(r.contains(needle), "README.md lacks: {needle}");
    }
}

#[test]
fn readme_lists_what_the_computer_must_have() {
    let r = text("README.md");
    for cmd in [
        "cargo --version",
        "git --version",
        "gh --version",
        "claude --version",
        "codex --version",
    ] {
        assert!(r.contains(cmd), "README.md lacks the command line: {cmd}");
    }
}

#[test]
fn readme_shows_the_install_commands() {
    let r = text("README.md");
    for cmd in [
        "/plugin marketplace add",
        "/plugin install codeumbrella@codeumbrella",
        "codex plugin marketplace add",
        "./install.sh",
        "cargo install --path . --locked",
    ] {
        assert!(r.contains(cmd), "README.md lacks the command line: {cmd}");
    }
}

#[test]
fn readme_codex_steps_use_the_plugins_menu() {
    let r = text("README.md");
    assert!(
        r.contains("`/plugins`"),
        "README.md must tell Codex users to type /plugins"
    );
}

#[test]
fn readme_says_install_sh_links_the_chat_command() {
    let r = text("README.md").to_lowercase();
    assert!(r.contains("also puts the chat command"));
    assert!(
        r.contains("does not run `./install.sh`"),
        "README.md must say the plugin is for people who do not run ./install.sh"
    );
}

#[test]
fn readme_tells_that_the_plugin_needs_the_command() {
    // The plugin brings only the chat command. The binary comes from cargo.
    let r = text("README.md").to_lowercase();
    assert!(
        r.contains("cargo install"),
        "README.md must tell how to get the binary"
    );
}

#[test]
fn readme_shows_how_to_start_a_first_run() {
    let r = text("README.md");
    assert!(r.contains("codeumbrella init"));
    assert!(r.contains("/codeumbrella"));
    assert!(r.contains("codeumbrella run"));
}

#[test]
fn agents_structure_table_lists_the_packaging_files() {
    let a = text("AGENTS.md");
    for path in [
        "`README.md`",
        "`.claude-plugin/`",
        "`.codex-plugin/`",
        "`.agents/plugins/`",
        "`tests/packaging.rs`",
    ] {
        let start = format!("| {path} |");
        assert!(
            a.lines().any(|l| l.starts_with(&start)),
            "AGENTS.md Structure table has no row for {path}"
        );
    }
}

/// Lesson of run-1795113806: a test with the version of today in quote
/// marks breaks at the next version. So no test file names it.
#[test]
fn no_test_file_names_the_version_of_today() {
    let toml = text("Cargo.toml");
    let line = toml
        .lines()
        .find(|l| l.starts_with("version = "))
        .expect("Cargo.toml has a version line");
    let quoted = line.trim_start_matches("version = ");
    let dir = root().join("tests");
    for entry in std::fs::read_dir(&dir).expect("tests folder") {
        let path = entry.expect("entry").path();
        // A folder holds no test file, and a crash input can be binary.
        if path.is_dir() {
            continue;
        }
        let read = std::fs::read_to_string(&path);
        let msg = format!("read {}", path.display());
        let body = read.expect(&msg);
        assert!(
            !body.contains(quoted),
            "{} names the version {quoted}",
            path.display()
        );
    }
}
