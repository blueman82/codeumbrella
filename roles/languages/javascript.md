# JavaScript: good practice for a reader of a change

This page follows the `MDN JavaScript Guide`, the MDN reference, the `Google JavaScript Style Guide` and the Node.js documentation.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

The layout tool and the linter of the project check layout and names, so this page has none of that.

## Write the code like this

- Use `const` as the default and `let` for a value that changes. Do not use `var`.
- Throw an `Error` or a subclass of it. Reject a promise with an `Error` too.
- When you catch an error and throw a new one, give the first error as `cause`. Write `new Error("text", { cause: err })`.
- Catch only the errors that you expect. Test them with `instanceof`, and throw the others again.
- Give back each promise from a `then` handler, so the next step waits for it.
- Keep promise chains flat, and put the error handling at the end. Prefer `async` and `await`.
- Run independent async work together with `Promise.all`. Wait one after the other only when a step uses the result of the step before.
- Use `Promise.allSettled` when you want the result of each promise, also the ones that break.
- Put `await` inside the `try` block. A promise that rejects without `await` skips the `catch`.
- Compare with `===` and `!==`. The test `== null` for `null` and `undefined` together is the one exception.
- Test for `NaN` with `Number.isNaN`. The global `isNaN` changes the value to a number first.
- Use `includes` to look for `NaN` in an array. `indexOf` cannot find it.
- Give `parseInt` a string and a radix. Without a radix, a `0x` prefix changes the radix to 16.
- Use an arrow function for a callback inside a method. Write object methods in the short method syntax, not as arrows.
- Wrap an object literal in parentheses when an arrow function gives it back: `() => ({ a: 1 })`.
- Give `sort` a compare function for numbers. Sort a copy or use `toSorted`, because `sort` changes the array.
- Treat spread and `Object.assign` as shallow copies. Use `structuredClone` when nested data must not be shared.
- Loop with `for...of` or `Object.keys`. Use `for...in` only on plain dictionary objects, with `hasOwnProperty`.
- Write array literals, not `new Array(n)`. Do not use `new` with `Number`, `String` or `Boolean`.
- Declare each variable before the first assignment. Strict mode, in modules and classes, turns a typo into a `ReferenceError`.
- Give each `EventEmitter` an `'error'` listener. Without one, the error is thrown and the Node.js `process` exits.
- Join streams with `stream.pipeline`. It destroys all streams on an error, and `pipe` does not.
- Stop writing when `write()` gives `false`. Wait for the `'drain'` event.
- Identify a Node.js error by `error.code`, not by its message text.

## A break

### Errors

- An empty `catch` block hides a fault. Handle the error, throw it again, or write the cause in a comment.
- `throw "text"` or a thrown plain object breaks the rule to throw only `Error` objects.
- A `return` or `throw` in a `finally` block replaces the `return` or `throw` that came before it.
- Code that compares `error.message` breaks when Node.js changes the text. Compare `error.code`.
- A callback with the signature `(err, data)` that does not check `err` ignores the error.
- A `try` block cannot catch an error that an event or a callback throws after the call has ended.

### Things that stay open

- A `then` handler that starts a promise and does not give it back keeps the promise floating. Nobody sees its error.
- A promise with no `catch` and no `await` inside a `try` block ends as an unhandled rejection.
- A `process.on('unhandledRejection')` listener with no handling code hides the errors.
- `pipe` does not close the target stream when the source breaks. Use `pipeline`.
- Each `on` call adds a listener. More than 10 for one event gives a leak warning, so remove them or use `once`.
- `stream.finished` keeps its listeners after the callback. Call the cleanup function that it gives back.
- A write loop that ignores `write()` giving `false` fills the memory.

### Work at the same time

- `forEach` does not wait for an `async` callback. Use `for...of` with `await`, or `Promise.all` with `map`.
- An `async` function as an event listener gives an unhandled rejection when it throws. Do not use one as an `'error'` listener.
- Mixing `on('data')`, `pipe()` and async iterators on one stream gives odd results.

### Types and borders

- `==` changes types. `0 == "0"` is `true`, and `null == undefined` is `true`.
- `NaN === NaN` is `false`. A `switch` with `case NaN` does not match.
- Global `isNaN(null)` and `isNaN("")` give `false`, because they change the value to `0` first.
- `parseInt` on a number or on `null` gives odd values. `parseInt(1e21, 10)` gives `1`. Use `Number` or `Math.trunc`.
- Integer text above the safe range gives a changed value from `parseInt`. Use `BigInt`.
- An arrow function as an object method has no `this` of the object.
- `() => { a: 1 }` gives `undefined`, because `a:` is a label and not a key.
- `sort()` with no compare function puts `80` before `9`. The compare function `(a, b) => a > b ? 1 : 0` gives results that differ by engine.
- `forEach` and other array methods skip the empty slots of a sparse array.
- `{ ...a, ...b }` is a shallow copy, so nested objects stay shared. The last value wins when two objects have the same name.
- An import is read-only. Only the module that exports it can assign a new value.

### Tests

- A test with `forEach` and an `async` callback ends before its checks run.

### Usual faults

- A script without strict mode makes a global from an undeclared name. It also ignores a write to a read-only property.
- A `for...in` loop on an array lists inherited keys. Use `for...of`.
- A `switch` without a `default` group.
- `eval`, `Function(string)` and `with` in a change.
- A change to a built-in prototype, such as `Array.prototype`.
