#!/usr/bin/env bash
# Installs the codeumbrella command and the chat command for Claude and Codex.
# Run it from the codeumbrella folder: ./install.sh
set -euo pipefail
cd "$(dirname "$0")"

for tool in cargo git gh; do
  command -v "$tool" >/dev/null || { echo "Not found: $tool. Install it first."; exit 1; }
done
command -v claude >/dev/null || command -v codex >/dev/null ||
  { echo "Not found: claude or codex. Install one of them first."; exit 1; }

cargo install --path . --locked
command -v codeumbrella >/dev/null ||
  echo "Your shell does not find the codeumbrella command. Add $HOME/.cargo/bin to your PATH."

links=0
for home in "$HOME/.claude" "$HOME/.codex"; do
  [ -d "$home" ] || continue
  mkdir -p "$home/skills"
  link="$home/skills/codeumbrella"
  # A folder that is not a link has a different copy of the chat command.
  if [ -d "$link" ] && [ ! -L "$link" ]; then
    echo "Stop: $link is a folder, not a link. Remove it, then run ./install.sh again."
    exit 1
  fi
  ln -sfn "$PWD/skills/codeumbrella" "$link"
  echo "The chat command /codeumbrella is in $home/skills."
  links=$((links + 1))
done
if [ "$links" -eq 0 ]; then
  echo "No chat command was installed: there is no folder $HOME/.claude or $HOME/.codex."
  echo "Start Claude or Codex one time, then run ./install.sh again."
  exit 1
fi
echo "Complete. In a project, type /codeumbrella and your task."
echo "Keep this folder where it is: the chat command is a link to it."
