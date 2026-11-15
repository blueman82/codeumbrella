//! The instruction sheet: the steps of the factory and the rules between them.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
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
}

/// What the supervisor does when a step reaches `max_tries`.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnLimit {
    /// Stop the run and tell the person.
    StopForPerson,
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
    /// What to do at the maximum.
    pub on_limit: Option<OnLimit>,
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
    /// rules page, a command step with no commands, a way back with no maximum, or `next` lines with no end.
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
                (Kind::Person | Kind::Command, Some(_)) => {
                    bail!("step {name}: only a worker step has a role")
                }
                _ => {}
            }
            if (step.kind == Kind::Command) == step.run.is_empty() {
                bail!("step {name}: a command step, and only a command step, has run lines");
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
