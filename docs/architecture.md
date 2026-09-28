# Architecture

Two packages in one workspace, and they know as little about each other as
possible.

```
Cargo.toml                   the workspace, and the CLI package
crates/huckleberry-api/      a reusable Huckleberry client
src/                         the command-line tool
```

`huckleberry-api` depends on nothing in this repository. It reads no
configuration file, prints nothing, and returns typed errors rather than
`anyhow`, so any Rust program can take it as a dependency. The CLI is its
first consumer and happens to live beside it.

## The library: `crates/huckleberry-api`

```
src/
├── lib.rs           the crate docs and the re-exports
├── auth.rs          Firebase sign-in, and renewing five minutes early
├── client.rs        `Huckleberry`: the session, the timezone, the clock
├── constants.rs     the Firebase project the app runs on
├── error.rs         one enum, with the distinctions a caller acts on
├── ids.rs           the identifier shapes the app itself writes
├── macros.rs        `string_enum!`: a fixed set of spellings, plus `Unknown`
├── paths.rs         where each thing lives in Firestore
├── timezone.rs      the offset-in-minutes every row carries
├── firestore/
│   ├── client.rs    the five REST requests everything is made of
│   ├── field_path.rs  update masks, and the backticks they need
│   ├── query.rs     structured queries
│   └── value.rs     the tagged wire format, in and out
├── models/          one module per collection, all plain serde
│   ├── sleep/       details, timer, interval
│   ├── feed/        units, timer, prefs, interval
│   └── …            one file each for the smaller trackers
└── ops/             the operations, one module per tracker
    └── feed/        nursing (a timer) and bottle (an instant event)
tests/
├── support/         a stub Firestore on a loopback socket
├── read_requests.rs what each read asks for
└── write_requests.rs what each write sends
```

Read [`api.md`](api.md) before changing it: it maps every method to the Python
original and records the four places the two deliberately differ.

## The tool: `src/`

```
src/
├── main.rs          parse, dispatch, report, exit
├── lib.rs           the module list
├── cli/             the clap surface: shape, value words, subcommand trees
├── config.rs        the settings: shape, file, defaults
├── credentials.rs   the secrets: a 0600 file, and the environment
├── session.rs       from a configuration file to a working client
├── dataset.rs       pulling one child into one value, and the snapshot format
├── edit.rs          what an edit is: which row, which fields, what they hold
├── prompt.rs        asking for a value that was left out
├── prompt/time.rs   shared clock-time and relative-start questions
├── theme.rs         semantic colours
├── domain/          everything the tool works out, and nothing it prints
│   └── clock.rs     a time somebody typed, and the instant it means
├── render/          domain values to lines of text
├── dashboard/       the full-screen version of the same values
├── picker/          choosing one entry off a list, full screen
└── commands/        one thin module per command
tests/
└── public_api.rs    the library surface, driven from outside the crate
```

## The four layers, and why the split is where it is

| Layer | Knows about | Never |
| --- | --- | --- |
| `huckleberry-api` | Firestore, Huckleberry's shapes | this tool |
| `domain` | events, days, arithmetic | the clock, the network, a terminal |
| `render` / `dashboard` / `picker` | text, colour, widgets | the network, the clock |
| `commands` | all of the above, and the outside world | arithmetic worth asserting |

`domain` takes `now` as an argument everywhere. That is the whole reason a
sleep across midnight, a night window that spans midnight and a 23-hour day are
testable rather than seasonal. Nothing in `domain` or `render` reads a clock, so
every screen in this repository is asserted line by line in a test.

## How a value is resolved

Every value a command needs is looked for in the same order:

1. the flag (`--amount 90`), which is what an agent or a script passes;
2. the configuration file, which is how a person avoids typing the same flag
   every day;
3. a question, asked through `src/prompt.rs`, when there is a terminal;
4. a failure naming the flag, when there is not.

