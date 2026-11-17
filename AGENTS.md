# Codeumbrella

This file is the one source of the project rules for all agents. `CLAUDE.md` imports it. Do not copy its text into a second file.

## What this project is

Codeumbrella is a software factory. A person gives it a task. It puts code into `main` only after two things. First, the code agrees with the tests that the person gave approval for. Second, the person gave approval for the PR.

The factory is written in Rust. The projects that go through it can be in each language. Put no knowledge of one language into the supervisor, the default sheet or the default rules pages.

The heart is an instruction sheet (`.codeumbrella/graph.yaml` in each project) and a small supervisor that reads the sheet and does what it says. The supervisor starts workers (Claude first, Codex after), keeps a notebook, and stops for the person.

The supervisor knows nothing about software work. All knowledge of the steps is in the sheet and in the rules pages.

## Words

| Word | Meaning |
|---|---|
| Sheet | `.codeumbrella/graph.yaml` in a project: the steps and the rules between them. `graph.example.yaml` is the default. |
| Step | One piece of the work. A worker does it, a person does it, or the supervisor runs its commands. |
| Fan | A step that starts two or more worker steps at the same time, and waits for all of them. |
| Supervisor | The code in `src/graph.rs`, `src/engine.rs`, `src/main.rs`, `src/log.rs` and `src/lib.rs`. |
| Tool part | The code that starts one type of worker. `src/claude.rs` is the part for Claude. `src/codex.rs` is the part for Codex. |
| Notebook | `.codeumbrella/runs/<run-id>/state.json`: where the run is. |
| Log | `log.jsonl` in the run folder: one line for each step that ended. |
| `trace` | `<step>.<number>.trace.jsonl` in the run folder: all that one worker did in one try. |
| Paper | A folder in `.codeumbrella/runs/<run-id>/papers/` that one step writes and other steps read. |
| Role | The type of a worker. Its rules page is `.codeumbrella/roles/<role>.md` in a project. The folder `roles` holds the defaults. |

## Commands

```sh
cargo fmt --check                              # layout
cargo clippy --all-targets -- -D warnings      # usual errors, and a warning stops the code
cargo test                                     # tests, with a simulated worker
cargo build --locked                           # build
cargo deny check && cargo audit                # look for bad packages, after a package change

codeumbrella init                              # put the default sheet and rules pages into a project
codeumbrella init --update                     # replace changed factory files with the new defaults, keep a copy of each as `.before`
codeumbrella run "<task>"                      # start a run with Claude workers
codeumbrella --tool codex run "<task>"         # start a run with Codex workers
codeumbrella status [run-id]                   # show where a run is
codeumbrella answer <run-id> <step> yes|no ["text"]   # answer the question of that step, then continue
codeumbrella resume <run-id>                   # try a stopped step again, with its count of tries at 0
```

Run all four `cargo` commands before you say that work is complete.

To install the command and the chat command on a computer, run `./install.sh`.

## Structure

| Path | Contents |
|---|---|
| `graph.example.yaml` | The default sheet, with a comment for each option. `init` copies it into a project. |
| `default.gitignore` | The default ignore file. `init` gives it to a project that has no `.gitignore`. |
| `roles/<role>.md` | The default rules page of each role, written by hand. |
| `roles/<role>.claude.json` | The Claude permissions and the sandbox settings of each role. In this file, `{project}` is the full path of the project folder and `{run}` is the id of the run. |
| `src/graph.rs` | Reads the sheet and finds its errors before a run. |
| `src/engine.rs` | Does one step at a time, keeps the notebook, keeps the papers safe. |
| `src/main.rs` | The `codeumbrella` command. |
| `src/log.rs` | Writes one line in the log of a run for each step. |
| `deny.toml` | The settings of the package check `cargo deny`. |
| `roles/<role>.codex.txt` | The Codex sandbox settings of each role. In this file, `{run}` is the id of the run and `{home}` is the home folder of the person. |
| `src/claude.rs` | Starts one `claude -p` worker. |
| `src/codex.rs` | Starts one `codex exec` worker. |
| `tests/engine.rs` | Tests of the supervisor with a simulated worker. |
| `skills/codeumbrella/SKILL.md` | The chat command `/codeumbrella`. Claude and Codex use the same file. |
| `ARCHITECTURE.md` | Two pictures: the parts of the factory and the path of a task. |
| `README.md` | The guide for a new person: what it is, what a computer must have, how to install, first run. |
| `.claude-plugin/` | The Claude Code plugin files: `plugin.json` and `marketplace.json`. |
| `.codex-plugin/` | The Codex plugin file: `plugin.json`. |
| `.agents/plugins/` | The Codex marketplace file `marketplace.json`. |
| `tests/packaging.rs` | Tests of the plugin files and the README. |
| `tests/walls.rs` | Tests of the safety settings and the safety checks. Each one becomes bad if a wall goes. |
| `install.sh` | Installs the command and the chat command. |
| `LICENSE` | The MIT licence. Do not change its text. |

