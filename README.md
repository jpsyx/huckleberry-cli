# huckleberry-cli

A command-line tool written in Rust (edition 2024). clap declares the
command-line surface, a serde struct stored as TOML holds the settings, and
anyhow carries errors with the context that produced them. Tasks run through
`just`, and `./install.sh` builds a release binary and installs it on your
PATH.

## Getting started

A Rust toolchain is the only prerequisite ([rustup.rs](https://rustup.rs)).
`just` is optional (`cargo install just`): every recipe is one command you can
also type by hand.

```sh
# run it (with no subcommand it prints its help)
cargo run -- --help
cargo run -- greet --name Ada                 # just run greet --name Ada
cargo run -- config show                      # just run config show

# the full test suite: the inline unit tests plus tests/
cargo test                                    # just test

# lint clean (clippy pedantic + nursery are on, warnings are errors)
cargo clippy --all-targets -- -D warnings     # just lint

# format
cargo fmt                                     # just fmt

# everything that has to pass before a change lands
just check

# install the binary at $BIN_DIR/<name> (default ~/.local/bin)
./install.sh                                  # just install
```

## Agent skills

Agent skills are not tracked in git, but `skills-lock.json` is.
`just skills-install` restores them after a clone.
Run `npx skills list` to see what is installed and
`just skills-update` to upgrade them. See
[`docs/skills.md`](docs/skills.md) for how the two skill managers
divide the work.

## Agent rules

Coding conventions live in `AGENTS.md`, which is the single source of truth.
`CLAUDE.md` (Claude Code) and `.cursor/rules/agents.mdc` (Cursor) are symlinks
to it, and it is also the file the Codex CLI reads natively. Update `AGENTS.md`
and every tool stays in sync.
