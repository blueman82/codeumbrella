# General standards: good practice for a reader of a change

The six principles are KISS, DRY, YAGNI, one source of truth, plain direct code and no boilerplate. Each one gives points that a reader can check at a line.

Use it only after the rules of `AGENTS.md` and `CLAUDE.md` of the project.

The layout tool and the linter of the project check layout and names, so this page has none of that.

## Write the code like this
- Write the least code that does what the spec says. Use a loop, a map or a function before a class or a layer.
- Use the standard library before a new helper or package. Look in the project for a helper that does the job.
- Add no code for a future wish that the spec does not name. Code for a guess about the future costs build time, reading time and repair time.
- Add an option, a parameter or a layer only when a caller uses it today.
- Give each fact or rule one home. Other code reads it from that home.
- Give a value that stands for a meaning a name, such as a constant. Do not write `1` for an admin role.
- Write the same logic one time. When one change means the same edit in two spots, move the logic to one function.
- Do not add a flag or a branch to a shared helper so that a new caller fits. Give that caller its own code.
- Write code that a reader follows from top to bottom. The reader should not hold the lines above in mind.
- Name a block by its purpose when a reader must study it to see what it does.
- Call a function directly when a wrapper adds no logic of its own.
- Delete code that nothing uses: a variable, a parameter, a method or a class.
- Write a comment for the why: a limit, a trick of the platform, or a business rule. Do not write a comment that repeats the code.
- Handle each error on purpose. Correct it, or add the context that the error lacks, or give it to the caller.
- Match the pattern of the code near the change. Do not add a second style.
- Keep the change to one task. Do a large refactor in its own change.

## A break

### KISS
- A class or a layer with one user that holds no logic of its own. A plain function does the same work.

Not a break:
- A long function is not a break by its length alone. A smell is a hint to look deeper, not a proof.
- A regex or a complex algorithm with a comment that explains it.

### DRY
- The same logic is in two or more spots of the change. A fix of one spot means the same fix in the other.
- The same condition is in many branches, or the same lines end each branch.
- The change copies logic that the project has in a second file.
- A shared helper has a flag or a branch for each caller. The helper is a bad abstraction, and the fix is to copy it back into the callers.

Not a break:
- Two lines that look the same but change for different causes. DRY is about one piece of knowledge, not the same text.
- A repeated check in simple tests, when the repeat makes each test easy to read.

### YAGNI
- Code in the change that no point of the spec uses.
- A hook, an option or a setting that no caller sets, written for a guess.
- A layer, an interface or a base class with one user, built for a future user.
- A name or a branch for a feature that the task does not have.

Not a break:
- Work that makes the code easier to change, such as a refactor or a test. YAGNI does not apply to it.
- Code of a framework or a library that callers in other projects use.
- A hook that a test uses.

### One source of truth
- One value has two homes. A literal is in the code and again in a setting, a test helper or a second file.
- A coded value has no name, such as `1` for a role or a string for a field name.

Not a break:
- Two values that are equal today but change for different causes. They are two facts, not one.

### Plain direct code
- A wrapper function or class that only forwards a call to one other function.
- A comment that says what the next line says.

Not a break:
- A comment that explains why the code is as it is.
- A comment on a complex algorithm that no simpler code can replace.
- A wrapper that cuts a dependency between two parts. A proxy or a decorator also adds a layer on purpose.
- A long function that reads well.

### No boilerplate
- An unused variable, parameter, field, method or class.

Not a break:
- A proxy, a decorator or a wrapper that cuts a dependency.
- A hook that a framework caller or a test uses.
- A repeated line in a simple test that keeps the test easy to read.
