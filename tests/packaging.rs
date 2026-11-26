//! Tests of the plugin files, the README and `install.sh`. They do not
//! start Claude, Codex or the network.

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

/// Runs a copy of `install.sh` in a new git folder. The tools `gh` and
/// `claude` are simulated, `cargo` is not there, and the store is a folder.
/// `store` is `good`, `bad` (a checksum that does not agree) or `none`.
/// `before` is a shell line that runs in the test folder, after the commit
/// and the store, and before the script. That folder has `repo`, `home`
/// and `bin`, the folder of the simulated tools.
/// It gives the output, the result and the home folder.
fn run_install(label: &str, store: &str, before: &str) -> (String, bool, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("cu-install-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (repo, home, bin) = (dir.join("repo"), dir.join("home"), dir.join("bin"));
    for d in [
        &repo.join("skills/codeumbrella"),
        &home.join(".claude"),
        &bin,
    ] {
        std::fs::create_dir_all(d).expect("make a folder");
    }
    std::fs::copy(root().join("install.sh"), repo.join("install.sh")).expect("copy install.sh");
    std::fs::write(repo.join("skills/codeumbrella/SKILL.md"), "x").expect("write a page");
    for tool in ["gh", "claude"] {
        std::fs::write(bin.join(tool), "#!/bin/sh\nexit 0\n").expect("write a tool");
        let mode = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(bin.join(tool), mode).expect("set the mode");
    }
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let sh = |cwd: &Path, line: &str| {
        let out = Command::new("/bin/sh")
            .args(["-c", line])
            .current_dir(cwd)
            .env("HOME", &home)
            .env("PATH", &path)
            .env("PACKED", env!("CARGO_BIN_EXE_codeumbrella"))
            .env(
                "CODEUMBRELLA_STORE",
                format!("file://{}/store", dir.display()),
            )
            .output()
            .expect("start a shell");
        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        (text, out.status.success())
    };
    let commit =
        "git init -q && git add . && git -c user.name=t -c user.email=t@t.t commit -q -m t";
    let (text, ok) = sh(&repo, commit);
    assert!(ok, "git: {text}");
    if store != "none" {
        // The same file names and folders as the GitHub step and Cloudsmith.
        let pack = "c=$(git -C repo rev-parse HEAD) n=codeumbrella-aarch64-apple-darwin
            f=codeumbrella-$c-aarch64-apple-darwin.tar.gz
            a=store/raw/names/$n/versions/$c b=store/raw/names/$n-sha256/versions/$c
            mkdir -p pack $a $b && cp \"$PACKED\" pack/codeumbrella
            tar -czf $a/$f -C pack codeumbrella && (cd $a && shasum -a 256 $f) > $b/$f.sha256";
        let (text, ok) = sh(&dir, &format!("cd {} && {pack}", dir.display()));
        assert!(ok, "pack: {text}");
        if store == "bad" {
            let (text, ok) = sh(
                &dir,
                "for f in store/raw/names/*-sha256/versions/*/*; do sed -i.x 's/^..../0000/' $f; done",
            );
            assert!(ok, "bad checksum: {text}");
        }
    }
    let (text, ok) = sh(&dir, before);
    assert!(ok, "before: {text}");
    let (text, ok) = sh(&repo, "./install.sh");
    (text, ok, home)
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn install_script_downloads_the_built_command_with_no_cargo() {
    let (text, ok, home) = run_install("good", "good", "");
    assert!(ok, "{text}");
    assert!(text.contains("Checksum: OK"), "{text}");
    assert!(home.join(".cargo/bin/codeumbrella").is_file(), "{text}");
    assert!(home.join(".claude/skills/codeumbrella").exists(), "{text}");
}

#[test]
fn install_script_tells_how_to_get_cargo_when_the_store_has_no_file() {
    let (text, ok, home) = run_install("none", "none", "");
    assert!(!ok, "{text}");
    for part in ["Not found: cargo", "sh.rustup.rs", "Nothing was installed."] {
        assert!(text.contains(part), "the output lacks `{part}`: {text}");
    }
    assert!(!home.join(".cargo").exists(), "{text}");
    assert!(!home.join(".claude/skills").exists(), "{text}");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn install_script_refuses_a_file_with_a_bad_checksum() {
    let (text, ok, home) = run_install("bad", "bad", "");
    assert!(!ok, "{text}");
    assert!(text.contains("checksum of the downloaded file"), "{text}");
    assert!(!home.join(".cargo").exists(), "{text}");
}

/// The value of the first line that starts with `key`, with no quote marks.
fn value_of<'a>(body: &'a str, key: &str) -> &'a str {
    body.lines()
        .find_map(|row| row.trim_start().strip_prefix(key))
        .unwrap_or_else(|| panic!("no line starts with {key}"))
        .trim_matches('"')
}

#[test]
fn install_script_and_release_have_the_same_file_names() {
    let script = text("install.sh");
    let release = text(".github/workflows/release.yml");
    let package = "codeumbrella-aarch64-apple-darwin";
    let sums = format!("{package}-sha256");
    assert_eq!(value_of(&script, "name="), package);
    let mut uploads: Vec<&str> = release
        .split("--name ")
        .skip(1)
        .filter_map(|part| part.split_whitespace().next())
        .collect();
    uploads.sort_unstable();
    assert_eq!(uploads, [package, sums.as_str()]);
    for part in [
        "raw/names/$name/versions/$commit/$file",
        "raw/names/$name-sha256/versions/$commit/$file.sha256",
    ] {
        assert!(script.contains(part), "install.sh lacks `{part}`");
    }
    let in_release = value_of(&release, "file=");
    let in_script = value_of(&script, "file=");
    let uploaded = in_release.replace("$GITHUB_SHA", "<commit>");
    let wanted = in_script.replace("$commit", "<commit>");
    assert_eq!(uploaded, wanted);
    assert_eq!(wanted, "codeumbrella-<commit>-aarch64-apple-darwin.tar.gz");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn install_script_does_not_download_for_a_folder_with_changes() {
    let before = "echo more >> repo/skills/codeumbrella/SKILL.md";
    let (text, ok, home) = run_install("changes", "good", before);
    assert!(!ok, "{text}");
    assert!(text.contains("changes of its own"), "{text}");
    assert!(!text.contains("Checksum: OK"), "{text}");
    assert!(!home.join(".cargo").exists(), "{text}");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn install_script_downloads_with_a_file_that_git_does_not_have() {
    let before = "mkdir repo/notes && echo more > repo/notes/page.txt";
    let (text, ok, home) = run_install("untracked", "good", before);
    assert!(ok, "{text}");
    assert!(text.contains("Checksum: OK"), "{text}");
    assert!(home.join(".cargo/bin/codeumbrella").is_file(), "{text}");
}

#[test]
fn install_script_stops_when_a_tool_is_not_there() {
    let (text, ok, home) = run_install("no-gh", "none", "rm bin/gh");
    assert!(!ok, "{text}");
    assert!(text.contains("Not found: gh"), "{text}");
    assert!(text.contains("Nothing was installed."), "{text}");
    assert!(!home.join(".cargo").exists(), "{text}");
}

/// The names of the files in `.cargo/bin` of a home folder.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn installed(home: &Path) -> Vec<String> {
    let Ok(list) = std::fs::read_dir(home.join(".cargo/bin")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = list
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The cause that the script prints for no download. It is one line.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn cause_of(output: &str) -> &str {
    let mut rows = output.lines();
    let first = rows
        .find_map(|row| row.strip_prefix("No download of the built command: "))
        .unwrap_or_else(|| panic!("no cause line: {output}"));
    assert_eq!(
        rows.next(),
        Some("The script builds the command on this computer."),
        "the cause is not one line: {output}"
    );
    first
        .strip_suffix('.')
        .expect("the cause ends with a point")
}

/// A shell line that puts a simulated tool into `bin`. In `body`, `$last`
/// is the last word that the tool got.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn fake_tool(name: &str, body: &str) -> String {
    let lines = "'#!/bin/sh' 'for last; do :; done'";
    let make = format!("printf '%s\\n' {lines} '{body}' > bin/{name}");
    format!("{make} && chmod +x bin/{name}")
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn install_script_names_the_folder_that_it_cannot_write_to() {
    use std::os::unix::fs::PermissionsExt;
    let before = "mkdir -p home/.cargo/bin && chmod 555 home/.cargo/bin";
    let (text, ok, home) = run_install("locked", "good", before);
    let folder = home.join(".cargo/bin");
    let found = installed(&home);
    let mode = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&folder, mode).expect("set the mode");
    assert!(!ok, "{text}");
    assert!(!text.contains("Checksum: OK"), "{text}");
    let named = folder.display().to_string();
    assert!(cause_of(&text).contains(&named), "{text}");
    assert_eq!(found, Vec::<String>::new(), "{text}");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn install_script_moves_a_side_file_to_the_command_name() {
    use std::process::Command;
    let first = "mkdir -p home/.cargo/bin && echo before > home/.cargo/bin/codeumbrella";
    let body = r#"printf "%s\n" "$@" >> "$0.log"; /bin/mv "$@""#;
    let before = format!("{first} && {}", fake_tool("mv", body));
    let (text, ok, home) = run_install("side", "good", &before);
    assert!(ok, "{text}");
    let moved = std::fs::read_to_string(home.with_file_name("bin").join("mv.log"));
    let moved = moved.expect("the script uses mv");
    let words: Vec<&str> = moved.lines().collect();
    let [.., from, to] = words[..] else {
        panic!("mv did not get two files: {moved}")
    };
    let folder = home.join(".cargo/bin");
    assert_eq!(Path::new(to), folder.join("codeumbrella"), "{moved}");
    let parent = Path::new(from).parent();
    assert_eq!(parent, Some(folder.as_path()), "{moved}");
    let name = Path::new(from).file_name().expect("a name");
    let dot = name.to_string_lossy().starts_with('.');
    assert!(dot, "the side name does not start with a dot: {moved}");
    assert_eq!(installed(&home), ["codeumbrella"], "{text}");
    let command = folder.join("codeumbrella");
    let out = Command::new(&command).arg("--version").output();
    let out = out.expect("the installed command starts");
    assert!(out.status.success(), "{text}");
}

/// A bad `tool` in `.cargo/bin`: the command from before stays as it is.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn keeps_the_command_from_before(label: &str, tool: &str, body: &str) {
    let first = "mkdir -p home/.cargo/bin && echo before > home/.cargo/bin/codeumbrella";
    let before = format!("{first} && {}", fake_tool(tool, body));
    let (text, ok, home) = run_install(label, "good", &before);
    assert!(!ok, "{text}");
    assert!(!text.contains("Checksum: OK"), "{text}");
    let folder = home.join(".cargo/bin").display().to_string();
    assert!(cause_of(&text).contains(&folder), "{text}");
    assert_eq!(installed(&home), ["codeumbrella"], "{text}");
    let kept = std::fs::read_to_string(home.join(".cargo/bin/codeumbrella"));
    assert_eq!(kept.expect("read the command"), "before\n", "{text}");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn install_script_keeps_no_half_copy() {
    let body = r#"case $last in */.cargo/*) echo half > "$last"; exit 1;; esac; /bin/cp "$@""#;
    keeps_the_command_from_before("half", "cp", body);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn install_script_keeps_no_side_file_after_a_bad_move() {
    let body = r#"case $last in */bin/codeumbrella) exit 1;; esac; /bin/mv "$@""#;
    keeps_the_command_from_before("move", "mv", body);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn install_script_names_the_folder_that_it_cannot_make() {
    let body = r#"case "$*" in *.cargo*) exit 1;; esac; /bin/mkdir "$@""#;
    let (text, ok, home) = run_install("mkdir", "good", &fake_tool("mkdir", body));
    assert!(!ok, "{text}");
    assert!(!text.contains("Checksum: OK"), "{text}");
    let folder = home.join(".cargo/bin").display().to_string();
    assert!(cause_of(&text).contains(&folder), "{text}");
    assert!(!home.join(".cargo").exists(), "{text}");
}

/// A bad `tool` before the download gives a cause of its own.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn gives_a_cause_for_a_bad_tool(label: &str, tool: &str, body: &str) {
    let (text, ok, home) = run_install(label, "good", &fake_tool(tool, body));
    assert!(!ok, "{text}");
    assert!(!text.contains("Checksum: OK"), "{text}");
    let cause = cause_of(&text);
    assert!(!cause.is_empty(), "{text}");
    assert!(!cause.contains("the store has no file"), "{text}");
    assert!(!home.join(".cargo").exists(), "{text}");
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn install_script_gives_a_cause_for_a_bad_rev_parse() {
    // The simulated `git` prints the commit and gives an error code.
    let body = r#"g=/usr/bin/git; case $1 in rev-parse) $g "$@"; exit 1;; esac; $g "$@""#;
    gives_a_cause_for_a_bad_tool("rev-parse", "git", body);
}

#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn install_script_gives_a_cause_for_a_bad_mktemp() {
    gives_a_cause_for_a_bad_tool("mktemp", "mktemp", "exit 1");
}

#[test]
fn agents_row_of_the_packaging_tests_names_the_install_script() {
    let a = text("AGENTS.md");
    let row = a
        .lines()
        .find(|l| l.starts_with("| `tests/packaging.rs` |"));
    let row = row.expect("a row for tests/packaging.rs");
    assert!(row.contains("`install.sh`"), "{row}");
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
