//! The tool part for Claude: it starts one `claude -p` worker for one step.

use crate::engine::{Job, Outcome, Worker};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::{fmt::Write as _, fs, fs::File, thread};

/// The format of the result that each worker must give.
const SCHEMA: &str = r#"{"type":"object","properties":{"ok":{"type":"boolean"},"summary":{"type":"string"}},"required":["ok","summary"]}"#;

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
        let settings = job.rules.with_extension("claude.json");
        let out = job.dir.join(format!("{}.out.json", job.step));
        let err = job.dir.join(format!("{}.err.txt", job.step));
        let mut command = Command::new("claude");
        command
            .args(["-p", &prompt(job), "--append-system-prompt", &rules])
            .args(["--output-format", "json", "--json-schema", SCHEMA])
            .args(["--permission-mode", "dontAsk", "--setting-sources", "project"])
            .stdin(Stdio::null())
            .stdout(File::create(&out)?)
            .stderr(File::create(&err)?);
        if settings.is_file() {
            command.arg("--settings").arg(&settings);
        }
        let mut child = command.spawn().context("cannot start the claude command")?;
        let end = Instant::now() + self.time_limit;
        while child.try_wait()?.is_none() {
            if Instant::now() > end {
                child.kill()?;
                child.wait()?;
                bail!("the worker did not end in {} minutes", self.time_limit.as_secs() / 60);
            }
            thread::sleep(Duration::from_millis(500));
        }
        let text = fs::read_to_string(&out)?;
        let reply: Reply = serde_json::from_str(&text)
            .with_context(|| format!("claude gave no result; see {}", err.display()))?;
        if reply.is_error {
            bail!("claude reported an error: {}", reply.result);
        }
        reply.structured_output.context("claude gave no result in the correct format")
    }
}

/// Writes the instructions for one worker.
fn prompt(job: &Job) -> String {
    let run = job.dir.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let papers = job.dir.join("papers");
    let mut text = format!(
        "Task: {}\n\nYou do the step \"{}\" of the run with the id {run}.\n\
         Each paper is a folder in {}.\n",
        job.task,
        job.step,
        papers.display()
    );
    let _ = writeln!(text, "Papers that you read: {}", list(job.reads));
    let _ = writeln!(text, "Papers that you write: {}", list(job.writes));
    if let Some(before) = job.before {
        let word = if before.ok { "good" } else { "bad" };
        let _ = writeln!(text, "Result of the step before ({word}): {}", before.summary);
    }
    text.push_str("When you end, give ok and a short summary, as your rules page says.");
    text
}

/// Makes a list of paper names, or the word "none".
fn list(papers: &[String]) -> String {
    if papers.is_empty() { "none".to_owned() } else { papers.join(", ") }
}
