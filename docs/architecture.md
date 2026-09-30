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
├── prompt/mod.rs        asking for a value that was left out
├── prompt/time.rs   shared clock-time and relative-event questions
├── theme.rs         semantic colours
├── setup/           first use: an account, and the settings with no default
├── domain/          everything the tool works out, and nothing it prints
│   ├── clock.rs     a time somebody typed, and the instant it means
│   └── today.rs     what "today" means to this family, and what it adds up to
├── render/          domain values to lines of text
├── tui/             the always-on shell `h` opens with no command
├── dashboard/       the full-screen version of the same values
├── listing/         every list: columns, groups, search, browsing
└── commands/        one thin module per command
tests/
└── public_api.rs    the library surface, driven from outside the crate
```

## The four layers, and why the split is where it is

| Layer | Knows about | Never |
| --- | --- | --- |
| `huckleberry-api` | Firestore, Huckleberry's shapes | this tool |
| `domain` | events, days, arithmetic | the clock, the network, a terminal |
| `render` / `dashboard` / `listing` / `tui` | text, colour, widgets | the network, the clock |
| `commands` | all of the above, and the outside world | arithmetic worth asserting |

`render::log::rows` takes `now` for the same reason: a row from the last five
hours says how long ago it was, and which rows those are is an argument rather
than a reading of the clock, so the list is asserted in a test.

`domain` takes `now` as an argument everywhere. That is the whole reason a
sleep across midnight, a night window that spans midnight and a 23-hour day are
testable rather than seasonal. Nothing in `domain` or `render` reads a clock, so
every screen in this repository is asserted line by line in a test.

## The shell, and how it keeps every command

`h` with no command opens a full-screen app on **stderr** and keeps it open:
`src/tui/`. Its design brief, and the one-handed rules every feature in this
repository is held to, are in [`tui.md`](tui.md).

It is split the way the dashboard is, so nothing that decides anything also
owns the terminal:

| Module | Holds |
| --- | --- |
| `tui/keys.rs` | what a keystroke means, as a `Motion`. Pure. |
| `tui/state.rs` | the menu stack, the cursor, and what a `Motion` does to them. Pure. |
| `tui/facts.rs` | the last reading, whether one is in flight, and what went wrong. Pure. |
| `tui/data.rs` | making a reading: no narration, no questions, off the event loop. |
| `tui/draw/mod.rs` | the frame, and where the widgets go in it. |
| `tui/draw/menu.rs` | the Menu view: the main panel. |
| `tui/draw/now.rs` | the Now widget: what `h now` says, permanently. |
| `tui/job.rs` | one command running inside the shell, and everything it has said. `Exchange` and the panel window are pure. |
| `tui/draw/flow.rs` | the flow panel: a command, drawn where the menu was. |
| `tui/shell.rs` | the alternate screen, and the event loop's keyboard. |

The words for these parts are fixed in
[`nomenclature.md`](nomenclature.md); use them rather than inventing a second
name for something that has one.

Reads happen on a background task rather than in the loop. A Huckleberry that
cannot be reached takes a minute to say so, and a shell that accepts no keys
for that minute is broken exactly when the network is. The loop polls for a
keystroke on a one-second tick, collects a finished read when there is one, and
draws either way, which is also what makes the live timers count up.

`tui/data.rs` neither narrates nor asks, and both follow from the shell owning
stderr: a progress line would scribble across the frame, and a prompt would
draw over the screen it is asking in front of. When the child is ambiguous it
fails naming the command that settles it, which is a row in the menu beside the
message.

Navigation is the only thing that moved out of `src/interactive/`. What stays
there is everything that happens once a row has been chosen: the menu tree
(`catalog`), the argument draft a command is built from (`draft`), dispatch and
its error recovery (`operation`), and the session-scoped overrides.

Choosing a command does not take the screen away. The command runs on its own
task and draws in the panel where the menu was, and the loop keeps drawing the
rest of the screen the whole time.

That works because every interactive loop in this tool is the same shape: draw
some lines, wait for a key, decide, repeat. `src/prompt/host.rs` is a single
global slot holding channels; while one is installed, the menus
(`prompt/select`), the text fields (`prompt/text`) and the browsable listings
(`listing`) hand their lines over and get keys back instead of owning the
terminal. With nothing installed they behave exactly as they did. So there is
one implementation of every prompt rather than one for the terminal and one for
the shell, and a themed line keeps its colour through `tui::draw::painted`,
which reads the escape codes back into roles.

Output goes the same way. `render::print` and `render::note` are the only two
places this tool writes, and both hand their lines to a host when one is
installed. Everything that used to `println!` or `eprintln!` now goes through
them.

`Exchange` in `tui/job.rs` holds the two rules that make typing into a hosted
question feel like typing: input that arrives before a question wants it is
queued rather than dropped, and the panel draws the last frame rather than the
pending one so it never blanks between a keystroke and its redraw. Both are
pure and tested; both were bugs first.

The dashboard is not an exception. `dashboard::draw::draw_in` takes an area
rather than the whole frame, so the shell draws it in the panel from the
reading it already holds. Inside the shell it is a view rather than a job:
`App::dashboard` holds the state, Back closes it, and a refresh replaces its
dataset along with the widgets'. `h dash` still opens it on its own, where it
owns the terminal as before.

Drawing on stderr rather than stdout is what keeps `h > entries.txt` filling the
file with what the commands printed. The menu is the conversation; the commands
are the data. It is also why `prompt::available` (stdin and stderr) is still the
right gate for opening it at all.

## Setup, and the settings with no default

Every command goes through `src/setup/` before it acts, and the gate lives in
[`dispatch`](#) rather than in `run` so that every route reaches it: an
explicit invocation, a row chosen in the shell, and a view retried after a
failure. `--help` and `--version` never reach it, because clap answers those
and exits first.

`setup/mod.rs` holds the decisions and is pure: which commands are exempt, what
is missing, which mode a baby of a given age is offered, and what the refusal
says. `setup/run.rs` is the thin shell that signs in, asks, and writes.

Two things have no default worth having, so the file starts them empty and
setup asks: **how a family counts a day**, and, for a family counting discrete
days, **when a day and a night begin**. Midnight is the one boundary that is
wrong for everybody. See [`setup.md`](setup.md).

Each half of the gate asks only for what it is for. Credentials are wanted by
anything that opens a socket, so not under `--offline`; the day settings are
wanted by the commands `shows_a_day` names, which are the ones whose figures
would otherwise be drawn against a boundary nobody chose.

The rule is applied to the dataset once, where it is read
(`DayRule::apply_to`), so every screen that asks the child about its night gets
the same answer without each of them having to know the setting exists. The
same applies on the dashboard's re-read and on the shell's refresh, or the
screen would quietly revert to the profile's night after a minute.

## Command presentation

`render/output.rs` formats receipts, labelled values and lists without I/O. It
measures terminal cells through ratatui, including wide names and emoji, and
provides a stacked layout for narrow terminals. `render/format.rs` provides local
date/time formatting with timezone abbreviations. Command handlers supply
explicit human fields alongside their existing machine payloads; `Context`
selects the terminal presentation using stdout's terminal status. It also gates
stdout colour independently of stderr. JSON and snapshot exports bypass receipts.

History construction accepts a volume formatter supplied by its caller. Commands
and the dashboard supply `render::format::volume` with the configured unit, so
the log, entry details, and edit/delete descriptions agree. The domain retains
millilitre quantities and has no dependency on rendering or CLI preferences.

This keeps numeric timestamps and seconds available to scripts while parents
see dates, durations and units. No API storage or operation semantics change.

## How a value is resolved

Every value a command needs is looked for in the same order:

1. the flag (`--amount 90`), which is what an agent or a script passes;
2. the configuration file, which is how a person avoids typing the same flag
   every day;
3. a question, asked through `src/prompt/mod.rs`, when there is a terminal;
4. a failure naming the flag, when there is not.

Event times keep their noninteractive default of now. `prompt/time.rs` shares
one relative/clock parser, question builder, helper text and AM/PM question
between manual sleeps, instant recordings, timer transitions and time edits.
Callers supply only the date rule for clock-only answers: most recent occurrence,
first occurrence after a manual start, or the existing history date. Relative
answers always retain their full instant and bypass those clock-only rules.
Manual sleeps require both times; event and timer questions default to now, and
edits default to keeping the original. `sleep end`
and `sleep stop` dispatch to the same handler. `domain/clock.rs` parses relative minutes using an
explicit `now`. Time is the first activity question. Times resolve against the clock after that
answer is read, before the remaining details.
Commands pass them to explicit-time API operations, which keep event times
separate from synchronization timestamps. `ops/timing.rs` validates transition
ordering and checks that backfilled instant events preserve newer summaries.

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
puts that reference on the event, the snapshot carries it, and `h edit` takes
it back to `update_history_row`.

The write is a field update rather than a rewrite. A row has fields this
repository does not model, and a batched row has neighbours in the same
document; naming the fields that change is what leaves both alone. See
[`api.md`](api.md).

`commands/edit/history.rs` combines the detail draft with a separate event time.
The form asks for that time first, defaulting to the exact stored instant; a
clock-only replacement uses the entry's original local date. `commands/edit/form.rs`
lists each field with the value it would be saved with, taken from the draft with
the pending changes applied, so nothing has to be opened to be seen. A sleep is
listed and asked as a start and a stop: the stop question is a time reader like
any other, and what is stored is still the duration between the two. Every located row
supports time corrections, including pumping and milestones. The edit picker
also reads health history so growth, medication and temperature times can be
corrected. These additional kinds retain their raw details rather than creating
new detail forms. The API's history-time operation updates row identity and
last-entry summaries together; the receipt returns the resulting selector.

The edit command also prepends the active sleep from the live state, under the
reserved selector `sleep/current`. `commands/edit/live.rs` routes that selection
to a timer update instead of a history draft. It reads the current session before
the prompt, then the API rechecks its identity and active state before patching
only the start and synchronization fields. The patch requires the Firestore
revision read with that timer, so a concurrent change causes a conflict instead
of applying a stale correction. Enter means no mutation, even if the existing
start field is absent. `ops/sleep_edit.rs` owns that operation;
a paused timer stays paused, a running timer keeps running, and history is untouched.

The `now` renderer combines the live sleep duration with the last completed
sleep from the domain view. Its secondary previous-sleep note uses the muted
role; relative time is measured from the completed sleep's end. Without a live
sleep it displays that elapsed time and the recorded duration.

Removing one is the same reference and one more step. Every tracker keeps a
copy of its most recent entry on its own document so the app can draw a home
screen without reading history, so `ops/removal.rs` rewrites any of those
copies that described the row that has gone. `h edit` and `h delete` share
one list (`src/picker/`), one set of tokens and one confirmation style; they
differ only in what they do with the entry they are handed.

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
| `ratatui` | The full-screen dashboard, and the shell `h` opens with no command. |
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
| `tui/` | what a key means, what it does, what it knows, how it draws, who owns the screen |
| `tui/draw/` | the frame, and one file per widget |
| `listing/` | what is in a list, how it is laid out, what a key does |
| `commands/edit/` | the command, and the questions it asks |
| `models/sleep/` | what was recorded, the timer, the history |
| `models/feed/` | the words, the timer, the summaries, the history |
| `ops/feed/` | nursing runs a timer; a bottle is one instant event |
| `tests/` | what a read asks for, and what a write sends |

Each `mod.rs` provides glue and re-exports for its subsystem.

The shared prompt selection model in `prompt/select` interprets arrow,
J/K/H/P, and 1-9 navigation without terminal I/O, with bounded cursor and scroll
state. Number shortcuts highlight the corresponding item in every command and
prompt menu; only Enter submits. Items after nine remain reachable by navigation.

Prompt input now uses a scoped raw-mode guard for text and selection. Typed
cancellation travels through anyhow without string matching. Text, questions,
password key interpretation, and menu selection live in separate prompt files.

Feed handlers are split into bottle, nursing, and solids modules under
commands/feed; the module retains the command entry point and public helpers.

The interactive catalog groups every canonical clap command into a menu tree.
Pending argument drafts resolve through clap without spawning a shell; passwords
are redacted in previews and parser diagnostics. Session options remain separate
from persistent settings.

Argument-binding helpers derive from clap metadata; the menu does not expose
the generic option editor. Specialized command prompts handle foods and entry
fields. Edit persistence retains original wire enum values when the corresponding
field was not changed.

The bare-command session owns navigation and inherits temporary overrides from
its initial CLI flags. Every command in `interactive/operation` enters dispatch
on selection, with no Run or options menu. A typed policy identifies read-only
views, which offer Back after output and an explicit Retry only after failure.
This follow-up menu holds static output on screen, replacing the separate pause.
Writes and exports return through their ordinary completion flow; errors return
to navigation after a pause and cannot automatically replay a write.
Each execution opens a fresh Context and calls the same typed dispatch as an
explicit CLI invocation. Menus release terminal ownership before handlers run,
cancellation returns to navigation, and failed writes are never automatically
replayed.

History editing now collects explicit field changes rather than rewriting a
normalized draft. The CLI maps those selected fields to the API's existing
`update_history_row` operation; omitted fields stay untouched, including unknown
values and meal food metadata. A changed meal amount updates only amounts in the
original food map. Direct edits use the field picker.
The API crate and its operation semantics are unchanged.

The session installs a Tokio Ctrl-C listener around operations, including picker
reads. Interruption stops waiting and returns to navigation, with a reminder to
check logs if a write might already have reached the service. Each selection creates a fresh command draft.
