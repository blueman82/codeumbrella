---
name: codeumbrella
description: Give a task to the codeumbrella software factory, show its questions to the person, and send the answers of the person. Use when the person types /codeumbrella or tells you to give a task to codeumbrella.
---

# Codeumbrella

Codeumbrella is a terminal command. It does one task with workers, and it stops two times for the person.

You are the window between the person and that command. You do not do the task.

## Rules

1. Do not answer a question of the run. Only the person says yes or no.
2. Do not change the code or the folder `.codeumbrella` during a run. But you can change a paper while the run waits for the person. Do this only if the person tells you to. Do it before you send the answer.
3. Do not push and do not merge. The command does that after the yes of the person.
4. If you are Codex, put `--tool codex` immediately after the word `codeumbrella` in the command `run` only. The run keeps its tool, so `answer` and `resume` need no `--tool`. If you are Claude, add nothing.
5. The command starts workers and uses the network. If your sandbox stops the command, get the permission of the person to run it with no sandbox.

6. Run `codeumbrella stop <run id>` only when the person tells you to stop the run. An AI does not stop a run by its own decision. Run it with no sandbox.

## Before the first command

Do these steps in sequence.

1. Run `test -d .git`. If it gives an error and a file `.git` is there, the folder is a linked git worktree. Then stop. Tell the person that the factory does not work in a linked git worktree. A full copy made with `git clone` works.
2. If the computer cannot find the `codeumbrella` command, try `~/.cargo/bin/codeumbrella`. If that file is not there, stop and tell the person.
3. If the project has no file `.codeumbrella/graph.yaml`, run `codeumbrella init`. Tell the person that the factory files are in `.codeumbrella/` and that the person can change them. Then run `git status --short`. If it shows a new file `.gitignore`, tell the person and commit that file.
4. Run `git status --short`. If it shows changes, stop. Tell the person to commit the changes or to put them away. If not, the changes get into the work of the run.
5. Run `git remote get-url origin`. If it gives an error, stop. Tell the person that the default sheet sends the work to GitHub, and that the project has no GitHub connection. Offer to make a private GitHub project. After a yes from the person, run `gh repo create <name of the project folder> --private --source . --remote origin --push`. Do not make a public project.
6. Run `codeumbrella status`. The error "there are no runs in this folder" means no run, so go on. If the newest run has the status `Running` or `Waiting`, tell the person the run id and its status. Start the new run only after the person says to start it. A task from the queue starts with `run --next` while a run is active.
7. Run `gh pr list`. If a PR is open, tell the person its number and its title before the start. This step stops nothing. The person can say to start. If `gh pr list` gives an error, tell the person and go on.
8. Make sure that the packages of the project are on the computer. See "Packages". A tool in the project folder, for example in `.venv`, needs its full path in the test command.

## Packages

The workers have no internet. They cannot download a package or a tool.

Before you start a run, you must put all that the task uses into the project folder or onto the computer:

- Read `AGENTS.md`, `CLAUDE.md` and the package files of the project, for example `package.json`, `pyproject.toml` or `Cargo.toml`.
- Read the task. If the task uses a new package or a new tool, add it to the list.
- For a new project with no code, get the language and the packages from the person.
- Install the packages with the usual install command of the project. Put them in the project folder when the language permits it, for example `node_modules` or `.venv`.
- Run the test command of the project one time. This shows that the tools start with no download.
- Put that test command at the end of the task text, for example: `Test command: <command>`. The workers then use the same command.
- A number in a task text is a limit only if the task names it as a limit. A run on 18 November 2026 made a goal for lines of code into a hard rule. The run stopped.
- Before the first run of a project, tell the person this. The workers run the test command in a sandbox, in a clone of the project. A tool of the project folder, a git hook and a second sandbox do not work there.
- The full check of a project can be bad in that sandbox. Then the task text gets two lines at its end. `Test command: <command>` is a command that works in the sandbox. `Full check: <command>` is the check that does not work there.
- Run the `Test command:` one time before the run, as above. Do not add a `Full check:` line if the full check works as the test command.

If a run stops because a package is not on the computer, install that package. Then run `codeumbrella resume <run id>` only after the person says yes.

## The command to run

| The person gives you | You run |
|---|---|
| A task | `codeumbrella run "<task>"` |
| A task, while a run of the project is `Running` or `Waiting` | Write the task as a file in `.codeumbrella/queue/`, for example `01-name.txt`. Add a line `Files: a, b` if you know the files. Tell the person the ids of the active runs first. Then run `codeumbrella run --next`. |
| The next task of the queue | `codeumbrella run --next` |
| No task | `codeumbrella status` |

