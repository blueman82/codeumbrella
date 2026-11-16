# Architecture

This page shows the parts of codeumbrella and the path of a task. The rules are in `AGENTS.md`.

## The parts

```
YOU ── in a terminal, or through an AI in a chat
 │
 ▼
SUPERVISOR ── has your GitHub key ──► GITHUB: push · PR · merge (merge only after your yes)
 │  reads the sheet · keeps the notebook, the log, a fingerprint of each paper
 │  starts one worker at a time
 ▼
 planner   no shell          ──writes──►  PAPERS: plan + tests
┌─ SANDBOX: no internet · no GitHub key · no write to the factory files ─┐
│ builder   reads the plan only        ──►  writes code on the run branch │
│ checker   reads the plan + the tests ──►  copies the tests, runs them   │
└──────────────────────────────────────────────────────────────────────────┘
```

- The supervisor is the `codeumbrella` command. It knows nothing about software work.
- The sheet tells the supervisor which step comes next.
- A paper is a folder that one step writes and other steps read. The plan paper has the name `spec`.
- The builder does not read the tests paper. The checker copies the tests from the paper each time.
- Thus the builder cannot change the tests that the person gave approval for.
- Only the supervisor has the GitHub key. Thus no worker can push, open a PR or merge.

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
push ──► find the PR, or open one         │
  ▼                                       │
YOU: PR? ─────────────────────────────────┘
  ▼ yes
merge

At each step: an error, a changed paper, or a maximum STOPS the run and tells you. `resume` continues.
```

- A no at the tests or at the PR sends the work to the planner. The person then sees the tests again.
- Each no uses one of the 3 tries of the plan step.
- The picture shows the default sheet, `graph.example.yaml`. A project can change its own sheet.
