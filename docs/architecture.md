# Architecture

huckleberry-cli is a single crate with two targets: a library that holds
everything worth testing, and a binary that is a shell around it. Extend this
document as the shape changes; it is meant to grow, not to be replaced.

## The layout

```
src/
├── main.rs          the binary: parse, dispatch, report, exit
├── lib.rs           the library root: module list and the product name
├── cli.rs           the clap types: every flag and subcommand
├── config.rs        the settings: their shape, their file, their defaults
├── prompt.rs        asking for a value that was left out
├── theme.rs         semantic colors for human-facing output
└── commands/
    ├── mod.rs       dispatch, plus the narration commands share
    ├── greet.rs     a command that asks when a value is missing
    ├── info.rs      a command whose output is data
    └── config/
        ├── mod.rs   routing for the `config` subcommand
        ├── show.rs  the effective settings, one key=value line each
        └── set.rs   change one setting and save it
tests/
└── public_api.rs    the library surface, driven from outside the crate
```

## Why a library plus a thin binary

`main.rs` does four things: parse the arguments, call `commands::run`, print a
failure on stderr, and return an exit code. Everything else is in the library,
so a test can reach it by calling a function instead of spawning a process.
`tests/public_api.rs` is that test: it drives the same public surface the
binary does, which is also a check that the surface is usable from outside.

## How a value is resolved

Every value a command needs is looked for in the same order, and `greet` is
the worked example:

1. the flag or argument (`--name Ada`), which is what an agent or a script
   passes and what makes every action reachable without a prompt;
2. the configuration file (`name = "Ada"`), which is how a person avoids
   typing the same flag every day;
3. a question, asked through `src/prompt.rs`, when there is a terminal to ask
   on;
4. a failure naming the flag, when there is not.

Steps 1 and 2 are pure functions (`commands::greet::resolve_name`), step 3 is
the one module allowed to read from the terminal, and step 4 is the reason an
agent never hangs on a question it cannot see.

## The pure/impure split

The split runs through every module, and it is the reason the test suite is
fast and boring:

| Pure, tested inline | Impure, deliberately thin |
| --- | --- |
| `cli.rs`: the argument types, and that they parse | `main.rs`: process plumbing |
| `theme.rs`: which role gets which color | `config::load` / `config::save`: reading and writing the file |
| `config`: parsing, validating a setting, where the file lives | `prompt::ask`: the terminal read/write loop |
| `prompt::interpret` / `render`: what an answer means, and how a question looks | `commands/*::run`: printing, prompting, saving |
| `commands::greet::greeting` and `resolve_name`: the message and where the name came from | |
| `commands::info::facts`: the facts and their order | |
| `commands::config::show::lines`: the shape of the output | |

When one of the thin shells grows an `if` that matters, that `if` moves into a
pure function next to it before it grows a second one.

## Output channels

Stdout carries data, stderr carries the conversation with the person: prompts,
progress, failures, and every confirmation. That is why `config show` writes
`key=value` lines on stdout while `config set` reports what it saved on
stderr, and why `theme.rs` decides whether to emit color by looking at stderr.
See `docs/rules/cli-ux.md`.

## The configuration file

The settings are one `serde` struct with defaults (`src/config.rs`), stored as
TOML under the XDG configuration directory: `$XDG_CONFIG_HOME/<package>/
config.toml`, or `~/.config/<package>/config.toml`. `--config <PATH>` (or
`CLI_CONFIG`) points every command at a different file, which is what tests
and scripts use so they never touch the real one. A missing file is not an
error: it means nothing has been configured yet, so the defaults apply.

Adding a setting is one field on `Config`, one arm in `Config::set`, one line
in `Config::describe`, and its name in `Config::KEYS`. The tests that walk
`KEYS` then cover it everywhere: `config show` prints it, `config set` offers
it, and the round trip through the file keeps it.

## The dependencies, and what each is for

| Crate | Why it is here |
| --- | --- |
| `anyhow` | One error type across the program, with a `.context(...)` chain the binary prints as the failure message. |
| `clap` (derive, env) | The command-line surface, declared next to the types it fills. `env` lets any flag also come from the environment. |
| `serde` (derive) | The configuration struct is (de)serialized rather than parsed by hand, so a new setting is one more field. |
| `toml` (preserve_order) | The configuration file format, because a file a person edits should look like one. `preserve_order` keeps a table in the order it was read rather than sorting it. |

The set is small on purpose (`docs/rules/rust.md`). Adding to it means saying
here what the crate is for, in one line, in the same change.

## Things to rename once

- `CLI_VERBOSE` and `CLI_CONFIG`, the environment variables behind `--verbose`
  and `--config` in `src/cli.rs`. Give them a prefix of your own when the
  tool's name settles.
- `APP_NAME` in `src/lib.rs` is the name people read; the crate name is what
  they type.