A `run`, `answer` or `resume` command can run longer than the time limit of a chat shell. A shell of the chat can end, or a time limit of the chat can stop it. The command then stops, and the file `lock` stays. So start each of these commands so that it continues after the shell ends and after a time limit. Send the output to a file. For a usual shell:

`nohup codeumbrella run "<task>" > .codeumbrella/out-<time>.txt 2>&1 &`

Pick a new file name for each `run` command, for example with the time. The chat does not know the run id before the start. Read the run id from the line `Run:` in that file. Two runs then write two files.

For `answer` and `resume`, use `nohup codeumbrella answer ... > .codeumbrella/out-<run id>.txt 2>&1 &`, with no `--tool`. Then run `codeumbrella status --wait <run id>`, not `status`. It waits until the status is not `Running`. Run it again each 3 minutes if your shell has a time limit. Read the output file after that.

`codeumbrella --step-minutes <number> run|answer|resume` gives a long check more time: "The maximum number of minutes for one step", default 30.

`status --wait` repairs a run with a dead command and shows Stopped. Tell the person. Then run `codeumbrella resume <run id>` only after the person says yes. Do not remove the file `lock` by hand.

## After each command

The output has one fact on each line: `Run:`, `Status:`, `Note:` and more. A run with status `Running` also shows `Step:` and `Since:`. The lines `Workers:`, `Time:` and `Cost:` give the total of the workers. `More:` gives the path of the log.

- Tell the person the lines `Workers:`, `Time:` and `Cost:`. Do this at each question, and at the status `Done` or `Stopped`.
- Also tell each limit of the numbers that fits the run. `AGENTS.md` of the factory has the list "Limits of the numbers". Example: a run with Codex workers has no full cost. The line `No cost:` shows such workers.
- `Time:` is in worker minutes. It is a sum of worker times, not the time on the clock. The workers of a fan work at the same time.
- `Cost:` is the number that the Claude tool writes. With a subscription, the person pays a different sum.
- Do not add the numbers of the worker files by hand. Do not tell the person a time or a cost that no file of the run shows.

- At the tests question, before your summary, give the person the full path of `spec.md`. Also give the full path of each test file of the paper `tests`.
- Status `Waiting` at the step `look_at_tests`: read the files in `.codeumbrella/runs/<run id>/papers/tests/`. Tell the person in plain words what each test looks at. Also read `.codeumbrella/runs/<run id>/papers/spec/spec.md`. Tell the person each decision in the list at its top. If the spec has no such list, tell the person that the planner made no decision. Then get a yes or a no from the person.
- If the note at `look_at_tests` names a bad build, tell the person what test the planner changed.
- At `look_at_tests`, the result before can start with `Not complete:`. Then tell the person that no worker ran that check on the paper tests. Offer to run that check before the person answers.
- After a no, at the next `look_at_tests`, read the part `Points of the no` at the top of `spec.md`. Tell the person each point. Thus the person sees that each point of the no is there.
- Status `Waiting` at the step `approval`: give the person the PR link from the note. Then get a yes or a no from the person.
- At `approval`, tell the person that a new master can bring the PR question a second time.
- At the question `approval`, read `.codeumbrella/runs/<run id>/papers/pr/title.txt` and `body.md` of the same paper. Show the person the title from `title.txt`. Show each `[ ]` line of the checklist in `body.md`, with its cause. If no line has `[ ]`, tell the person that.
- At the question `approval`, read the `look_at_tests` lines of `log.jsonl` that start with `No tests question.`. Each such line names a paper test that changed with no tests question. Tell the person each such test change in one line.
- At the question `approval`, read the lines of `check_use` in `log.jsonl` of the run. Tell the person each `Not run:` line before the person answers. Such an example did not run in the sandbox.
- At the question `approval`, read the task of the run in `state.json` of the run folder. If the task has a line `Full check:`, tell the person that no worker ran that check.
- Then offer to run the full check on the run branch, with no sandbox. Run it only after the person says yes. The cause: the check runs code that a worker wrote, with no sandbox.
- Run the full check in a new clone of the run branch from GitHub, in the temp folder. Do not run it in the project folder. Tell the person the result before the person answers.
- A bad full check is not an answer. The person can say no with the text of the result. That no sends the work back to the planner.
- At the question `approval`, compare `.codeumbrella/runs/<run id>/papers/lessons/lessons.md` with `.codeumbrella/lessons.md` of the project. Show the person the change of the paper `lessons`. Also tell the person if the PR adds a lesson test. The file `tests.txt` of that paper names each lesson test.
- Before you send an answer, tell the person what it does. A yes continues the run. A no at `look_at_tests` sends the work back to the planner. A no at `approval` sends the work back to the planner. The person then sees the tests again. The PR stays open.
- If the person gives you corrections, that is a no with a text. Send the full text of the person.
- An answer names the step of the question: `look_at_tests` or `approval`. The note of the run gives the step.
- After a yes, run `codeumbrella answer <run id> <step> yes`.
- After a no, run `codeumbrella answer <run id> <step> no "<what the person said>"`.
- If the command says that a different command works on the run, do not send the answer again. Run `codeumbrella status` and tell the person.
- Status `Stopped`: show the note to the person. The one exception is the rule in "A stop with a rule". After the person corrects the cause, run `codeumbrella resume <run id>` only after the person says yes.
- If the note says that a paper is not as its writer left it, `resume` does not help. A person must put the paper back first, or start a new run.
- If the step `open_pr` stopped the run after a bad `find_pr`, read the note. A paper `pr` with a link is refused, so only the files of the paper go to GitHub. Show this to the person.
- If the step `no_locked_files` stopped the run, the work changes `.github`, `.claude` or `.codeumbrella`. Show this to the person. Do not go around it.
- Status `Done`: tell the person that the merge of the PR is complete. Tell the person to run `git pull`. The workers wrote in a copy, so the project branch did not change. The step `clean_up` removed the copies `work` and `clean`. The record stays: `state.json`, `log.jsonl` and the papers.

