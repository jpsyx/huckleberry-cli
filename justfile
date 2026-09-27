# Task entry points for huckleberry-cli.
#
# `just` is a convenience, never a dependency: every recipe below is a single
# command you can also type by hand. Install it with `cargo install just`.

# Recipe arguments arrive as "$@" rather than just's own interpolation.
set positional-arguments

# Show the available tasks.
default:
    @just --list

# Compile the debug binary.
build:
    cargo build

# Run the CLI, for example: just run greet --name Ada
run *args:
    cargo run -- "$@"

# Run every test: the inline unit tests and tests/.
test:
    cargo test

# Lint with clippy (pedantic + nursery are on); any warning fails.
lint:
    cargo clippy --all-targets -- -D warnings

# Format the source.
fmt:
    cargo fmt

# Fail if anything is unformatted, without rewriting it.
fmt-check:
    cargo fmt --check

# Everything that has to pass before a change lands (reports only: `just fmt` fixes).
check: fmt-check lint test

# Build release and install the binary into $BIN_DIR (default ~/.local/bin).
install:
    ./install.sh

# Restore the agent skills pinned in skills-lock.json (run once after cloning).
skills-install:
    npx --yes skills experimental_install

# Update every agent skill this project has.
skills-update:
    ./scripts/skills/update-skills.sh
