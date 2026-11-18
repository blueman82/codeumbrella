# JavaScript: good practice for a reader of a change

This page follows these public documents: `MDN JavaScript Guide` and `Google JavaScript Style Guide`.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Functions and the names of values use `camelCase`. Classes use `PascalCase`. Constants use `UPPER_CASE`.
- Use `const` as the default, and `let` when a value changes. Do not use `var`.
- A name for a yes or no value reads as a question, such as `isReady`.

## Errors
- Use `throw` with an `Error` object, not with a string.
- An empty `catch` block hides a fault. Correct the error or send it on.
- Each promise gets a `catch`, or an `await` in a `try` block. If not, the full application can stop.
- Check input at the edge of the code.

## Structure
- Use `async` and `await`, not long promise chains.
- Use ES modules with `import` and `export`.
- Use small functions that change nothing but their result. Do not change shared objects.
- Use `Array` methods such as `map` and `filter` when they make the code clearer.

## Tests
- A test checks one behavior and names it.
- Use `await` on each async call in a test. If not, a test can be good by error.
- A test must not use a timer or the sequence of the tests for its result.

## Usual faults
- Use `===`. The operator `==` changes types by rules that are hard to remember.
- The value of `this` changes in a plain callback. Use an arrow function.
- `forEach` does not wait for an async callback.
- `parseInt` gets a radix. Check for `NaN` with `Number.isNaN`.