## The log and the `trace` files

The run folder is `.codeumbrella/runs/<run id>/`.

- `log.jsonl` has one line for each step that ended. The line has the time, the step, good or bad, the seconds and the summary. The line of a worker step also has the numbers of its worker, for example `worker_seconds` and `dollars`.
- `<step>.<number>.trace.jsonl` has all that one worker did in one try. These files are large.

## The queue

- After status `Done` of a run, run `codeumbrella queue`. If the queue has a task, start the next one with `run --next` (as you start `run`, with `nohup`). Tell the person which task started. You start 3 tasks one after the other at the maximum, then you wait for the person.
- The new run stops at `look_at_tests`, as each run does. Do not answer it. If the queue is empty, say nothing more.
- Count only the tasks that you start without a word of the person. A task that the person tells you to start does not count. A word of the person to go on sets the count to 0. A yes to a tests question or to a PR question does not. Each such yes is for one run only.
- At the stop after 3 tasks, if the queue has tasks, tell the person each one. Give its file name, its first line and its line `Purpose:`.
- Before you start a task of the queue, note its file name.

## A stop with a rule

A plan step is `plan`, `correct_plan`, `correct_plan_2` or `correct_plan_3`. A run can stop at a plan step with `no paper to correct` in the note.

- The rule counts only if the run branch has commits after `mark`. In the work copy of the run, run `git -c core.fsmonitor=false -c core.hooksPath=/dev/null rev-list --count refs/codeumbrella/before..HEAD`. A number of 1 or more means such commits.
- For a stop with no paper to correct, copy the `task.txt` of the run back into `.codeumbrella/queue/`, with the file name of before. Then start it with `run --next`, as a new run from master. This start counts as one of the 3 tasks.
- Also write the cause of the stop as a new task file at the end of the queue. The file has a title, `Purpose:`, `Cause:`, `Example:`, `Files:` and `Test command:`.
- At a count of 3, put the two files into the queue and start nothing.
- A task gets this rule for no paper to correct one time only. A second such stop goes to the person. Look in the run folders for a different run with the same `task.txt` and the same stop.
- The stop goes to the person if the run has no `task.txt`. It also goes to the person if you do not know the file name of before.
- If a different file is first in the queue, start nothing and tell the person.
- Each other stop goes to the person. Run `resume` only after a yes of the person.
- Tell the person each action of a rule in one line, at the next question. If a PR of the stopped run is open, that line names it.

When the status is `Done` or `Stopped`, read `log.jsonl`. Give the person a short table: the time of each step, the slowest step, the number of repair tries, and the answers of the person.

Do not read a full `trace` file. Use `grep` to find one thing in it, for example a command or an error. Do this only when the person wants to know why a step was slow or bad. For the cost of the run, use the line `Cost:` of the status.

Write to the person in short, plain sentences. Give the meaning of each technical word.
