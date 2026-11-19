//! The log of a run: one line for each step that ended.

use crate::engine::Numbers;
use anyhow::Result;
use serde_json::{Value, json};
use std::{fs::OpenOptions, io::Write, path::Path, time::Instant};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// Adds one line to `log.jsonl` in the run folder `dir`, and prints one short
/// line for the person. The time is in UTC. `start` is when the step started.
/// `numbers` are the numbers of the worker of the step, if it has one.
///
/// # Errors
/// Gives an error if the log file cannot be written.
pub fn record(
    dir: &Path,
    step: &str,
    ok: bool,
    summary: &str,
    start: Instant,
    numbers: Option<&Numbers>,
) -> Result<()> {
    let (time, took) = (now()?, start.elapsed().as_secs());
    let mut line =
        json!({"time": time, "step": step, "ok": ok, "seconds": took, "summary": summary});
    if let (Some(n), Some(map)) = (numbers, line.as_object_mut()) {
        let fields = [
            ("worker_seconds", n.seconds.map(Value::from)),
            ("dollars", n.dollars.map(Value::from)),
            ("turns", n.turns.map(Value::from)),
            ("model", n.model.clone().map(Value::from)),
            ("tokens_in", n.tokens_in.map(Value::from)),
            ("tokens_out", n.tokens_out.map(Value::from)),
        ];
        for (key, value) in fields {
            if let Some(value) = value {
                map.insert(key.to_owned(), value);
            }
        }
    }
    let path = dir.join("log.jsonl");
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    eprintln!("{step}  {}  {took} s", if ok { "good" } else { "BAD" });
    Ok(())
}

/// The time of this moment in UTC, in the format of the log (RFC 3339).
///
/// # Errors
/// Gives an error if the time cannot be written.
pub fn now() -> Result<String> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}
