# Codeumbrella

This file is the one source of the project rules for all agents. `CLAUDE.md` imports it. Do not copy its text into a second file.

## What this project is

Codeumbrella is a software factory. A person gives it a task. It gives back good code in `main`.

The heart is an instruction sheet (`graph.yaml`) and a small supervisor that reads the sheet and does what it says. The supervisor starts workers (Claude first, Codex after), keeps a notebook, and stops for the person.

The supervisor knows nothing about software work. All knowledge of the steps is in the sheet and in the rules pages.

## Words

| Word | Meaning |
|---|---|
| Sheet | `graph.yaml`: the steps and the rules between them. |
| Step | One piece of the work. A worker does it, a person does it, or the supervisor runs its commands. |
| Supervisor | The code in `src/graph.rs`, `src/engine.rs` and `src/main.rs`. |
| Tool part | The code that starts one type of worker. `src/claude.rs` is the part for Claude. `src/codex.rs` is the part for Codex. |
| Notebook | `.codeumbrella/runs/<run-id>/state.json`: where the run is. |
| Paper | A folder in `.codeumbrella/runs/<run-id>/papers/` that one step writes and other steps read. |
| Role | The type of a worker. Its rules page is `roles/<role>.md`. |

## Commands

```sh
cargo fmt --check                              # layout
cargo clippy --all-targets -- -D warnings      # usual errors, and a warning stops the code
cargo test                                     # tests, with a simulated worker
cargo build --locked                           # build

codeumbrella run "<task>"                      # start a run with Claude workers
codeumbrella --tool codex run "<task>"         # start a run with Codex workers
codeumbrella status [run-id]                   # show where a run is
codeumbrella answer <run-id> yes|no ["text"]   # answer the open question, then continue
codeumbrella resume <run-id>                   # try a stopped step again, with its count of tries at 0
```

Run all four `cargo` commands before you say that work is complete.

## Structure

| Path | Contents |
|---|---|
| `graph.yaml` | The first sheet: plan, look at tests, build, check, repair, open the PR, approval, merge. |
| `roles/<role>.md` | The rules page of each role, written by hand. |
| `roles/<role>.claude.json` | The Claude permissions and the sandbox settings of each role. |
| `src/graph.rs` | Reads the sheet and finds its errors before a run. |
| `src/engine.rs` | Does one step at a time, keeps the notebook, keeps the papers safe. |
| `src/main.rs` | The `codeumbrella` command. |
| `roles/<role>.codex.txt` | The Codex sandbox settings of each role. In this file, `{run}` is the id of the run. |
| `src/claude.rs` | Starts one `claude -p` worker. |
| `src/codex.rs` | Starts one `codex exec` worker. |
| `tests/engine.rs` | Tests of the supervisor with a simulated worker. |

## The sheet

A step can have only these lines: `role`, `type`, `run`, `reads`, `writes`, `next`, `on_bad`, `max_tries`, `on_limit`. The sheet also has one `start` line.

- A step with `type: person` stops the run until the person answers yes or no.
- A step with `type: command` has a `run` line: a list of commands. The supervisor runs them itself, with no shell and no worker. In a word, `{run}` is the id of the run. If a command gives an error, the result is bad.
- `next` is the step after a good result. `on_bad` is the step after a bad result.
- A step that an `on_bad` line goes to must have `max_tries`.
- A line that the supervisor does not know is an error.

## Safety rules

- The supervisor opens the PR. Then the run always stops for the approval of the person. The merge comes only after a yes.
- No worker can push, open a PR or merge. Only the command steps `open_pr` and `merge` can, with the set commands in the sheet.
- A paper can change only in a step that lists it in `writes`. The supervisor keeps a fingerprint of each paper. If a paper is different when a step reads it, the run stops.
- Each Claude role has a short list of permitted commands in `roles/<role>.claude.json`. The mode `dontAsk` refuses all other commands.
- The builder and the checker run in the Claude sandbox. They cannot write to `.codeumbrella`, `roles`, `graph.yaml`, `.claude`, `.github`, `.git/hooks` or `.git/config`. They cannot write to a folder that is not in the project.
- A deny rule such as `Edit(roles/**)` is what makes the sandbox lock a path. The tests of 15 November 2026 showed this. The setting `sandbox.filesystem.denyWrite` with a relative path did nothing in those tests.
- A deny rule without the sandbox is not sufficient. A script that a permitted command starts can go around it.
- Each Codex role runs in the Codex sandbox with the settings in `roles/<role>.codex.txt`. The locked paths are the same as for Claude. The planner can write only to its two papers. The Codex sandbox has no network, so a Codex worker cannot push.
- Codex has no list of permitted commands. The sandbox is its only wall.
- The permitted test commands are for a Rust project (`cargo`). A different project must change the lists of the builder and the checker.
- The command steps were not tested with a GitHub project.
- Each role has one rules page.

## Rust rules

| Topic | Rule |
|---|---|
| Version | Stable Rust, edition 2024. `rust-toolchain.toml` sets the version. |
| Layout | `rustfmt` with the default settings. |
| Usual errors | Clippy, with the group `pedantic` on. A warning stops the code. |
| Check settings | In the `[lints]` table of `Cargo.toml`. |
| `unsafe` | Not permitted. |
| Errors | Functions give back a `Result`. No `unwrap`, `expect` or `panic` in code that is not a test. Each error message tells what went bad and where. |
| Names and structure | The Rust API Guidelines: https://rust-lang.github.io/api-guidelines/checklist.html |
| Descriptions | A `///` description on each public item. |
| Tests | Each rule between steps has a test. Tests use a simulated worker, not Claude or Codex. |
| Async | No async package. Use the standard library to start workers. |
| Build | `Cargo.lock` is in version control. Builds use `--locked`. |

## Packages

A package is code from other persons. Each package is a risk, so the list is short.

| Package | What it does |
|---|---|
| `serde` | Changes file text into data, and back. |
| `serde_json` | Reads and writes the notebook. |
| `yaml_serde` | Reads the sheet. |
| `clap` | Reads the words after the command, and shows the help text. |
| `anyhow` | Makes clear error messages. |

A new package gets into this list only when all of these are correct:

1. `cargo deny check` and `cargo audit` find no problem with it.
2. Its makers changed it in the last 12 months.
3. This table gets one line that tells, in plain words, what the package does.
4. The message to the person tells why the code cannot do the work without it.

## Limits for lines of code

- The supervisor (`src/graph.rs`, `src/engine.rs`, `src/main.rs`, `src/lib.rs`): 400 lines of code.
- Each tool part: 100 lines of code.
- Empty lines, comment lines and tests do not count.
- Count with: `grep -v -E '^\s*(//|$)' <file> | wc -l`

## Rules for growth

1. Add a part only after a run shows that it is necessary. A part is a new type of step, a new line for the sheet, or a new package.
2. Obey the limits for lines of code.
3. Each time you add a part, look for a part to remove.
4. If a rule is too complex for the sheet, the worker makes that decision, not the supervisor.
5. Copy no part from a different project until a run shows that it is necessary.
6. One source for Claude and Codex. Only the tool parts and the `roles/<role>.<tool>.json` files are different for each tool.

## Not in version 1

- Pieces of a task that run at the same time.
- A different worker that writes the tests.
- A step that writes lessons after each run.
- Custom hooks.

## How to write for the person

The person who owns this project does not know Rust. Write messages and documents in short, plain sentences. Give the meaning of each technical word the first time that you use it.
