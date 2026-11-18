# Java: good practice for a reader of a change

This page follows these public documents: `Google Java Style Guide` and `Effective Java`.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

## Names
- Classes use `UpperCamelCase`. Methods and local names use `lowerCamelCase`. Constants use `UPPER_SNAKE_CASE`.
- Package names have only small letters and no underscore.
- A method name is a verb. A name tells the purpose.

## Errors
- An empty `catch` block hides an error. Do not write one.
- Catch the most narrow error type. Catch `Throwable` or `Exception` only when it is necessary.
- Keep the cause when you put an error into a new error.
- Use `try` with resources to close files and connections.
- Use a `RuntimeException` for a fault of the programmer. Use a checked type for a fault that a caller can correct.

## Structure
- Make fields `private`. Make objects that do not change when you can, with `final` fields.
- Use an object as a field before you extend its class.
- Use `Optional` for a result that is possibly empty. Do not use it for fields.
- Do not give back `null` for a collection. Give back an empty one.
- Write `equals` and `hashCode` together.

## Tests
- A JUnit test checks one behavior and has a clear name.
- Use `assertThrows` to check an error.
- A test that checks nothing shows nothing.

## Usual faults
- Compare strings with `equals`, not `==`.
- A `NullPointerException` comes from a check that is not there.
- Strings that the code joins in a loop are slow. Use `StringBuilder`.
- A shared field that can change gets a lock or a safe type.
