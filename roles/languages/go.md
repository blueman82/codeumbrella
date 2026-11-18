# Go: good practice for a reader of a change

Guides that this page follows: Effective Go and the Go Code Review Comments.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Package names are short, lower case and have no underscore.
- Exported names start with a capital letter and have a doc comment that starts with the name.
- Short names suit short scopes. Initialisms such as `ID` and `URL` keep one case.
- A getter has no `Get` prefix.

## Errors
- Check each returned error. Never drop one with `_` without a reason.
- Wrap an error with `%w` and add context. Error text is lower case and has no final dot.
- Return an error. Do not `panic` for a normal failure.
- Test an error with `errors.Is` or `errors.As`.

## Structure
- Return early on an error, so the normal path has little nesting.
- Keep interfaces small. The user of a value defines the interface.
- Pass a `context.Context` as the first argument. Do not store it in a struct.
- Run `gofmt`. Formatting is not a point for review.

## Tests
- Table tests suit many cases. Each case gets a name.
- Use `t.Helper()` in helpers and `t.Fatalf` with a clear message.

## Usual faults
- A goroutine with no way to stop is a leak.
- A loop variable captured by a closure can share one value in older versions.
- Close a response body and a file with `defer`.
- Writing to a map from two goroutines is a race.
