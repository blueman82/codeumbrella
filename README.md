# Codeumbrella

Codeumbrella is a software factory. You give it a task. It gives the task to workers (AI sessions). They plan, write and check the code.

Code goes into `main` only after two things. First, the code agrees with the tests that you gave approval for. Second, you gave approval for the PR.

The factory stops two times for you. First, you look at the tests. Second, you give approval for the `pull request` (PR). A PR is a proposal to add code to `main`. You answer yes or no each time.

The page [ARCHITECTURE.md](ARCHITECTURE.md) shows the parts and the path of a task in two pictures.

An attacker looks for weak points in the plan before you see the tests. A simplifier then makes the new code simpler. Four checkers look at each piece of work at the same time. One runs the tests that you gave approval for. One reads the change for safety problems. A reviewer reads it for errors. A security checker reads it for security problems.

## Requirements: what the computer must have

- Rust with `cargo`, to build the `codeumbrella` command. Check: `cargo --version`
- `git`, to keep the versions of the code. Check: `git --version`
- `gh`, the GitHub command, to open the PR. Check: `gh --version`
- `claude` (Claude Code) or `codex` (Codex). These start the workers. Check: `claude --version` or `codex --version`
- A GitHub project for your code, with the name `origin`. Check: `git remote get-url origin`
- `gh` signed in. Check: `gh auth status`
- macOS. There is no test on Linux or Windows.

## Install

The plugin brings only the chat command `/codeumbrella`. The `codeumbrella` command is a different part. Install it in step 1.

1. Install the command. In this folder, run one of these:
   - `./install.sh`. It also puts the chat command into `~/.claude` and `~/.codex`. The plugin is for a person who does not run `./install.sh`.
   - `cargo install --path . --locked`
2. Install the plugin for the chat command.
   - Claude Code:
     ```
     /plugin marketplace add <path or GitHub name of this folder>
     /plugin install codeumbrella@codeumbrella
     ```
   - Codex:
     ```
     codex plugin marketplace add <path or GitHub name of this folder>
     ```
     Then open Codex, type `/plugins`, and install `codeumbrella`.

## Before a run

- Workers have no internet. Install the packages of your project first.
- Workers can connect to a server on your computer, for example a test database.
- Put the test command at the end of the task: `Test command: <command>`
- A tool in your project folder, for example in `.venv`, needs its full path in the test command.
- The workers write in a copy in the run folder. A run does not change your project folder or its branch.
- After status Done, get the code with `git pull`.

## First run

1. Go to your project folder. Commit your changes first.
2. Run `codeumbrella init`. It puts the factory files in `.codeumbrella/`. You can change them.
3. Give the task. Use one of these:
   - In a chat: `/codeumbrella <task>`
   - In a terminal: `codeumbrella run "<task>"`
4. When the run stops, answer yes or no:
   - In a chat, tell the chat your answer.
   - In a terminal, run `codeumbrella answer <run-id> <step> yes` or `codeumbrella answer <run-id> <step> no "why"`. The step is `look_at_tests` or `approval`.

To see where a run is, run `codeumbrella status`. To wait until it ends, run `codeumbrella status --wait <run-id>`.

## After you install a new version

Run `codeumbrella init --update` in your project. It replaces each factory file that is different from the new default. It keeps a copy of your file with `.before` at the end, for example `graph.yaml.before`. Move your changes back by hand. It prints one line for each file that it changed. It does not touch your runs or the `.gitignore` of your project.

## Your two answers

| Stop | Yes | No, with your text |
|---|---|---|
| The tests | The builder starts. | The planner writes the tests again. |
| The PR | The supervisor merges. | The planner corrects the spec and the tests. You see the tests again. The same PR gets the new code. |

A point of your task can have two meanings. Then the planner makes a decision and lists it at the top of the spec. At the tests stop, the chat tells you each such decision. In a terminal, read the top of `.codeumbrella/runs/<run-id>/papers/spec/spec.md`.

## Codex workers

Add `--tool codex` after the word `codeumbrella` in the command `run` only:
`codeumbrella --tool codex run "<task>"`

The run keeps the tool of the run in its notebook. `answer` and `resume` use it. A different `--tool` gives an error.

## When a run stops

| Look here | What it has |
|---|---|
| `.codeumbrella/runs/<run-id>/log.jsonl` | One line for each step: time, result, seconds. |
| `.codeumbrella/runs/<run-id>/<step>.<number>.trace.jsonl` | All that one worker did. |
| `codeumbrella resume <run-id>` | Tries the stopped step again. A stopped run prints this line, but not after a changed paper. Run it only after you say yes. |

Three stops have their own cause:

| The note says | What you do |
|---|---|
| A paper is not as its writer left it | A worker changed a paper. Put the paper back, or start a new run. `resume` does not help. |
| The step `no_locked_files` gave a bad result | The work changes `.github`, `.claude` or `.codeumbrella`. If the task must do that, remove the folder from that step in `graph.yaml`. |
| A different command works on this run | Wait until that command ends. After a kill, remove the file `lock` in the run folder. |

The factory does not work in a linked git worktree.

## Change the factory

| File in `.codeumbrella/` | What you change |
|---|---|
| `graph.yaml` | The steps and their sequence. Comments in the file tell each option. |
| `roles/<role>.md` | The rules of one worker. |
| `roles/<role>.claude.json`, `roles/<role>.codex.txt` | What one worker can do. |

## What a worker can and cannot do

| Can | Cannot |
|---|---|
| Write code in the project, and commit | Push, open a PR, or merge |
| Run commands in a sandbox | Use the internet or GitHub |
| Connect to a server on your computer | Write to `.codeumbrella`, `.claude`, `.github` or the git settings |
| | Write to a folder that is not the project |
| | Read your key folders, for example `~/.ssh` and the macOS key store |
| | Use the settings or the hooks of the project (Claude workers) |

## Licence

The licence is MIT. The file `LICENSE` has the full text.
