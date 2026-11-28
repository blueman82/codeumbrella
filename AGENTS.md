# Codeumbrella

This file is the one source of the project rules for all agents. `CLAUDE.md` imports it. Do not copy its text into a second file.

## What this project is

Codeumbrella is a software factory. A person gives it a task. It puts code into `main` only after two things. First, the code agrees with the tests that the person gave approval for. Second, the person gave approval for the PR.

The factory is written in Rust. The projects that go through it can be in each language. Knowledge of one language is only in the language pages (`roles/languages/`). The supervisor, the default sheet and the rules pages of the roles have none. Only the seven page file names in the file list of `init` are permitted.

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
| Notebook | `.codeumbrella/runs/<run-id>/state.json`: where the run is, and the tool of the run. `since` is the start time of the step of this moment. `total` has the sum of the numbers of the workers: `workers`, `seconds`, `dollars`, `tokens_in`, `tokens_out` and `no_cost`. |
| Tool of the run | The tool that `run` got from `--tool` (default Claude). The notebook keeps it. `answer` and `resume` use it. |
| Log | `log.jsonl` in the run folder: one line for each step that ended. The line of a worker step also has the numbers of its worker: `worker_seconds`, `dollars`, `turns`, `model`, `tokens_in` and `tokens_out`. A field is there only if the tool gives it. |
| `worker.<number>` | A file in the run folder. It holds the start time of the worker with that number. The computer gives this number to each command. |
| `trace` | `<step>.<number>.trace.jsonl` in the run folder: all that one worker did in one try. |
| Paper | A folder in `.codeumbrella/runs/<run-id>/papers/` that one step writes and other steps read. |
| Paper `pr` | The paper with the PR text. `title.txt` is the title, in one line. `body.md` is the text. The planner writes the title and parts 1 to 5. The writer adds parts 6 to 8. After a bad check, the writer can correct a sentence of parts 1 to 5 that the report names. |
| Lesson | A problem of a run that can come again in a different task. The learner makes it a lesson test, or a short entry. |
| `.codeumbrella/lessons.md` | The entries of the lessons of a project. The step `save_lessons` adds to it after the merge. The step `tidy_lessons` removes done entries. Git does not see it. |
| Role | The type of a worker. Its rules page is `.codeumbrella/roles/<role>.md` in a project. The folder `roles` holds the defaults. |
| Writer | The role `writer`. It reads the change with git. It adds the version line, the areas of the change and the checklist to `body.md` of the paper `pr`. After a bad check, it also corrects named sentences of parts 1 to 5. |
| Queue | The folder `.codeumbrella/queue/`: one text file for each task that waits. The file name sets the sequence. `run --next` starts the first one. |

## Commands

```sh
cargo fmt --check                              # layout
cargo clippy --all-targets -- -D warnings      # usual errors, and a warning stops the code
cargo test                                     # tests, with a simulated worker
cargo build --locked                           # build
cargo deny check && cargo audit                # look for bad packages, after a package change
cargo +nightly fuzz run <target> --sanitizer none   # fuzz one target, with nightly Rust

codeumbrella init                              # put the default sheet and rules pages into a project
codeumbrella init --update                     # replace changed factory files with the new defaults, keep a copy of each as `.before`
codeumbrella run "<task>"                      # start a run with Claude workers
codeumbrella run --next                        # start the first task of the queue `.codeumbrella/queue/`. An error if the queue is empty. It starts also while a different run is `Running` or `Waiting`
codeumbrella queue                             # show the tasks that wait: the file name, its `Files:` line and its `Purpose:` line
codeumbrella --tool codex run "<task>"         # start a run with Codex workers. The run keeps this tool
codeumbrella status [run-id]                   # show where a run is: the step of this moment and the total of the workers. After a kill, it repairs the run first (see Safety rules)
codeumbrella status --wait <run-id>            # wait until the run is not `Running`, then show it
codeumbrella answer <run-id> <step> yes|no ["text"]   # answer the question of that step, then continue
codeumbrella stop <run-id>                     # end an active run: its command and its workers. Only after the person says stop
codeumbrella resume <run-id>                   # try a stopped step again, with its count of tries at 0. Only after the person says yes, or for a `known cause`
```

Run all four `cargo` commands before you say that work is complete.

To install the command and the chat command on a computer, run `./install.sh`.

## Structure

