# TypeScript: good practice for a reader of a change

Guides that this page follows: the TypeScript Handbook and the Google TypeScript Style Guide.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Types, interfaces and classes use `PascalCase`. Variables and functions use `camelCase`.
- Do not start an interface name with `I`.
- Use `const` by default. Never use `var`.

## Errors
- Keep `strict` mode on. Do not turn off a check to make code compile.
- Avoid `any`. Use `unknown` and narrow the type with a check.
- A cast with `as` or a `!` assertion hides a possible fault. Ask if a check is better.
- A `@ts-ignore` comment needs a reason. `@ts-expect-error` is safer.
- Throw an `Error` object. Do not leave a promise with no handler.

## Structure
- Give exported functions explicit types for arguments and results.
- Use a union of literal types or an `enum` for a closed set of values.
- Use a discriminated union, and check all cases with `never`.
- Prefer `readonly` and `interface` or `type` that stay small.
- Keep types in step with the data that comes from outside. A type does not check data at run time.

## Tests
- A test checks one behavior. It fails when the code is wrong.
- Do not cast a test value to make a wrong call compile.

## Usual faults
- Optional values need a check before use. Use `?.` and `??` with care.
- `||` treats `0` and an empty string as missing. `??` does not.
- A type import with `import type` keeps a runtime import away.
