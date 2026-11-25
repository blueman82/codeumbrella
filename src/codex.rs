//! The tool part for Codex: it starts one `codex exec` worker for one step.

use crate::claude::{INIT, SCHEMA, files, prompt, wait};
use crate::engine::{Job, Numbers, Outcome, Worker};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::process::{Command, Stdio};
use std::{env, fs, fs::File, path::Path, time::Duration};

/// Starts Codex workers.
#[derive(Debug)]
pub struct Codex {
    /// The maximum time for one step.
    pub time_limit: Duration,
}

impl Worker for Codex {
    fn run(&self, job: &Job) -> Result<Outcome> {
        self.run_counted(job).0
    }

    fn run_counted(&self, job: &Job) -> (Result<Outcome>, Numbers) {
        let mut numbers = Numbers::default();
        let result = self.work(job, &mut numbers);
        (result, numbers)
    }
}

impl Codex {
    /// Does one step. It writes the numbers of the worker into `numbers`.
    fn work(&self, job: &Job, numbers: &mut Numbers) -> Result<Outcome> {
        let rules = fs::read_to_string(job.rules).with_context(|| {
            format!("cannot read the rules page {}. {INIT}", job.rules.display())
        })?;
        // The sandbox settings of the role. In the file, `{run}` is the run id,
        // `{project}` is the project folder and `{home}` is the home folder of the person.
        let settings = job.rules.with_extension("codex.txt");
        let settings = fs::read_to_string(&settings)
            .with_context(|| format!("cannot read the settings {}. {INIT}", settings.display()))?;
        let run = job.dir.file_name().unwrap_or_default().to_string_lossy();
        // With no correct home folder, the read locks point at no folder.
        let home = env::var("HOME").context("the value HOME is not set")?;
        let bad = |c| ['"', '\\', '{', '}', '\n'].contains(&c);
        ensure!(
            home.starts_with('/') && !home.contains(bad),
            "the value HOME is not a path"
        );
        let project = env::current_dir()?.to_string_lossy().into_owned();
        ensure!(
            !project.contains(bad),
            "the project path has a bad character"
        );
        let settings = settings.trim().replace("{run}", &run);
        let settings = settings.replace("{home}", &home);
        let settings = settings.replace("{project}", &project);
        let schema = job.dir.join("schema.json");
        fs::write(&schema, SCHEMA)?;
        let out = job.dir.join(format!("{}.out.json", job.step));
        // A new try must write its own result.
        let _ = fs::remove_file(&out);
        let (trace, err) = files(job);
        let child = Command::new("codex")
            .args(["exec", "--json", "--ephemeral", "--ignore-user-config"])
            // With no proxy, a role with network settings gets the full internet.
            .args(["--enable", "network_proxy"])
            .args(["-c", "default_permissions=\"role\""])
            .args(["-c", &format!("permissions.role={settings}")])
            .args(job.model.iter().flat_map(|m| ["-m", m]))
            .args(
                job.effort
                    .map(|e| format!("model_reasoning_effort=\"{e}\""))
                    .iter()
                    .flat_map(|e| ["-c", e]),
            )
            .arg("--output-schema")
            .arg(&schema)
            .arg("--output-last-message")
            .arg(&out)
            .arg(format!("{rules}\n\n{}", prompt(job)))
            // The worker does not get the GitHub token of the person.
            .env_remove("GH_TOKEN")
            .env_remove("GITHUB_TOKEN")
            .current_dir(job.dir.join("work"))
            .stdin(Stdio::null())
            .stdout(File::create(&trace)?)
            .stderr(File::create(err)?)
            .spawn()
            .context("cannot start the codex command")?;
        let (status, seconds) = wait(child, self.time_limit, job.dir)?;
        numbers.seconds = Some(seconds);
        tokens(&trace, numbers);
        ensure!(status.success(), "codex ended with an error: {status}");
        let text = fs::read_to_string(&out).with_context(|| {
            format!(
                "codex gave no result; see the files of the step {}",
                job.step
            )
        })?;
        serde_json::from_str(&text).context("codex gave no result in the correct format")
    }
}

/// Reads the trace file and counts its tokens.
fn tokens(trace: &Path, numbers: &mut Numbers) {
    count_tokens(&fs::read_to_string(trace).unwrap_or_default(), numbers);
}

/// Adds the tokens of each `turn.completed` line of the trace into `numbers`.
/// The cached tokens and the reasoning tokens are parts of these two numbers.
/// A trace with no such line gives no token numbers. A sum that is too large
/// stays at the maximum.
pub fn count_tokens(text: &str, numbers: &mut Numbers) {
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if event["type"] != "turn.completed" {
            continue;
        }
        let count = |key: &str| event["usage"][key].as_u64().unwrap_or_default();
        let add = |sum: &mut Option<u64>, key| {
            *sum = Some(sum.unwrap_or_default().saturating_add(count(key)));
        };
        add(&mut numbers.tokens_in, "input_tokens");
        add(&mut numbers.tokens_out, "output_tokens");
    }
}
