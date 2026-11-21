# A fuzzer for the code of codeumbrella

Date: 19 November 2026. Status: design. The person gave no approval for it.

## Why

Codeumbrella reads text that its code does not make. The person writes the sheet. An AI writes the reply of each worker. A fuzzer gives a very large number of automatic inputs to the code that reads this text. It finds inputs that cause a crash or a stop.

The steps of this design come from external guides, not from the memory of an AI. The sources are at the end.

## Goals

1. Three fuzz targets, one for each text format that codeumbrella reads.
2. Each target runs in each `cargo test`, with random inputs and the saved inputs.
3. A person can run each target by hand with `cargo bolero test`, with coverage.
4. Each input that causes a crash becomes a saved input. Thus each `cargo test` tests it again.

## Not in this design

- Fuzzing in each run of the factory. The fuzzer tests the factory code only, not the projects of the factory.
- A fuzz step in the sheet, or a fuzz rules page for a worker.
- Memory checks (sanitizers). Codeumbrella forbids `unsafe`.
- A word list (dictionary) for the sheet. The bolero pages do not tell how to give one.

## Bad results, also when the fuzzer works

- `cargo test` uses 15 seconds more, or more than 15 seconds more, for the three targets together.
- A rule of `deny.toml` or of AGENTS.md becomes less strict, so that bolero can come in.
- The supervisor gets more lines of code.

## Limits

- Stable Rust 1.100.0 stays the version of `rust-toolchain.toml`.
- The supervisor has 1016 lines of code. Its limit is 1100. This design adds no line to the supervisor.
- `src/claude.rs` has 150 lines of code and `src/codex.rs` has 100. The limit for each is 200.
- Each new package must agree with the four package checks of AGENTS.md.
- `deny.toml` gives approval only to the licenses MIT, Apache-2.0 and Unicode-3.0.

## Approaches

| Tool | Good | Bad | Result |
|---|---|---|---|
| cargo-fuzz | The Rust Fuzz Book and Trail of Bits tell you to use it. All its files are in a different `fuzz/` folder. | It must have nightly Rust. Its targets do not run in `cargo test`. | The person did not select it. |
| bolero | It runs on stable Rust. Its targets run in `cargo test`. | It adds a package and a `[profile.fuzz]` block to `Cargo.toml`. Mostly one person keeps it up to date. | **The person selected it.** |
| proptest | It runs in `cargo test`. | It uses no coverage. Its page says that it is "extremely unlikely to find single-value edge cases". | Removed. |
| afl.rs | It uses AFL++. | It has no releases. Its setup uses `sudo`. | Removed. |

## Decisions of the person, 19 November 2026

1. The tool is bolero.
2. The targets run in each `cargo test`. A person also runs `cargo bolero test` by hand, for coverage.
3. Rule 1 of "Rules for growth" says: add a part only after a run shows that it is necessary. No run showed it. The person gave approval for this one part with no run. AGENTS.md records this approval.

## The design

### Step 1. The targets

| Name | Input | Code that it tests |
|---|---|---|
| `fuzz_sheet` | The text of a sheet | `yaml_serde::from_str::<Graph>`, then `Graph::check` |
| `fuzz_claude_reply` | The text of a Claude result file | The read of the last line, `src/claude.rs:99` to `src/claude.rs:117` |
| `fuzz_codex_trace` | The text of a Codex `trace` file | The read of the token lines, `src/codex.rs:106` to `src/codex.rs:119` |

Each target gets bytes from bolero. Bytes that are not UTF-8 text end the input with no test.

### Step 2. Small changes to the code

- `fuzz_sheet` uses only public functions. It changes no code.
- `src/claude.rs`: move the read of the reply text into a new public function. It gets the text and the `Numbers`, and gives the `Outcome`. It reads no file. The worker function calls it.
- `src/codex.rs`: the function `tokens` reads a file. Make a new public function that gets the text. `tokens` reads the file and calls it.
- Each new public function gets a `///` description.
- The factory does the same work as before. The tests of today must give a good result with no change.

### Step 3. The rules of each target

From the guides:

- It accepts each input: empty, very large or bad.
- An `Err` result is correct for a bad input. The target ignores it.
- It writes no file and writes nothing on the screen.
- It gives the same result for the same input.

For `fuzz_sheet`: `Graph::check` looks for the rules page of each role on the disk. The target gives it the folder `roles/` of this repository. Thus a sheet with a correct role gets to the other parts of `Graph::check`. The target writes nothing in that folder.

A crash is a `panic`, or a target that does not stop. A target that does not stop shows a circle in `Graph::check`.

### Step 4. Saved inputs

- First saved inputs:
  - `fuzz_sheet`: `graph.example.yaml`.
  - `fuzz_claude_reply`: the reply lines `LAST` and `PLAIN` of the simulated Claude in `tests/run_total.rs:444` and `tests/run_total.rs:455`.
  - `fuzz_codex_trace`: the two `turn.completed` lines `TURN_ONE` and `TURN_TWO` in `tests/run_total.rs:467` and `tests/run_total.rs:474`.
- Bolero runs the saved inputs and the crash inputs again in each `cargo test`. Git keeps them.

### Step 5. How the targets run