| Path | Contents |
|---|---|
| `graph.example.yaml` | The default sheet, with a comment for each option. `init` copies it into a project. |
| `default.set-version` | The default script of the step `set_version`. It does nothing. `init` writes it to `.codeumbrella/set-version`, and `init --update` keeps a script that is there. |
| `scripts/set-version` | The script of this project. The person copies it to `.codeumbrella/set-version`. It sets the next patch version in the version files, also in `fuzz/Cargo.lock`. |
| `.codeumbrella/queue/` | The queue: one text file for each task that waits. The file name sets the sequence. A line `Files:` names the files of the task. `run --next` moves the first file into the run as `task.txt`. |
| `tests/same_time.rs` | Tests of two runs at the same time. They test `run --next`, the line `Run:`, `take_master`, `fix_merge`, `same_master`, `work_was_done`, `do_work` and the documents. |
| `tests/queue_rules.rs` | Tests of the `Purpose:` line of `queue`, of `skip_if`, of the tests question, of the chat page and of the documents. |
| `tests/queue.rs` | Tests of the queue, `run --next`, `set_version`, `files.txt`, the default script, the script of this project and the documents. |
| `default.gitignore` | The default ignore file. `init` gives it to a project that has no `.gitignore`. |
| `roles/<role>.md` | The default rules page of each role, written by hand. |
| `roles/learner.md` | The rules page of the learner. It reads the logs of the runs and writes the paper `lessons`. |
| `roles/writer.md` | The rules page of the writer. It adds parts 6 to 8 of the PR text and the checklist. |
| `roles/writer.claude.json` | The Claude settings of the writer: the walls of the reviewer, and one write entry for the paper `pr`. |
| `roles/writer.codex.txt` | The Codex settings of the writer: the reviewer file, and one write entry for the paper `pr`. |
| `roles/languages/<language>.md` | The standards page of one language: Python, Go, JavaScript, TypeScript, Java, Rust. `general.md` has the general coding standards. Each page has 300 lines at the maximum and two parts: `## Write the code like this` and `## A break`. The builder, the reviewer and the simplifier read the pages. `init` copies them. |
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
| `tests/packaging.rs` | Tests of the plugin files, the README and `install.sh`. |
| `tests/walls.rs` | Tests of the safety settings and the safety checks. Each one becomes bad if a wall goes. |
| `tests/step_tools.rs` | Tests of `tool`, `model` and `effort`, with a simulated `claude` and `codex`. |
| `tests/attacker.rs` | Tests of the plan attacker: its step, its files, its settings and its rules page. |
| `tests/attacker_words.rs` | Tests of the plain words check of the attacker and of its documents. |
| `tests/scratch_folder.rs` | Tests of the scratch folder of the attacker, its Codex entry and the documents. |
| `tests/run_tool.rs` | Tests of the stop message and of the tool of the run. |
| `tests/new_roles.rs` | Tests of the roles `reviewer`, `security` and `simplifier`, the new steps and the documents. |
| `tests/small_repairs.rs` | Tests of version 0.1.11: the lock number, `status --wait`, the work folder, run ids, the stop message. |
| `tests/kill_repair.rs` | Tests of the repair after a kill. The last four use `ps` and `pkill`, and skip themselves in a sandbox. |
| `tests/stop.rs` | Tests of `stop`, with a simulated `claude`. The last three use `ps` and `pkill`, and skip themselves in a sandbox. |
| `tests/copy_tests.rs` | Tests of the step `copy_tests`, the list of changed papers, the checker page and the documents. |
| `tests/copy_repairs.rs` | Tests of the run branch that keeps its commits, the merge that did not complete, the note and the lesson test rules. |
| `tests/find_each_point.rs` | Tests of `mark`, `work_was_done`, the new rules lines and the repair that changes no file. |
| `tests/language_pages.rs` | Tests of the seven standards pages, `init`, three rules pages and the documents. |
| `tests/bad_repair.rs` | Tests of the bad repair that goes to the plan, the new rules lines, the chat page and the documents. |
| `tests/user_checker.rs` | Tests of the fifth checker `check_use`: its step, the files of the role `user`, its rules page and the documents. |
| `tests/learner.rs` | Tests of the role `learner`: the steps `learn` and `save_lessons`, its files, its rules page and the documents. |
| `tests/lessons_keep.rs` | Tests of the lessons that stay: the steps `copy`, `save_lessons` and `tidy_lessons`, the rules pages and the documents. |
| `tests/clean_up.rs` | Tests of the step `clean_up`: the default sheet, runs with the built command and the documents. |
| `tests/pr_text.rs` | Tests of the PR text: the paper `pr`, the role `writer` and the step `write_pr`. Also `find_pr` and `open_pr`, with a simulated `gh`. |
| `tests/writer_fix.rs` | Tests of the writer that corrects a sentence of the PR text after a bad check: the run path, the rules pages, the sheet and the documents. |
| `tests/run_total.rs` | Tests of the numbers of each worker, the total in the notebook, the lines of `status` and the documents. |
| `tests/full_check.rs` | Tests of the full check that cannot run in the sandbox: the chat page, the rules pages, `init` and the documents. |
| `tests/run_id.rs` | Tests of the run id with four digits, of `status` on an id of before, of the chat page and the documents. |
| `tests/plain_words.rs` | Tests of the rules for plain words: the rules pages, the list of `AGENTS.md`, the sheet and the documents. |
| `tests/known_causes.rs` | Tests of the `resume` after a known cause: the chat page and the documents. |
| `fuzz/` | The fuzzer, with its own workspace. A fuzzer gives the code many automatic inputs. Its targets are `fuzz_sheet`, `fuzz_claude_reply` and `fuzz_codex_trace`. |
| `fuzz/deny.toml` | The package check of `fuzz/`. It also accepts the license NCSA, in `fuzz/` only. |
| `fuzz/seeds/` | Good inputs for each target. The fuzzer starts from them. |
| `tests/fuzz_crashes/` | One folder for each target, with the inputs that caused a crash. |
| `tests/fuzz_crashes.rs` | Runs each crash input again in `cargo test`, on stable Rust. |
| `scripts/fuzz-repair` | A script for the Mac. It starts a run for each open issue `fuzz-crash` that the GitHub machine opened. While a run for that issue is not Done, it starts no new run for it. |
| `scripts/fuzz-repair.plist` | The launchd pattern that starts `scripts/fuzz-repair` each hour. |
| `.github/workflows/fuzz.yml` | The fuzz steps on GitHub: `fuzz_pr` on each PR, and `fuzz_master` each night. A chat session adds this file, because a run cannot change `.github`. |
| `install.sh` | Installs the command and the chat command. |
| `LICENSE` | The MIT licence. Do not change its text. |

