# Go: good practice for a reader of a change

This page follows `Effective Go`, `Go Code Review Comments`, the Google Go Style Guide and the Go blog.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

The layout tool and the linter of the project check layout and names, so this page has none of that.

## Write the code like this

- Check each `error` that a call gives back. Handle it or give it back. Do not drop it with `_`.
- Give back `error` as the last result, and `nil` on success. Do not signal a fault with a value such as `-1` or `""`.
- Wrap with `%w` only when the caller should inspect the cause. Use `%v` to hide details such as a database error.
- Put `%w` at the end of the message, so the text matches the chain of causes.
- Compare errors with `errors.Is` and `errors.As`. Do not compare the text of `err.Error()`.
- Give back an error or log it, not two of them. The caller then controls the log.
- Use `panic` only for a fault at start-up. Give back an `error` for faults of usual work.
- Call a `Must` helper only at start-up or in package set-up, not on user input.
- Give `context.Context` as the first argument. Do not keep it in a `struct`.
- After `context.WithCancel` or `context.WithTimeout`, write `defer cancel()`.
- Make clear when each goroutine ends, and who waits for it. A blocked channel keeps its goroutine for ever.
- Protect data that goroutines share with a mutex, a channel or `sync/atomic`.
- Use a pointer receiver and a pointer to the type when a `struct` has a `sync.Mutex`. Do not copy it.
- Use the two-value check for a type assertion and for a map read. Write `v, ok := x.(T)` and `v, ok := m[k]`.
- Write `s = append(s, v)`. `append` gives back the new slice header, so keep it.
- Declare an empty slice with `var s []string`. Test for empty with `len(s) == 0`.
- Use `crypto/rand` for keys and tokens. The `math/rand` package gives values that others can predict.
- Define an `interface` in the package that uses it, when a use is in the code.
- Close a file with `defer f.Close()` right after the check of the error from `os.Open`.

## A break

### Errors

- A call that drops its `error` with `_`, and then uses the value, crashes when the call has failed.
- A function that gives back a nil `*MyError` as an `error` gives a value that is not `nil`. Give back a plain `nil`.
- Code that tells errors apart with `strings.Contains(err.Error(), ...)` breaks when someone changes the message.
- A function that logs an error and also gives it back. The same line then appears two or more times.
- A `panic` on bad input of a user or a network call stops the server.
- A function that gives back `""` or `-1` for "no value" hides a valid result of the same kind.

### Things that stay open

- A goroutine waits on a channel that no one writes to or closes. It keeps its memory until the code ends.
- A send on a closed channel panics. The closer must be the one that knows no more sends will come.
- A `go` statement with no wait, no cancel and no error path hides each fault of that work.
- A goroutine that panics with no `recover` stops the server, not only that request.
- A `context.WithCancel` or `WithTimeout` with no call of `cancel` keeps its resources.
- `defer` reads the arguments when the `defer` line runs, not when the call runs.

### Work at the same time

- Two goroutines that write to one `map`, or one writes and one reads, can crash the code. Add a mutex.
- A plain `int`, `bool` or `int64` that two goroutines use can give stale values. Use `sync/atomic` or a mutex.
- In a module below Go 1.22, `go func() { use(i) }()` in a loop shares one loop variable. Check the version in `go.mod`.
- A goroutine that writes to an `err` of the outer scope races with other writers. Declare a new `err` with `:=`.
- A send and a `close` on one channel with no wait between them is a race. Receive before the `close`.
- A change of an input slice or map after a goroutine got it is a race.
- A copy of a `sync.Mutex` or `bytes.Buffer` copies the lock or the buffer. Give a pointer.

### Types and borders

- A single-value type assertion `x.(T)` panics when the type is different. Use the two-value check.
- A call of `append` with no assignment of its result drops the new element.
- Two slices can share one array. A write through one changes the other, until `append` makes a new array.
- A method with a value receiver that changes a field changes only a copy.
- A `String` method that formats its own receiver with `%s` calls itself for ever. Convert it to `string` first.
- A type that should have `MarshalJSON` but has a different signature compiles with no error. Add `var _ json.Marshaler = (*T)(nil)`.
- A key of `context.WithValue` that is a plain `string` can clash with the key of a different package. Use an unexported type.
- A custom type that wraps `context.Context` breaks the standard use of a context in other code. Use the plain type.

### Tests

- A failure message with no input, no got value and no want value hides the cause. Write `F(%v) = %v, want %v`.
- A test that compares an error by its text breaks when someone rewrites the message. Use `errors.Is`.

### Usual faults

- Keys and tokens from `math/rand` are predictable, even with a seed from the clock.
- A `_ = x` or `var _ = pkg.F` line that silences an unused name. It hides a name that should go.