- In `cargo test`: each target has a time limit, with `with_test_time` or `with_iterations`. The three targets together add 15 seconds or less.
- By hand: `cargo bolero test <target> --sanitizer NONE`. This uses libFuzzer and stable Rust. The person sets the time.
- `Cargo.toml` gets the `[profile.fuzz]` block of the bolero page "Library Installation". Copy it with no change.

### Step 6. Crashes

1. Bolero saves the crash input. `cargo test` then runs it again.
2. Bolero makes the input as small as possible ("input shrinking").
3. The builder repairs the code. The saved input stays as a test.

A possible crash that the builder must look for: `src/claude.rs:106`, `src/claude.rs:107`, `src/codex.rs:116` and `src/codex.rs:117` add token numbers. A very large number causes a `panic` in a debug build. The repair that I recommend: the sum stays at the maximum value (`saturating_add`). The person can change this.

### Step 7. Documents

- AGENTS.md, "Packages": one line for `bolero`. It tells what bolero does, and the packages that come with it.
- AGENTS.md, "Structure": one line for each new test file and each saved-input folder.
- AGENTS.md, "Commands": the line `cargo bolero test <target> --sanitizer NONE`.
- AGENTS.md, "Rules for growth": the approval of 19 November 2026 for this part with no run.
- The message to the person tells why the code cannot do this work without bolero. This is check 4 of the package rule.

## Who builds it: the factory, after a download

Decision of the person, 20 November 2026: first download bolero, then give the task to the factory.

The workers of the factory have no internet. Thus a chat session does these steps before the run, in a scratch project out of the repository:

1. Install `cargo-bolero` with `cargo install cargo-bolero`.
2. Make a small scratch project with `bolero` in `[dev-dependencies]`. `cargo fetch` puts the packages on this computer.
3. Run one bolero target in the scratch project with `cargo bolero test <target> --sanitizer NONE`. The bolero pages do not tell if Apple Silicon Macs work. This step shows it.
4. Run `cargo deny check` in the scratch project with a copy of the `deny.toml` of codeumbrella.
5. If step 3 or step 4 gives a bad result, the session stops and tells the person. The person then makes a new decision.

Then the session gives the task to codeumbrella. The run copies only what is in Git. Thus the task text holds the full text of this design, not only its name. The builder adds bolero with `--offline`, because the packages are on the computer.

## Calls of the designer, after the review

The person can change each of these.

- `bolero` goes into `[dev-dependencies]`. Thus `cargo build --locked` builds no bolero code. The builder makes sure that the release build stays the same.
- A package of bolero can have a license that `deny.toml` gives no approval to. Then the builder stops and tells the person. The builder does not change `deny.toml`.
- No `panic` is correct. A repair that gives an error for that input is correct. "The factory does the same work as before" is a rule for the changes of step 2 only.
- If the crash is in a package, for example `yaml_serde` or `serde_json`, the builder stops and tells the person.
- The 15-second limit is a measured check. The builder runs `cargo test` before and after the change, and compares the clock times.
- `fuzz_sheet` reads the disk for each input. This is correct, because it gives the same result each time. It writes nothing.
- The builder copies the first saved inputs into files one time. No test writes them.
- The builder makes sure with `git check-ignore` that Git keeps the folders of the saved inputs.

## Left to the builder

- The shape of each target: a `#[test]` in one file, or a different test target as the bolero page shows.
- The folders of the saved inputs and the crash inputs. The bolero pages do not tell them.
- The exact time limit or count of inputs for each target in `cargo test`.
- The names of the new functions in `src/claude.rs` and `src/codex.rs`.

## Checks before the PR

1. The four `cargo` commands of AGENTS.md give a good result.
2. `cargo deny check` and `cargo audit` give a good result with no change to `deny.toml`.
3. Each new package had a release in the last 12 months.
4. The PR opens. Before the yes, the chat session runs `cargo bolero test` for 10 minutes on each target, on this Mac. A worker does not do this step, because of its time limit.
5. The count of lines of code of the supervisor did not change.

## Sources

- [What makes a good fuzz target](https://github.com/google/fuzzing/blob/master/docs/good-fuzz-target.md)
- [OSS-Fuzz: ideal integration](https://google.github.io/oss-fuzz/advanced-topics/ideal-integration)
- [Chromium: fuzzing with good results](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/testing/libfuzzer/efficient_fuzzing.md)
- [Trail of Bits Testing Handbook: fuzzing](https://appsec.guide/docs/fuzzing/) and [cargo-fuzz](https://appsec.guide/docs/fuzzing/rust/cargo-fuzz/)
- [Rust Fuzz Book](https://rust-fuzz.github.io/book/print.html) and [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)
- [bolero](https://github.com/camshaft/bolero), [bolero book](https://camshaft.github.io/bolero/), [check! macro](https://docs.rs/bolero/latest/bolero/macro.check.html), [DefaultEngine](https://docs.rs/bolero/latest/bolero/struct.DefaultEngine.html), [package file](https://docs.rs/crate/bolero/latest/source/Cargo.toml.orig)
- [proptest](https://github.com/proptest-rs/proptest) and [afl.rs](https://github.com/rust-fuzz/afl.rs)