## The sheet

The file `graph.example.yaml` tells what each line does. A step can have only these lines: `role`, `type`, `run`, `reads`, `writes`, `next`, `on_bad` and `max_tries`. It can also have `fan`, `tool`, `model`, `effort` and `skip_if`. The sheet also has one `start` line.

- A step with `type: person` stops the run until the person answers yes or no.
- A person step can have a `skip_if` line: a list of commands, as `run`. If all are good, the run goes to `next` with no question. If one is bad, the run waits.
- A `skip_if` line on a different type of step is an error. So is a command with no words.
- The chat starts 3 tasks of the queue by itself at the maximum, one after the other. Then it waits for the person and tells each task that waits.
- A run can stop at a plan step with no paper to correct, with commits after `mark`. Then the chat puts its `task.txt` back into the queue and starts it again, one time only. It writes the cause as a new task at the end of the queue.
- A step with `type: fan` has a `fan` line: a list of worker steps. The supervisor does them at the same time and waits for all of them. The result is good only if all of them are good.
- A step in a `fan` line has only `role`, `reads`, `tool`, `model` and `effort`. It writes no paper.
- Only a worker step can have `tool`, `model` and `effort`. `tool` is `claude` or `codex`. With no line, the step uses the tool of the run.
- The step gives `model` and `effort` to its tool. The role needs the settings file of that tool.
- A step with `type: command` has a `run` line: a list of commands. The supervisor runs them itself, with no shell and no worker. In a word, `{run}` is the id of the run. If a command gives an error, the result is bad. The error text comes before the command.
- `next` is the step after a good result. `on_bad` is the step after a bad result.
- A step that an `on_bad` line goes to must have `max_tries`.
- A line that the supervisor does not know is an error.
- The command step `set_version` comes after `learn` and before `write_pr`. It runs `.codeumbrella/set-version` on the work copy. A bad result stops the run. A task text names no version: only the script sets it.
- `work_was_done` also writes the files of the new commits into `files.txt` in the run folder. With no new commit, the list stays.
- `check_review` reads `spec`, `pr` and `lessons`. It reads the paper `lessons` for plain words.
- The command step `mark` has no AI. It comes after `look_at_tests` and before `copy_tests`. It writes the commit of that moment into `refs/codeumbrella/before` of the work copy.
- In its first time, `mark` also writes that commit into `first-yes.txt` in the run folder. It does this only if that file and the git name are not there.
- The attacker runs the check "good before the code is written" on the commit in `first-yes.txt`. With no file, it uses the commit of the start of the run.
- After an attack, a planner can have no paper to correct. It gives `ok: false` only with a cause that a person can act on. It names that cause in the first line of its summary.
- Before that, `mark` writes `new-work.bundle` and `files.txt` for the commits of a builder that no check saw. It does this only while the run folder has no `new-work.bundle`.
- The command step `copy_tests` has no AI. It comes after `mark` and before `build`. It copies the paper `tests` into the work copy and commits the copied files. Then it moves `refs/codeumbrella/before` to that commit. A bad result stops the run.
- `copy_tests` keeps the commits of the run branch. With a merge that did not complete in the work copy, it gives a bad result.
- A worker step that runs a second time gets one line for each paper that changed since its last run. The notebook keeps the data in `seen`.
- The steps of the planner write the papers `spec`, `tests` and `pr`.
- The worker step `write_pr` comes after `set_version` and after `repair`, before `check`. The writer completes the PR text in the paper `pr`.
- With problems only in `body.md` of the paper `pr`, `repair` changes no file. The run then goes to `write_pr`, and the writer corrects the sentences.
- `work_was_done` comes right after `simplify`. It makes `new-work.bundle` in the run folder from the commits after `copy_tests`. Only the commits of `build` and `simplify` count.
- With no new commit and no `new-work.bundle`, git gives an error. The run then goes to the builder step `do_work`. Then it checks again. After the third try of `do_work` the run stops.
- After a no, `work_was_done` is good with no new commit if the run folder has `new-work.bundle`. The run then goes on to the checks.
- `take_master` comes after `work_was_done` and before `learn`. It merges the newest master into the branch. It has `max_tries: 3`. A bad merge goes to the builder step `fix_merge`, then back to `take_master`.
- `same_master` comes after `approval` and before `merge`. A branch with no newest master goes to `take_master`. The person then gets the PR question again.
- `check` goes to `no_locked_files`. Only `same_master` goes to `merge`.

## Safety rules

