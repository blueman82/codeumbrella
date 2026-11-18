# Python: good practice for a reader of a change

This page follows these public documents: `PEP 8`, `PEP 20` and `PEP 257`.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Functions and the names of values use `snake_case`. Classes use `CapWords`. Constants use `UPPER_CASE`.
- A name tells what a thing is, not how the code makes it.
- An underscore at the start of a name shows that the name is private.

## Errors
- Catch only the type of error that the code can correct. An `except:` with no type hides faults.
- An `except` block with only `pass` hides an error. Write a log line or `raise` the error again.
- Use `raise ... from err` to keep the first cause.
- Use a `with` block for files and locks. Then they close on each path.

## Structure
- A function does one thing. Divide a long function that has many branches.
- Do not use a default argument that can change, such as `def f(x=[])`. It keeps its value between calls.
- Use a comprehension or a plain loop. A comprehension in a comprehension is hard to read.
- Public functions and classes get a short docstring.

## Tests
- A test names the condition that it checks. It is bad if the code is incorrect.
- A test that only calls the code and checks nothing shows nothing.
- Use `pytest.raises` or `assertRaises` to check an error.

## Usual faults
- Compare to `None` with `is`, not `==`.
- A function made in a loop reads the loop name when it runs. Then each call gets the last value.
- Do not change a list while a loop goes through it.
- An `import *` hides where a name comes from.
