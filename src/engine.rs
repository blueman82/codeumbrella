//! The supervisor: it does the sheet one step at a time and keeps the notebook.

use crate::graph::{Graph, Kind, Step};
use crate::log;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::{fs, path::Path, path::PathBuf, process::Command, time::Instant};

/// What one step gives back.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Outcome {
    /// `true` if the step worked. For a check: the work agrees with the tests.
    pub ok: bool,
    /// A short text about the result.
    pub summary: String,
}

/// The work for one worker.
#[derive(Debug)]
pub struct Job<'a> {
    /// The task of the run.
    pub task: &'a str,
    /// The name of the step.
    pub step: &'a str,
    /// The rules page of the role.
    pub rules: &'a Path,
    /// The folder of the run. The papers are in `papers/` in this folder.
    pub dir: &'a Path,
    /// The papers that the worker reads.
    pub reads: &'a [String],
    /// The papers that the worker writes.
    pub writes: &'a [String],
    /// The result of the step before this one.
    pub before: Option<&'a Outcome>,
}

/// A tool that can do a worker step, such as Claude or Codex.
pub trait Worker {
    /// Does one step and gives back its result.
    ///
    /// # Errors
    /// Gives an error if the tool does not start, does not end in time, or
    /// gives no result.
    fn run(&self, job: &Job) -> Result<Outcome>;
}

/// Where a run is.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Status {
    /// The supervisor does steps.
    #[default]
    Running,
    /// The supervisor waits for an answer from the person.
    Waiting,
    /// All steps are complete.
    Done,
    /// A problem stopped the run. `resume` tries the same step again.
    Stopped,
}

/// The notebook of one run. The supervisor saves it after each step.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    /// The task of the run.
    pub task: String,
    /// Where the run is.
    pub status: Status,
    /// The step to do next. `None` when the run is complete.
    pub at: Option<String>,
    /// The question for the person, or the cause of a stop.
    pub note: String,
    /// The answer of the person, until a person step uses it.
    pub answer: Option<Outcome>,
    /// How many times the run did each step that has a maximum.
    pub tries: BTreeMap<String, u32>,
    /// The last result of each step.
    pub results: BTreeMap<String, Outcome>,
    /// The fingerprint of each paper when its writer ended.
    pub prints: BTreeMap<String, u64>,
    /// The result of the last step that ended.
    pub last: Option<Outcome>,
}

/// One run: its folder and its notebook.
#[derive(Debug)]
pub struct Run {
    /// The folder of the run.
    pub dir: PathBuf,
    /// The notebook.
    pub state: State,
}

impl Run {
    /// Starts a new run in the folder `dir`.
    ///
    /// # Errors
    /// Gives an error if the folder or the notebook cannot be written.
    pub fn start(dir: PathBuf, task: &str, graph: &Graph) -> Result<Self> {
        fs::create_dir_all(dir.join("papers"))?;
        let mut run = Self {
            dir,
            state: State::default(),
        };
        task.clone_into(&mut run.state.task);
        run.state.at = Some(graph.start.clone());
        run.save()?;
        Ok(run)
    }

    /// Opens a run from its folder.
    ///
    /// # Errors
    /// Gives an error if the notebook is not there or is not readable.
    pub fn open(dir: PathBuf) -> Result<Self> {
        let path = dir.join("state.json");
        let text = fs::read_to_string(&path)
            .with_context(|| format!("there is no notebook at {}", path.display()))?;
        let state = serde_json::from_str(&text).context("the notebook has an error")?;
        Ok(Self { dir, state })
    }

    /// Writes the notebook.
    ///
    /// # Errors
    /// Gives an error if the file cannot be written.
    pub fn save(&self) -> Result<()> {
        let text = serde_json::to_string_pretty(&self.state)?;
        fs::write(self.dir.join("state.json"), text).context("cannot write the notebook")
    }

    /// Records the answer of the person to the open question, and makes the
    /// fingerprints of the papers again.
    ///
    /// # Errors
    /// Gives an error if the run does not wait for an answer, or if a paper
    /// is not readable.
    pub fn answer(&mut self, ok: bool, text: &str) -> Result<()> {
        if self.state.status != Status::Waiting {
            bail!("this run has no open question");
        }
        // The person answers for the papers as they are at this time.
        let papers = self.dir.join("papers");
        for (paper, print) in &mut self.state.prints {
            *print = fingerprint(&papers.join(paper))?;
        }
        let summary = format!("The person said {}. {text}", if ok { "yes" } else { "no" });
        self.state.answer = Some(Outcome { ok, summary });
        self.save()
    }

