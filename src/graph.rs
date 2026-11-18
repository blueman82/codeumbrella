//! The instruction sheet: the steps of the factory and the rules between them.

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

/// Who does a step.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A worker does the step.
    #[default]
    Worker,
    /// A person does the step: the supervisor stops and waits for an answer.
    Person,
    /// The supervisor runs the set commands of the step.
    Command,
    /// The supervisor does the steps in the `fan` line at the same time.
    Fan,
}

/// The tool that does a worker step.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    /// The `claude` command.
    Claude,
    /// The `codex` command.
    Codex,
}

/// One step of the work.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// The role of the worker. Its rules page is `roles/<role>.md`.
    pub role: Option<String>,
    /// Who does the step.
    #[serde(rename = "type", default)]
    pub kind: Kind,
    /// The commands of a command step. Each command is a list of words.
    #[serde(default)]
    pub run: Vec<Vec<String>>,
    /// The steps of a fan step. They run at the same time.
    #[serde(default)]
    pub fan: Vec<String>,
    /// The papers that the step reads.
    #[serde(default)]
    pub reads: Vec<String>,
    /// The papers that the step writes.
    #[serde(default)]
    pub writes: Vec<String>,
    /// The step after a good result. Without it, the run ends here.
    pub next: Option<String>,
    /// The step after a bad result. Without it, a bad result stops the run.
    pub on_bad: Option<String>,
    /// The maximum number of times that the run can do this step.
    pub max_tries: Option<u32>,
    /// The tool of a worker step. Without it, the tool of the command.
    pub tool: Option<Tool>,
    /// The model of the tool. Without it, the tool uses its default.
    pub model: Option<String>,
    /// How much the AI thinks. The tool knows the values.
    pub effort: Option<String>,
}

/// The full instruction sheet.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    /// The name of the first step.
    pub start: String,
    /// All the steps, by name.
    pub steps: BTreeMap<String, Step>,
}

impl Graph {
    /// Reads a sheet and checks it. `roles` is the folder with the rules pages.
    ///
    /// # Errors
    /// Gives an error if the file is not readable, is not a correct sheet, or
    /// does not obey the rules in [`Graph::check`].
    pub fn load(path: &Path, roles: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("cannot read the sheet {}", path.display()))?;
        let graph: Self = yaml_serde::from_str(&text)
            .with_context(|| format!("the sheet {} has an error", path.display()))?;
        graph.check(roles)?;
        Ok(graph)
    }

    /// Finds errors in the sheet before the work starts.
    ///
    /// # Errors
    /// Gives an error for a step that does not exist, a worker step with no
    /// rules page, a command step with no commands, a way back with no maximum,
    /// a maximum of 0, a name that is a path, a fan part that is not a plain
    /// worker step, a bad `model` or `effort`, `tool`, `model` or `effort`
    /// on a step that is not a worker step, or `next` lines with no end.
    pub fn check(&self, roles: &Path) -> Result<()> {
        self.step(&self.start).context("the start line")?;
        for (name, step) in &self.steps {
            for target in [&step.next, &step.on_bad].into_iter().flatten() {
                self.step(target).with_context(|| format!("step {name}"))?;
            }
            if let Some(bad) = &step.on_bad
                && self.step(bad)?.max_tries.is_none()
            {
                bail!("step {name}: on_bad goes to {bad}, which has no max_tries");
            }
            match (step.kind, &step.role) {
                (Kind::Worker, Some(role)) if !roles.join(format!("{role}.md")).is_file() => {
                    bail!("step {name}: there is no rules page for the role {role}")
                }
                (Kind::Worker, None) => bail!("step {name}: a worker step must have a role"),
                (Kind::Person | Kind::Command | Kind::Fan, Some(_)) => {
                    bail!("step {name}: only a worker step has a role")
                }
                _ => {}
            }
            let own = [&step.model, &step.effort];
            let safe = |c: char| c.is_ascii_alphanumeric() || ".-_".contains(c);
            let good = |v: &String| !v.is_empty() && !v.starts_with('-') && v.chars().all(safe);
            let fine = own.into_iter().flatten().all(good);
            let set = step.tool.is_some() || own.iter().any(|o| o.is_some());
            let wrong = set && step.kind != Kind::Worker;
            ensure!(
                fine && !wrong,
                "step {name}: a bad tool, model or effort line"
            );
            if (step.kind == Kind::Command) == step.run.is_empty() {
                bail!("step {name}: a command step, and only a command step, has run lines");
            }
            ensure!(
                (step.kind == Kind::Fan) != step.fan.is_empty(),
                "step {name}: a fan step, and only a fan step, has a fan line"
            );
            // A part of a fan only gives a result. The fan step has the paths.
            for part in &step.fan {
                let s = self.step(part).with_context(|| format!("step {name}"))?;
                let plain = s.kind == Kind::Worker && s.writes.is_empty() && s.max_tries.is_none();
                ensure!(
                    plain && s.next.is_none() && s.on_bad.is_none(),
                    "step {name}: the fan part {part} must be a worker step with no writes, next, on_bad or max_tries"
                );
            }
            if step.max_tries == Some(0) || step.run.iter().any(Vec::is_empty) {
                bail!("step {name}: max_tries must be 1 or more, and a command must have words");
            }
            // A paper or a role is one folder or file name, not a path.
            let mut names = step.reads.iter().chain(&step.writes).chain(&step.role);
            if let Some(bad) = names.find(|n| n.contains(['/', '\\']) || n.starts_with('.')) {
                bail!("step {name}: the name {bad} must not have a `/` or start with a `.`");
            }
            let (mut at, mut hops) = (Some(name), 0);
            while let Some(here) = at {
                hops += 1;
                if hops > self.steps.len() {
                    bail!("step {name}: the next lines go in a circle");
                }
                at = self.step(here)?.next.as_ref();
            }
        }
        Ok(())
    }

    /// Finds one step by its name.
    ///
    /// # Errors
    /// Gives an error if the sheet has no step with this name.
    pub fn step(&self, name: &str) -> Result<&Step> {
        self.steps
            .get(name)
            .with_context(|| format!("there is no step with the name {name}"))
    }
}
