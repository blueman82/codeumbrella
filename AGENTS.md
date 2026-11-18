# Codeumbrella

This file is the one source of the project rules for all agents. `CLAUDE.md` imports it. Do not copy its text into a second file.

## What this project is

Codeumbrella is a software factory. A person gives it a task. It puts code into `main` only after two things. First, the code agrees with the tests that the person gave approval for. Second, the person gave approval for the PR.

The factory is written in Rust. The projects that go through it can be in each language. Knowledge of one language is only in the language pages (`roles/languages/`). The supervisor, the default sheet and the rules pages of the roles have none. Only the six page file names in the file list of `init` are permitted.

The heart is an instruction sheet (`.codeumbrella/graph.yaml` in each project) and a small supervisor that reads the sheet and does what it says. The supervisor starts workers, keeps a notebook, and stops for the person. Claude came first and Codex after, and a step can pick its tool.

The supervisor knows nothing about software work. All knowledge of the steps is in the sheet and in the rules pages.

## Words

| Word | Meaning |
|---|---|
| Sheet | `.codeumbrella/graph.yaml` in a project: the steps and the rules between them. `graph.example.yaml` is the default. |
| Step | One piece of the work. A worker does it, a person does it, or the supervisor runs its commands. |
| Fan | A step that starts two or more worker steps at the same time, and waits for all of them. |
| Supervisor | The code in `src/graph.rs`, `src/engine.rs`, `src/main.rs`, `src/log.rs` and `src/lib.rs`. |
| Tool part | The code that starts one type of worker. `src/claude.rs` is the part for Claude. `src/codex.rs` is the part for Codex. |
| Notebook | `.codeumbrella/runs/<run-id>/state.json`: where the run is, and the tool of the run. |
| Tool of the run | The tool that `run` got from `--tool` (default Claude). The notebook keeps it. `answer` and `resume` use it. |
| Log | `log.jsonl` in the run folder: one line for each step that ended. |
| `trace` | `<step>.<number>.trace.jsonl` in the run folder: all that one worker did in one try. |
| Paper | A folder in `.codeumbrella/runs/<run-id>/papers/` that one step writes and other steps read. |
| `worker.<number>` | A file in the run folder while a worker lives. It holds the start time of the worker. The repair uses it to end a worker that stays. |
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
codeumbrella --tool codex run "<task>"         # start a run with Codex workers. The run keeps this tool
codeumbrella status [run-id]                   # show where a run is. It changes a file only to repair a run after a kill
codeumbrella status --wait <run-id>            # wait until the run is not `Running`, then show it. It changes them only to repair a run after a kill
codeumbrella answer <run-id> <step> yes|no ["text"]   # answer the question of that step, then continue
codeumbrella stop <run-id>                     # end an active run: its command and its workers. Only after the person says stop
codeumbrella resume <run-id>                   # try a stopped step again, with its count of tries at 0. Only after the person says yes
```

Run all four `cargo` commands before you say that work is complete.

To install the command and the chat command on a computer, run `./install.sh`.

## Structure

| Path | Contents |
|---|---|
| `graph.example.yaml` | The default sheet, with a comment for each option. `init` copies it into a project. |
| `default.gitignore` | The default ignore file. `init` gives it to a project that has no `.gitignore`. |
| `roles/<role>.md` | The default rules page of each role, written by hand. |
| `roles/languages/<language>.md` | The good practice of one language: Python, Go, JavaScript, TypeScript, Java, Rust. Only the reviewer and the simplifier read them. `init` copies them. |
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
| `tests/step_tools.rs` | Tests of `tool`, `model` and `effort`, with a simulated `claude` and `codex`. |
| `tests/attacker.rs` | Tests of the plan attacker: its step, its files, its settings and its rules page. |
| `tests/scratch_folder.rs` | Tests of the scratch folder of the attacker, its Codex entry and the documents. |
| `tests/run_tool.rs` | Tests of the stop message and of the tool of the run. |
| `tests/new_roles.rs` | Tests of the roles `reviewer`, `security` and `simplifier`, the new steps and the documents. |
| `tests/small_repairs.rs` | Tests of version 0.1.11: the lock number, `status --wait`, the work folder, run ids, the stop message. |
| `tests/stop.rs` | Tests of `stop`, with a simulated `claude`. The last three use `ps` and `pkill`, and skip themselves in a sandbox. |
| `tests/language_pages.rs` | Tests of the six language pages, `init` and the two rules pages. |
| `install.sh` | Installs the command and the chat command. |
| `LICENSE` | The MIT licence. Do not change its text. |

## The sheet

The file `graph.example.yaml` tells what each line does. A step can have only these lines: `role`, `type`, `run`, `reads`, `writes`, `next`, `on_bad` and `max_tries`. It can also have `fan`, `tool`, `model` and `effort`. The sheet also has one `start` line.

- A step with `type: person` stops the run until the person answers yes or no.
- A step with `type: fan` has a `fan` line: a list of worker steps. The supervisor does them at the same time and waits for all of them. The result is good only if all of them are good.
- A step in a `fan` line has only `role`, `reads`, `tool`, `model` and `effort`. It writes no paper.
- Only a worker step can have `tool`, `model` and `effort`. `tool` is `claude` or `codex`. With no line, the step uses the tool of the run.
- The step gives `model` and `effort` to its tool. The role needs the settings file of that tool.
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
- Only one command can work on a run at a time. The file `lock` in the run folder shows this. It holds the number that the computer gives to that command. After a kill, the next command for the run repairs it. A person does not remove the file by hand.
- `stop` works only on a run with status Running. It ends the workers and the command, removes `lock`, and writes Stopped. `resume` then tries the same step.
- Three runs on 18 November 2026 showed the cause. A chat killed `run` or `answer`. The file `lock` stayed, the notebook said Running, and the worker lived on.
- The repair is one function. `status`, `status --wait`, `resume`, `answer` and `stop` call it before their own work. No code catches a signal, so it works for a hard kill too.
- It starts only if `lock` holds a number and that command is dead or is not `codeumbrella`. An empty `lock` or a live `codeumbrella` starts no repair.
- If the command is alive and `ps` cannot tell its name, the command gives an error with the words "no sandbox" and changes no file.
- The repair ends the workers that stay, removes `lock`, and writes Stopped with a note for a Running run. A Waiting, Done or Stopped run keeps its notebook. It prints `Repaired: the command of this run was dead.`
- A tool part writes `worker.<number>` when a worker starts and removes it when the worker ends. With no `ps`, it writes no file. The repair ends the worker only if `ps -o lstart=` gives the text of the file. The computer can give a dead number to a different command.
- `stop` ends a command only if `ps` shows that its name is `codeumbrella`. The computer can give a dead number to a different command.
- If the command is alive and `ps` cannot tell its name, `stop` gives an error and changes no file. A sandbox blocks `ps`, so the person runs `stop` with no sandbox.
- The chat runs `stop` only after the person says stop, with no sandbox. An AI does not stop a run by its own decision.
- An answer and a resume get the lock before they read the notebook. Thus a second command cannot write an answer into a run that is active.
- The clean copy reads the `origin` settings from `.git/config` of the project. Thus the factory does not work in a linked git worktree.
- `model` and `effort` have only letters, digits, `.`, `-` and `_`. Thus no quote or brace can get to the tool command.
- `model` and `effort` cannot start with `-`. Thus the tool does not read them as an option.
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
- The Claude planner has no shell. The builder, the checker and the simplifier can run all commands, but only in the sandbox.
- The builder, the checker and the simplifier can connect to a server on the same computer. An example is a test database. A test on 16 November 2026 showed that the internet and GitHub stay closed with this setting. A worker can thus talk to each server that runs on the computer of the person.
- The Codex part always turns on the Codex network proxy. Without the proxy, a Codex role that has network settings gets the full internet.
- Workers have no internet. The packages of a project must be on the computer before a run. A test on 16 November 2026 showed the cause. With internet, a worker can use the GitHub key of the person.
- The builder, the checker and the simplifier run in the Claude sandbox. They cannot write to `.codeumbrella`, `.claude`, `.github`, `.git/hooks` or `.git/config`. They write only in the work copy of their run.
- They also cannot write the files `.git/commondir` and `.git/config.worktree`. With such a file, a worker can give git its own settings. Then git runs a script of the worker with no sandbox.
- The steps with `gh` read the clean copy, not the `.git` folder of the project. The word `GIT_DIR` in the sheet does this.
- A deny rule such as `Edit(.codeumbrella/**)` is what makes the sandbox lock a path. The tests of 15 November 2026 showed this. The setting `sandbox.filesystem.denyWrite` with a relative path did nothing in those tests.
- A deny rule without the sandbox is not sufficient. A script that a permitted command starts can go around it.
- Each Codex role runs in the Codex sandbox with the settings in `roles/<role>.codex.txt`. The locked paths are the same as for Claude. The planner can write only to the two papers of its run. The Codex sandbox has no internet, so a Codex worker cannot push.
- The sandbox is the only wall for the commands of a worker, for Claude and for Codex.
- `init --update` copies each changed factory file to `<name>.before`, then writes the default. It does not touch `runs` or the `.gitignore` of the project folder.
- A paper that a step writes is not compared before that step. Thus a writer can run again after a try that broke its paper.
- After a stop for a changed paper, `resume` stops again. The stop message does not print the `resume` line. A person must put the paper back first.
- The plan step has 3 tries for one run. The first plan uses one. A no of the person or a bad build uses one more. Thus 2 of them can go back, and the third stops the run.
- The step `build` has `on_bad: plan`. Runs on 18 November 2026 stopped at `build` for one bad paper test each. The code was good. A stopped run refuses a changed paper. The step `repair` has no such line.
- The attacker runs the test command in its scratch copy. A test that is bad in its own setup is a problem. A run on 18 November 2026 showed the cause. The attacker built the tests and found 10 bad tests, and it gave one cause for all.
- The attacker reads the spec and the tests after the planner. It writes only in a scratch folder in the temp folder of the computer. There it builds the paper tests and runs the checks. Then it removes that folder.
- A bad result of the attacker goes to a step `correct_plan`. The planner corrects the plan, and the attacker looks again.
- There are 3 attacks at the maximum for each plan. After the third correction, the person sees the tests.
- The sheet has these six steps in one line, with no path back. Thus a circle is not possible.
- A live test on 17 November 2026 showed the cause. A Codex attacker gave 5 bad results for a small task, and the run stopped.
- For Claude, the settings of the attacker are the same as those of the safety checker. For Codex, they have one more entry: write in the temp folder.
- Each role has one rules page.
- The chat starts `run`, `answer` and `resume` with `nohup`, so a command lives after the shell ends. It adds `--tool codex` to `run` only. The chat uses `status --wait`. That command repairs a run with a dead command and shows Stopped. The chat tells the person, and runs `resume` only after the person says yes. It does not remove `lock` by hand.
- The checker compares each test file of the paper `tests` with the project in each try. A file that is different or not there gets the paper version, because the person approved the paper. The builder does not change such a file.
- The default check step is a fan of four checkers. `check_tests` runs the tests. `check_safety`, `check_review` and `check_security` read the change. They cannot write a file.
- The step `simplify` follows `build`. The simplifier changes no test file, and it commits only if the test command passes. The step `repair` does not go through it.
- The reviewer and the `security` checker have the same settings as the safety checker. The simplifier has the same settings as the builder.
- The stopped run prints `To continue: codeumbrella resume <run id>` and a line that says: resume only after the person says yes. An AI must not give this approval. The limit for the note does not cut them.
- `answer` and `resume` with a `--tool` that is different from the tool of the run give an error. They give it before a step starts and before the answer is written. A notebook with no tool uses Claude, and `--tool` writes it. `status` changes the notebook only in a repair.
- The safety checker also reads the files that use the changed lines, for example settings files. A run on 17 November 2026 showed the cause.
- The four checkers work in the same copy. Only the tests checker writes there.
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

- The supervisor (`src/graph.rs`, `src/engine.rs`, `src/main.rs`, `src/log.rs`, `src/lib.rs`): 1000 lines of code.
- Each tool part: 200 lines of code.
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
6. One source for Claude and Codex. Only the tool parts and the settings files of the roles are different for each tool. A step can pick its tool.

## Not in version 1

- Builders that work at the same time. The fan of today is only for workers that write no file.
- A different worker that writes the tests.
- A step that writes lessons after each run.
- Custom hooks.

## How to write for the person

The person who owns this project does not know Rust. Write messages and documents in short, plain sentences. Give the meaning of each technical word the first time that you use it.
