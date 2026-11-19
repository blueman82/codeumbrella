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

1. If the computer cannot find the `codeumbrella` command, try `~/.cargo/bin/codeumbrella`. If that file is not there, stop and tell the person.
2. If the project has no file `.codeumbrella/graph.yaml`, run `codeumbrella init`. Tell the person that the factory files are in `.codeumbrella/` and that the person can change them. Then run `git status --short`. If it shows a new file `.gitignore`, tell the person and commit that file.
3. Run `git status --short`. If it shows changes, stop. Tell the person to commit the changes or to put them away. If not, the changes get into the work of the run.
4. Run `git remote get-url origin`. If it gives an error, stop. Tell the person that the default sheet sends the work to GitHub, and that the project has no GitHub connection. Offer to make a private GitHub project. After a yes from the person, run `gh repo create <name of the project folder> --private --source . --remote origin --push`. Do not make a public project.
5. Make sure that the packages of the project are on the computer. See "Packages". A tool in the project folder, for example in `.venv`, needs its full path in the test command.

## Packages

The workers have no internet. They cannot download a package or a tool.

Before you start a run, you must put all that the task uses into the project folder or onto the computer:

- Read `AGENTS.md`, `CLAUDE.md` and the package files of the project, for example `package.json`, `pyproject.toml` or `Cargo.toml`.
- Read the task. If the task uses a new package or a new tool, add it to the list.
- For a new project with no code, get the language and the packages from the person.
- Install the packages with the usual install command of the project. Put them in the project folder when the language permits it, for example `node_modules` or `.venv`.
- Run the test command of the project one time. This shows that the tools start with no download.
- Put that test command at the end of the task text, for example: `Test command: <command>`. The workers then use the same command.

If a run stops because a package is not on the computer, install that package. Then run `codeumbrella resume <run id>` only after the person says yes.

## The command to run

| The person gives you | You run |
|---|---|
| A task | `codeumbrella run "<task>"` |
| No task | `codeumbrella status` |

A `run`, `answer` or `resume` command can run for 15 to 25 minutes. A shell of the chat can end, or a time limit of the chat can stop it. The command then stops, and the file `lock` stays. So start each of these commands so that it continues after the shell ends and after a time limit. Send the output to a file. For a usual shell:

`nohup codeumbrella run "<task>" > .codeumbrella/out.txt 2>&1 &`

Use the same command line for `answer` and `resume`, with no `--tool`. Then run `codeumbrella status --wait <run id>`, not `status`. It waits until the status is not `Running`. Run it again each 3 minutes if your shell has a time limit. Read the output file after that.

`codeumbrella --step-minutes <number> run|answer|resume` gives a long check more time: "The maximum number of minutes for one step", default 30.

`status --wait` repairs a run with a dead command and shows Stopped. Tell the person. Then run `codeumbrella resume <run id>` only after the person says yes. Do not remove the file `lock` by hand.

## After each command

The output has three parts: the run id, the status, and a note.

- Status `Waiting` at the step `look_at_tests`: read the files in `.codeumbrella/runs/<run id>/papers/tests/`. Tell the person in plain words what each test looks at. Also read `.codeumbrella/runs/<run id>/papers/spec/spec.md`. Tell the person each decision in the list at its top. If the spec has no such list, tell the person that the planner made no decision. Then get a yes or a no from the person.
- If the note at `look_at_tests` names a bad build, tell the person what test the planner changed.
- At `look_at_tests`, the result before can start with `Not complete:`. Then tell the person that no worker ran that check on the paper tests. Offer to run that check yourself before the person answers.
- After a no, at the next `look_at_tests`, read the part `Points of the no` at the top of `spec.md`. Tell the person each point. Thus the person sees that each point of the no is there.
- Status `Waiting` at the step `approval`: give the person the PR link from the note. Then get a yes or a no from the person.
- At the question `approval`, read the lines of `check_use` in `log.jsonl` of the run. Tell the person each `Not run:` line before the person answers. Such an example did not run in the sandbox.
- Before you send an answer, tell the person what it does. A yes continues the run. A no at `look_at_tests` sends the work back to the planner. A no at `approval` sends the work back to the planner. The person then sees the tests again. The PR stays open.
- If the person gives you corrections, that is a no with a text. Send the full text of the person.
- An answer names the step of the question: `look_at_tests` or `approval`. The note of the run gives the step.
- After a yes, run `codeumbrella answer <run id> <step> yes`.
- After a no, run `codeumbrella answer <run id> <step> no "<what the person said>"`.
- If the command says that a different command works on the run, do not send the answer again. Run `codeumbrella status` and tell the person.
- Status `Stopped`: show the note to the person. After the person corrects the cause, run `codeumbrella resume <run id>` only after the person says yes.
- If the note says that a paper is not as its writer left it, `resume` does not help. A person must put the paper back first, or start a new run.
- If the step `no_locked_files` stopped the run, the work changes `.github`, `.claude` or `.codeumbrella`. Show this to the person. Do not go around it.
- Status `Done`: tell the person that the merge of the PR is complete. Tell the person to run `git pull`. The workers wrote in a copy, so the project branch did not change.

## The log and the `trace` files

The run folder is `.codeumbrella/runs/<run id>/`.

- `log.jsonl` has one line for each step that ended. The line has the time, the step, good or bad, the seconds and the summary.
- `<step>.<number>.trace.jsonl` has all that one worker did in one try. These files are large.

When the status is `Done` or `Stopped`, read `log.jsonl`. Give the person a short table: the time of each step, the slowest step, the number of repair tries, and the answers of the person.

Do not read a full `trace` file. Use `grep` to find one thing in it, for example a command, an error or `total_cost_usd`. Do this only when the person wants to know why a step was slow or bad, or what the run cost.

Write to the person in short, plain sentences. Give the meaning of each technical word.
