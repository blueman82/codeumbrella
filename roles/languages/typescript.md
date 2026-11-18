# TypeScript: good practice for a reader of a change

This page follows these public documents: `TypeScript Handbook` and `Google TypeScript Style Guide`.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Types, interfaces and classes use `PascalCase`. Functions and the names of values use `camelCase`.
- Do not start the name of an `interface` with `I`.
- Use `const` as the default. Do not use `var`.

## Errors
- Keep `strict` mode on. Do not turn off a check to make the code build.
- Do not use `any`. Use `unknown` and make the type narrow with a check.
- A cast with `as` or a `!` hides a possible fault. See if a check is better.
- A `@ts-ignore` comment must give its cause. `@ts-expect-error` is safer.
- Use `throw` with an `Error` object. Each promise gets a `catch` or an `await`.

## Structure
- Write the types of the arguments and of the result of each `export` function.
- Use a union of literal types or an `enum` for a closed set of values.
- Use a union with a tag field, and check each member with the type `never`.
- Use `readonly`. Keep each `interface` and each `type` small.
- Keep the types the same as the data that comes into the code. A type does not check data at run time.

## Tests
- A test checks one behavior. It is bad when the code is incorrect.
- Do not cast a test value to make an incorrect call build.

## Usual faults
- A value that is possibly not there gets a check before use. Use `?.` and `??` carefully.
- `||` reads `0` and an empty string as "no value". `??` does not.
- `import type` gets only the type, so no code loads at run time.
