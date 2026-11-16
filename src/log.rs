//! The log of a run: one line for each step that ended.

use anyhow::Result;
use serde_json::json;
use std::{fs::OpenOptions, io::Write, path::Path, time::Instant};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// Adds one line to `log.jsonl` in the run folder `dir`, and prints one short
/// line for the person. The time is in UTC. `start` is when the step started.
///
/// # Errors
/// Gives an error if the log file cannot be written.
pub fn record(dir: &Path, step: &str, ok: bool, summary: &str, start: Instant) -> Result<()> {
    let (time, took) = (
        OffsetDateTime::now_utc().format(&Rfc3339)?,
        start.elapsed().as_secs(),
    );
    let line = json!({"time": time, "step": step, "ok": ok, "seconds": took, "summary": summary});
    let path = dir.join("log.jsonl");
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    eprintln!("{step}  {}  {took} s", if ok { "good" } else { "BAD" });
    Ok(())
}
