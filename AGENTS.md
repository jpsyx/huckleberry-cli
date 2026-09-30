# Agent Rules

## Stack

- This project is a Rust workspace (edition 2024) with two packages. There is
  no web server and no browser: the interface is the terminal.
- `crates/huckleberry-api` is a reusable client for the Huckleberry baby
  tracking app. **It depends on nothing else in this repository**, returns
  typed errors rather than `anyhow`, and prints nothing. Keep it that way: it
  is meant to be publishable on its own. Read
  [`docs/api.md`](docs/api.md) before changing it.
- **That crate is a port of
  [py-huckleberry-api](https://github.com/Woyken/py-huckleberry-api) by Woyken,
  MIT licensed.** The schema, the field names, the units and the operation
  semantics are its original research. Keep the credit in `README.md`,
  `NOTICE`, `docs/api.md` and the crate docs intact and accurate, and when you
  port something new from it, say so.
- `src/` is the command-line tool. The command-line surface is **clap**
  (derive), the settings are a **serde** struct stored as **TOML**, the
  full-screen dashboard is **ratatui**, and failures travel as **anyhow**
  errors carrying the context that produced them.
- `src/main.rs` is deliberately thin. It parses arguments and hands them to the
  library in `src/`, so every decision is reachable from a test without
  spawning a process. Read [`docs/architecture.md`](docs/architecture.md)
  before adding a module.
- **`h` with no command opens a full-screen ratatui app (`src/tui/`), and it is
  the primary way people use this tool.** Its design brief is
  [`docs/tui.md`](docs/tui.md), and it is written for a parent at 3am, in the
  dark, with one hand free.
- **The shell's Now drawer shows exactly what `h now` prints: same facts, same
  wording, same order.** `render::now::screen` is the one function that decides
  what either says. A request to change what the Now drawer shows is a request
  to change `h now`, and the other way round; never change one alone.
- **A command never takes the screen away from the shell.** It runs in the
  panel where the menu was, drawing its questions through
  `src/prompt/host.rs` so there is one implementation of every prompt rather
  than one for the terminal and one for the shell. Output goes through
  `render::print` and `render::note`, which are the only two places this tool
  writes, so a host can take them. **There are no exceptions**: the dashboard
  is drawn in the panel too, from the reading the shell already has.
- **Typical ranges live in [`data/reference.toml`](data/reference.toml)**, one
  band per age, each naming where it came from. There is no band for milk
  volume at any age and there must never be one: see the file's own header for
  why. Labels state what is typical and never tell anybody what to do.
- **The family's day (`day_start`, `day_end`, `day_mode`) governs arithmetic,
  never timestamps.** Anything that aggregates uses it: `summary`, `trends`,
  `stripes`, the running totals. Anything that reports an individual entry uses
  an ordinary calendar day, midnight to midnight: `log`, `edit`, `delete`.
  "When did this happen" has one answer and it is the one on the clock. A new
  screen joins the rule by asking whether it aggregates. See
  [`docs/setup.md`](docs/setup.md). **Read it before designing any feature, not only
  ones inside `src/tui/`**: menus over typing, a sensible default already
  highlighted so Enter alone answers, an arrow and a letter for every
  direction, and nothing ever reachable only from the command line. A feature
  that ships with a flag and no route through the shell is unfinished.
- **`src/domain` never reads the clock, touches the network, or writes to a
  terminal**, and `src/render` returns lines rather than printing them. Every
  function takes `now` as an argument. That is what makes the date arithmetic
  testable, and it is not negotiable.
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
  read all of the source. The current set is
  [`architecture.md`](docs/architecture.md), [`api.md`](docs/api.md),
  [`cli.md`](docs/cli.md), [`dashboards.md`](docs/dashboards.md),
  [`decisions.md`](docs/decisions.md), [`nomenclature.md`](docs/nomenclature.md),
  [`setup.md`](docs/setup.md), [`skills.md`](docs/skills.md) and
  [`tui.md`](docs/tui.md).
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

- Read [`docs/tui.md`](docs/tui.md). It applies to every user-facing change,
  because the shell is how this tool is used; treat its rules as requirements
  rather than as aspirations.
- Use the words in [`docs/nomenclature.md`](docs/nomenclature.md) for the
  parts you are changing, and add to that file when a new part gets a name. A
  part with two names grows two implementations.
- Determine which other files in `docs/` are relevant to read.
- Determine which available skills are relevant. Run
  `npx skills list` to see what this project has installed.
- Determine which tests, if any, need to be written: for a feature, the ones
  that describe the behavior; for a bug, the one that reproduces it.

**Write every change, feature or fix, using red/green TDD by default:**

1. **Red**: write a failing test that describes the desired behavior, and run
   it to confirm it fails for the expected reason before writing any
   implementation.
2. **Green**: write the minimum implementation needed to make the test pass,
   and run the test to confirm it passes.
3. **Refactor**: clean up the implementation while keeping the tests green.

**A bug fix is red/green too, and it is where the rule matters most.** The
moment a bug is diagnosed, write the test that reproduces it and watch it fail
for that reason before touching the fix. Diagnosing is not a licence to skip
ahead: a diagnosis is a hypothesis, and a test that fails the way you predicted
is what turns it into a fact. A fix that lands without one leaves nothing
behind to stop the bug coming back, and no evidence it was ever the cause.

If the buggy behavior is buried in something impure and awkward to reach, that
is a reason to pull the decision out into a pure function and test that, not a
reason to skip the test.

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
cargo run -- now                              # just run now
cargo run -- config show                      # just run config show

# the full test suite for both packages: inline unit tests plus tests/
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
- [`docs/tui.md`](docs/tui.md): who the tool is for. One hand, in the dark,
  holding a baby. Menus before typing, Enter as a complete answer, an arrow and
  a letter for every direction, and no functionality that the full-screen shell
  cannot reach.

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
