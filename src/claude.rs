//! The tool part for Claude: it starts one `claude -p` worker for one step.

use crate::engine::{Job, Outcome, Worker};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use std::{env, fmt::Write as _, fs, fs::File, path::PathBuf, thread};

/// The format of the result that each worker must give.
pub(crate) const SCHEMA: &str = r#"{"type":"object","properties":{"ok":{"type":"boolean"},"summary":{"type":"string"}},"required":["ok","summary"],"additionalProperties":false}"#;

/// Starts Claude workers.
#[derive(Debug)]
pub struct Claude {
    /// The maximum time for one step.
    pub time_limit: Duration,
}

/// The part of the Claude output that the supervisor uses.
#[derive(Deserialize)]
struct Reply {
    is_error: bool,
    #[serde(default)]
    result: String,
    structured_output: Option<Outcome>,
}

impl Worker for Claude {
    fn run(&self, job: &Job) -> Result<Outcome> {
        let rules = fs::read_to_string(job.rules)
            .with_context(|| format!("cannot read the rules page {}", job.rules.display()))?;
        // `{project}` in the settings is the full path of the project folder.
        let settings = fs::read_to_string(job.rules.with_extension("claude.json"));
        let project = env::current_dir()?;
        let settings = settings
            .unwrap_or_default()
            .replace("{project}", &project.to_string_lossy());
        let (out, err) = files(job);
        let mut command = Command::new("claude");
        command
            .args(["-p", &prompt(job), "--append-system-prompt", &rules])
            .args(["--output-format", "stream-json", "--verbose"])
            .args(["--json-schema", SCHEMA])
            .args(["--permission-mode", "dontAsk"])
            .args(["--setting-sources", "project"])
            .stdin(Stdio::null())
            .stdout(File::create(&out)?)
            .stderr(File::create(&err)?);
        if !settings.is_empty() {
            command.args(["--settings", &settings]);
        }
        let child = command.spawn().context("cannot start the claude command")?;
        wait(child, self.time_limit)?;
        // The last line of the trace is the result.
        let text = fs::read_to_string(&out)?;
        let reply: Reply = serde_json::from_str(text.lines().last().unwrap_or_default())
            .with_context(|| format!("claude gave no result; see {}", err.display()))?;
        if reply.is_error {
            bail!("claude reported an error: {}", reply.result);
        }
        reply
            .structured_output
            .context("claude gave no result in the correct format")
    }
}

/// The trace file and the error file of this try. A new try gets a new number.
pub(crate) fn files(job: &Job) -> (PathBuf, PathBuf) {
    let name = |n: u32, end: &str| job.dir.join(format!("{}.{n}.{end}", job.step));
    let n = (1..10_000)
        .find(|n| !name(*n, "trace.jsonl").exists())
        .unwrap_or(1);
    (name(n, "trace.jsonl"), name(n, "err.txt"))
}

/// Waits until a worker ends. Stops the worker at the time limit.
pub(crate) fn wait(mut child: Child, limit: Duration) -> Result<()> {
    let end = Instant::now() + limit;
    while child.try_wait()?.is_none() {
        if Instant::now() > end {
            child.kill()?;
            child.wait()?;
            bail!("the worker did not end in {} minutes", limit.as_secs() / 60);
        }
        thread::sleep(Duration::from_millis(500));
    }
    Ok(())
}

/// Writes the instructions for one worker.
pub(crate) fn prompt(job: &Job) -> String {
    let run = job
        .dir
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let papers = job.dir.join("papers");
    let mut text = format!(
        "Task: {}\n\nYou do the step \"{}\" of the run with the id {run}.\n\
         Each paper is a folder in {}.\n",
        job.task,
        job.step,
        papers.display()
    );
    let _ = writeln!(text, "Papers that you read: {}", job.reads.join(", "));
    let _ = writeln!(text, "Papers that you write: {}", job.writes.join(", "));
    if let Some(before) = job.before {
        let word = if before.ok { "good" } else { "bad" };
        let _ = writeln!(
            text,
            "Result of the step before ({word}): {}",
            before.summary
        );
    }
    text.push_str("When you end, give ok and a short summary, as your rules page says.");
    text
}
