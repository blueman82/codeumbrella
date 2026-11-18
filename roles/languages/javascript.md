# JavaScript: good practice for a reader of a change

Guides that this page follows: the MDN JavaScript Guide and the Google JavaScript Style Guide.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Variables and functions use `camelCase`. Classes use `PascalCase`. Constants use `UPPER_CASE`.
- Use `const` by default, `let` when a value changes, and never `var`.
- A boolean name reads as a question, such as `isReady`.

## Errors
- Throw an `Error` object, not a string.
- An empty `catch` block hides a fault. Handle the error or pass it on.
- Each promise needs a `catch` or an `await` inside `try`. An unhandled rejection can stop the process.
- Check input at the edge of the code.

## Structure
- Use `async` and `await` in place of long promise chains.
- Use ES modules with `import` and `export`.
- Prefer small pure functions. Avoid change of shared objects.
- Use `Array` methods such as `map` and `filter` when they make the code clearer.

## Tests
- A test checks one behavior and names it.
- Await each async call in a test. A missing `await` makes a test pass by mistake.
- Do not rely on timers or on test order.

## Usual faults
- Use `===`. The loose `==` converts types in odd ways.
- `this` changes inside a plain callback. Use an arrow function.
- `forEach` does not wait for an async callback.
- `parseInt` needs a radix. Check for `NaN` with `Number.isNaN`.
