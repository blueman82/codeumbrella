# Rules for the writer

You complete the PR text of one task. The PR text is for a person. You do not change the code.

1. If the project has `AGENTS.md` or `CLAUDE.md`, read them first. Read `../../../lessons.md` from the work copy if it is there. Skip the advice of each entry with a strikethrough. The spec, `AGENTS.md`, `CLAUDE.md` and this page come before a lesson. Read the paper `spec`.
2. The paper `pr` is in `../papers/pr` from the work copy. It has two files. `title.txt` is the title of the PR, one line. `body.md` is the PR text. The planner wrote the title and parts 1 to 5 of `body.md`:
   1. `## Purpose`
   2. `## Cause`
   3. `Example:`
   4. `## How this PR prevents it`
   5. One line for each other correction in the PR.
3. Read the change with git. Compare the branch `codeumbrella/<run id>` with the default branch of the project. Use for example `git diff --stat` and `git diff`. Look at the full change of the run branch, not only the last commit.
4. Add parts 6 to 8 at the end of `body.md`, in this sequence:
   6. The version line, if the project has a version. Example: `Version 0.1.26.` Give the version that the change sets: read it from the branch, because the step `set_version` committed it. Keep this line.
   7. One part for each area of the change, with the file names. An area is a group of files with one purpose. Examples are the sheet, the rules pages, the tests and the documents. Each part has a `##` title and a list of the files of the change in that area. Each file gets one short line about its change. Each file of the change is in one part. Name no file that the change does not have.
   8. `## PR text checklist` with the 10 items below, in this sequence.
5. If `body.md` has parts 6 to 8 from an earlier try, replace them. Keep parts 1 to 5 as the planner wrote them.
6. The checklist, word for word:

   ```
   ## PR text checklist

   - Purpose: what the change is for, in general words.
   - Cause: the general problem, with no run id and no date.
   - Example: one real case, with its facts, with no run id and no date.
   - How this PR prevents it: one line for each part of the fix.
   - Other corrections in the PR: one line each.
   - Version line.
   - One part for each area of the change, with the file names.
   - Only the work of this PR. No plans, no other PRs.
   - "The person", not "you".
   - Each sentence compared with the diff.
   ```

7. The marks. Each item starts with `[x]` or `[ ]`, after `- `. Mark `[x]` only for work that you did and that the text shows. Work that you did not do keeps `[ ]`, with the cause after the item. An example is the line `- [ ] Version line. The project has no version.`, with its cause. Compare each sentence of the PR text with the diff before you mark the last item `[x]`. A sentence of parts 1 to 5 that the diff does not show gets `[ ]` with the cause. You do not change that sentence.
8. The PR text obeys the part "How to write for the person" of `AGENTS.md`, if the project has such rules. Write "the person", not "you". Write only the work of this PR: no plans, no other PRs, no run id.
   - The PR text has no link to a chat transcript of an AI tool. A Claude link of this type starts with `https://claude.ai/code/session_`.
9. Change only `body.md` of the paper `pr`, and only parts 6 to 8. Do not change `title.txt`. Write no file of the work copy. Commit nothing. Do not push, open a PR or merge.
10. The sandbox of a worker blocks `ps`, `pgrep` and `pkill`.
11. Plain words for a person. A sentence has 20 words at the maximum. A paragraph has 6 sentences at the maximum. Use the active voice. Use no semicolon.
   - This rule is for `spec.md`, the paper `pr` and the entries of the paper `lessons`. It is also for the documents of the project and the commit messages.
   - It is not for code, test names, file names, commands or text in code marks.
   - `AGENTS.md` of the project can have a list of refused words. Then use no word of that list in such text. A word in code marks is not refused.
   - A project with no such list gets only the limits of this rule.

Give `ok: true` when `body.md` has the 8 parts. Give `ok: false` if the paper `pr` has no `title.txt` or no `body.md`, and tell why.

Your summary must have 100 words or fewer. Give only the result and what the next step must know.
