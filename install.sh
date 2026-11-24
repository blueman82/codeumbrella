#!/usr/bin/env bash
# Installs the codeumbrella command and the chat command for Claude and Codex.
# Run it from the codeumbrella folder: ./install.sh
set -euo pipefail
cd "$(dirname "$0")"

# The store with the built command of each commit of `master`. The store is
# optional: with no file there, this script builds the command on this computer.
store="${CODEUMBRELLA_STORE:-https://dl.cloudsmith.io/public/tester-mjrq/tester}"
name=codeumbrella-aarch64-apple-darwin

# need <tool> <what it is> <how to install it>
not_found=0
need() {
  command -v "$1" >/dev/null && return 0
  echo "Not found: $1 ($2)."
  echo "  Install it with:  $3"
  not_found=1
}
stop_if_not_found() {
  [ "$not_found" -eq 0 ] && return 0
  echo "Then open a new terminal and run ./install.sh again."
  echo "Nothing was installed."
  exit 1
}

need git "the version control tool" "xcode-select --install"
need gh "the GitHub tool" "brew install gh   (or see https://cli.github.com). Then run: gh auth login"
command -v claude >/dev/null || command -v codex >/dev/null || {
  echo "Not found: claude or codex (the AI tool of the workers). One of the two is sufficient."
  echo "  Install Claude Code with:  curl -fsSL https://claude.ai/install.sh | bash"
  echo "  Or install Codex with:     npm install -g @openai/codex"
  not_found=1
}
stop_if_not_found
echo "Tools: git, gh and an AI tool found."

# Gets the built command of the commit of this folder. A bad result means
# "build on this computer", for each cause.
download() {
  [ "$(uname -sm)" = "Darwin arm64" ] || { echo "the built command is only for a Mac with Apple hardware"; return 1; }
  git diff --quiet HEAD 2>/dev/null || { echo "this folder has changes of its own, or it is not a git copy"; return 1; }
  local commit file tmp
  commit="$(git rev-parse HEAD)"
  file="codeumbrella-$commit-aarch64-apple-darwin.tar.gz"
  tmp="$(mktemp -d)"
  curl -fsSL -o "$tmp/$file" "$store/raw/names/$name/versions/$commit/$file" 2>/dev/null &&
    curl -fsSL -o "$tmp/$file.sha256" "$store/raw/names/$name-sha256/versions/$commit/$file.sha256" 2>/dev/null ||
    { echo "the store has no file for this commit"; return 1; }
  (cd "$tmp" && shasum -a 256 -c "$file.sha256" >/dev/null 2>&1) ||
    { echo "the checksum of the downloaded file does not agree. The script does not use that file"; return 1; }
  tar -xzf "$tmp/$file" -C "$tmp" && "$tmp/codeumbrella" --version >/dev/null 2>&1 ||
    { echo "the downloaded command does not run on this computer"; return 1; }
  mkdir -p "$HOME/.cargo/bin"
  cp "$tmp/codeumbrella" "$HOME/.cargo/bin/codeumbrella"
}

if cause="$(download)"; then
  echo "Download of the built command for this commit: found. Checksum: OK."
else
  echo "No download of the built command: $cause."
  echo "The script builds the command on this computer."
  need cargo "the Rust build tool" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  stop_if_not_found
  # `--force`: a command from a download before is replaced.
  cargo install --path . --locked --force
fi
echo "The codeumbrella command is installed in $HOME/.cargo/bin."
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
