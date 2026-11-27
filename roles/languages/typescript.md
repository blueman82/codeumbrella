# TypeScript: good practice for a reader of a change

This page follows these public documents: the `TypeScript Handbook` and the `Google TypeScript Style Guide`.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

The layout tool and the linter of the project check layout and names, so this page has none of that.

## Write the code like this

- Keep `strict` on in `tsconfig.json`. Do not turn a check off to make the build work.
- Use `unknown` for a value of unknown shape. Narrow it with a check before use. Do not use `any`.
- Check data from a border (`JSON.parse`, network, files) with code at run time. A type or `as` checks nothing.
- Treat the `catch` variable as `unknown`. Test it with `instanceof Error` before you read `.message`.
- Throw `new Error(...)` or a subclass. Do not throw a string, a number or `undefined`.
- Handle each caught error or throw it again. Keep the `try` block small.
- Model a set of shapes as a union with one literal tag field. Check the tag with `switch`.
- End that `switch` with a `default` that assigns the value to a `never` variable. A new member then breaks the build.
- Give each `switch` a `default`. End each non-empty `case` with `break`, `return` or `throw`.
- Use a type annotation, not `as`, to give a type to an object literal.
- Write `string`, `number`, `boolean` and `object`. Do not write `String`, `Number`, `Boolean` or `Object`.
- Give a callback that ignores its result the type `void`, not `any`.
- Mark a callback parameter optional only when the caller can omit it.
- Put specific overload signatures before general ones. Use an optional parameter or a union when one signature is enough.
- Mark a class property `readonly` when only the constructor sets it.
- Use `import type` for a name that serves only as a type. Use `export type` to send it on.
- Await each promise, or send it to a `.catch`. Start independent calls together with `Promise.all`.
- Use `Promise.allSettled` when you want the result of every call, also after one call breaks.
- Use `for...of` for arrays. Use `Number.isNaN` to test for `NaN`.
- Do not write an API with a type parameter that shows only in the result type.

## A break

### Errors

- `any` in new code, also a parameter with no type that `noImplicitAny` would stop.
- `@ts-ignore` or `@ts-nocheck` in the change. `@ts-expect-error` is only for a test that checks a refused call.
- A cast with `as`, or a `!`, with no check on the line before. Each one changes the type and checks nothing at run time.
- A double cast through `unknown`, such as `x as unknown as T`.
- `strict` or one of its flags turned off in `tsconfig.json` in a change that adds code.
- A `throw` of a string, a number, `null` or `undefined`.
- An empty `catch` block, or a `catch` that only logs and goes on as if all went well.
- `err.message` read in a `catch` with no `instanceof Error` check.

### Things that stay open

- A call to an `async` function with no `await`, no `return` and no `.catch`. A rejection then goes unseen.
- No `await` before a promise that the next line reads. The code then reads a `Promise`, not its value.
- `Promise.all(...)` with no `.catch` or `try` around the `await`.

### Work at the same time

- `items.forEach(async ...)`. `forEach` does not wait, so the code goes on before the work ends.
- `Promise.all` when one broken call must not hide the others. Use `Promise.allSettled`.

### Types and borders

- Data from `JSON.parse` or a network call typed with `as` and used with no check.
- A union with a tag field handled by `if` chains with no `never` check at the end.
- An `in` test on a union with optional properties in its members. The test keeps each member.
- An object property of type `string` sent to a parameter of literal types. Use `as const` or an annotation.
- A value from `array[i]` or an index signature used as if present. Add the check, or use `noUncheckedIndexedAccess`.
- The result of `find` used with no check for `undefined`.
- Overloads that put a general signature before a specific one. The general one hides the specific one.
- A callback type with an optional parameter that the caller always sends.
- A type parameter that no part of the type uses.
- `Object`, `String`, `Number` or `Boolean` as a type, or `new String(...)` as a value.
- `{}` as a type for a value of unknown shape. Use `unknown` or `object`.
- A `const enum`. Use a union of literals, or a plain `enum`.

### Tests


### Usual faults

- `||` for a default when `0`, `""` or `false` is a valid value. Use `??`.
- `if (value)` on a `number` or a `string`. It also drops `0` and `""`. Compare with `null` or `undefined`.
- `==` or `!=`. Use `===` and `!==`.
- `array.sort()` on numbers with no compare function. It sorts as text. It also changes the array itself.
- `x === NaN`. It is always `false`. Use `Number.isNaN`.
- `parseInt`, `parseFloat` or `+text` with no check of the result for `NaN`.
- `for...in` on an array. It gives the indices as text.
- `export let`. A reader of the module sees a value that changes.
- `var`, or a variable used before its declaration.
