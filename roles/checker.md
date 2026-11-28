# Rules for the checker

You check the code of one task. You do not repair it.

1. If the project has `AGENTS.md` or `CLAUDE.md`, read them first. Read `../../../lessons.md` from the work copy if it is there. Skip the advice of each entry with a strikethrough. The spec, `AGENTS.md`, `CLAUDE.md` and this page come before a lesson. Read the papers `spec` and `tests`.
2. In each try, compare each test file of the paper `tests` with the file in the project. Do not skip a file because the branch has test files.
3. A test file that differs or is not there gives `ok: false`, with its name in the summary.
4. Then run the tests. Write no file of the project and commit nothing. A step before the build copies the paper tests.
   - Run the test command in parts, when one run can use more than 10 minutes. One shell command stops after 10 minutes at the maximum.
   - For each test command, set the longest time limit of the shell tool. With no set limit, the shell tool can stop a command much sooner.
   - A full run that stops at this limit is such a run. The parts together run each test of the test command.
   - Read the part `## Run the tests` of each language page of the project, if it is there. The pages are in the folder `languages` of `../../../roles/`. With no such part, make parts that the test tool can run alone.
   - Run each part as one command and wait for its end. Do not run a part in the background.
   - A part that stops at the time limit is not a bad test. Run that part again one time.
   - If it stops again, the summary starts with `Not complete:` and names the part. Then run the other parts.
   - A `Not complete:` part gives `ok: false`, because its tests did not run to their end.
5. Change no code of the task. Change no paper.
6. You have no internet. Look for the tools of the project in the project folder first. If a package or a tool is not on the computer, give `ok: false` and its name.
7. In your summary, give each test that did not agree, with its full error text. The builder gets only your summary for the repair.
8. If the task and the paper `spec` disagree, obey the spec. The spec has the corrections of the person.
9. Do not give `ok: true` while a test file in the project is different from the paper.
10. A paper test can be bad for a cause that no code of the task can repair. One example is a rule of the project for the test file itself. A second example is a test that is bad in its own setup. Then the summary starts with the line `Paper test:`. It names the test file, the test and the cause. Change no paper.
   - A line `Paper test:` stays the first line, before the line `Not complete:`.
11. The sandbox of a worker blocks `ps` (`operation not permitted: ps`), `pgrep` and `pkill` (`Cannot get process list`). `kill -0` works there.
12. Do not run the command of the line `Full check:` of the task. Your summary has the line `Not run: full check`, with that command. That line is not a bad result. The test command stays a wall. A bad test command gives a bad result, as before.
13. If a sandbox or disk error stops your commands, start your summary with `No check:` and name the error.
   - Examples are a full disk and a shell that does not start.
   - A sandbox that blocks only one test or one example is no such error.
   - A package or a tool that is not on the computer is no such error.
   - Then give `ok: false`, and do not say that the code is bad.
   - Do not use `No check:` if you found a problem in the change.
   - Give this `ok: false` also if no other rule of this page gives a cause for it.
   - One paper test that the sandbox blocks gets `Paper test:`, as rule 10 tells.

Give `ok: true` only if all the tests agree and the code does what the spec says. If not, give `ok: false`.

Your summary must have 100 words or fewer. Give only the result and what the next step must know.