## The sheet

The file `graph.example.yaml` tells what each line does. A step can have only these lines: `role`, `type`, `run`, `reads`, `writes`, `next`, `on_bad`, `max_tries`, `fan`. The sheet also has one `start` line.

- A step with `type: person` stops the run until the person answers yes or no.
- A step with `type: fan` has a `fan` line: a list of worker steps. The supervisor does them at the same time and waits for all of them. The result is good only if all of them are good.
- A step in a `fan` line has only `role` and `reads`. It writes no paper.
- A step with `type: command` has a `run` line: a list of commands. The supervisor runs them itself, with no shell and no worker. In a word, `{run}` is the id of the run. If a command gives an error, the result is bad.
- `next` is the step after a good result. `on_bad` is the step after a bad result.
- A step that an `on_bad` line goes to must have `max_tries`.
- A line that the supervisor does not know is an error.

## Safety rules

- The supervisor opens the PR. Then the run always stops for the approval of the person. The merge comes only after a yes. A no with a text sends the work back to the planner. The person then sees the tests again.
- No worker can push, open a PR or merge. Only the command steps `push`, `open_pr` and `merge` can, with the set commands in the sheet.
- The step `no_locked_files` stops the run if the work changes `.github`, `.claude` or `.codeumbrella`. A worker cannot write to these folders, but git can put such a file into a commit.
- That step reads a capital letter and a small letter as the same letter. A test on 16 November 2026 showed the cause. On a Mac, a commit with `.Claude/settings.json` got through the step and changed `.claude/settings.json`.
- The push does not send tags.
- The first step `copy` clones the project into `work` in the run folder. Each worker starts in that copy.
- A run does not change the project folder or its branch. After status Done, the person gets the code with `git pull`.
- A tool in the project folder, for example in `.venv`, needs its full path in the test command.
- A worker can write to the `.git` folder of its copy. Thus that step and the push do not use it. They use a clean copy in the run folder, which no worker can write to.
- The clean copy gets the code of GitHub from GitHub. It gets only the `branch` of the run from `../work`. The push sends the commit that the step compared.
- The push gives the full name of the run `branch` two times. Thus it cannot go to a different `branch`. Tests on 16 November 2026 showed these attacks with git.
- An answer names its step. An answer for a different step is an error. Thus a yes for the tests cannot become a yes for the merge.
- Only one command can work on a run at a time. The file `lock` in the run folder shows this. After a kill, a person removes that file.
- An answer and a resume get the lock before they read the notebook. Thus a second command cannot write an answer into a run that is active.
- The clean copy reads the `origin` settings from `.git/config` of the project. Thus the factory does not work in a linked git worktree.
- A worker that ends with an error code gives no good result.
- A worker does not get the values `GH_TOKEN` and `GITHUB_TOKEN` of the person.
- For a Claude worker, only the settings file of its role counts. The worker uses no settings, hooks or tool servers of the project or of the person.
- A live test on 16 November 2026 showed the cause. With the project settings, a hook of the project ran in a worker, with no sandbox.
- Thus Claude does not read `AGENTS.md` and `CLAUDE.md` of a project by itself. Each rules page tells the worker to read them.
- No worker can read `~/.ssh`, `~/.config/gh`, `~/.netrc`, `~/.git-credentials` and `~/Library/Keychains`. A Claude worker has no web tools.
- A live test on 16 November 2026 showed the cause. A worker in the sandbox got the GitHub key from the macOS key store. The lock on `~/Library/Keychains` stopped it, for Claude and for Codex.
- A role with no settings file is an error. A worker does not start with no settings.
- A paper can change only in a step that lists it in `writes`. The supervisor keeps a fingerprint of each paper. Before each step, the supervisor compares all the papers with their fingerprints. If a paper is different, the run stops. The person can change a paper while the run waits for an answer. The answer makes the fingerprints again.
- All factory files of a project are in the folder `.codeumbrella`: the sheet, the rules pages and the runs. The folder has its own `.gitignore`, so git does not see it.
- The Claude planner has no shell. The builder and the checker can run all commands, but only in the sandbox.
- The builder and the checker can connect to a server on the same computer, for example a test database. A test on 16 November 2026 showed that the internet and GitHub stay closed with this setting. A worker can thus talk to each server that runs on the computer of the person.
- The Codex part always turns on the Codex network proxy. Without the proxy, a Codex role that has network settings gets the full internet.
- Workers have no internet. The packages of a project must be on the computer before a run. A test on 16 November 2026 showed the cause. With internet, a worker can use the GitHub key of the person.
- The builder and the checker run in the Claude sandbox. They cannot write to `.codeumbrella`, `.claude`, `.github`, `.git/hooks` or `.git/config`. They cannot write to a folder that is not in the project.
- They also cannot write the files `.git/commondir` and `.git/config.worktree`. With such a file, a worker can give git its own settings. Then git runs a script of the worker with no sandbox.
- The steps with `gh` read the clean copy, not the `.git` folder of the project. The word `GIT_DIR` in the sheet does this.
- A deny rule such as `Edit(.codeumbrella/**)` is what makes the sandbox lock a path. The tests of 15 November 2026 showed this. The setting `sandbox.filesystem.denyWrite` with a relative path did nothing in those tests.
- A deny rule without the sandbox is not sufficient. A script that a permitted command starts can go around it.
- Each Codex role runs in the Codex sandbox with the settings in `roles/<role>.codex.txt`. The locked paths are the same as for Claude. The planner can write only to the two papers of its run. The Codex sandbox has no internet, so a Codex worker cannot push.
- The sandbox is the only wall for the commands of a worker, for Claude and for Codex.
- `init --update` copies each changed factory file to `<name>.before`, then writes the default. It does not touch `runs` or the `.gitignore` of the project folder.
- A paper that a step writes is not compared before that step. Thus a writer can run again after a try that broke its paper.
- After a stop for a changed paper, `resume` stops again. A person must put the paper back first.
- The plan step has 3 tries for one run. The first plan uses one. Thus 2 no answers are possible before a stop.
- Each role has one rules page.
- The default check step is a fan of two checkers. One runs the tests. One reads the change for safety problems, and it cannot write a file.
- The two checkers work in the same copy. Only the tests checker writes there.
- The safety checker does not read the commit that the tests checker makes at the same time. The person sees that commit in the PR.

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
| `time` | Writes the date and the time in the log. It comes with 5 small packages of the same makers. |

