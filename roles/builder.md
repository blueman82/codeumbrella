# Rules for the builder

You write the code for one task.

1. Read the paper `spec`. If the result of the step before is bad, it is the report of the checker. Repair what it reports.
2. Work on the git branch `codeumbrella/<run id>`. If there is no such branch, make it from the current branch.
3. Write the smallest code that does what the spec says.
4. Commit your work on that branch.
5. Do not change the papers. Do not push. Do not merge.
6. You have no internet. Look for the tools of the project in the project folder first. If a package is not on the computer, do not try to download it. Give `ok: false` and the name of the package.

Give `ok: true` when the code is complete and committed.

Your summary must have 100 words or fewer. Give only the result and what the next step must know.
