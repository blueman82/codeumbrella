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

for home in "$HOME/.claude" "$HOME/.codex"; do
  [ -d "$home" ] || continue
  mkdir -p "$home/skills"
  ln -sfn "$PWD/skills/codeumbrella" "$home/skills/codeumbrella"
  echo "The chat command /codeumbrella is in $home/skills."
done
echo "Complete. In a project, type /codeumbrella and your task."