    /// Does steps until the run ends, stops, or waits for the person.
    ///
    /// # Errors
    /// Gives an error only if the notebook cannot be written. A problem in a
    /// step stops the run and goes into the notebook.
    pub fn go(&mut self, graph: &Graph, roles: &Path, worker: &dyn Worker) -> Result<()> {
        self.state.status = Status::Running;
        while let Some(name) = self.state.at.clone() {
            let step = graph.step(&name)?;
            let start = Instant::now();
            let outcome = match self.step(&name, step, roles, worker) {
                Ok(Some(outcome)) => outcome,
                Ok(None) => return self.save(),
                Err(error) => {
                    let note = format!("Step {name}: {error:#}");
                    log::record(&self.dir, &name, false, &note, start)?;
                    return self.halt(Status::Stopped, note);
                }
            };
            log::record(&self.dir, &name, outcome.ok, &outcome.summary, start)?;
            let target = if outcome.ok { &step.next } else { &step.on_bad };
            if !outcome.ok && target.is_none() {
                let note = format!("Step {name} gave a bad result: {}", outcome.summary);
                return self.halt(Status::Stopped, note);
            }
            self.state.at.clone_from(target);
            self.state.last = Some(outcome.clone());
            self.state.results.insert(name, outcome);
            self.save()?;
        }
        self.halt(Status::Done, "All steps are complete.".to_owned())
    }

    /// Does one step. `None` means that the run waits or stopped.
    fn step(
        &mut self,
        name: &str,
        step: &Step,
        roles: &Path,
        worker: &dyn Worker,
    ) -> Result<Option<Outcome>> {
        let before = self.state.last.clone();
        // Before each step, also a step for the person: no paper changed.
        // A paper that this step writes is not compared: the step writes it again.
        let papers = step.reads.iter().chain(self.state.prints.keys());
        for paper in papers.filter(|p| !step.writes.contains(p)) {
            if self.state.prints.get(paper) != Some(&fingerprint(&self.paper(paper))?) {
                bail!("the paper {paper} is not as its writer left it");
            }
        }
        if step.kind == Kind::Person {
            let answer = self.state.answer.take();
            if answer.is_none() {
                let last = before.map_or_else(String::new, |o| o.summary);
                let papers = self.paper("").display().to_string();
                let note = format!(
                    "Step {name} is for you. Look at {papers} and at the result before. Answer yes or no. Result before: {last}"
                );
                self.halt(Status::Waiting, note)?;
            }
            return Ok(answer);
        }
        if let Some(max) = step.max_tries {
            let tries = self.state.tries.entry(name.to_owned()).or_default();
            if *tries >= max {
                let note = format!("Step {name} reached its maximum of {max} tries.");
                return self.halt(Status::Stopped, note).map(|()| None);
            }
            *tries += 1;
        }
        if step.kind == Kind::Command {
            return self.commands(&step.run).map(Some);
        }
        for paper in &step.writes {
            fs::create_dir_all(self.paper(paper))?;
        }
        let role = step.role.as_deref().unwrap_or_default();
        let outcome = worker.run(&Job {
            task: &self.state.task,
            step: name,
            rules: &roles.join(format!("{role}.md")),
            dir: &self.dir,
            reads: &step.reads,
            writes: &step.writes,
            before: before.as_ref(),
        })?;
        for paper in &step.writes {
            let print = fingerprint(&self.paper(paper))?;
            self.state.prints.insert(paper.clone(), print);
        }
        Ok(Some(outcome))
    }

    /// Runs the commands of a command step, with no shell. It stops at the
    /// first command that gives an error. `{run}` in a word is the run id.
    // ponytail: a command has no time limit. Add one if a command hangs.
    fn commands(&self, run: &[Vec<String>]) -> Result<Outcome> {
        let id = self.dir.file_name().unwrap_or_default().to_string_lossy();
        let mut summary = String::new();
        for words in run {
            let words: Vec<String> = words.iter().map(|w| w.replace("{run}", &id)).collect();
            let (first, others) = words.split_first().context("a command has no words")?;
            let out = Command::new(first)
                .args(others)
                .output()
                .with_context(|| format!("cannot start the command {first}"))?;
            if !out.status.success() {
                let error = String::from_utf8_lossy(&out.stderr);
                let summary = format!("The command `{}` gave an error: {error}", words.join(" "));
                return Ok(Outcome { ok: false, summary });
            }
            String::from_utf8_lossy(&out.stdout)
                .trim()
                .clone_into(&mut summary);
        }
        Ok(Outcome { ok: true, summary })
    }

    /// The folder of one paper.
    fn paper(&self, name: &str) -> PathBuf {
        self.dir.join("papers").join(name)
    }

    /// Sets the status and the note, and writes the notebook.
    fn halt(&mut self, status: Status, note: String) -> Result<()> {
        self.state.status = status;
        self.state.note = note;
        self.save()
    }
}

/// Makes one number from all the files in a paper folder.
// ponytail: not a cryptographic hash, and it can change with the Rust version.
// Use the `sha2` package if a worker tries to make a false paper on purpose.
fn fingerprint(dir: &Path) -> Result<u64> {
    let (mut folders, mut files) = (vec![dir.to_path_buf()], Vec::new());
    while let Some(folder) = folders.pop() {
        let entries = fs::read_dir(&folder)
            .with_context(|| format!("there is no paper folder {}", folder.display()))?;
        for entry in entries {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut hasher = DefaultHasher::new();
    for file in files {
        file.strip_prefix(dir)?.hash(&mut hasher);
        fs::read(&file)?.hash(&mut hasher);
    }
    Ok(hasher.finish())
}
