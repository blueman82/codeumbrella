//! The tool part for Claude: it starts one `claude -p` worker for one step.

use crate::engine::{Job, Outcome, Worker};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::process::{Child, Command, ExitStatus, Stdio};
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
        // In the settings, `{project}` is the full path of the project folder
        // and `{run}` is the run id. A role with no settings file is an error.
        let path = job.rules.with_extension("claude.json");
        let settings = fs::read_to_string(&path)
            .with_context(|| format!("cannot read the settings {}", path.display()))?;
        let run = job.dir.file_name().unwrap_or_default().to_string_lossy();
        let settings = settings
            .replace("{project}", &env::current_dir()?.to_string_lossy())
            .replace("{run}", &run);
        let (out, err) = files(job);
        let child = Command::new("claude")
            .args(["-p", &prompt(job), "--append-system-prompt", &rules])
            .args(["--output-format", "stream-json", "--verbose"])
            .args(["--json-schema", SCHEMA])
            // Only the settings of the role count. The worker loads no settings,
            // hooks or tool servers of the project or of the person.
            .args(["--permission-mode", "dontAsk", "--strict-mcp-config"])
            .args(["--setting-sources", ""])
            .args(["--settings", &settings])
            // The worker does not get the GitHub token of the person.
            .env_remove("GH_TOKEN")
            .env_remove("GITHUB_TOKEN")
            .current_dir(job.dir.join("work"))
            .stdin(Stdio::null())
            .stdout(File::create(&out)?)
            .stderr(File::create(&err)?)
            .spawn()
            .context("cannot start the claude command")?;
        let status = wait(child, self.time_limit)?;
        // The last line of the trace is the result.
        let text = fs::read_to_string(&out)?;
        let reply: Reply = serde_json::from_str(text.lines().last().unwrap_or_default())
            .with_context(|| format!("claude gave no result; see {}", err.display()))?;
        if reply.is_error {
            bail!("claude reported an error: {}", reply.result);
        }
        ensure!(status.success(), "claude ended with an error: {status}");
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

/// Waits until a worker ends, and gives its exit status. Stops the worker at
/// the time limit.
pub(crate) fn wait(mut child: Child, limit: Duration) -> Result<ExitStatus> {
    let end = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() > end {
            child.kill()?;
            child.wait()?;
            bail!("the worker did not end in {} minutes", limit.as_secs() / 60);
        }
        thread::sleep(Duration::from_millis(500));
    }
}

/// Writes the instructions for one worker.
pub(crate) fn prompt(job: &Job) -> String {
    let run = job.dir.file_name().unwrap_or_default().to_string_lossy();
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
        let (word, sum) = (if before.ok { "good" } else { "bad" }, &before.summary);
        let _ = writeln!(text, "Result of the step before ({word}): {sum}");
    }
    text.push_str("When you end, give ok and a short summary, as your rules page says.");
    text
}
