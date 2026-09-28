# Decisions

One section per decision worth remembering: what was chosen, what it rules
out, and what would make it worth revisiting. Add to the bottom; do not rewrite
history.

## The dependency set stays at four crates

**Decision.** The project starts with `anyhow`, `clap`, `serde` and `toml`, and
nothing else.

**Why.** Each of the four answers a question this tool cannot avoid: how errors
travel, how arguments are declared, how the settings become a struct, and what
the settings file looks like on disk. Everything else a starter usually reaches
for (a logging framework, an async runtime, an HTTP client, a color crate, a
crate that finds the config directory) answers a question this tool has not
been asked yet. A dependency is cheap to add later and expensive to remove, so
the default answer is no until a real need names it.

**Consequences.** Color is our own nine semantic tokens in `src/theme.rs`
rather than a color crate, progress lines are `eprintln!` rather than a logger,
the prompts are a few lines of `read_line` in `src/prompt.rs` rather than a
prompting library, and the configuration directory is resolved from
`XDG_CONFIG_HOME` and `HOME` in `src/config.rs` rather than by a crate that
knows every platform's conventions. That last one is the trade most likely to
be revisited: a tool that has to feel native on Windows wants `directories`.

**Revisit when.** A need arrives that the standard library and these four
cannot meet without an awkward hand-rolled substitute. Add the crate, say in
`docs/architecture.md` what it is for, and record the trade here.

## The settings are one typed struct in one TOML file

**Decision.** Everything the tool remembers is one `serde` struct (`Config` in
`src/config.rs`), serialized as TOML to
`$XDG_CONFIG_HOME/<package>/config.toml` (or `~/.config/<package>/config.toml`),
and `--config <PATH>` points at a different file.

**Why.** A typed struct with `#[serde(default)]` makes a partial file valid, a
missing file mean "nothing configured yet", and a new setting one more field.
`deny_unknown_fields` turns a typo in a hand-edited file into an error that
names the settings that do exist, which is worth more than tolerating a line
that silently does nothing. TOML is the format the reader already meets in
`Cargo.toml`, and it survives hand editing better than JSON, which has no
comments and objects to a trailing comma. The XDG location is where other
command-line tools keep their settings, and `--config` exists so a test, a
script, or a second profile never has to touch the real file.

**Consequences.** Saving rewrites the file from the struct, so comments a
person wrote in it are lost: `config set` says so under `--verbose`. Settings
are flat, one level deep, which is all this shape supports without nesting
tables. `toml`'s `preserve_order` keeps a table in the order it was read rather
than sorting it, so a file this tool rewrites keeps the shape its author
recognizes.

**Revisit when.** The settings grow a nested section, a profile per
environment, or a value that must not be written in plain text. The first two
are more TOML; the third is a different mechanism entirely.

## Asking for a missing value happens in exactly one module

**Decision.** `src/prompt.rs` owns every question: whether there is anybody to
ask, how the question is drawn, what a typed answer means, and what the failure
says when stdin is not a terminal. Commands describe the value they are missing
and call `prompt::ask`.

**Why.** The two-audiences rule in `docs/rules/cli-ux.md` (an agent drives
everything with flags, a person who omits a value is asked rather than
rejected) is easy to state and easy to break one command at a time. Keeping it
in one module means a new command inherits the behavior instead of
reimplementing it, and the tricky parts (an empty answer taking the default, a
choice picked by number, the message that names the flag) are pure functions
with tests rather than something only reachable by typing at a terminal.

**Consequences.** A question is data (`Question`), so a command cannot ask
without also naming the flag that would have answered, which is what keeps the
non-interactive path honest. The terminal half stays untested on purpose: it
reads a line and writes a line, and everything it decides lives next to it in a
pure function.

**Revisit when.** The tool needs a question this shape cannot express: a
password with no echo, a multi-select, or a full-screen picker. That is the
moment to weigh a prompting crate against the four dependencies above.

## `just` is the task runner, and it is optional

**Decision.** Tasks live in a `justfile`, and every recipe is a single command
a person could also type by hand.

