//! The supervisor: it does the sheet one step at a time and keeps the notebook.

use crate::graph::{Graph, Kind, Step, Tool};
use crate::log;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::{
    fmt::Write as _, fs, path::Path, path::PathBuf, process::Command, thread, time::Instant,
};

/// The stop note for a worker folder that is empty or not there.
const EMPTY: &str = "the work folder is empty or not there. The sheet is from before version 0.1.2. Run `codeumbrella init --update` and start a new run";

/// For each paper, the fingerprint of each of its files.
type Seen = BTreeMap<String, BTreeMap<String, u64>>;

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
    /// The tool of the step. `None` means the tool of the run.
    pub tool: Option<Tool>,
    /// The model of the step. `None` means the default of the tool.
    pub model: Option<&'a str>,
    /// The effort of the step. `None` means the default of the tool.
    pub effort: Option<&'a str>,
    /// The result of the step before this one.
    pub before: Option<&'a Outcome>,
    /// One line for each paper that changed since the last run of this step.
    pub changed: &'a [String],
}

/// A tool that can do a worker step, such as Claude or Codex.
pub trait Worker: Sync {
    /// Does one step and gives back its result.
    ///
    /// # Errors
    /// Gives an error if the tool does not start, does not end in time, or
    /// gives no result.
    fn run(&self, job: &Job) -> Result<Outcome>;

    /// Does one step, and gives its result and the numbers of its worker.
    /// The default gives no numbers.
    fn run_counted(&self, job: &Job) -> (Result<Outcome>, Numbers) {
        (self.run(job), Numbers::default())
    }
}

/// The numbers of one worker. A number is there only if the tool gives it.
#[derive(Debug, Default, Clone)]
pub struct Numbers {
    /// The time of the worker, from the start of its command to its end.
    pub seconds: Option<f64>,
    /// The cost that the tool wrote.
    pub dollars: Option<f64>,
    /// The number of turns of the worker.
    pub turns: Option<u64>,
    /// The model of the worker.
    pub model: Option<String>,
    /// The tokens that went into the model.
    pub tokens_in: Option<u64>,
    /// The tokens that came out of the model.
    pub tokens_out: Option<u64>,
}

/// The total of the numbers of the workers of a run.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Total {
    /// The count of the workers that gave their time. Each try counts.
    pub workers: u64,
    /// The sum of the worker times.
    pub seconds: f64,
    /// The sum of the costs.
    pub dollars: f64,
    /// The sum of the tokens into the model.
    pub tokens_in: u64,
    /// The sum of the tokens out of the model.
    pub tokens_out: u64,
    /// The count of the workers that gave their time but no cost.
    pub no_cost: u64,
}

impl Total {
    /// Adds the numbers of one worker.
    pub fn add(&mut self, n: &Numbers) {
        if let Some(seconds) = n.seconds {
            self.workers += 1;
            self.seconds += seconds;
            self.no_cost += u64::from(n.dollars.is_none());
        }
        self.dollars += n.dollars.unwrap_or_default();
        self.tokens_in = self.tokens_in.saturating_add(n.tokens_in.unwrap_or(0));
        self.tokens_out = self.tokens_out.saturating_add(n.tokens_out.unwrap_or(0));
    }
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
    /// The tool of the run. An older notebook has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<Tool>,
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
    /// For each step, the files of the papers when its worker last gave a
    /// result. An older notebook has none.
    #[serde(default)]
    pub seen: BTreeMap<String, Seen>,
    /// The result of the last step that ended.
    pub last: Option<Outcome>,
    /// The start time of the step of this moment. An older notebook has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// The total of the numbers of the workers. An older notebook has none.
    #[serde(default)]
    pub total: Total,
}

/// The lock of one run: the file `lock` in the run folder. Only one command
/// can work on a run at a time. The lock goes when the command ends.
#[derive(Debug)]
pub struct Lock(PathBuf);

