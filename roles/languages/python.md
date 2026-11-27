# Python: good practice for a reader of a change

This page follows `PEP 8`, `PEP 20`, `PEP 257`, the official Python documentation and the Google Python Style Guide.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

The layout tool and the linter of the project check layout and names, so this page has none of that.

## Write the code like this

- Compare with `None` using `is` or `is not`. Write `is not None` when `0` or an empty list is a good value.
- Give an argument the default `None` when its value is a list, a dict or a set. Make the object inside the function.
- Write `X | None` when `None` is a good value. A default alone does not make a type optional.
- Catch the narrowest error type. Catch `Exception` only where the code logs the error and re-raises it, or ends one unit of work.
- Keep the `try` block to the lines that can raise the error. Put the code for the good result in `else`.
- Write `raise NewError from err` when you change an error type. Use `from None` only after you copy the details into the new error.
- Use `with` for files, sockets and locks. Use `contextlib.closing()` for an object that has no `with` support.
- Raise a built-in error such as `ValueError` for a bad argument. A custom error derives from `Exception` and its name ends in `Error`.
- Check input with `if` and `raise`, not with `assert`. Python can switch asserts off.
- Make each `return` of a function give a value, or make none give one. Write `return None` when some do.
- Test types with `isinstance()`, not with `type(x) == T`. The second test misses subclasses.
- Use `sorted(items)` to get a new list. The method `items.sort()` gives back `None`.
- Copy a list with `list(items)` or `items[:]`. The line `b = a` makes a second name for one list.
- Join text pieces with `"".join(parts)`. Do not build a long text with `+=` in a loop.
- Make a time value with `datetime.now(UTC)`. Do not use `utcnow()` or `utcfromtimestamp()`, which give a naive time.
- Give `subprocess.run` a list of arguments. Use `shell=True` only for shell features, and quote each value then.
- Set `check=True` and a `timeout` on `subprocess.run`. Set `text=True` and `encoding` when you read the output as text.
- Give values to a log call as arguments: `logger.info("job %s done", job_id)`. Do not use an f-string.
- Name a logger with `logging.getLogger(__name__)`. Only the application sets up handlers. A library adds a `NullHandler` only.
- Call `logger.exception()` inside an `except` block to log the error with its call stack.
- Move data between threads with `queue.Queue`. Use a lock for shared values, because plain assignment is not atomic.
- Start many coroutines at the same time with `asyncio.TaskGroup`. It awaits each task and raises their errors together.
- Give a public function, class and module a docstring. It starts with one summary line and tells the arguments, the result and the errors raised.

## A break

### Errors

- An `except:` with no type also catches `KeyboardInterrupt` and `SystemExit`. Send the change back.
- A `return`, `break` or `continue` in a `finally` block cancels the error that is in flight.
- An `assert` that checks input or drives logic breaks when Python runs with asserts off.
- A new error raised in an `except` block with no `from` hides the link to the first cause.
- Code that catches `asyncio.CancelledError` and does not raise it again breaks `TaskGroup` and `asyncio.timeout()`.

### Things that stay open

- A file, socket or lock that the code opens without `with` stays open when an error comes.
- Code that relies on `__del__` to close a resource gets no promise of when it runs. Use a `close()` method.
- `Popen.wait()` with `stdout=PIPE` can hang when the child writes much output. Use `communicate()`.
- A `Popen` call with a `timeout` does not stop the child. The code must call `proc.kill()` itself.
- A task from `asyncio.create_task()` with no strong reference can end before it finishes. Keep it in a set.
- A coroutine that is called with no `await` does not run. Python only logs a warning.
- A task that no code awaits hides its error. Python logs `Task exception was never retrieved`.
- An async generator that the code abandons closes at an odd time. Wrap it in `contextlib.aclosing()`.

### Work at the same time

- A blocking call in a `async def` function stops every other task. Use `loop.run_in_executor()`.
- Most `asyncio` objects are not thread-safe. A second thread must use `loop.call_soon_threadsafe()` or `asyncio.run_coroutine_threadsafe()`.
- Two threads that change one value with no lock or `queue.Queue` can drop a change.
- With `asyncio.gather()`, the first error reaches the caller and the other tasks keep running. Use `TaskGroup` or set `return_exceptions=True`.
- An `except TimeoutError` inside an `async with asyncio.timeout()` block does not catch the timeout. Put it after the block.

### Types and borders

- A naive `datetime` and an aware one give `TypeError` when the code compares or subtracts them. Equality gives `False`.
- A value that the code gets from `subprocess` is `bytes` unless the call sets `text=True` or `encoding`.
- A string with a user value that goes into `subprocess.run(..., shell=True)` allows shell injection. Use a list.
- A parameter typed `Any` turns off the type check for that value. Use `object` when the code does not know the type.
- A parameter typed `Optional[X]` does not mean that the caller may leave it out. It means that `None` is a good value.

### Usual faults

- A `lambda` or inner function made in a loop reads the loop name when it runs. Bind the value with a default argument.
- An `if x:` test treats `0` and an empty text as no value. Use `is None` when those are good values.
- A name that the function assigns anywhere is local in all of the function. A read before the assignment gives `UnboundLocalError`.
- Code that writes `self.count = 1` makes an instance attribute. It does not change the class attribute that other objects share.
- `a_tuple[i] += [x]` changes the list and then gives `TypeError`. The list has the new item after the error.
- A test with `is` on numbers or text can be `False` for equal values. Use `==`, and keep `is` for `None` and sentinels.
- Two modules that run `from a import name` on each other can give `ImportError`. Use `import a`, or move the import into the function.