Optional timer starts keep their noninteractive default of now.
`prompt/time.rs` shares the clock reader and AM/PM question between manual
sleeps and timer starts; `domain/clock.rs` parses relative minutes using an
explicit `now`. Timer starts resolve against the clock after the answer is read.

Step 4 is why an agent never hangs on a question it cannot see. See
[`rules/cli-ux.md`](rules/cli-ux.md).

## Where the secrets are

`config.toml` holds settings and no secret: it is safe to show somebody, paste
into a bug report, or check into a dotfiles repository. The email, the password
and the session live in `credentials.toml` beside it, written with mode `0600`
before anything is put in it.

`HUCKLEBERRY_EMAIL` and `HUCKLEBERRY_PASSWORD` override the file, and a
password that came from the environment is never written back to disk.

## Changing something already recorded

Reading a row and changing it are different requests, and the difference is
structural: a decoded row carries nothing that says which document it came out
of, and Huckleberry packs older history into batch documents with the rows
nested under `data`. So every windowed read hands back `Located<T>`: the row,
and a `RowRef` naming its document and, inside a batch, its key. `normalize`
puts that reference on the event, the snapshot carries it, and `hb edit` takes
it back to `update_history_row`.

The write is a field update rather than a rewrite. A row has fields this
repository does not model, and a batched row has neighbours in the same
document; naming the fields that change is what leaves both alone. See
[`api.md`](api.md).

## Reading without an account

`export` writes a snapshot, and `--offline <PATH>` makes any read-only command
take that snapshot instead of the network. It is not a fallback but a different
source, chosen deliberately: nothing under `--offline` opens a socket. That is
what makes the screens demonstrable, and what makes the tests in `tests/`
possible without credentials.

## The dependencies, and what each is for

### The library

| Crate | Why it is here |
| --- | --- |
| `reqwest` (rustls) | Firestore over REST. rustls keeps OpenSSL out of the build. |
| `serde`, `serde_json` | Every model is a serde struct, and the Firestore codec converts through `serde_json::Value` rather than hand-walking each type. |
| `thiserror` | Typed errors, because a library that returns `anyhow::Error` makes its callers match on message text. |
| `jiff` | IANA timezones, for the offset every Huckleberry row carries. |
| `uuid` | The identifier shapes the app itself writes. |
| `tokio` (`time`) | Polling a document on an interval. The caller owns the runtime. |

### The tool

| Crate | Why it is here |
| --- | --- |
| `anyhow` | One error type across the program, with a `.context(...)` chain the binary prints as the failure message. |
| `clap` (derive, env) | The command-line surface, declared next to the types it fills. |
| `serde`, `toml` | The settings and the credentials, as files a person can read. |
| `serde_json` | The snapshot format. |
| `jiff` | Day boundaries in the family's timezone, DST included. |
| `ratatui` | The full-screen dashboard. |
| `crossterm` | Reading a password without an echo, and the dashboard's backend. |
| `tokio` | The async runtime the client needs. |
| `huckleberry-api` | The client. The CLI holds no knowledge of Firestore. |

Adding to either set means saying here what the crate is for, in one line, in
the same change. See [`rules/rust.md`](rules/rust.md).

## The 400-line rule in practice

Several directories exist because a file crossed it:

| Directory | The seam it was split along |
| --- | --- |
| `cli/` | the shape, the words the flags take, the deeper subcommand trees |
| `domain/` | one module per question the tool answers |
| `render/` | one module per screen |
| `dashboard/draw/` | the frame, and one function per tab |
| `picker/` | what is on the list, and what it looks like |
| `commands/edit/` | the command, and the questions it asks |
| `models/sleep/` | what was recorded, the timer, the history |
| `models/feed/` | the words, the timer, the summaries, the history |
| `ops/feed/` | nursing runs a timer; a bottle is one instant event |
| `tests/` | what a read asks for, and what a write sends |

Each `mod.rs` is glue and re-exports. `src/commands/feed.rs` sits just over the
line at around 400 and is deliberately left whole: it is one command's tree
together with the prompts that fill it in, and splitting it would separate a
question from the thing it asks about.