**Why.** Cargo has no place to hang project tasks (install, lint with the flags
we mean, update the agent skills), so a Rust project either invents a script
directory, a `cargo xtask` crate, or uses a task runner. `just` is itself a
Rust tool (`cargo install just`), a recipe costs one line, and `just --list`
makes the tasks discoverable the way a `package.json` scripts block is. A
`cargo xtask` crate earns its keep when a task needs real Rust code (codegen,
packaging, a release pipeline); a whole crate that shells out to `cargo clippy`
does not. A `Makefile` needs nothing installed, but its tab rules are a trap
for a one-line recipe and it is not what a Rust project reaches for.

**Consequences.** Nobody is blocked by not having `just`: `just lint` is
`cargo clippy --all-targets -- -D warnings` and `just install` is
`./install.sh`. Recipes take their arguments through
`set positional-arguments` and `"$@"` rather than just's own interpolation, so
the file stays readable as shell.

**Revisit when.** A task needs logic rather than a command line. That is the
moment `cargo xtask` starts paying for itself.

## The client is a package of its own, not a module

**Decision.** The Huckleberry client lives in `crates/huckleberry-api`, a
workspace member with its own version, its own lint configuration and its own
typed error enum. The CLI depends on it by path.

**Why.** The brief was that the API should read as a library any Rust program
could take, which happens to be consumed here. A module inside the binary
crate cannot be that: it would inherit the binary's `anyhow`, its
`missing_errors_doc = "allow"`, and eventually a use of its configuration.
Separating them makes each of those a compile error instead of a code review
note. Publishing it is a `cargo publish` away.

**Consequences.** Two `Cargo.toml` files, and a lint configuration that is not
shared: the library documents `# Errors` on every fallible function and the
binary does not, which is the right bar for each. The workspace sets
`default-members` so a bare `cargo test` still covers both.

**Revisit when.** Never, unless the client stops being useful on its own.

## Firestore is reached over REST

**Decision.** The client speaks the Firestore REST API directly rather than
using a gRPC client.

**Why.** No Rust Firestore crate accepts a bare Firebase ID token; they expect
Google service-account credentials. Adapting one means either forging
credentials or vendoring its auth layer. Meanwhile the whole surface this
client needs is five requests, and REST serves all five with nothing but
`reqwest` and `serde`. A gRPC stack (`tonic`, `prost`, generated protos) would
have been larger than the rest of the dependency tree combined.

**Consequences.** No real-time listeners: `Listen` is a bidirectional gRPC
stream with no REST equivalent, so `setup_*_listener` becomes polling in
`ops::watch`. A polled document is late by up to its interval and costs one
read per interval. For a dashboard that is acceptable; for a home-automation
integration it might not be.

The Firestore value codec is ours: `firestore::value` converts between the
tagged wire format and `serde_json::Value`, which is forty lines and the
reason every model can be a plain serde struct.

**Revisit when.** A Rust Firestore client appears that takes an ID token, or
this grows a use that genuinely needs push.

## Secrets live in a second file, with a mode

**Decision.** `config.toml` holds settings and nothing secret. The email, the
password and the session live in `credentials.toml` beside it, written with
mode `0600` set at open time. `HUCKLEBERRY_EMAIL` and `HUCKLEBERRY_PASSWORD`
override the file, and a password that came from the environment is never
written back to it.

**Why.** The settings file wants to be shown to people: pasted into a bug
report, committed to a dotfiles repository, printed by `config show`. A
password in it makes all of that a mistake. Two files make the distinction
structural rather than a matter of remembering, and a test asserts that no
setting is named `password`, `email`, `token` or `session`.

The OS keychain was the alternative. It is more secure and it was rejected for
now: it adds a dependency with real platform caveats, and it makes the
CI and agent paths harder exactly where this tool wants them easy.

**Consequences.** The refresh token is on disk in plain text, readable by the
account that put it there, and anybody who can read that file can read the
account. `auth logout` deletes it. On Windows there is no portable `0600`, and
the code says so at the one place it matters.

**Revisit when.** The tool is used somewhere a local file is not an acceptable
place for a token, or somebody wants a shared machine. That is the moment to
weigh `keyring` against this.

