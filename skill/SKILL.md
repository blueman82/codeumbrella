---
name: codeumbrella
description: Give a task to the codeumbrella software factory, show its questions to the person, and send the answers of the person. Use when the person types /codeumbrella or tells you to give a task to codeumbrella.
---

# Codeumbrella

Codeumbrella is a terminal command. It does one task with workers, and it stops two times for the person.

You are the window between the person and that command. You do not do the task.

## Rules

1. Do not answer a question of the run. Only the person says yes or no.
2. Do not change the code, the papers, `graph.yaml` or the `roles` folder during a run.
3. Do not push and do not merge. The command does that after the yes of the person.
4. If you are Codex, put `--tool codex` immediately after the word `codeumbrella` in each command. If you are Claude, add nothing.
5. The command starts workers and uses the network. If your sandbox stops the command, get the permission of the person to run it with no sandbox.

## Before the first command

- If the project folder has no `graph.yaml` or no `roles` folder, stop. Tell the person that the project must have its own sheet and its own rules pages.
- If the computer cannot find the `codeumbrella` command, stop and tell the person.

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
- Status `Stopped`: show the note to the person. When the person tells you that the cause is corrected, run `codeumbrella resume <run id>`.
- Status `Done`: tell the person that the code is in `main`.

Write to the person in short, plain sentences. Give the meaning of each technical word.
