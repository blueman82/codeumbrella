# Architecture

This page shows the parts of codeumbrella and the path of a task. The rules are in `AGENTS.md`. Its table "Words" gives the meaning of each word in the pictures.

## The parts

```
YOU ── in a terminal, or through an AI in a chat
 │
 ▼
SUPERVISOR ── uses your GitHub key ──► GITHUB: push · PR · merge (merge only after your yes)
 │  reads the sheet · keeps the notebook, the log, a fingerprint of each paper
 │  compares and pushes the work from a CLEAN COPY that no worker can write to
 │  starts one worker at a time, or the workers of one fan at the same time
 │  one command for each run at a time (the lock)
 ▼
┌─ WORKERS: no internet · no read of your key folders · no write to the factory files ─────┐
│ planner   writes the PAPERS: plan + tests. Claude: no shell. Codex: a sandbox.           │
│ attacker  in a sandbox · gets the plan + the tests ──►  finds weak points, writes no file │
│ builder   in a sandbox · gets the plan             ──►  writes code on the run branch    │
│ checker   in a sandbox · gets the plan + the tests ──►  copies the tests, runs them      │
│ safety    in a sandbox · gets the plan             ──►  reads the change, writes no file │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

- The plan paper has the name `spec`.
- The builder and the checker can connect to a server on the same computer, for example a database.
- A worker can read the tests paper, but it cannot change it. The papers folder is locked for the builder and the checker.
- The supervisor compares each paper with its fingerprint before each step.
- The workers write in a copy of the project in the `work` folder of the run. A run does not change the project folder or its branch.
- After status Done, the person gets the code with `git pull`.
- A worker can write to the `.git` folder of its copy. Thus the supervisor gets the work into a clean copy before the push.
- A Claude worker uses only the settings of its role, not the settings of the project.

## The path of a task

```
plan (5 tries) ◄─────────────────────────────┬──────────┐
  ▼                                          │          │
attack_plan: the attacker looks for weak     │          │
points ── a problem ─────────────────────────┤          │
  ▼ none                                     │          │
YOU: tests? ── no ───────────────────────────┘          │ no
  ▼ yes                                                 │
build                                                   │
  ▼                                                     │
check: a FAN of 2 checkers at the same time             │
  ├──► check_tests    runs the approved tests           │
  └──► check_safety   reads the change for problems     │
  │                                                     │
  ├── one of them bad ──► repair (3 tries) ──► check    │
  ▼ all good                                            │
no change in a locked folder? (clean copy)              │
  ▼ yes                                                 │
push ──► find the PR, or open one                       │
  ▼                                                     │
YOU: PR? ───────────────────────────────────────────────┘
  ▼ yes
merge

At each step, four things STOP the run and tell you:
an error · a bad result with no path for it · a changed paper · a maximum
```

- A fan is a step that starts two or more worker steps at the same time. It waits for all of them.
- `resume` tries a stopped step again. After a changed paper, a person must put the paper back first.
- The plan step has 5 tries for one run, and the first plan uses one.
- A bad result of the attacker also uses one try of the plan step.
- You can remove the step `attack_plan` for a faster run.
- The picture shows the default sheet, `graph.example.yaml`. A project can change its own sheet.