- The supervisor opens the PR. Then the run always stops for the approval of the person. The merge comes only after a yes. A no with a text sends the work back to the planner. The person then sees the tests again.
- After a bad build or a bad repair, the tests question stays away only with these three facts. The planner changed only paper tests that the bad result names. The spec did not change, and the first attack was good and complete. The person sees that change at the PR question.
- The commands of `skip_if` compare the papers with the copy that `mark` keeps in the run folder. No worker can write that copy. Each doubt or error brings the question.
- No worker can push, open a PR or merge. Only the command steps `push`, `find_pr`, `open_pr` and `merge` can, with the set commands in the sheet.
- The script `.codeumbrella/set-version` of the step `set_version` runs with no sandbox, as each command step does. Only the person writes it, because workers cannot write in `.codeumbrella`. The step `work_was_done` and the commit of the script run git in the work copy, where a worker can write. So the diff has `--no-ext-diff` and `--no-textconv`, and the commit has `-c core.hooksPath=/dev/null` and `--no-verify`. Each git call of the script also has `-c core.fsmonitor=false`. The script stops if the config of the work copy has a name that a clean clone does not have. `set_version` does not push.
- The config check of that script reads only the settings of the work copy and each file that they include. A CI run on 21 November 2026 showed the cause. A computer with Git LFS has a filter in the settings of the computer, and the script stopped there.
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
- Only one command can work on a run at a time. The file `lock` in the run folder shows this. It holds the number that the computer gives to that command. After a kill, the next command for the run repairs it.
- `stop` works only on a run with status Running. It ends the workers and the command, removes `lock`, and writes Stopped. `resume` then tries the same step.
- After a kill of a command, `lock`, a Running notebook and workers stay. Three runs on 18 November 2026 showed this. No code catches a signal, so the next command repairs the run.
- `status`, `status --wait`, `resume` and `answer` do the repair first. It starts only if `lock` holds a dead number or a live command that is not `codeumbrella`.
- For a run with status Running, the repair ends the workers, removes `lock` and writes Stopped with a note. A run with a different status keeps its notebook.
- An empty `lock` is not repaired, because a new command writes its number after it makes the file.
- The repair reads `lock` again after it opens the notebook. A new text means a new command: the repair does nothing.
- A tool part writes `worker.<number>` with the start time of the worker, and removes it at the end. It writes none if `ps` cannot run.
- The repair ends a worker only if its start time is the one in the file. The computer can give a dead number to a different command.
- `stop` ends a command only if `ps` shows that its name is `codeumbrella`. The computer can give a dead number to a different command.
- A live command can have a name that `ps` cannot give. Then `stop` and the repair give an error and change no file. A sandbox blocks `ps`, so the person runs `stop` with no sandbox.
- The chat runs `stop` only after the person says stop, with no sandbox. An AI does not stop a run by its own decision.
- An answer and a resume get the lock before they read the notebook. Thus a second command cannot write an answer into a run that is active.
- The clean copy reads the `origin` settings from `.git/config` of the project. Thus the factory does not work in a linked git worktree. The chat page stops there before the first command.
- The id of a new run is `run-<Unix seconds>-<four digits>`. The four digits come from the clock and the command number.
- Two runs in two folders got the same id on 19 November 2026. The two runs pushed to one branch.
- The same id in two folders is possible, but the chance is small. `run` makes no call to GitHub.
- The chat tells the person about a run that is not complete before it starts a new run.
- `model` and `effort` have only letters, digits, `.`, `-` and `_`. Thus no quote or brace can get to the tool command.
- `model` and `effort` cannot start with `-`. Thus the tool does not read them as an option.
- A worker that ends with an error code gives no good result.
- The result of a Claude worker is the last line of its reply with `"type":"result"`. Lines after it do not count.
- The cause: on 27 November 2026 the disk of the Mac was full. After the result of a safety checker, `claude` wrote a `session_writes_lost` line. The run stopped and did not use the result.
- A worker does not get the values `GH_TOKEN` and `GITHUB_TOKEN` of the person.
- For a Claude worker, only the settings file of its role counts. The worker uses no settings, hooks or tool servers of the project or of the person.
- A live test on 16 November 2026 showed the cause. With the project settings, a hook of the project ran in a worker, with no sandbox.
- Thus Claude does not read `AGENTS.md` and `CLAUDE.md` of a project by itself. Each rules page tells the worker to read them.
- No worker can read `~/.ssh`, `~/.config/gh`, `~/.netrc`, `~/.git-credentials` and `~/Library/Keychains`. A Claude worker has no web tools.
- A live test on 16 November 2026 showed the cause. A worker in the sandbox got the GitHub key from the macOS key store. The lock on `~/Library/Keychains` stopped it, for Claude and for Codex.
- A role with no settings file is an error. A worker does not start with no settings.
- A paper can change only in a step that lists it in `writes`. The supervisor keeps a fingerprint of each paper. Before each step, the supervisor compares all the papers with their fingerprints. If a paper is different, the run stops. The person can change a paper while the run waits for an answer. The answer makes the fingerprints again.
- All factory files of a project are in the folder `.codeumbrella`: the sheet, the rules pages and the runs. The folder has its own `.gitignore`, so git does not see it.
- The Claude planner has no shell. The builder, the checker, the simplifier and the learner can run all commands, but only in the sandbox.
- The builder, the checker, the simplifier, the learner and the user checker have a local network. They can connect to a server on the same computer. An example is a test database. A test on 16 November 2026 showed that the internet and GitHub stay closed with this setting. A worker can thus talk to each server that runs on the computer of the person.
- The Codex part always turns on the Codex network proxy. Without the proxy, a Codex role that has network settings gets the full internet.
- Workers have no internet. The packages of a project must be on the computer before a run. A test on 16 November 2026 showed the cause. With internet, a worker can use the GitHub key of the person.
- The builder, the checker, the simplifier and the learner run in the Claude sandbox. They cannot write to `.codeumbrella`, `.claude`, `.github`, `.git/hooks` or `.git/config`. They write only in the work copy of their run.
- They also cannot write the files `.git/commondir` and `.git/config.worktree`. With such a file, a worker can give git its own settings. Then git runs a script of the worker with no sandbox.
- The steps with `gh` read the clean copy, not the `.git` folder of the project. The word `GIT_DIR` in the sheet does this.
- A deny rule such as `Edit(.codeumbrella/**)` is what makes the sandbox lock a path. The tests of 15 November 2026 showed this. The setting `sandbox.filesystem.denyWrite` with a relative path did nothing in those tests.
- A deny rule without the sandbox is not sufficient. A script that a permitted command starts can go around it.
- Each Codex role runs in the Codex sandbox with the settings in `roles/<role>.codex.txt`. The locked paths are the same as for Claude. The planner can write only to the three papers of its run: `spec`, `tests` and `pr`. The Codex sandbox has no internet, so a Codex worker cannot push.
- The sandbox is the only wall for the commands of a worker, for Claude and for Codex.
- `init --update` copies each changed factory file to `<name>.before`, then writes the default. It does not touch `runs` or the `.gitignore` of the project folder.
- A paper that a step writes is not compared before that step. Thus a writer can run again after a try that broke its paper.
- After a stop for a changed paper, `resume` stops again. The stop message does not print the `resume` line. A person must put the paper back first.
- The plan step has 3 tries for one run. The first plan uses one. A no, a bad build or a bad repair uses one more. Thus 2 of them can go back, and the third stops the run.
- The step `build` has `on_bad: plan`. Runs on 18 November 2026 stopped at `build` for one bad paper test each. The code was good. A stopped run refuses a changed paper.
- The step `repair` has `on_bad: plan` too. Two runs on 19 November 2026 stopped at `repair` for one bad paper test.
- The builder cannot change a paper test. After the third try of `repair`, the run stops, as before.
- A checker that finds a paper test that no code can repair starts its summary with `Paper test:`. A run on 19 November 2026 showed the cause.
- The attacker waits until each command ends before it removes its scratch folder. On 19 November 2026 a removal broke a check.
- The attacker runs the test file checks of the project on the paper tests. A rule of the project refused 4 tests on 19 November 2026.
- A check of the attacker that did not complete gives the line `Not complete:`. The chat offers to run that check. This came from the runs of 19 November 2026.
- The attacker runs the test command in its scratch copy. A test that is bad in its own setup is a problem. A run on 18 November 2026 showed the cause. The attacker built the tests and found 10 bad tests, and it gave one cause for all.
- The attacker reads the spec and the tests after the planner. It writes only in a scratch folder in the temp folder of the computer. There it builds the paper tests and runs the checks. Then it removes that folder.
- A bad result of the attacker goes to a step `correct_plan`. The planner corrects the plan, and the attacker looks again.
- There are 3 attacks at the maximum for each plan. After the third correction, the person sees the tests.
- The sheet has these six steps in one line, with no path back. Thus a circle is not possible.
- A live test on 17 November 2026 showed the cause. A Codex attacker gave 5 bad results for a small task, and the run stopped.
- For Claude, the settings of the attacker are the same as those of the safety checker. For Codex, they have one more entry: write in the temp folder.
- Each role has one rules page.
- The chat starts `run`, `answer` and `resume` with `nohup`, so a command lives after the shell ends. It adds `--tool codex` to `run` only. The chat uses `status --wait`. That command repairs a dead run and shows Stopped. It does not remove `lock`.
- The chat runs `resume` only after the person says yes. A `known cause` is the exception.
- For a known cause only, the chat starts `resume` with no question. The person decided this on 27 November 2026. The cause: in this project, a worker could not reach the Claude server. GitHub also refused a merge while the checks had no result. The person said yes to each `resume`, and wants no such simple question.
- The checker compares each test file of the paper `tests` with the project in each try. A file that is different or not there gives `ok: false`. The checker writes no file. The builder in `repair` then copies the paper file again.
- The step `copy_tests` copies the paper tests before each build, because the person approved the paper. The builder does not change such a file. A run of this project showed the cause. The builder worked against a test from before a correction of the planner.
- The paper test always wins, also over a change or a removal from master. The person decided this for this project.
- `copy_tests` runs with no sandbox in the work copy. Each git call has `-c core.fsmonitor=false`, `-c core.hooksPath=/dev/null` and `-c commit.gpgSign=false`, and the commit has `--no-verify`. It refuses a config name that `take_master` refuses. The cause: a worker of a project can write that copy and the paper.
- `copy_tests` refuses a link in the paper or on a target path, and a paper file with a second hard link. It refuses a path with `.git`, `.github`, `.claude` or `.codeumbrella`. Each refusal comes before the first copy. The cause is the same: a worker of a project must not reach a file of the person.
- The default check step is a fan of five checkers. `check_tests` runs the tests. `check_safety`, `check_review` and `check_security` read the change. They cannot write a file. `check_use` runs the examples of the spec as a user does. It writes only in a scratch folder in the temp folder.
- The step `simplify` follows `build`. The simplifier changes no test file, and it commits only if the test command passes. The step `repair` does not go through it.
- The reviewer and the `security` checker have the same settings as the safety checker. The simplifier has the same settings as the builder.
- The stopped run prints `To continue: codeumbrella resume <run id>` and a line that says: resume only after the person says yes. An AI must not give this approval. A `known cause` is the exception. The limit for the note does not cut them.
- `answer` and `resume` with a `--tool` that is different from the tool of the run give an error. They give it before a step starts and before the answer is written. A notebook with no tool uses Claude, and `--tool` writes it. `status` changes the notebook only in the repair.
- The safety checker also reads the files that use the changed lines, for example settings files. A run on 17 November 2026 showed the cause.
- Four checkers work in the same copy. No checker writes a file of the project. `check_use` clones the run branch into its scratch folder.
- A run on 18 November 2026 showed this. After a no of the person, the builder changed no file. All four checkers gave a good result.
- After a no, the planner puts a part `Points of the no` at the top of `spec.md`. It is a list with numbers.
- Each point of that part is one file, one behaviour or one sentence. A point with two demands becomes two points.
- The part stays in each correction of the run. Each new no adds its points.
- Rule 1 of the reviewer gives one line for each point. It is `found` with the file and the line, or `not found`.
- One `not found` gives `ok: false`. A commit or a word of the builder is not a proof.
- The reviewer looks in the full change of the run branch. A point that a change cannot show gets `kept` or `not kept`.
- The attacker names each new paper test that is good before the code is written. Such a test shows nothing.
- The step `mark` keeps the commit of the yes for the tests. The step `work_was_done` gives a bad result with no commit after it and no `new-work.bundle`.
- `work_was_done` is good with no new commit if `new-work.bundle` is in the run folder. A worker cannot write that file.
- A run of this project showed the cause. A build with complete code stopped a run after a no.
- The check cannot see why the run came back. A bad repair or `fix_merge` that goes to the plan gets the same path.
- `mark` and `work_was_done` run git in the work copy with no sandbox. The git calls in their shell text have `-c core.fsmonitor=false`. The diff of `work_was_done` does not have it.
- A worker can write in `.git` of its work copy. So `work_was_done` is not a wall against a worker that cheats. It finds a worker that did nothing by error.
- `run --next` starts while a different run is active. The move of the queue file to the run folder gives the task to one command only. A command that gets no task ends with an error and removes its run folder. Cause: on 20 November 2026 four tasks waited in a queue, one at a time.
- `take_master` merges the newest master into the branch with a merge commit. The push stays plain. It has no rebase and no forced push. The clean copy fetches from GitHub. The work copy fetches only from the clean copy. A `filter.` entry or a merge driver in the config of the work copy gives a bad result. Cause: on 20 November 2026 two PRs of two runs could change the same file or get the same version. A worker can write that config.
- `take_master` and the version script let through only the config names of a clean clone, and `user.name` and `user.email`. A test on 21 November 2026 showed the cause. Four other entries ran a command of the work copy with no sandbox, for example `core.sshCommand`. The step `mark` has `-c core.hooksPath=/dev/null`, because it ran a hook in that test.
- `same_master` gives a good result only if the pushed branch has the newest master. Cause: on 20 November 2026 master could change while the person looked at the PR. Only `same_master` goes to `merge`.
- `work_was_done` comes before `take_master` and counts only the commits of `build` and `simplify`, not the commit of `copy_tests`. With `new-work.bundle` from before, no new commit is good. A bad result goes to `do_work`. Cause: on 20 November 2026 the commits of `set_version` and `take_master` hid a builder that did nothing.
- The end of the workers first looks at each file `worker.<number>`. Thus an error of `ps` changes no file.
- On 18 November 2026 a person ran the built command by hand with a live worker. The person found two errors that no checker found.
- Thus the role `user` runs the software as a user. Its step is `check_use` in the fan of `check`.
- For Claude, the settings of `user` are those of the attacker, character for character.
- For Codex, the file of `user` is the attacker file with one more entry: `network={enabled=true, mode="limited", allow_local_binding=true}`. Thus the software can start a server on the same computer. The network proxy keeps the internet closed.
- The user checker makes a scratch folder with `mktemp`. It clones the run branch there with `git clone`, and builds it.
- It runs each example of the part `Examples for a user` of `spec.md`. It sets `HOME` to a folder in the scratch folder for each command of the software.
- It changes no file of the project, of the work copy or of the papers. At the end, it removes only its scratch folder.
- It ends each command that it starts with `kill`, and looks with `kill -0`. The sandbox blocks `ps`, `pgrep` and `pkill`.
- An example that cannot run in the sandbox gets `Not run:`. That is not a bad result. The chat tells the person each such line at the approval.
- Only an example that ran and differs from the spec gives a bad result. The user checker does not run the test command.
- The planner writes the examples, with the names `E1`, `E2` and so on. A task with no command for a user gets `No example:` and a cause.
- The step `learn` follows `take_master`. After a no of the person, the run comes to it again. The step `repair` does not go through it.
- The learner has the settings of the simplifier and one write entry for the paper `lessons`. A test on 19 November 2026 showed that this entry works for Claude.
- The Codex learner has no entry for the temp folder. A test on 19 November 2026 showed that the sandbox of the simplifier can write there.
- The learner reads the log of this run and of each run after the line `Runs read to:`. Thus a stopped run also gives its lessons.
- A lesson test must be bad on the commit from before the repair, and good on the branch. If not, the lesson becomes an entry. The person made this decision on 19 November 2026.
- The learner keeps its copy in the temp folder and removes no file. The person cleans it up. On 19 November 2026 a removal of the attacker broke a check.
- A lesson cannot make a rule, a check, a test or a wall weaker. The person made this decision on 19 November 2026, because a log is data from a worker.
- The safety checker reads the paper `lessons`. An entry that makes a wall weaker gives a bad result. This came from the decisions of 19 November 2026.
- Only the steps `save_lessons` and `tidy_lessons` write `.codeumbrella/lessons.md`. The person made this decision on 19 November 2026.
- `save_lessons` adds the changes of the paper `lessons` with `git merge-file --union`. `tidy_lessons` removes an entry only with a strikethrough and a merged PR. The date is 25 November 2026.
- The cause: a limit removed useful entries, and a copy could remove the entries of a second run.
- `save_lessons` refuses a paper with a symbolic link or a second hard link. Thus only a file of the paper goes into the lessons file.
- The builder does not change a lesson test. The file `tests.txt` of the paper `lessons` names each lesson test.
- Each lesson test goes into a new file of its own. A file of the paper `tests` is not a lesson test, also if `tests.txt` names it.
- After the merge, the step `clean_up` removes `work` and `clean` of the run. On 19 November 2026 the run folders held 6.8 GB.
- `open_pr` opens the PR with the title of `title.txt` and the text of `body.md` of the paper `pr`. It does not use `--fill`.
- On 19 November 2026 a PR had the branch name as its title. Its text had only the commit lines.
- After a no, `find_pr` puts the new title and text of the paper `pr` into the open PR. Thus a PR has no text of an earlier try.
- The step `write_pr` comes after `set_version` and after each `repair`. Thus the PR text agrees with the change before each check.
- A bad result of `write_pr` stops the run.
- A `repair` with problems only in the paper `pr` changes no file and uses no try of `plan`. A problem in `title.txt` goes to `plan`.
- After a bad check, the writer corrects only the named sentences of parts 1 to 5. It keeps their meaning, the title, the purpose and each point of the no.
- The cause: in a run of this project, the reviewer refused two words of the PR text. The run used its last try of `plan`.
- The writer has the walls of the reviewer and one write entry for the paper `pr`. It writes no file of the work copy and commits nothing.
- In Claude settings a deny rule comes before an allow rule. So the writer has a deny rule for the work copy, not for the full project folder.
- The reviewer compares the paper `pr` with the change. One `[x]` that the change does not show gives a bad result.
- A file in part 7 that the change does not have gives a bad result. So does a file of the change that no part 7 names.
- A worker can write the paper `pr`, but `find_pr` and `open_pr` run with no sandbox.
- So their script refuses the paper `pr` if its folder or one of its two files is a symbolic link.
- It also refuses a file with two or more hard links. Thus no file of the person goes to GitHub.
- A refused paper gives a bad result. `find_pr` goes to `open_pr`, which refuses too, and the run stops.
- A task can end with `Test command:` and `Full check:` lines. The cause: a run of 19 November 2026 in a different project.
- A worker gives `Not run: full check` for the `Full check:` command. The test command stays a wall. The cause: a run of 19 November 2026 in a different project.
- The attacker names a paper test that reads `git status`. It is bad after the merge. A run of 19 November 2026 showed it.
- The chat runs the full check with no sandbox only after the person says yes. The cause: the check runs code that a worker wrote. The person decided this on 19 November 2026, after a run in a different project.
- The planner, the writer, the learner and the builder write 20 words at the maximum in a sentence. The cause: workers wrote long sentences on 19 November 2026.
- A paragraph of these four workers has 6 sentences at the maximum.
- These four rules pages also tell the workers to use the active voice and no semicolon. They name no word of a list.
- The reviewer checks the limits and the refused words of `AGENTS.md`, from 19 November 2026. The cause: three workers of one run wrote refused words.
- In the change or in the paper `pr`, such a problem gives a bad result of the reviewer.
- A problem in the paper `lessons` gives only a note of the reviewer. The chat shows that note at the question `approval`.
- No repair worker can write the paper `lessons`. So the chat corrects that paper on the word of the person.
- A break of a point of a standards page gives a bad result of the reviewer, from 25 November 2026. The step `repair` then gets the work.
- The person of this project decided this on 24 and 25 November 2026. A break was only a note, and the person does not read the code.
- The reviewer reads each point of the part `## A break`. The check is "does the change follow this point, yes or no".
- The person gave approval for the pages on 25 November 2026. So no worker writes or changes a point of a page.
- A checker that cannot run its commands starts its summary with `No check:` and names the error.
- It does not say that the code is bad.
- If each bad report of the checks starts with `No check:`, the planner changes no paper.
- The planner then gives `ok: true`, and the run goes to the build and the checks again.
- This uses a try of the plan, so an error that stays stops the run after the third try.
- Each such result also uses a try of `repair`.
- The cause: on 27 November 2026 each shell call of the checkers stopped with `ENOTSUP`.
- The planner found no paper to correct, and the run stopped.
- The attacker checks the comments and texts of the paper tests for the refused words and sentence limits.
- Such a problem gives a bad result of the attacker. The step `correct_plan` corrects it before the tests question.
- The attacker and the reviewer use one list of refused words. It stays in `AGENTS.md` only.
- A run of this project showed the cause on 25 November 2026. A refused word in a paper test stopped it.

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
| `libfuzzer-sys` | Connects a fuzz target to the fuzzer libFuzzer. It is in `fuzz/` only. The code cannot do this work without it, because the standard library has no fuzzer. |

