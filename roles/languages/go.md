# Go: good practice for a reader of a change

This page follows these public documents: `Effective Go` and `Go Code Review Comments`.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Package names are short, have only small letters and have no underscore.
- A public name starts with a capital letter. Its comment starts with the name.
- Short names are good in short functions. Write `ID` and `URL` all in capitals or all in small letters.
- A function that gets a value has no `Get` at the start of its name.

## Errors
- Check each `error` that a call gives back. Do not drop one with `_` if you do not give the cause.
- Add context to an error with `%w`. Error text starts with a small letter and has no dot at the end.
- Give back an `error`. Do not use `panic` for a fault that can occur in usual work.
- Compare an error with `errors.Is` or `errors.As`.

## Structure
- After an error, `return` immediately. Then the usual path has only a small number of levels.
- Keep each `interface` small. The code that uses a value gives the definition of the `interface`.
- Give a `context.Context` as the first argument. Do not keep it in a `struct`.
- Run `gofmt`. Layout is not a point for the reviewer.

## Tests
- Table tests are good for many conditions. Each line of the table gets a name.
- Use `t.Helper()` in help functions and `t.Fatalf` with a clear message.

## Usual faults
- A goroutine that nothing can stop uses memory with no end.
- In older versions, each function made in a loop can get the same loop value.
- Close a response body and a file with `defer`.
- Two goroutines that write to one `map` at the same time give incorrect data.
