#!/usr/bin/env bash
# Install or update huckleberry-cli as a command on this machine.
#
# It has to work for a stranger who just cloned the repository: when something
# is missing, or the result would not be runnable, say so and print the fix.
set -euo pipefail

usage() {
  printf '%s\n' 'Install or update huckleberry-cli as an executable command.

Usage: ./install.sh [--name <command>] [-h|--help]

Options:
  --name <command>  Command filename (default: huckleberry-cli).
  -h, --help        Show this help without installing anything.

Environment:
  BIN_DIR           Installation directory (default: $HOME/.local/bin).

Examples:
  ./install.sh
  BIN_DIR="$HOME/bin" ./install.sh --name huckleberry-cli-dev'
}

binary_name="huckleberry-cli"
command_name="$binary_name"
while (($#)); do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    --name)
      if (($# < 2)); then
        usage >&2
        exit 2
      fi
      command_name="$2"
      shift 2
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

# A command name that is empty, hidden, a path, or full of shell metacharacters
# is a mistake worth catching before anything is built.
case "$command_name" in
  '' | [.-]* | *..* | *[!a-zA-Z0-9_.-]*)
    usage >&2
    exit 2
    ;;
esac

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# This installs from source, so a Rust toolchain is the one prerequisite. Say
# so plainly rather than letting the build die on "cargo: command not found".
if ! command -v cargo >/dev/null 2>&1; then
  echo "error: 'cargo' not found; huckleberry-cli builds from source and needs a Rust toolchain." >&2
  echo "       Install one from https://rustup.rs, then re-run this script." >&2
  exit 1
fi

bin_dir="${BIN_DIR:-$HOME/.local/bin}"
mkdir -p "$bin_dir"
bin_dir="$(cd -- "$bin_dir" && pwd)"
installed="$bin_dir/$command_name"
if [[ -d "$installed" ]]; then
  echo "error: cannot replace directory: $installed" >&2
  exit 1
fi

echo "Building huckleberry-cli (release)..." >&2
(cd "$script_dir" && cargo build --release) >&2

built="$script_dir/target/release/$binary_name"
if [[ ! -x "$built" ]]; then
  echo "error: the release build produced no executable at $built." >&2
  exit 1
fi

# A fixed filename, written through a temporary file: every run overwrites the
# previous binary in place, so this script is also the updater and never leaves
# a second copy behind.
temporary="$(mktemp "$bin_dir/.install.XXXXXXXX")"
trap 'rm -f -- "$temporary"' EXIT
install -m 0755 "$built" "$temporary"
mv -f -- "$temporary" "$installed"

echo "installed $command_name -> $installed" >&2

# A binary nobody can invoke is not an install. Say so, with the fix.
case ":${PATH}:" in
  *":$bin_dir:"*) ;;
  *)
    echo >&2
    echo "note: $bin_dir is not on your \$PATH, so \`$command_name\` will not be found yet." >&2
    echo "      Add it to your shell startup file, for example:" >&2
    echo "        echo 'export PATH=\"$bin_dir:\$PATH\"' >> ~/.zshrc   # or ~/.bashrc" >&2
    ;;
esac
