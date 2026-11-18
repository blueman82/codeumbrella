# Rust: good practice for a reader of a change

This page follows these public documents: `Rust API Guidelines` and `Rust Style Guide`.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Functions and the names of values use `snake_case`. Types and traits use `UpperCamelCase`. Constants use `SCREAMING_SNAKE_CASE`.
- A method that changes a type starts with `as_`, `to_` or `into_`. The start shows its cost.
- A method that gets a value has no `get_` at the start of its name.

## Errors
- Give back a `Result` for a fault that can occur. Keep `panic!` for a broken rule of the code itself.
- Report `unwrap` and `expect` in library code. They hide a possible fault.
- Use the `?` operator to send an error up, with context for the person.
- Do not drop a `Result` with `let _ =` if you do not give the cause.

## Structure
- Use `&str` and `&[T]` as arguments, not `&String` and `&Vec<T>`.
- A `clone` that is not necessary frequently hides a problem with a borrow.
- Use iterators and `match`, not loops with an index and long `if` chains.
- Public items get `///` comments. Keep each `unsafe` block small, with a comment that tells why it is safe.
- Run `rustfmt` and Clippy. Their output is not a point for the reviewer.

## Tests
- Unit tests are in a `#[cfg(test)]` module. Integration tests are in `tests/`.
- A test checks a result. It is bad if the code is incorrect.

## Usual faults
- An integer result from numbers that a person controls can go above the maximum.
- A cut of a `String` at a byte that is not the edge of a character stops the code.
- A lock that the code holds during a long call makes other work wait.
- A cast with `as` can cut a number and gives no message.