A new package gets into this list only when all of these are correct:

1. `cargo deny check` and `cargo audit` find no problem with it.
2. Its makers changed it in the last 12 months.
3. This table gets one line that tells, in plain words, what the package does.
4. The message to the person tells why the code cannot do the work without it.

## Limits for lines of code

- The supervisor (`src/graph.rs`, `src/engine.rs`, `src/main.rs`, `src/log.rs`, `src/lib.rs`): 2000 lines of code.
- Each tool part: 200 lines of code.
- Empty lines, comment lines and tests do not count.
- Count with: `grep -v -E '^\s*(//|$)' <file> | wc -l`

## Output for a chat

An AI in a chat runs the command most of the time. Keep the output of the command short, so the chat keeps its context. The command prints one short line for each step and a note of 300 characters maximum. The full text goes into the log and the `trace` files.

The status text has one fact on each line. Each value starts at character 11. The lines are `Run:`, `Status:` and `Note:`. A run with status Running also gets `Step:` and `Since:`. Then come `Workers:`, `Time:`, `Cost:` and `No cost:`.

`More:` with the path of the log is the last line. A line with no value is not there.

- `Workers:` is the count of the workers that gave their time. Each try counts.
- `Time:` is in worker minutes. It is a sum of worker times, not the time on the clock. The workers of a fan work at the same time.
- `Cost:` is the sum of the dollars that the Claude tool writes. With a subscription, the person pays a different sum.
- `No cost:` is the count of the workers with no cost. A Codex worker gives tokens and no cost, because the Codex tool writes no cost.