A new package gets into this list only when all of these are correct:

1. `cargo deny check` and `cargo audit` find no problem with it.
2. Its makers changed it in the last 12 months.
3. This table gets one line that tells, in plain words, what the package does.
4. The message to the person tells why the code cannot do the work without it.

## Limits for lines of code

- The supervisor (`src/graph.rs`, `src/engine.rs`, `src/main.rs`, `src/log.rs`, `src/lib.rs`): 560 lines of code.
- Each tool part: 100 lines of code.
- Empty lines, comment lines and tests do not count.
- Count with: `grep -v -E '^\s*(//|$)' <file> | wc -l`

## Output for a chat

An AI in a chat runs the command most of the time. Keep the output of the command short, so the chat keeps its context. The command prints one short line for each step and a note of 300 characters maximum. The full text goes into the log and the `trace` files.

## Rules for growth

1. Add a part only after a run shows that it is necessary. A part is a new type of step, a new line for the sheet, or a new package.
2. Obey the limits for lines of code.
3. Each time you add a part, look for a part to remove.
4. If a rule is too complex for the sheet, the worker makes that decision, not the supervisor.
5. Copy no part from a different project until a run shows that it is necessary.
6. One source for Claude and Codex. Only the tool parts and the settings files of the roles are different for each tool.

## Not in version 1

- Builders that work at the same time. The fan of today is only for workers that write no file.
- A different worker that writes the tests.
- A step that writes lessons after each run.
- Custom hooks.

## How to write for the person

The person who owns this project does not know Rust. Write messages and documents in short, plain sentences. Give the meaning of each technical word the first time that you use it.
