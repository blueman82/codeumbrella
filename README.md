# Codeumbrella

Codeumbrella is a software factory. You give it a task. It gives the task to workers (AI programs). They plan, write and check the code. At the end, good code goes into `main`.

The factory stops two times for you. First, you look at the tests. Second, you approve the pull request (PR, a request to add code to `main`). You answer yes or no each time.

## What the computer must have

- Rust with `cargo`, to build the `codeumbrella` command.
- `git`, to keep the versions of the code.
- `gh`, the GitHub command, to open the PR.
- `claude` (Claude Code) or `codex` (Codex). These start the workers.

## Install

The plugin brings only the chat command `/codeumbrella`. The `codeumbrella` command is a different part. Install it in step 1.

1. Install the command. In this folder, run one of these:
   - `./install.sh` (it also links the chat command into `~/.claude` and `~/.codex`)
   - `cargo install --path . --locked`
2. Install the plugin for the chat command.
   - Claude Code:
     ```
     /plugin marketplace add <path or GitHub name of this folder>
     /plugin install codeumbrella
     ```
   - Codex:
     ```
     codex plugin marketplace add <path or GitHub name of this folder>
     ```
     Then add the `codeumbrella` plugin from that marketplace.

## First run

1. Go to your project folder. Commit your changes first.
2. Run `codeumbrella init`. It puts the factory files in `.codeumbrella/`. You can change them.
3. Give the task. Use one of these:
   - In a chat: `/codeumbrella <task>`
   - In a terminal: `codeumbrella run "<task>"`
4. When the run stops, answer yes or no:
   - In a chat, tell the chat your answer.
   - In a terminal, run `codeumbrella answer <run-id> yes` or `codeumbrella answer <run-id> no "why"`.

To see where a run is, run `codeumbrella status`.
