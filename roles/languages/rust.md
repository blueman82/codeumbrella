# Rust: good practice for a reader of a change

Guides that this page follows: the Rust API Guidelines and the Rust Style Guide.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Functions and variables use `snake_case`. Types and traits use `UpperCamelCase`. Constants use `SCREAMING_SNAKE_CASE`.
- A conversion method starts with `as_`, `to_` or `into_` by its cost.
- A getter has no `get_` prefix.

## Errors
- Return a `Result` for a fault that can happen. Reserve `panic!` for a broken rule inside the code.
- `unwrap` and `expect` in library code are a point to raise. They hide a possible fault.
- Use the `?` operator to pass an error up, with context for the person.
- Do not drop a `Result` with `let _ =` without a reason.

## Structure
- Take `&str` and `&[T]` as arguments, not `&String` and `&Vec<T>`.
- A needless `clone` often hides a borrow problem.
- Use iterators and `match` in place of index loops and long `if` chains.
- Public items get `///` docs. Keep `unsafe` blocks small, with a comment on why they are safe.
- Run `rustfmt` and Clippy. Their output is not a point for review.

## Tests
- Unit tests sit in a `#[cfg(test)]` module. Integration tests sit in `tests/`.
- A test checks a result and fails if the code is wrong.

## Usual faults
- Integer overflow in arithmetic that a person controls.
- Slicing a `String` at a byte that is not a character edge.
- A lock held across a long call.
- `as` casts that cut numbers without a message.