impl Lock {
    /// Takes the lock of the run in the folder `dir`.
    ///
    /// # Errors
    /// Gives an error if a different command has the lock, or if there is no
    /// such run folder.
    pub fn take(dir: &Path) -> Result<Self> {
        let path = dir.join("lock");
        let mut file = fs::File::create_new(&path).with_context(|| {
            let path = path.display();
            format!("a different command works on this run. If not, remove the file {path}")
        })?;
        // `status --wait` reads this number to see if the command lives.
        std::io::Write::write_all(&mut file, std::process::id().to_string().as_bytes())?;
        Ok(Self(path))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// One run: its folder and its notebook.
#[derive(Debug)]
pub struct Run {
    /// The folder of the run.
    pub dir: PathBuf,
    /// The notebook.
    pub state: State,
    /// The numbers of the worker of the last worker step, for its log line.
    numbers: Option<Numbers>,
}

impl Run {
    /// Starts a new run in the folder `dir`.
    ///
    /// # Errors
    /// Gives an error if the folder or the notebook cannot be written.
    pub fn start(dir: PathBuf, task: &str, graph: &Graph) -> Result<Self> {
        fs::create_dir_all(dir.join("papers"))?;
        fs::create_dir_all(dir.join("work"))?;
        let mut run = Self {
            dir,
            state: State::default(),
            numbers: None,
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
        Ok(Self {
            dir,
            state,
            numbers: None,
        })
    }

    /// Writes the notebook.
    ///
    /// # Errors
    /// Gives an error if the file cannot be written.
    pub fn save(&self) -> Result<()> {
        let text = serde_json::to_string_pretty(&self.state)?;
        // First a second file, then a rename. Thus a kill cannot break the notebook.
        let tmp = self.dir.join("state.json.tmp");
        fs::write(&tmp, text).context("cannot write the notebook")?;
        fs::rename(tmp, self.dir.join("state.json")).context("cannot write the notebook")
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
        // The notebook shows `Running` from the start.
        self.halt(Status::Running, String::new())?;
        while let Some(name) = self.state.at.clone() {
            let step = graph.step(&name)?;
            let start = Instant::now();
            self.state.since = Some(log::now()?);
            self.save()?;
            let result = self.step(&name, step, graph, roles, worker);
            let numbers = self.numbers.take();
            let outcome = match result {
                Ok(Some(outcome)) => outcome,
                Ok(None) => return self.save(),
                Err(error) => {
                    let note = format!("Step {name}: {error:#}");
                    log::record(&self.dir, &name, false, &note, start, numbers.as_ref())?;
                    return self.halt(Status::Stopped, note);
                }
            };
            let summary = &outcome.summary;
            log::record(
                &self.dir,
                &name,
                outcome.ok,
                summary,
                start,
                numbers.as_ref(),
            )?;
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
        graph: &Graph,
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
        if step.kind == Kind::Fan {
            return self
                .fan(step, graph, roles, worker, before.as_ref())
                .map(Some);
        }
        for paper in &step.writes {
            fs::create_dir_all(self.paper(paper))?;
        }
        let (outcome, numbers) = self.work(name, step, roles, worker, before.as_ref());
        self.state.total.add(&numbers);
        self.numbers = Some(numbers);
        let outcome = outcome?;
        for paper in &step.writes {
            let print = fingerprint(&self.paper(paper))?;
            self.state.prints.insert(paper.clone(), print);
        }
        self.state.seen.insert(name.to_owned(), self.snapshot());
        Ok(Some(outcome))
    }

    /// Gives one worker step to a worker.
    fn work(
        &self,
        name: &str,
        step: &Step,
        roles: &Path,
        worker: &dyn Worker,
        before: Option<&Outcome>,
    ) -> (Result<Outcome>, Numbers) {
        let role = step.role.as_deref().unwrap_or_default();
        let filled = fs::read_dir(self.dir.join("work")).is_ok_and(|mut d| d.next().is_some());
        if !filled {
            return (Err(anyhow!(EMPTY)), Numbers::default());
        }
        worker.run_counted(&Job {
            task: &self.state.task,
            step: name,
            rules: &roles.join(format!("{role}.md")),
            dir: &self.dir,
            reads: &step.reads,
            writes: &step.writes,
            tool: step.tool,
            model: step.model.as_deref(),
            effort: step.effort.as_deref(),
            before,
            changed: &self.changed(name),
        })
    }

    /// The files of each paper that has a fingerprint, as they are at this moment.
    // A paper that is not readable is not there. The next step stops for it.
    fn snapshot(&self) -> Seen {
        let papers = self.state.prints.keys();
        let read = |p: &String| Some((p.clone(), file_prints(&self.paper(p)).ok()?));
        papers.filter_map(read).collect()
    }

    /// One line for each paper with a new, changed or removed file since the
    /// last run of the step `name`. A first run gets no line.
    fn changed(&self, name: &str) -> Vec<String> {
        let Some(old) = self.state.seen.get(name) else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        for (paper, now) in self.snapshot() {
            let before = old.get(&paper).cloned().unwrap_or_default();
            let all: BTreeSet<&String> = before.keys().chain(now.keys()).collect();
            let differ = all.into_iter().filter(|f| before.get(*f) != now.get(*f));
            let files: Vec<String> = differ.map(|f| format!("`{f}`")).collect();
            if !files.is_empty() {
                let files = files.join(", ");
                lines.push(format!("the paper `{paper}` changed: {files}"));
            }
        }
        lines
    }

    /// Does the parts of a fan step at the same time, and waits for all of
    /// them. The result is good only if all the parts are good. The summary
    /// has the result of each part.
    // ponytail: the log gives each part the time of the full fan.
    fn fan(
        &mut self,
        step: &Step,
        graph: &Graph,
        roles: &Path,
        worker: &dyn Worker,
        before: Option<&Outcome>,
    ) -> Result<Outcome> {
        let start = Instant::now();
        let results: Vec<(Result<Outcome>, Numbers)> = thread::scope(|scope| {
            let part = |name| match graph.step(name) {
                Ok(s) => self.work(name, s, roles, worker, before),
                Err(e) => (Err(e), Numbers::default()),
            };
            let fan = step.fan.iter();
            let parts: Vec<_> = fan.map(|n| scope.spawn(move || part(n))).collect();
            let broke = || (Err(anyhow!("it broke")), Numbers::default());
            let end = |p: thread::ScopedJoinHandle<_>| p.join().unwrap_or_else(|_| broke());
            parts.into_iter().map(end).collect()
        });
        // All the parts ended. Their numbers go into the total in one thread.
        for (_, numbers) in &results {
            self.state.total.add(numbers);
        }
        let (mut ok, mut summary) = (true, String::new());
        for (name, (result, numbers)) in step.fan.iter().zip(results) {
            let outcome = result.with_context(|| format!("the fan part {name}"))?;
            let text = &outcome.summary;
            log::record(&self.dir, name, outcome.ok, text, start, Some(&numbers))?;
            self.state.seen.insert(name.clone(), self.snapshot());
            ok &= outcome.ok;
            let word = if outcome.ok { "good" } else { "bad" };
            let _ = writeln!(summary, "{name} ({word}): {}", outcome.summary);
        }
        Ok(Outcome { ok, summary })
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
    let mut hasher = DefaultHasher::new();
    for file in files(dir)? {
        file.strip_prefix(dir)?.hash(&mut hasher);
        fs::read(&file)?.hash(&mut hasher);
    }
    Ok(hasher.finish())
}

/// Makes one number for each file in a paper folder.
fn file_prints(dir: &Path) -> Result<BTreeMap<String, u64>> {
    let mut prints = BTreeMap::new();
    for file in files(dir)? {
        let mut hasher = DefaultHasher::new();
        fs::read(&file)?.hash(&mut hasher);
        let name = file.strip_prefix(dir)?.to_string_lossy().into_owned();
        prints.insert(name, hasher.finish());
    }
    Ok(prints)
}

/// All the files in a paper folder, at each depth, in sequence.
fn files(dir: &Path) -> Result<Vec<PathBuf>> {
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
    Ok(files)
}
