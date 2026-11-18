# Python: good practice for a reader of a change

Guides that this page follows: PEP 8, PEP 20 and PEP 257.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Functions and variables use `snake_case`. Classes use `CapWords`. Constants use `UPPER_CASE`.
- A name tells what a thing is, not how it is built.
- A leading underscore marks a name as private.

## Errors
- Catch the narrowest error that you can handle. A bare `except:` hides faults.
- A handler that does `pass` hides an error. Log it or raise it again.
- Use `raise ... from err` to keep the first cause.
- Use a `with` block for files and locks, so they close on every path.

## Structure
- A function does one thing. Long functions with many branches need a split.
- Avoid a mutable default argument such as `def f(x=[])`. It keeps state between calls.
- Prefer a comprehension or a plain loop. A nested comprehension is hard to read.
- Public functions and classes get a short docstring.

## Tests
- A test names the case that it checks. It fails if the code is wrong.
- Avoid a test that only calls the code and asserts nothing.
- Use `pytest.raises` or `assertRaises` to check an error.

## Usual faults
- Compare to `None` with `is`, not `==`.
- Late binding in a loop closure gives the last value to every call.
- Do not change a list while you loop over it.
- A wildcard import hides where a name comes from.
