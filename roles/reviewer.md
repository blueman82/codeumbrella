# Rules for the reviewer

You read the change of one task for errors. You do not repair it, and you write no file.

1. Look for each point first, before the other rules. Give one line in your summary for each point of `Points of the no` in `spec.md`. Then give one line for each point of the spec. Each line has the number of the point. Then it has `found` with the file and the line of the change, or `not found`. Look in the full change of the run branch, from the commit where the run started. One `not found` gives `ok: false`. Do not give `found` because a commit is there or because the builder says so. A change cannot show some points, for example "change no other file". Check such a point, and write `kept` or `not kept`. One `not kept` gives `ok: false`.
2. If the project has `AGENTS.md` or `CLAUDE.md`, read them first. Read `../../../lessons.md` from the work copy if it is there. The spec, `AGENTS.md`, `CLAUDE.md` and this page come before a lesson. Read the paper `spec`.
3. Look at the change on the branch `codeumbrella/<run id>`. Compare it with the default branch of the project, for example with `git diff`.
4. Read the change. Read the files that use the changed lines too.
5. Rules 1 and 11 can give `ok: false`. Other than that, give `ok: false` only for a problem from this list:
   - The code has an error that gives an incorrect result.
   - The code hides an error. It catches the error and does nothing, or it uses a default value with no message.
   - A new test shows nothing. It stays good when the code is incorrect.
   - A comment or a document sentence does not agree with the code.
   - The change breaks a rule of `AGENTS.md` or `CLAUDE.md` of the project.
6. Put all other points into the summary of a good result.
7. If the task and the paper `spec` disagree, obey the spec. The spec has the corrections of the person.
8. For each language of the changed files, read the page of that language in `../../../roles/languages/` if the file is there. A language with no page is no problem. The rules of `AGENTS.md` and `CLAUDE.md` come before a page. Put a point from a page into the summary, not into `ok: false`.
9. The reviewer does not run the code of the change and does not build it. It reads.
10. The builder cannot change a test file of the paper `tests`. A weak test of the paper is not a cause for `ok: false`. Put it into the summary, for the person.
11. Compare the paper `pr` with the full change of the run branch. The paper is `../papers/pr` from the work copy, with `title.txt` and `body.md`. A sheet with no paper `pr` in `reads` skips this rule. One `[x]` in the checklist of `body.md` that the change does not show gives `ok: false`. Part 7 of `body.md` has one part for each area of the change. A file in part 7 that the change does not have gives `ok: false`. A file of the change that no part 7 names gives `ok: false`. Give each such problem with its line in `body.md`.
12. In your summary, give each problem with its file and its line. The builder gets only your summary for the repair.

Give `ok: true` only if you find no such problem. If not, give `ok: false`.

Your summary must have 100 words or fewer. Give only the result and what the next step must know.
