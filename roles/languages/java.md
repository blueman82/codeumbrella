# Java: good practice for a reader of a change

This page follows `Google Java Style Guide` and `Effective Java`. It also follows the Oracle Java tutorials and the SEI CERT Oracle Coding Standard for Java.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

The layout tool and the linter of the project check layout and names, so this page has none of that.

## Write the code like this

- Handle each caught error: recover, throw it again, or throw a fitting new one. If you ignore it on purpose, write a comment that says why.
- Catch the narrowest type. Do not catch `NullPointerException`, `RuntimeException`, `Exception` or `Throwable`. Fix the null instead.
- Throw a specific type. Do not throw or declare `RuntimeException`, `Exception` or `Throwable`.
- Use a checked type when the caller can recover. Use an unchecked type when the caller cannot.
- When you wrap an error in a new one, give the first error as the cause.
- Open each closeable resource in a `try` with resources. It closes the resource in reverse sequence and keeps the first error.
- Keep paths, queries and internal names out of error messages that reach untrusted users.
- Mark each method that overrides with `@Override`.
- Write `hashCode` each time you write `equals`. Equal objects must give equal hash codes.
- Make `equals` reflexive, symmetric, transitive and consistent. Give `false` for `null`.
- Give back a copy of a private mutable field. Copy a mutable input before you store it.
- Use `Optional` only as the result type of a method. Do not give back a `null` `Optional`.
- Read an `Optional` with `orElseThrow()`, not `get()`.
- Send values to SQL with `PreparedStatement` placeholders. Do not join text into a query.
- Count money in cents, or use `BigDecimal`. Do not use `float` or `double`.
- Build a `BigDecimal` from a `String` or with `BigDecimal.valueOf`, not from a `double`.
- Check integer arithmetic on input values for overflow. Use a wider type or `BigInteger`.
- Name the charset when you turn text into bytes or bytes into text between systems.
- Lock on a private `final Object`. Do not lock on `this`, a `String`, a boxed value or an `Optional`.
- Make a shared field `volatile` or protect each read and write with a lock. Make a read and write in one step atomic.
- Release a lock in a `finally` block. Acquire it right before the `try`.
- Call `wait()` and `await()` in a loop that tests the condition again.
- Give each `switch` a `default` label, so it covers every value.
- Do not override `finalize`. Close resources by hand.

## A break

### Errors

- An empty `catch` block with no comment that says why.
- A `catch` of `Exception` or `Throwable`, or of `NullPointerException`, that hides a null fault.
- A new error thrown in a `catch` block with no cause. The reader cannot see the first error.
- A `close()` call in `finally` that can throw. It can replace the first error and skip the next cleanup.
- A public method that declares `throws Exception`.
- An error message sent to a user that shows a path, a query or an internal name.

### Things that stay open

- A file, stream or connection opened with no `try` with resources.
- A `lock()` call with no `unlock()` in a `finally` block.

### Work at the same time

- A call of `Thread.run()` where the code wants `start()`. It runs on the calling thread.
- A `synchronized` block on a `String`, a boxed value, an interned value or a public object.
- A shared field that one thread reads in a loop and a second thread writes. It has no `volatile` and no lock.
- A `count++` on a shared field. `volatile` alone does not make it atomic.
- A `wait()` or `await()` call inside an `if` and not in a loop.
- A task in a bounded thread pool that waits for a different task of the same pool.
- A collection changed while a loop reads it. Do not use `ConcurrentModificationException` as a check for this.

### Types and borders

- `==` on strings or boxed numbers. Use `equals`. Small `Integer` values hide the fault because of the cache.
- `equals` called on arrays. Use `Arrays.equals`.
- A raw type such as `List` with no type argument. It turns off the type check.
- `equals` written with no `hashCode`. A `HashMap` lookup then misses the object.
- A subclass `equals` that adds a field and breaks symmetry. Use composition.
- A method that gives back its private array or list, or stores the caller array with no copy.
- `new BigDecimal(0.1)`, or two `BigDecimal` values compared with `equals`. Build it from a `String` and use `compareTo`.
- A `float` or `double` field that holds money.
- An `int` sum or product of input values with no overflow check.
- A `get()` call on an `Optional` with no test. A comparison of an `Optional` with `==`.
- A byte conversion with the default charset between two systems.
- SQL built from text that holds a user value.

### Usual faults

- A result that the code ignores, such as `String.replace` or `File.delete()`.
- A method that overrides with no `@Override`, so a bad signature goes unseen.
- A `switch` with no `default` label.
