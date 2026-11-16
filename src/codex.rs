//! The tool part for Codex: it starts one `codex exec` worker for one step.

use crate::claude::{SCHEMA, files, prompt, wait};
use crate::engine::{Job, Outcome, Worker};
use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use std::{fs, fs::File, time::Duration};

/// Starts Codex workers.
#[derive(Debug)]
pub struct Codex {
    /// The maximum time for one step.
    pub time_limit: Duration,
}

impl Worker for Codex {
    fn run(&self, job: &Job) -> Result<Outcome> {
        let rules = fs::read_to_string(job.rules)
            .with_context(|| format!("cannot read the rules page {}", job.rules.display()))?;
        // The sandbox settings of the role. `{run}` in the file is the run id.
        let settings = job.rules.with_extension("codex.txt");
        let settings = fs::read_to_string(&settings)
            .with_context(|| format!("cannot read the settings {}", settings.display()))?;
        let run = job.dir.file_name().unwrap_or_default().to_string_lossy();
        let settings = settings.trim().replace("{run}", &run);
        let schema = job.dir.join("schema.json");
        fs::write(&schema, SCHEMA)?;
        let out = job.dir.join(format!("{}.out.json", job.step));
        let (trace, err) = files(job);
        let child = Command::new("codex")
            .args(["exec", "--json", "--ephemeral", "--ignore-user-config"])
            // With no proxy, a role with network settings gets the full internet.
            .args(["--enable", "network_proxy"])
            .args(["-c", "default_permissions=\"role\""])
            .args(["-c", &format!("permissions.role={settings}")])
            .arg("--output-schema")
            .arg(&schema)
            .arg("--output-last-message")
            .arg(&out)
            .arg(format!("{rules}\n\n{}", prompt(job)))
            .stdin(Stdio::null())
            .stdout(File::create(trace)?)
            .stderr(File::create(err)?)
            .spawn()
            .context("cannot start the codex command")?;
        wait(child, self.time_limit)?;
        let text = fs::read_to_string(&out).with_context(|| {
            format!(
                "codex gave no result; see the files of the step {}",
                job.step
            )
        })?;
        serde_json::from_str(&text).context("codex gave no result in the correct format")
    }
}