### Limits of the numbers

The factory keeps each number that a tool gives. The documents tell each limit.

- A Codex worker has no cost, no turns and no model, because the Codex tool writes none.
- The dollars of a Claude worker are the number of the Claude tool. With a subscription they are not the money that the person pays.
- The factory calculates no cost from tokens, because it has no price list.
- Worker minutes are a sum. The workers of a fan work at the same time.
- A worker that a kill or the time limit ended can have no numbers.
- A run from before version 0.1.24 has no total.
- The time that a run waits for the person is not in the numbers.

## Rules for growth

1. Add a part only after a run shows that it is necessary. A part is a new type of step, a new line for the sheet, or a new package.
2. Obey the limits for lines of code.
3. Each time you add a part, look for a part to remove.
4. If a rule is too complex for the sheet, the worker makes that decision, not the supervisor.
5. Copy no part from a different project until a run shows that it is necessary.
6. One source for Claude and Codex. Only the tool parts and the settings files of the roles are different for each tool. A step can pick its tool.

The person gave approval on 19 November 2026 for one part with no run: the fuzzer in `fuzz/`.

## Not in version 1

- Two runs of a project can work at the same time, each with its own builder. A fan of builders in one run stays out of version 1. The fan of today is only for workers that write no file.
- A different worker that writes the tests. The learner writes only lesson tests, not tests for the task.
- Custom hooks.

