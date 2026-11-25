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
│ planner   writes the PAPERS: plan + tests + PR. Claude: no shell. Codex: a sandbox.      │
│ attacker  in a sandbox · gets the plan + the tests ──►  finds weak points, scratch folder only │
│ builder   in a sandbox · gets the plan             ──►  writes code on the run branch    │
│ checker   in a sandbox · gets the plan + the tests ──►  compares the tests, runs them    │
│ safety    in a sandbox · gets the plan             ──►  reads the change, writes no file │
│ simplifier in a sandbox · gets the plan            ──►  makes the new code simpler       │
│ reviewer  in a sandbox · gets the plan             ──►  reads the change for errors      │
│ security  in a sandbox · gets the plan             ──►  reads the change, security       │
│ user      in a sandbox · gets the plan             ──►  runs the examples, scratch only  │
│ learner   in a sandbox · gets the plan             ──►  writes lessons, lesson tests     │
│ writer    in a sandbox · gets the plan + the PR    ──►  completes the PR text only       │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

- The plan paper has the name `spec`.
- The PR paper has the name `pr`. It has the title of the PR in `title.txt` and the PR text in `body.md`.
- The planner writes the title and parts 1 to 5 of the PR text. The writer adds parts 6 to 8: the version line, the areas of the change, and the checklist.
- The reviewer and the simplifier read the language pages in `roles/languages/` for each language in the change. No other role reads them.
- The builder, the checker, the simplifier and the user checker can connect to a server on the same computer. An example is a database.
- A worker can read the tests paper, but it cannot change it. The builder and the checker cannot write to the papers folder.
- The supervisor compares each paper with its fingerprint before each step.
- The workers write in a copy of the project in the `work` folder of the run. A run does not change the project folder or its branch.
- After status Done, the person gets the code with `git pull`. The run folder then has no `work` and no `clean` copy.
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
copy_tests: copies the paper tests, commits (no AI)     │
  ▼                                                     │
build: a bad result goes to plan, as a no does          │
  ▼                                                     │
simplify: the simplifier makes the code simpler         │
  ▼                                                     │
work_was_done: a new commit? no ──► do_work             │
  ▼ yes (or after do_work)                              │
take_master: merge the newest master (no AI)            │
  ├── a bad merge ──► fix_merge ──► take_master         │
  ▼ good (3 tries)                                      │
learn: the learner reads the logs, writes the lessons   │
  ▼                                                     │
set_version: the script of your project sets the        │
version and commits it (no AI)                          │
  ▼                                                     │
write_pr: the writer completes the PR text              │
  ▼                                                     │
check: a FAN of 5 checkers at the same time             │
  ├──► check_tests    runs the approved tests           │
  ├──► check_safety   reads the change for problems     │
  ├──► check_review   reads the change for errors       │
  ├──► check_security reads the change for security     │
  └──► check_use      runs the examples as a user       │
  │                                                     │
  ├── one of them bad ──► repair (3 tries) ──► write_pr │
  │                   a bad repair goes to plan         │
  ▼ all good                                            │
no change in a locked folder? (clean copy)              │
  ▼ yes                                                 │
push ──► find the PR, give it the new text, or open one │
  ▼                                                     │
YOU: PR? ───────────────────────────────────────────────┘
  ▼ yes
same_master: the branch has the newest master? no ──►   │
  take_master, then the PR question again               │
  ▼ yes                                                 │
merge
  ▼
save_lessons: copies the lessons into your project (no AI)
  ▼
clean_up: removes the two copies work and clean (no AI)

The queue: the folder .codeumbrella/queue/ holds the tasks that wait.
After Done, `codeumbrella run --next` starts the first one and stops
at look_at_tests, as each run does.

At each step, four things STOP the run and tell you:
an error · a bad result with no path for it · a changed paper · a maximum
```

- A fan is a step that starts two or more worker steps at the same time. It waits for all of them.
- `resume` tries a stopped step again. The person must say yes first. After a changed paper, a person must put the paper back first.
- The plan step has 3 tries for one run, and the first plan uses one. A no, a bad build or a bad repair uses one more. After the third try the run stops.
- A bad build goes to `plan`. The planner corrects only the bad paper test. The attacker looks, you see the tests again, and `build` runs again.
- A bad repair also goes to `plan`, on the same path. After the third try of `repair`, the run stops.
- A bad result of the attacker goes to a step `correct_plan`. The planner corrects the plan, and the attacker looks again.
- There are 3 attacks at the maximum for each plan. After the third correction, the person sees the tests.
- The six steps go in one line, with no path back. Thus the two workers cannot go in a circle.
- You can remove the six attack steps for a faster run.
- The picture shows the default sheet, `graph.example.yaml`. A project can change its own sheet.
- The run keeps the tool of the run in its notebook. `answer` and `resume` use it.
- The notebook also keeps the total of the numbers of the workers. `status` shows it.
- The queue is the folder `.codeumbrella/queue/`: one text file for each task that waits. `codeumbrella queue` shows it. `codeumbrella run --next` starts the first file and moves it into the run as `task.txt`. The supervisor does not start a run by itself.
- The step `set_version` comes after `learn` and before `write_pr`. It runs the script `.codeumbrella/set-version` on the work copy. The script sets the version and commits it. No task text names a version. `work_was_done` also writes `files.txt`, the names of the changed files.
- The step `mark` writes the commit of your yes for the tests into a git name. It has no AI.
- The step `copy_tests` copies the paper tests into the work copy and commits them. Then it moves that git name to its commit. It has no AI.
- A worker that runs a second time in a run gets the list of the papers that changed since its last run.
- The step `work_was_done` comes right after `simplify`. With no commit of `build` or `simplify` after `copy_tests`, git gives an error. The run then goes to `do_work` and back to this check. After the third try of `do_work` the run stops.
- A worker can write in `.git`, so `work_was_done` is not a wall against a worker that cheats. It finds a worker that did nothing by error.
- The step `simplify` and the three new checkers are for new runs. You can remove them for a faster run.
- The step `take_master` merges the newest master into the branch before `learn`. Two runs of a project can work at the same time. The step `same_master` before `merge` sends the run back to `take_master` if master changed.
- The step `learn` comes after `take_master`, and after each no it runs again. The step `repair` goes to `write_pr` with no `learn`.
- The step `write_pr` comes after `set_version` and after each `repair`. Then the PR text agrees with the change before each check. A bad result of the writer stops the run.
- The reviewer compares the paper `pr` with the change. One `[x]` of the checklist that the change does not show is a bad result.
- `open_pr` opens the PR with the title and the text of the paper `pr`. After a no, `find_pr` puts the new title and text into the open PR.
- `find_pr` and `open_pr` refuse a paper `pr` with a link. Then only the files of the paper go to GitHub.
- The learner writes a lesson test on the run branch, or a short entry in the paper `lessons`.
- After the merge, the step `save_lessons` copies that paper to `.codeumbrella/lessons.md`. Git does not see that file.
- Then the step `clean_up` removes the two copies `work` and `clean`. The notebook, the log and the papers stay.
- A run that stops before the merge keeps its copies, because `resume` uses them.
- For a faster run, remove four parts together. They are the step `learn`, `lessons` in `check_safety`, `lessons` in `check_review`, and `next: save_lessons` of `merge`. Then the run ends after `merge`, and the copies stay.
- `check_use` clones the run branch into a scratch folder in the temp folder. It builds the work there and runs the examples of the spec.
