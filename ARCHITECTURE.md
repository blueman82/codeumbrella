# Architecture

This page shows the parts of codeumbrella and the path of a task. The rules are in `AGENTS.md`. Its table "Words" gives the meaning of each word in the pictures.

## The parts

```
YOU ── in a terminal, or through an AI in a chat
 │
 ▼
SUPERVISOR ── uses your GitHub key ──► GITHUB: push · PR · merge (merge only after your yes)
 │  reads the sheet · keeps the notebook, the log, a fingerprint of each paper
 │  starts one worker at a time
 ▼
┌─ WORKERS: no internet, thus no use of your GitHub key · no write to the factory files ─┐
│ planner   writes the PAPERS: plan + tests. Claude: no shell. Codex: a sandbox.         │
│ builder   in a sandbox · gets the plan             ──►  writes code on the run branch  │
│ checker   in a sandbox · gets the plan + the tests ──►  copies the tests, runs them    │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

- The plan paper has the name `spec`.
- The builder and the checker can connect to a server on the same computer, for example a database.
- A worker can read the tests paper, but it cannot change it. The papers folder is locked for the builder and the checker.
- The supervisor compares each paper with its fingerprint before each step.

## The path of a task

```
plan (3 tries) ◄──────────────┬───────────┐
  ▼                           │ no        │ no
YOU: tests? ──────────────────┘           │
  ▼ yes                                   │
build                                     │
  ▼                                       │
check ──bad──► repair (3 tries) ──► check │
  ▼ good                                  │
no change in a locked folder?             │
  ▼ yes                                   │
push ──► find the PR, or open one         │
  ▼                                       │
YOU: PR? ─────────────────────────────────┘
  ▼ yes
merge

At each step, four things STOP the run and tell you:
an error · a bad result with no path for it · a changed paper · a maximum
```

- `resume` tries a stopped step again. After a changed paper, a person must put the paper back first.
- The plan step has 3 tries for one run, and the first plan uses one. Thus 2 no answers are possible.
- The picture shows the default sheet, `graph.example.yaml`. A project can change its own sheet.
