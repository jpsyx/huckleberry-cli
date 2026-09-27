# Agent Rules

## Stack

- This project is a command-line tool written in Rust (edition 2024) and built
  with cargo. There is no web server and no browser: the interface is the
  terminal.
- The command-line surface is **clap** (derive), the settings are a **serde**
  struct stored as **TOML**, and failures travel as **anyhow** errors carrying
  the context that produced them.
- `src/main.rs` is deliberately thin. It parses arguments and hands them to the
  library in `src/`, so every decision is reachable from a test without
  spawning a process. Read [`docs/architecture.md`](docs/architecture.md)
  before adding a module.
- Every action is reachable with flags alone. A value left out on a terminal is
  asked for through `src/prompt.rs`, never demanded through a prompt an agent
  cannot answer.
- Tasks live in the `justfile` (`just --list`). Every recipe is a single
  command you could type by hand, so `just` is a convenience and never a
  dependency.

## Documentation

- Use `docs/` for architectural notes, design decisions, functionality
  overviews, and checklists (for example `docs/<topic>.md`). These docs exist
  so future humans and LLMs can learn the codebase quickly without having to
  read all of the source.
- **Keep the docs current as you build. This is a rule, not a suggestion.**
  Whenever you add, change, or remove a feature, module, route, data model, or
  architectural boundary, create or update the relevant file(s) in `docs/` as
  part of the same change. Treat updating the docs as part of the definition of
  done, not an afterthought.
  - New capability or subsystem: add or extend the `docs/` file that covers it.
  - Changed behavior, API, schema, or architecture: update the affected doc so
    it reflects reality. Do not leave stale descriptions behind.
  - Removed feature: delete or revise the parts of `docs/` that described it.
- Write docs at a high level: what a module or feature does, how the pieces fit
  together, and why the key decisions were made. Do not restate the code
  line by line.
- Before writing code, read the relevant files in `docs/` first (see
  "Implementation approaches").
- If Context7 MCP is configured, use it to reference the most up-to-date
  documentation of any library when you need it.

## Scope

- Only implement what is requested. Do not fix other bugs, clean up any other
  code, or do any refactors outside of what you were specifically asked to do.
- Only modify the files or directories that you are told to work on.
- If you absolutely must make modifications outside of the scope of
  files/directories you were told, then output a list of the files you changed
  that were outside of the requested scope of files. Include a 1-sentence
  explanation for each file about what changed.

## Implementation approaches

Before writing code:

- Determine which files in `docs/` are relevant to read.
- Determine which available skills are relevant. Run
  `npx skills list` to see what this project has installed.
- Determine which tests, if any, need to be written to test the requested
  functionality.

**Implement functionality using red/green TDD by default:**

1. **Red**: write a failing test that describes the desired behavior, and run
   it to confirm it fails for the expected reason before writing any
   implementation.
2. **Green**: write the minimum implementation needed to make the test pass,
   and run the test to confirm it passes.
3. **Refactor**: clean up the implementation while keeping the tests green.

As a rule, do not write implementation code before there is a failing test for
it. You may skip TDD only when writing a test adds no real value, for example:

- The change is trivial (e.g. copy tweaks, styling, renaming, config).
- The only test you could write would be redundant with existing coverage.
- The test would be tautological, asserting the implementation restates itself
  (e.g. simply checking that a variable is set, or that a hardcoded variable
  actually has the value we wrote).

When in doubt, write the test.

## Build, test, lint

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

## General Code Style & Formatting

## Comments

- Do not use em dashes (—). Prefer a colon for explanations, or a hyphen (-)
  as a short dash for aside explanations where you would have used an em dash.
- Use block comments or docstrings to document exported or public interfaces,
  constants, objects, functions, and classes.

## Naming conventions

- Follow naming conventions for the language you are using.
- Use descriptive variable names with auxiliary verbs (e.g., isLoading,
  hasError).
- Avoid abbreviated names, such as `val`, use the full word `value`, unless
  this were to cause a naming collision with another variable in scope.
- Avoid vague names like `next`, `prev`, or `n`, that don't say what the
  variable actually actually holds. Always include a noun, such as `nextPage`,
  `prevRow` or `numPeople`.
- Builder functions for objects or classes should be named `create{Type}`.
  E.g. `createUser`
- Builder functions for strings or primitives should be named `build{Thing}`.
  E.g. `buildRoleKey`
- Builder functions that take some seed data to build an output should use the
  `*From{Seed}` format. E.g. `createUserFromId` or `buildKeyFromRole`
- Conversion or cast functions should use "to". E.g. `roleToDisplayLabel`
  or `app_type_to_key`.

## Functions & Logic

- Keep functions short (<= 45 lines).
- Extract logic into utility functions if:
  - The function will be too long otherwise
  - The logic will be reused

## Language & framework rules

The rules for this project's language and frameworks live in `docs/rules/`:

- [`docs/rules/rust.md`](docs/rules/rust.md): how Rust is written here. No
  `unsafe`, clippy clean with pedantic and nursery on, decisions in pure
  functions, one module per file.
- [`docs/rules/cli-ux.md`](docs/rules/cli-ux.md): how the tool talks. Stdout is
  data and stderr is the conversation, every action is scriptable, every value
  a person omits is asked for, every color is semantic.

## Agent skills

- This project's agent skills are installed by two tools and neither should be
  driven by hand: `npx skills` for everything in `skills-lock.json`, and
  `npx impeccable` for the `impeccable` skill, which ships its own installer.
- **Never create or edit anything under `.agents/`, `.claude/skills/`,
  `.cursor/skills/`, or `.opencode/`.** Those directories are generated, and
  they are gitignored: `skills-lock.json` is the only skills file in git.
- `just skills-install` restores them after a clone.
  `npx skills list` lists what is installed,
  `just skills-install` is the same step run on demand, and
  `just skills-update` upgrades everything to the latest.
- `just skills-update` runs `scripts/skills/update-skills.sh`, which
  updates both managers: `npx skills` for the lock, and each self-installing
  skill (the list at the top of the script) through its own CLI. Update skills
  there, never by calling one manager by hand.
- To add or remove a skill, use `npx skills add` / `npx skills remove` and
  commit the resulting `skills-lock.json` change in the same commit.
- The wrapper lives in `scripts/skills`. See [`docs/skills.md`](docs/skills.md)
  before changing it.
