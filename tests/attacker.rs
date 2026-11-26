//! Tests of the plan attacker: its step in the sheet, its role files, its
//! settings and its rules page. Also the commit from before the build, that
//! the attacker uses. The worker is simulated. No Claude, no Codex.

use anyhow::Result;
use codeumbrella::engine::{Job, Outcome, Run, Status, Worker};
use codeumbrella::graph::{Graph, Kind};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, fs, process};

const FILES: [&str; 3] = [
    "roles/attacker.md",
    "roles/attacker.claude.json",
    "roles/attacker.codex.txt",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn text(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn json(rel: &str) -> Value {
    serde_json::from_str(&text(rel)).unwrap_or_else(|e| panic!("{rel} is not JSON: {e}"))
}

fn folder() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    env::temp_dir().join(format!("codeumbrella-attacker-{}-{n}", process::id()))
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

fn default_graph() -> Graph {
    Graph::load(&root().join("graph.example.yaml"), &root().join("roles")).unwrap()
}

#[test]
fn the_default_sheet_has_the_attack_step_between_the_plan_and_the_person() {
    let graph = default_graph();
    let attack = graph.step("attack_plan").unwrap();
    assert_eq!(attack.kind, Kind::Worker);
    assert_eq!(attack.role.as_deref(), Some("attacker"));
    assert_eq!(attack.reads, ["spec", "tests"]);
    assert_eq!(attack.writes, [] as [String; 0]);
    assert_eq!(attack.on_bad.as_deref(), Some("correct_plan"));
    assert_eq!(attack.next.as_deref(), Some("look_at_tests"));
    let plan = graph.step("plan").unwrap();
    assert_eq!(plan.next.as_deref(), Some("attack_plan"));
    assert_eq!(plan.max_tries, Some(3));
    // Three attacks at the maximum, in one line. After each bad attack, the
    // planner corrects the plan. The last correction goes to the person.
    let line = [
        ("attack_plan", "correct_plan", "look_at_tests"),
        ("correct_plan", "", "attack_plan_2"),
        ("attack_plan_2", "correct_plan_2", "look_at_tests"),
        ("correct_plan_2", "", "attack_plan_3"),
        ("attack_plan_3", "correct_plan_3", "look_at_tests"),
        ("correct_plan_3", "", "look_at_tests"),
    ];
    for (name, on_bad, next) in line {
        let step = graph.step(name).unwrap();
        let attacker = name.starts_with("attack");
        let role = if attacker { "attacker" } else { "planner" };
        assert_eq!(step.role.as_deref(), Some(role), "{name}");
        assert_eq!(step.writes.is_empty(), attacker, "{name}");
        assert_eq!(step.on_bad.as_deref().unwrap_or(""), on_bad, "{name}");
        assert_eq!(step.next.as_deref(), Some(next), "{name}");
    }
    // Only `plan` goes to the first attack. Thus a circle is not possible.
    for (name, step) in &graph.steps {
        let to_attack = [&step.next, &step.on_bad].contains(&&Some("attack_plan".to_owned()));
        assert_eq!(to_attack, name == "plan", "{name}");
    }
}

#[test]
fn the_three_role_files_exist_and_init_writes_them() {
    for rel in FILES {
        assert!(!text(rel).trim().is_empty(), "{rel} is empty");
    }
    let dir = folder();
    fs::create_dir_all(&dir).unwrap();
    assert!(cu(&dir, &["init"]));
    for rel in FILES {
        let written = dir.join(".codeumbrella").join(rel);
        assert_eq!(fs::read_to_string(&written).unwrap(), text(rel), "{rel}");
    }
    // `init --update` puts a missing file back.
    let gone = dir.join(".codeumbrella/roles/attacker.codex.txt");
    fs::remove_file(&gone).unwrap();
    assert!(cu(&dir, &["init", "--update"]));
    assert_eq!(
        fs::read_to_string(&gone).unwrap(),
        text("roles/attacker.codex.txt")
    );
}

fn list(value: &Value) -> Vec<&str> {
    let items = value.as_array().expect("a list");
    items.iter().map(|v| v.as_str().expect("text")).collect()
}

#[test]
fn the_attacker_has_a_shell_in_the_sandbox_and_writes_only_in_the_temp_folder() {
    let file = json("roles/attacker.claude.json");
    let allow = list(&file["permissions"]["allow"]);
    assert_eq!(allow, ["Read", "Glob", "Grep", "Bash"]);
    let deny = list(&file["permissions"]["deny"]);
    assert!(deny.contains(&"Edit(/{project}/**)"), "{deny:?}");
    assert!(deny.contains(&"Bash(git push *)"), "{deny:?}");
    for rule in ["WebFetch", "WebSearch"] {
        assert!(deny.contains(&rule), "{rule}");
    }
    assert!(!allow.iter().any(|a| a.starts_with("Edit")), "{allow:?}");
    assert!(!allow.contains(&"Write"), "{allow:?}");
    assert_eq!(file["disableAllHooks"], true);
    assert_eq!(file["sandbox"]["enabled"], true);
    assert_eq!(file["sandbox"]["allowUnsandboxedCommands"], false);
    assert_eq!(file["sandbox"]["failIfUnavailable"], true);
    let network = file["sandbox"]["network"].as_object().expect("network");
    assert_eq!(network.keys().collect::<Vec<_>>(), ["allowLocalBinding"]);
    let keys = list(&file["sandbox"]["filesystem"]["denyRead"]);
    assert_eq!(keys.len(), 5, "{keys:?}");
    for key in ["~/.ssh", "~/.config/gh", "~/.netrc", "~/.git-credentials"] {
        assert!(keys.contains(&key), "{key}");
    }
    assert!(keys.contains(&"~/Library/Keychains"));
    // The settings are the same as the settings of the safety checker.
    assert_eq!(file, json("roles/safety.claude.json"));
    let codex = text("roles/attacker.codex.txt");
    assert!(codex.starts_with("{extends=\":read-only\""), "{codex}");
    // One more entry than the safety checker: write in the temp folder.
    assert_eq!(codex.matches("\"write\"").count(), 1, "{codex}");
    assert!(!codex.contains("danger"));
    let safety = text("roles/safety.codex.txt");
    assert_eq!(codex.replace(", \":tmpdir\"=\"write\"", ""), safety);
}

/// The numbers at the start of the rules of a page.
fn numbers(rel: &str) -> Vec<usize> {
    let page = text(rel);
    let lines = page.lines();
    lines
        .filter_map(|l| l.split_once(". ")?.0.parse().ok())
        .collect()
}

#[test]
fn the_rules_page_of_the_attacker_names_the_three_methods() {
    let page = text("roles/attacker.md").to_lowercase();
    for method in ["pre-mortem", "pre-parade", "red team"] {
        assert!(page.contains(method), "{method}");
    }
    for word in ["agents.md", "claude.md", "ok: false", "ok: true"] {
        assert!(page.contains(word), "{word}");
    }
    assert!(page.contains("100 words or fewer"));
    for rel in ["roles/attacker.md", "roles/safety.md", "roles/planner.md"] {
        let found = numbers(rel);
        let expected: Vec<usize> = (1..=found.len()).collect();
        assert!(!found.is_empty(), "{rel}");
        assert_eq!(found, expected, "{rel}");
    }
}

#[test]
fn the_planner_and_the_safety_checker_have_their_new_rules() {
    let planner = text("roles/planner.md").to_lowercase();
    assert!(planner.contains("attacker"));
    let safety = text("roles/safety.md");
    assert!(safety.contains("settings files"));
    assert!(safety.contains("17 November 2026") || safety.contains("use the changed lines"));
}

/// A worker that gives a bad result for one step a set number of times.
struct Script {
    bad_step: &'static str,
    bad_times: Mutex<u32>,
    calls: Mutex<Vec<String>>,
}

impl Script {
    fn new(bad_step: &'static str, bad_times: u32) -> Self {
        Self {
            bad_step,
            bad_times: Mutex::new(bad_times),
            calls: Mutex::default(),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Worker for Script {
    fn run(&self, job: &Job) -> Result<Outcome> {
        // The parts of the fan run at the same time, so their order is not fixed.
        if ![
            "check_safety",
            "check_review",
            "check_security",
            "check_use",
        ]
        .contains(&job.step)
        {
            self.calls.lock().unwrap().push(job.step.to_owned());
        }
        for paper in job.writes {
            fs::write(job.dir.join("papers").join(paper).join("file.md"), job.step)?;
        }
        let mut left = self.bad_times.lock().unwrap();
        let mut ok = true;
        if job.step.starts_with(self.bad_step) {
            ok = *left == 0;
            *left = left.saturating_sub(1);
        }
        Ok(Outcome {
            ok,
            summary: format!("{} ran", job.step),
        })
    }
}

/// The default sheet, with a command that does nothing in each command step.
fn sheet() -> (Graph, PathBuf) {
    let roles = root().join("roles");
    let mut graph = default_graph();
    for step in graph.steps.values_mut().filter(|s| !s.run.is_empty()) {
        step.run = vec![vec!["true".to_owned()]];
    }
    (graph, roles)
}

#[test]
fn a_good_attack_result_goes_to_the_person() {
    let (graph, roles) = sheet();
    let worker = Script::new("attack_plan", 0);
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    std::fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    assert_eq!(worker.calls(), ["plan", "attack_plan"]);
}

#[test]
fn one_bad_attack_gets_one_correction_and_a_second_attack() {
    let (graph, roles) = sheet();
    let worker = Script::new("attack_plan", 1);
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    std::fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    assert_eq!(
        worker.calls(),
        ["plan", "attack_plan", "correct_plan", "attack_plan_2"]
    );
}

#[test]
fn an_attacker_that_is_always_bad_gets_three_rounds_and_then_the_person_sees_the_tests() {
    let (graph, roles) = sheet();
    let worker = Script::new("attack_plan", 99);
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    std::fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.status, Status::Waiting);
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    assert_eq!(
        worker.calls(),
        [
            "plan",
            "attack_plan",
            "correct_plan",
            "attack_plan_2",
            "correct_plan_2",
            "attack_plan_3",
            "correct_plan_3"
        ]
    );
    assert!(!run.state.results.contains_key("build"));
}

#[test]
fn a_no_from_the_person_still_goes_to_the_planner_and_the_attacker() {
    let (graph, roles) = sheet();
    let worker = Script::new("attack_plan", 0);
    let mut run = Run::start(folder(), "a task", &graph).unwrap();
    std::fs::write(run.dir.join("work/file"), "x").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    run.answer(false, "The tests are too easy.").unwrap();
    run.go(&graph, &roles, &worker).unwrap();
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"));
    assert_eq!(
        worker.calls(),
        ["plan", "attack_plan", "plan", "attack_plan"]
    );
}

#[test]
fn the_four_version_numbers_are_the_same() {
    let cargo = text("Cargo.toml");
    let line = cargo
        .lines()
        .find(|l| l.starts_with("version = \""))
        .expect("version line");
    let version = line
        .trim_start_matches("version = \"")
        .trim_end_matches('"');
    let lock = text("Cargo.lock");
    let (_, after) = lock
        .split_once("name = \"codeumbrella\"\n")
        .expect("package");
    let locked = after.lines().next().expect("version line");
    assert_eq!(locked, format!("version = \"{version}\""));
    for rel in [".claude-plugin/plugin.json", ".codex-plugin/plugin.json"] {
        assert_eq!(json(rel)["version"], version, "{rel}");
    }
}

/// Counts as `grep -v -E '^\s*(//|$)' <file> | wc -l` does.
fn code_lines(rel: &str) -> usize {
    let page = text(rel);
    let lines = page.lines().map(str::trim);
    lines
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .count()
}

#[test]
fn the_code_obeys_the_limits_for_lines() {
    let supervisor = ["graph", "engine", "main", "log", "lib"];
    let total: usize = supervisor
        .iter()
        .map(|name| code_lines(&format!("src/{name}.rs")))
        .sum();
    assert!(total <= 2000, "the supervisor has {total} lines");
    for part in ["claude", "codex"] {
        let lines = code_lines(&format!("src/{part}.rs"));
        assert!(lines <= 200, "{part} has {lines} lines");
    }
}

// ---------------------------------------------------------------------
// The commit from before the build
// ---------------------------------------------------------------------

/// Runs `git` in a folder, with a name and an email. Gives the output text.
fn git(dir: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    command.args(["-c", "user.name=t", "-c", "user.email=t@t"]);
    command.args(["-c", "commit.gpgsign=false"]);
    let out = command.args(args).output().expect("start git");
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "git {args:?}: {error}");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Writes a file `name` in the folder and commits it.
fn commit(dir: &Path, name: &str) {
    fs::write(dir.join(name), name).unwrap();
    git(dir, &["add", name]);
    git(dir, &["commit", "--quiet", "-m", name]);
}

/// A new project with one commit. The path is the full one.
fn project() -> PathBuf {
    let dir = folder();
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let dir = fs::canonicalize(&dir).unwrap();
    git(&dir, &["init", "--quiet"]);
    commit(&dir, "file");
    dir
}

/// The default sheet for a run in the project `dir`. The steps `copy` and
/// `mark` run their commands there. Each other command step does nothing.
fn git_sheet(dir: &Path) -> Graph {
    let mut graph = default_graph();
    let top = dir.display().to_string();
    let full = format!("{top}/.codeumbrella/runs/{{run}}");
    for (name, step) in &mut graph.steps {
        if ["copy", "mark"].contains(&name.as_str()) {
            for word in step.run.iter_mut().flatten() {
                if word == "." {
                    word.clone_from(&top);
                }
                *word = word.replace(".codeumbrella/runs/{run}", &full);
            }
        } else if !step.run.is_empty() {
            step.run = vec![vec!["true".to_owned()]];
        }
    }
    graph
}

/// A simulated worker. The builder commits a file `build` and gives a bad
/// result. The attacker looks for that file in the commit from before.
#[derive(Default)]
struct BadBuild {
    calls: Mutex<Vec<String>>,
    looked: Mutex<Vec<String>>,
}

impl Worker for BadBuild {
    fn run(&self, job: &Job) -> Result<Outcome> {
        self.calls.lock().unwrap().push(job.step.to_owned());
        for paper in job.writes {
            let path = job.dir.join("papers").join(paper);
            fs::write(path.join("file.md"), job.step)?;
        }
        let work = job.dir.join("work");
        let mut ok = true;
        if job.step == "build" {
            commit(&work, "build");
            ok = false;
        }
        if job.step.starts_with("attack") {
            let noted = fs::read_to_string(job.dir.join("first-yes.txt"));
            // With no file, the commit of the start of the run.
            let before = noted.unwrap_or_else(|_| {
                let all = git(&work, &["log", "-g", "--format=%H", "HEAD"]);
                all.lines().last().unwrap_or_default().to_owned()
            });
            let before = before.trim().to_owned();
            let files = git(&work, &["ls-tree", "--name-only", &before]);
            ok = !files.lines().any(|l| l == "build");
            self.looked.lock().unwrap().push(before);
        }
        Ok(Outcome {
            ok,
            summary: format!("{} ran", job.step),
        })
    }
}

/// A run in the project `dir` that waits at `look_at_tests`.
fn to_the_tests(dir: &Path, graph: &Graph, worker: &BadBuild) -> Run {
    let place = dir.join(".codeumbrella/runs/run-1");
    let mut run = Run::start(place, "a task", graph).unwrap();
    run.go(graph, &root().join("roles"), worker).unwrap();
    let note = run.state.note.clone();
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"), "{note}");
    run
}

#[test]
fn after_a_bad_build_with_a_commit_the_attack_sees_the_code_from_before() {
    let dir = project();
    let first = git(&dir, &["rev-parse", "HEAD"]);
    let graph = git_sheet(&dir);
    let worker = BadBuild::default();
    let mut run = to_the_tests(&dir, &graph, &worker);
    run.answer(true, "").unwrap();
    run.go(&graph, &root().join("roles"), &worker).unwrap();

    let note = run.state.note.clone();
    assert_eq!(run.state.status, Status::Waiting, "{note}");
    assert_eq!(run.state.at.as_deref(), Some("look_at_tests"), "{note}");
    let calls = worker.calls.lock().unwrap().clone();
    let want = ["plan", "attack_plan", "build", "plan", "attack_plan"];
    assert_eq!(calls, want);
    // The work copy holds the commit of the bad build.
    let work = run.dir.join("work");
    assert_ne!(git(&work, &["rev-parse", "HEAD"]), first);
    let wrote = fs::read_to_string(run.dir.join("first-yes.txt"));
    let wrote = wrote.expect("no file first-yes.txt");
    assert_eq!(wrote.trim(), first);
    let looked = worker.looked.lock().unwrap().clone();
    assert_eq!(looked, [first.clone(), first]);
}

/// Does only the step `mark`.
fn mark_alone(run: &mut Run, dir: &Path) {
    let mut graph = git_sheet(dir);
    let step = graph.steps.get_mut("mark").unwrap();
    step.next = None;
    run.state.at = Some("mark".to_owned());
    let worker = BadBuild::default();
    run.go(&graph, &root().join("roles"), &worker).unwrap();
    assert_eq!(run.state.status, Status::Done, "{}", run.state.note);
}

#[test]
fn mark_writes_the_file_only_in_its_first_time() {
    let dir = project();
    let first = git(&dir, &["rev-parse", "HEAD"]);
    let graph = git_sheet(&dir);
    let mut run = to_the_tests(&dir, &graph, &BadBuild::default());
    let work = run.dir.join("work");
    let file = run.dir.join("first-yes.txt");

    mark_alone(&mut run, &dir);
    let wrote = fs::read_to_string(&file).expect("no first-yes.txt");
    assert_eq!(wrote.trim(), first);

    // With the file there, `mark` keeps it, also with no git name.
    commit(&work, "build");
    git(&work, &["update-ref", "-d", "refs/codeumbrella/before"]);
    mark_alone(&mut run, &dir);
    let kept = fs::read_to_string(&file).expect("no first-yes.txt");
    assert_eq!(kept.trim(), first);

    // With the git name there, this is not the first time of `mark`.
    fs::remove_file(&file).unwrap();
    mark_alone(&mut run, &dir);
    assert!(!file.exists(), "mark wrote the file in a second time");
}

/// The lines of the rule of a page that has the words `words`.
fn rule_with(rel: &str, words: &str) -> String {
    let page = text(rel);
    let mut rules: Vec<String> = Vec::new();
    for line in page.lines() {
        let head = line.split_once(". ").map(|(n, _)| n);
        if head.is_some_and(|n| n.parse::<u32>().is_ok()) {
            rules.push(line.to_owned());
        } else if let Some(rule) = rules.last_mut() {
            rule.push('\n');
            rule.push_str(line);
        }
    }
    let found = rules.into_iter().find(|r| r.contains(words));
    found.unwrap_or_else(|| panic!("{rel}: no rule with {words}"))
}

#[test]
fn the_attacker_page_runs_one_check_on_the_code_from_before() {
    let rule = rule_with("roles/attacker.md", "mktemp");
    let words = [
        "good before the code is written",
        "`../first-yes.txt`",
        "`mark`",
        "git clone",
        "git checkout",
        "other checks",
        "work copy",
    ];
    for word in words {
        assert!(rule.contains(word), "{word}: {rule}");
    }
}

#[test]
fn the_command_of_the_page_gives_the_commit_of_the_start_of_the_run() {
    let rule = rule_with("roles/attacker.md", "mktemp");
    let mut marked = rule.split('`').skip(1).step_by(2);
    let found = marked.find(|c| c.starts_with("git log -g"));
    let found = found.unwrap_or_else(|| panic!("no git log -g: {rule}"));

    let dir = project();
    let first = git(&dir, &["rev-parse", "HEAD"]);
    let copy = folder();
    let _ = fs::remove_dir_all(&copy);
    let target = copy.display().to_string();
    git(&dir, &["clone", "--quiet", ".", &target]);
    commit(&copy, "build");
    commit(&copy, "repair");
    let mut shell = Command::new("sh");
    shell.args(["-c", found]).current_dir(&copy);
    let out = shell.output().expect("start sh");
    let got = String::from_utf8_lossy(&out.stdout);
    assert_eq!(got.trim(), first, "{found}");
}

#[test]
fn the_planner_page_tells_when_no_paper_to_correct_is_a_bad_result() {
    let rule = rule_with("roles/planner.md", "no paper to correct");
    let words = [
        "attack",
        "`ok: false`",
        "person can act on",
        "first line",
        "summary",
        "`ok: true`",
    ];
    for word in words {
        assert!(rule.contains(word), "{word}: {rule}");
    }
}

/// The comment lines right above the step `name`, as one text.
fn comment_above(name: &str) -> String {
    let page = text("graph.example.yaml");
    let lines: Vec<&str> = page.lines().collect();
    let head = format!("  {name}:");
    let at = lines.iter().position(|l| *l == head).expect(&head);
    let above = lines[..at].iter().rev();
    let comments = above.take_while(|l| l.trim_start().starts_with('#'));
    let mut found: Vec<&str> = comments
        .map(|l| l.trim_start().trim_start_matches('#').trim())
        .collect();
    found.reverse();
    found.join(" ").to_lowercase()
}

#[test]
fn the_comments_of_the_sheet_tell_the_new_rule() {
    let mark = comment_above("mark");
    assert!(mark.contains("first-yes.txt"), "{mark}");
    let attack = comment_above("attack_plan");
    for word in ["first-yes.txt", "person can act on"] {
        assert!(attack.contains(word), "{word}: {attack}");
    }
}

#[test]
fn agents_md_tells_the_new_rule() {
    let page = text("AGENTS.md");
    let (_, after) = page.split_once("\n## The sheet\n").expect("part");
    let (part, _) = after.split_once("\n## ").expect("a next part");
    let wanted = [
        ("`mark`", "first-yes.txt"),
        ("attacker", "first-yes.txt"),
        ("planner", "person can act on"),
    ];
    for (one, two) in wanted {
        let has = |l: &str| l.contains(one) && l.contains(two);
        assert!(part.lines().any(has), "no line with {one} and {two}");
    }
}
