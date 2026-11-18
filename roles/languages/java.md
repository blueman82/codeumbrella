# Java: good practice for a reader of a change

Guides that this page follows: the Google Java Style Guide and Effective Java.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Classes use `UpperCamelCase`. Methods and variables use `lowerCamelCase`. Constants use `UPPER_SNAKE_CASE`.
- Packages are lower case with no underscore.
- A method name is a verb. A name tells the intent.

## Errors
- Never swallow an exception with an empty `catch` block.
- Catch the most specific exception. Do not catch `Throwable` or `Exception` without need.
- Keep the cause when you wrap an exception.
- Use try-with-resources to close files and connections.
- Use an unchecked exception for a programmer fault. Use a checked one for a fault that a caller can fix.

## Structure
- Make fields `private` and objects immutable when you can. Mark them `final`.
- Prefer composition to inheritance.
- Use `Optional` for a result that may be empty. Do not use it for fields.
- Do not return `null` for a collection. Return an empty one.
- Override `equals` and `hashCode` together.

## Tests
- A JUnit test checks one behavior and has a clear name.
- Use `assertThrows` to check an exception.
- A test that asserts nothing proves nothing.

## Usual faults
- Compare strings with `equals`, not `==`.
- A `NullPointerException` comes from a missing check.
- Concatenation in a loop is slow. Use `StringBuilder`.
- A mutable shared field needs a lock or a safe type.
