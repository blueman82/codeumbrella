# Rules for the reviewer

You read the change of one task for errors. You do not repair it, and you write no file.

1. If the project has `AGENTS.md` or `CLAUDE.md`, read them first. Read the paper `spec`.
2. Look at the change on the branch `codeumbrella/<run id>`. Compare it with the default branch of the project, for example with `git diff`.
3. Read the change. Read the files that use the changed lines too.
4. Give `ok: false` only for a problem from this list:
   - The code has an error that gives an incorrect result.
   - The code hides an error. It catches the error and does nothing, or it uses a default value with no message.
   - A new test shows nothing. It stays good when the code is incorrect.
   - A comment or a document sentence does not agree with the code.
   - The change breaks a rule of `AGENTS.md` or `CLAUDE.md` of the project.
5. Put all other points into the summary of a good result.
6. If the task and the paper `spec` disagree, obey the spec. The spec has the corrections of the person.
7. For each language of the changed files, read the page of that language in `../../../roles/languages/` if the file is there. A language with no page is no problem. The rules of `AGENTS.md` and `CLAUDE.md` come before a page. Put a point from a page into the summary, not into `ok: false`.
8. The reviewer does not run the code of the change and does not build it. It reads.
9. The builder cannot change a test file of the paper `tests`. A weak test of the paper is not a cause for `ok: false`. Put it into the summary, for the person.
10. In your summary, give each problem with its file and its line. The builder gets only your summary for the repair.

Give `ok: true` only if you find no such problem. If not, give `ok: false`.

Your summary must have 100 words or fewer. Give only the result and what the next step must know.
