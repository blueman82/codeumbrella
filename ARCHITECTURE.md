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
│ attacker  in a sandbox · gets the plan + the tests ──►  finds weak points, scratch folder only │
│ builder   in a sandbox · gets the plan             ──►  writes code on the run branch    │
│ checker   in a sandbox · gets the plan + the tests ──►  copies the tests, runs them      │
│ safety    in a sandbox · gets the plan             ──►  reads the change, writes no file │
│ simplifier in a sandbox · gets the plan            ──►  makes the new code simpler       │
│ reviewer  in a sandbox · gets the plan             ──►  reads the change for errors      │
│ security  in a sandbox · gets the plan             ──►  reads the change, security       │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

- The plan paper has the name `spec`.
- The reviewer and the simplifier read the language pages in `roles/languages/` for each language in the change. No other role reads them.
- The builder, the checker and the simplifier can connect to a server on the same computer, for example a database.
- A worker can read the tests paper, but it cannot change it. The builder and the checker cannot write to the papers folder.
- The supervisor compares each paper with its fingerprint before each step.
- The workers write in a copy of the project in the `work` folder of the run. A run does not change the project folder or its branch.
- After status Done, the person gets the code with `git pull`.
- A worker can write to the `.git` folder of its copy. Thus the supervisor gets the work into a clean copy before the push.
- A Claude worker uses only the settings of its role, not the settings of the project.

## The path of a task

```
plan (3 tries) ◄─────────────────────────────┬──────────┐
  ▼                                          │          │
attack_plan: the attacker looks for weak     │          │
points, 3 times at the maximum               │          │
  ├── a problem ──► correct_plan ──► attack  │          │
  │                 again, or after the      │          │
  ▼ none            third time: go on ──┐    │          │
YOU: tests? ── no ───────────────────────────┘          │ no
  ▼ yes                                                 │
mark: git keeps the commit of your yes (no AI)          │
  ▼                                                     │
build: a bad result goes to plan, as a no does           │
  ▼                                                     │
simplify: the simplifier makes the code simpler         │
  ▼                                                     │
check: a FAN of 4 checkers at the same time             │
  ├──► check_tests    runs the approved tests           │
  ├──► check_safety   reads the change for problems     │
  ├──► check_review   reads the change for errors       │
  └──► check_security reads the change for security     │
  │                                                     │
  ├── one of them bad ──► repair (3 tries) ──► check    │
  ▼ all good                                            │
work_was_done: a commit after mark? no ──► repair       │
  ▼ yes                                                 │
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
- `resume` tries a stopped step again. The person must say yes first. After a changed paper, a person must put the paper back first.
- The plan step has 3 tries for one run, and the first plan uses one. A no or a bad build uses one more. The third bad build stops the run.
- A bad build goes to `plan`. The planner corrects only the bad paper test. The attacker looks, you see the tests again, and `build` runs again.
- A bad result of the attacker goes to a step `correct_plan`. The planner corrects the plan, and the attacker looks again.
- There are 3 attacks at the maximum for each plan. After the third correction, the person sees the tests.
- The six steps go in one line, with no path back. Thus the two workers cannot go in a circle.
- You can remove the six attack steps for a faster run.
- The picture shows the default sheet, `graph.example.yaml`. A project can change its own sheet.
- The run keeps the tool of the run in its notebook. `answer` and `resume` use it.
- The step `mark` writes the commit of your yes for the tests into a git name. It has no AI.
- The step `work_was_done` comes after a good check. With no commit after `mark`, git gives an error. The run then goes to `repair`.
- A worker can write in `.git`, so `work_was_done` is not a wall against a worker that cheats. It finds a worker that did nothing by error.
- The step `simplify` and the two new checkers are for new runs. You can remove them for a faster run.
