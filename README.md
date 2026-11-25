# Codeumbrella

Codeumbrella is a software factory. You give it a task. It gives the task to workers (AI sessions). They plan, write and check the code.

Code goes into `main` only after two things. First, the code agrees with the tests that you gave approval for. Second, you gave approval for the PR.

The factory stops two times for you. First, you look at the tests. Second, you give approval for the `pull request` (PR). A PR is a proposal to add code to `main`. You answer yes or no each time.

The page [ARCHITECTURE.md](ARCHITECTURE.md) shows the parts and the path of a task in two pictures.

An attacker looks for weak points in the plan before you see the tests. A simplifier then makes the new code simpler, and a learner reads the logs of the runs. Five checkers look at each piece of work at the same time. One runs the tests that you gave approval for. One reads the change for safety problems. A reviewer reads it for errors.

A `security` checker reads it for weak points that a person can use to do damage. A user checker builds the work in a scratch folder. It runs the examples of the spec as a user does.

The learner turns each problem of a run into a lesson. A lesson that a test can show becomes a lesson test on the branch. Each other lesson becomes a short entry. After the merge, the step `save_lessons` copies the entries to `.codeumbrella/lessons.md`. Then the step `clean_up` removes the two copies of the run.

Git does not see the file `.codeumbrella/lessons.md`. The workers of the next run read it.

If the builder finds a bad paper test, the planner corrects only that test, and you see the tests again.

Each PR has a set PR text, so you see what the change is for. The planner writes the title and the purpose, the cause, an example and the fix. After the build, the role `writer` adds the version line, the files of each area of the change, and a checklist. Its step is `write_pr`. It runs again after each repair, so the text agrees with the change.

Each item of the checklist has `[x]`, or `[ ]` with a cause. The reviewer compares the PR text with the change. At the PR stop, the chat shows you the title and each `[ ]` line.

Steps with no AI look for work. After your yes for the tests, the step `mark` keeps the commit of that moment. The step `copy_tests` then commits the paper tests and moves that mark. After the simplifier, the step `work_was_done` looks for a new commit after the mark. With no new commit, the builder step `do_work` must do the work. Then `work_was_done` checks again, and the run stops after the third try.

Two runs can work at the same time. Before the PR question, the step `take_master` merges the newest master into the branch. If master changes while you look at the PR, you can get the PR question two times. The step `take_master` has 3 tries. After the third try the run stops. A bad merge goes to the builder step `fix_merge`.

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
     On a Mac with Apple hardware, it first tries to download the built command of your commit. The file comes from a public Cloudsmith store, and the script compares its checksum. With no file there, it builds the command on your computer with Rust. If a tool is not on the computer, it tells you how to install it. It then stops and installs nothing.
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

## The queue

The folder `.codeumbrella/queue/` holds one text file for each task that waits. The file name sets the sequence, for example `01-clean-up.txt` before `02-pr-text.txt`. A line `Files: a.rs, b.rs` in a file names the files of the task. `codeumbrella queue` shows the tasks. `codeumbrella run --next` starts the first one and moves its file into the run. It gives an error if the queue is empty or a run is active.

The step `set_version` sets the version of the project before the PR text. It runs the script `.codeumbrella/set-version`. `init` writes a script that does nothing. Put your own version commands in it. A task text names no version.

## Before a run

- Workers have no internet. Install the packages of your project first.
- Workers can connect to a server on your computer, for example a test database.
- Put the test command at the end of the task: `Test command: <command>`
- A tool in your project folder, for example in `.venv`, needs its full path in the test command.
- The workers run the test command in a sandbox, in a clone of your project. A git hook and a second sandbox do not work there.
- Your full check can be bad in the sandbox. Then end the task with `Test command: <command>` and `Full check: <command>`.
- The test command works in the sandbox. No worker runs the full check. The chat offers to run it at the PR question.
- The workers write in a copy in the run folder. A run does not change your project folder or its branch.
- After status Done, get the code with `git pull`. The step `clean_up` removed the copies of the run. Its record stays.

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

The status shows one fact on each line: the run, the status, the note, the step of this moment and the total of the workers. The total has the lines `Workers:`, `Time:` and `Cost:`.

## After you install a new version

Run `codeumbrella init --update` in your project. It replaces each factory file that is different from the new default. It keeps a copy of your file with `.before` at the end, for example `graph.yaml.before`. Move your changes back by hand. It prints one line for each file that it changed. It does not touch your runs or the `.gitignore` of your project.

## Your two answers

| Stop | Yes | No, with your text |
|---|---|---|
| The tests | The builder starts. | The planner writes the tests again. |
| The PR | The supervisor merges. Then it copies the lessons to `.codeumbrella/lessons.md`. Then `clean_up` removes the two copies of the run. | The planner corrects the spec and the tests. You see the tests again. The same PR gets the new code, a new title and a new text. |

A point of your task can have two meanings. Then the planner makes a decision and lists it at the top of the spec. At the tests stop, the chat tells you each such decision. In a terminal, read the top of `.codeumbrella/runs/<run-id>/papers/spec/spec.md`.

## Codex workers

Add `--tool codex` after the word `codeumbrella` in the command `run` only:
`codeumbrella --tool codex run "<task>"`

The run keeps the tool of the run in its notebook. `answer` and `resume` use it. A different `--tool` gives an error.

## When a run stops

| Look here | What it has |
|---|---|
| `.codeumbrella/runs/<run-id>/log.jsonl` | One line for each step: time, result, seconds. A worker step also gets the numbers of its worker, for example `dollars`. |
| `.codeumbrella/runs/<run-id>/<step>.<number>.trace.jsonl` | All that one worker did. |
| `codeumbrella stop <run-id>` | Ends an active run: its command and its workers. The status becomes Stopped. Run it with no sandbox, and only when you say stop. |
| `codeumbrella resume <run-id>` | Tries the stopped step again. A stopped run prints this line, but not after a changed paper. Run it only after you say yes. |

Three stops have their own cause:

| The note says | What you do |
|---|---|
| A paper is not as its writer left it | A worker changed a paper. Put the paper back, or start a new run. `resume` does not help. |
| The step `no_locked_files` gave a bad result | The work changes `.github`, `.claude` or `.codeumbrella`. If the task must do that, remove the folder from that step in `graph.yaml`. |
| A different command works on this run | Wait until that command ends. After a kill, run `codeumbrella status <run-id>`. It repairs the run and shows Stopped. |

The factory does not work in a linked git worktree.

## Change the factory

| File in `.codeumbrella/` | What you change |
|---|---|
| `graph.yaml` | The steps and their sequence. Comments in the file tell each option. |
| `roles/<role>.md` | The rules of one worker. |
| `roles/languages/<language>.md` | The language pages for Python, Go, JavaScript, TypeScript, Java and Rust. The reviewer and the simplifier read the page of each language in the change. |
| `roles/<role>.claude.json`, `roles/<role>.codex.txt` | What one worker can do. |

The workers write short sentences for you: 20 words at the maximum. A project can put a list of refused words into its `AGENTS.md`. Then the workers use no word of that list, and the reviewer checks it.

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
