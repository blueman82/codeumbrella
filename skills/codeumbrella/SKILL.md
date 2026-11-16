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
4. If you are Codex, put `--tool codex` immediately after the word `codeumbrella` in each command. If you are Claude, add nothing.
5. The command starts workers and uses the network. If your sandbox stops the command, get the permission of the person to run it with no sandbox.

## Before the first command

Do these steps in sequence.

1. If the computer cannot find the `codeumbrella` command, stop and tell the person.
2. If the project has no file `.codeumbrella/graph.yaml`, run `codeumbrella init`. Tell the person that the factory files are in `.codeumbrella/` and that the person can change them.
3. Run `git status --short`. If it shows changes, stop. Tell the person to commit the changes or to put them away. If not, the changes get into the work of the run.
4. Run `git remote get-url origin`. If it gives an error, stop. Tell the person that the default sheet sends the work to GitHub. The project must have a GitHub connection with the name `origin`.
5. Make sure that the packages of the project are on the computer. See "Packages".

## Packages

The workers have no internet. They cannot download a package or a tool.

Before you start a run, you must put all that the task uses into the project folder or onto the computer:

- Read `AGENTS.md`, `CLAUDE.md` and the package files of the project, for example `package.json`, `pyproject.toml` or `Cargo.toml`.
- Read the task. If the task uses a new package or a new tool, add it to the list.
- For a new project with no code, get the language and the packages from the person.
- Install the packages with the usual install command of the project. Put them in the project folder when the language permits it, for example `node_modules` or `.venv`.
- Run the test command of the project one time. This shows that the tools start with no download.
- Put that test command at the end of the task text, for example: `Test command: <command>`. The workers then use the same command.

If a run stops because a package is not on the computer, install that package. Then run `codeumbrella resume <run id>`.

## The command to run

| The person gives you | You run |
|---|---|
| A task | `codeumbrella run "<task>"` |
| No task | `codeumbrella status` |

One command can run for many minutes. Give it a time limit of 10 minutes. If it does not end in that time, run `codeumbrella status` to see where the run is.

## After each command

The output has three parts: the run id, the status, and a note.

- Status `Waiting` at the step `look_at_tests`: read the files in `.codeumbrella/runs/<run id>/papers/tests/`. Tell the person in plain words what each test looks at. Then get a yes or a no from the person.
- Status `Waiting` at the step `approval`: give the person the PR link from the note. Then get a yes or a no from the person.
- After a yes, run `codeumbrella answer <run id> yes`.
- After a no, run `codeumbrella answer <run id> no "<what the person said>"`.
- Status `Stopped`: show the note to the person. After the person corrects the cause, run `codeumbrella resume <run id>`.
- Status `Done`: tell the person that the code is in `main`.

## The log and the `trace` files

The run folder is `.codeumbrella/runs/<run id>/`.

- `log.jsonl` has one line for each step that ended. The line has the time, the step, good or bad, the seconds and the summary.
- `<step>.<number>.trace.jsonl` has all that one worker did in one try. These files are large.

When the status is `Done` or `Stopped`, read `log.jsonl`. Give the person a short table: the time of each step, the slowest step, the number of repair tries, and the answers of the person.

Do not read a full `trace` file. Use `grep` to find one thing in it, for example a command, an error or `total_cost_usd`. Do this only when the person wants to know why a step was slow or bad, or what the run cost.

Write to the person in short, plain sentences. Give the meaning of each technical word.