## Nothing about a baby's day is ever coloured as a problem

**Decision.** The screens never paint a number red, never flag it, and never
alarm. Typical ranges are grey text and the tool says where a number sits
relative to one, in a declarative sentence, and stops.

**Why.** This is a tool a frightened first-time parent opens at 3am. A red
number is a verdict, and this program is in no position to deliver one. The
rule is inherited from the dashboard this grew out of, along with its sharper
corollary: there is no reference band for milk volume at any age, because the
obvious one describes established feeding and drawing it in week one would
tell a parent they are underfeeding their baby.

**Consequences.** `domain::reference` has no volume metric at all, so it cannot
be added by accident; a test walks every label and fails on "should", "must",
"need" and "doctor"; another asserts the day table emits no `error` or
`warning` tone. `Tone::Error` is still used, for the tool's own failures, which
are the tool's fault and not the baby's.

**Revisit when.** Never on the medical side. The mechanism could change.

## The analysis layer never reads the clock

**Decision.** Everything in `src/domain` takes `now` as an argument. Nothing in
it calls `SystemTime::now`, opens a socket, or writes to a terminal.

**Why.** Almost every interesting bug in this problem domain is a date bug: a
sleep across midnight, a night window that spans midnight, a day that is 23
hours long because the clocks went forward, a "last night" that means something
different at noon than at 3am. All of those are one-line tests when the
function takes the instant and untestable when it reads one.

**Consequences.** A lot of `now: f64` parameters, and one place
(`huckleberry_api::client::now_seconds`) that actually reads the clock. The
domain tests run in milliseconds and have no fixtures on disk. The same
discipline extends to `render`, which returns `Vec<String>` rather than
printing, so every screen is asserted line by line.

**Revisit when.** It does not need revisiting; it needs keeping.

## An entry is corrected by asking again, not by a flag per field

**Decision.** `hb edit` finds a row, fills a draft with what is on it, and
runs the tracker's own questions with those values as the defaults. The
non-interactive path is `--id <ENTRY> --set key=value`, one generic setter for
every kind of entry, rather than a flag per field per tracker.

**Why.** Six kinds of entry with five to seven fields each is forty flags, and
most of them would differ from the create command's only in being optional. A
person does not want them anyway: what they want is to be asked again, with
last time's answers in front of them, because the mistake they are fixing is
usually one answer out of six. `--set` keeps the agent path complete without
paying for it in surface area, and `--list` gives a script the name of the
entry it means, so both audiences stay first class as
`docs/rules/cli-ux.md` requires.

**Consequences.** `--set` values are the app's own spellings (`--set
type="Breast Milk"`), with this tool's spellings taken as well where they
differ. An empty value clears a field, which is the only way to say "there is
no colour after all". A field a kind does not have fails naming the ones it
does, and that list is [`crate::edit::Draft::fields`], so the failure cannot
drift from what `set` accepts. `edit` never writes `start`: a row's id leads
with its own millisecond timestamp, and moving the moment would leave history
sorted by a time the row no longer claims. Deleting is still the app's job.

**Revisit when.** Somebody needs to correct *when* something happened. That is
a delete and a re-log, which needs a delete this crate does not have yet.

## The full-screen picker is the one exception to `src/prompt.rs`

**Decision.** Choosing which entry to edit is a `ratatui` screen in
`src/picker/`, not a numbered prompt.

**Why.** The prompt module's decision above left the door open for exactly
this: "a question this shape cannot express … a full-screen picker". Forty
entries over a week is not a numbered list anybody can read, and the answer a
person is looking for is "the nappy at about half ten", which they find by
scrolling to it.

**Consequences.** The picker is split like the dashboard: `state.rs` decides
and is pure, `draw.rs` draws, and `commands/edit` owns the terminal. It is
deliberately the only one: a question with a fixed set of answers still belongs
in `src/prompt.rs`, and the edit form itself is prompts. With no terminal there
is nobody to pick, so `edit` fails naming `--id` and points at `--list`.

**Revisit when.** A second screen wants to choose something. Then the picker
becomes generic over what it is listing, rather than a second copy of it
appearing.
