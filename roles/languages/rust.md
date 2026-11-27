# Rust: good practice for a reader of a change

This page follows the Rust API Guidelines, the Rust Style Guide, the Rust book, the standard library docs and Clippy.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

The layout tool and the linter of the project check layout and names, so this page has none of that.

## Write the code like this

- Give back a `Result` for a fault that normal use can cause, such as a bad file. Use `panic!` only when the caller broke the contract of the function.
- Write the contract in the docs. A public function that gives back `Result` gets an `# Errors` section. One that can panic gets `# Panics`.
- Send errors up with `?`. Add a `From` impl so that `?` converts the cause into the error type of the function.
- Give the error type a name of its own, with `Error`, `Display`, `Send` and `Sync`. Do not use `()` as an error type.
- Use `expect` only for a value that the code itself guarantees, such as a constant. The message says why the value should be `Ok`.
- Make bad input impossible with a type. Use a newtype for a value with a rule, and an enum for a `bool` argument.
- Check arguments at the border of a module. Use `debug_assert!` for a check that costs much.
- Keep the fields of a struct private when the struct holds a rule. Check the rule in the constructor.
- Own the value when the function stores the value. Borrow when it only reads, so the caller keeps the value and clones nothing.
- Use `From` and `TryFrom` for conversions, not a hand-written `Into`. Use `TryFrom` to narrow a number instead of `as`.
- Use `checked_add` and the methods like it for numbers that come from a file, a user or a network. Plain `+` panics on overflow in a debug build.
- Cut a `str` with `get(a..b)` when the offsets come from data. A cut like `&s[a..b]` panics inside a multi-byte character.
- Hold a `MutexGuard` for the shortest scope. Drop it before a slow call, a `join` or a second lock.
- Move data into a thread with a `move` closure. Share it with `Arc`. Keep the `JoinHandle` and `join` it.
- Handle a poisoned lock on purpose. Either `unwrap` to stop, or use `into_inner` to go on.
- Name a method `as_` only if it costs nothing. Use `to_` for a method that does work, and `into_` for one that owns its input.
- Keep each `unsafe` block small, with a `// SAFETY:` comment that tells why the rules hold. Wrap it in a safe function that no caller can misuse.
- Write `Drop` code that cannot panic and does not block. Offer a `close` method that gives back a `Result`.
- Write doc examples with `?`, not `unwrap`.
- Write a test with a `Result` result when its body uses `?`.

## A break

### Errors

- `unwrap` or `expect` outside a test, on data from a file, a user or a network. Tests may use them.
- A `panic!` for a fault that normal use causes, such as malformed data.
- `let _ = call()` on a `Result` drops the error. So does `map_err(|_| ...)`, which drops the cause.
- An error type of `()`, or a custom error type with no `Error` impl.
- `expect` with a message that does not say why the value should be there.
- `unwrap_or(f())` where `f` costs time or has an effect. `f()` runs even when the value is `Ok`. Use `unwrap_or_else`.

### Things that stay open

- A `MutexGuard` held across a slow call or a `join`. Other threads wait for it.
- A second `lock` of the same mutex in the same thread. It can panic or hang.
- A spawned thread with no `join`. The code stops it when `main` ends.
- A `Drop` impl that can panic. A panic during a panic stops the code.

### Work at the same time

- A `RefCell` borrowed with `borrow_mut` while a second borrow lives. It panics at run time. Use `try_borrow_mut` when the code cannot show it is safe.

### Types and borders

- An `as` cast between number types. It cuts high bits, changes the sign, and turns `NaN` into `0` for a float.
- Plain `+`, `-` or `*` on numbers from data. The code panics in a debug build.
- A `/` or `%` of the smallest signed integer by `-1`. It panics even when overflow checks are off.
- `&s[a..b]` or `v[i]` with an index from data. Use `get`.
- Two units, such as miles and kilometres, in the same `f64` type with no newtype.
- A public field on a struct that holds a rule. A caller can break the rule.
- A `.clone()` of an argument inside a function that could own it.
- An `unsafe` block with no `// SAFETY:` comment, or larger than the one operation that needs it.
- A safe function with `unsafe` inside that a safe caller can use to cause undefined behavior.
- `unsafe` code that trusts a safe trait impl, such as `Ord`, for memory safety.

### Tests

- `#[should_panic]` with no `expected` text. The test succeeds on each panic.
- A test that gives back `Result` and tries to check an `Err` with `?`. Use `assert!(x.is_err())`.

### Usual faults

- `dbg!` left in the change.
- A doc example that uses `unwrap`.
- An `as_` method that allocates or copies.
- A public function that gives back `Result` with no `# Errors` section.