## How to write for the person

The person who owns this project does not know Rust. Write messages and documents in short, plain sentences. Give the meaning of each technical word the first time that you use it.

### Refused words

- Words: `program`, `never`, `security`, `process`, `already`, `way`, `case`, `need`. `return`, `place`, `permit`, `now`, `real`, `exact`, `later`, `confirm`. `decide`, `true`, `false`, `few`, `leaves`, `choice`, `take`, `avoid`. `aim`, `old`, `ask`, `both`, `wrong`, `pass`, `whose`, `outside`. `still`, `missing`, `absent`, `preparation`, `rest`, `yet`, `ready`, `conflict`. `once`, `just`, `size`, `state`, `round`, `early`, `findings`, `carry`. `order`, `split`, `save`, `guard`, `among`, `under`, `separate`, `another`. `yourself`, `past`, `little`, `exist`, `lost`, `approve`, `search`, `form`. `whole`, `otherwise`, `any`, `trace`, `gap`, `zero`, `fail`, `loses`. `splits`, `harmful`.
- The workers obey this list in each text for a person.
- A word in code marks is not refused.
- Write `security` in code marks.
- Write `process` in code marks, or write `command`.
- Write `trace` in code marks.
- Write `command`, not `program`.
- This list is for this project only. Other projects write their own list, or none.
