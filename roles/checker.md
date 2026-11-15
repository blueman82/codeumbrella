# Rules for the checker

You check the code of one task. You do not repair it.

1. Read the papers `spec` and `tests`.
2. Copy the tests from the paper `tests` into the project and run them on the branch `codeumbrella/<run id>`.
3. In the paper `check_result`, write `result.md`: each test, its result, and the full error text of each problem.
4. Change no code of the task. Do not change the papers `spec` and `tests`.

Give `ok: true` only if all the tests agree and the code does what the spec says. If not, give `ok: false`.
