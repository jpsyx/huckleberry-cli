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

**Decision.** `h edit` finds a row, fills a draft with what is on it, and
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
drift from what `set` accepts. Time is handled separately from detail drafts,
so every located entry can change `at`, including kinds without a detail form.

**Time corrections (September 2026).** The original restriction against changing
history timestamps has been removed. A correction preserves the raw row and
moves timestamp-based IDs using an atomic, revision-checked write that also
repairs affected latest-entry summaries. This avoids leaving IDs ordered by the
old time or losing unknown fields through a delete-and-log reconstruction.
Interactive logging asks for time first. Editing offers the stored time, with
Enter preserving its exact instant; a clock-only edit keeps the original date.

**Fields read as values (September 2026).** The field picker shows what each
field holds rather than promising to "keep current", and a sleep is asked as a
start and a stop rather than an instant and a length: nobody remembers a nap as
ninety minutes, they remember when the baby went down and when they woke up.
The record still stores a duration, so the stop is converted on the way in and
`--set duration=<MINUTES>` is unchanged. A stop already chosen survives a later
correction to the start, because the person who typed it meant that clock time,
not that length.

## The full-screen picker is the one exception to `src/prompt.rs`

**Decision.** Choosing which entry to edit is a `ratatui` screen in
`src/picker/`, not a numbered prompt.

**Why.** The prompt module's decision above left the door open for exactly
this: "a question this shape cannot express … a full-screen picker". Forty
entries over a week is not a numbered list anybody can read, and the answer a
person is looking for is "the diaper at about half ten", which they find by
scrolling to it.

**Consequences.** The picker is split like the dashboard: `state.rs` decides
and is pure, `draw.rs` draws, and `commands/edit` owns the terminal. It is
deliberately the only one: a question with a fixed set of answers still belongs
in `src/prompt.rs`, and the edit form itself is prompts. With no terminal there
is nobody to pick, so `edit` fails naming `--id` and points at `--list`.

**Revisit when.** A second screen wants to choose something. Then the picker
becomes generic over what it is listing, rather than a second copy of it
appearing.

## Every recording command asks every question its answer implies

**Decision.** In interactive mode, a command that records something asks about
every field that entry can carry, in the tracker's own order, with the value
the record would otherwise get already in the question. A flag answers its own
question and no others. Nothing but the first value is required: Enter leaves a
field out.

**Why.** The rule this replaces was "the flags are the fast path, so a person
who passed one is not asked for the rest", and what it produced was fields
nobody could record at all. Choosing "wet" ended the diaper conversation, so
there was no way to say it was a big one; a bottle never asked which units, so
a family recording in ounces silently recorded millilitres. A question with its
answer already in it costs one keystroke, which is the right price for a field
that is otherwise unreachable.

**Consequences.** `h diaper` is up to six questions and `h feed bottle` is
four, each a press of Enter. The defaults do the work, so they have to be
right: the bottle amount offered is the last bottle's **converted into the
units being asked about**, because the app stores it in whatever units it was
entered in. The settings (`units`, `measurements`) are what the questions
offer, never what the commands assume, so `h config set units oz` changes what
Enter takes and nothing else. With no terminal nothing is asked and nothing
changes: a missing optional value is absent, not a failure.

**Revisit when.** A tracker grows a field this tool does not model. Then it is
a question here and a field in `huckleberry-api`, in the same change.

## A time is read the way it is said

**Decision.** `src/domain/clock.rs` reads `3:57am`, `3:57 AM`, `3:57 a.m.`,
`357am`, `0357`, `03:57`, `21:30`, `2130`, `9pm` and `7` as times of day, and a
bare `3:57` as **both** times rather than as a guess. A time with no day is the
most recent one at or before now; an end time is the first such time after the
start. All activity-time inputs, including both manual-sleep endpoints, also
accept `now` and relative minutes through the same parser. Relative answers
retain their full timestamp instead of being converted to a clock time and
assigned a new date. Their examples and retry hints come from one shared string.

**Why.** The only person typing a sleep in after the fact is doing it at 3am
from memory, and making them match a format is making them think. The two
resolution rules are what a person means without saying it: "he went down at
11:30" said at 3am is last night, and "up at 1:15" after that is the same
night. Guessing the half of the day is the one thing the parser will not do,
because being wrong by twelve hours puts a sleep in the wrong night and the
mistake is invisible afterwards.

**Consequences.** `sleep manual --start 3:57` is refused naming both
spellings; asked interactively, it offers `3:57 am` and `3:57 pm` and takes the
answer. A leading zero (`0357`, `03:57`) is read as a 24-hour clock, because
that is what writing it out means. Everything is pure and takes `now` as an
argument, so midnight, DST and the 3am case are tests.

**Revisit when.** Somebody wants to name a day ("yesterday 9pm", "Tuesday").
That is a date parser, and it belongs beside this one rather than inside it.

## A delete repairs the tracker it deleted from

**Decision.** `h delete` removes the row and then rewrites any of the
tracker's "last entry" summaries that described it, from whatever is now the
newest row of that kind. A summary that is missing while history has one to
fill it is written too.

**Why.** Every tracker keeps a copy of its most recent entry on its own
document so the app can draw a home screen without reading history. Remove the
row that copy describes and the app goes on showing an entry that is no longer
there, which is worse than the entry having been there in the first place: it
is a record that disagrees with itself. The self-healing half came from
watching this fail — a summary written without the `lastUpdated` its model
requires read back as nothing at all, and the growth card went blank — so the
repair now fixes a summary it finds wrong rather than only the one it broke.

**Consequences.** A delete is three or four requests rather than one, and one
of them lists the tracker's history. Which rows fill which summary, and which
of their fields it copies, is a table in `ops/removal.rs` rather than six
hand-written repairs; adding a tracker means adding a row to it. Deleting is
still the one thing that cannot be undone, so it is the one thing that asks
twice.

**Revisit when.** A tracker keeps a summary that is not a copy of one row.
Then the table describes something it cannot, and that tracker wants its own
repair.

## Each kind of entry has a colour, and the cursor is brighter than all of them

**Decision.** Wherever entries of several kinds are listed together, each
tracker is drawn in a hue of its own: sleep blue, feeding cyan, diapers
magenta, pumping green, milestones yellow. The row under the cursor is bold
white.

**Why.** The stream is the screen somebody opens when they are looking for one
entry among forty, and a list painted in one colour is read line by line. Hue
makes it scannable by shape before it is read by word, which is what "which of
these is the diaper" actually needs.

**Consequences.** Five kinds and eight bright colours means a kind shares its
hue with a role: pumping is the green of `Success` and milestones the yellow of
`Warning`. They never appear beside those roles, and in a list of kinds a
colour is a category rather than a verdict, so the rule the palette holds to is
the older one: colour is never the only carrier, and every row says its kind in
words. A test asserts the five stay distinct from each other and dimmer than
the cursor.

**Revisit when.** A sixth tracker is listed. Eight bright colours minus grey,
white and red does not stretch much further, and that is the moment to weigh
256-colour hues against inheriting the terminal's palette.

## One module draws every list

**Decision.** `src/listing/` owns every list this tool shows: `log`, `edit`,
`delete` and `foods list` hand it rows and it owns the columns, the group
headings, the colours, the search and the scrolling. It is a port of the
listing view in this author's `jpsyx` CLI.

**Why.** The first two lists here were written twice — `edit` and `delete` each
had a picker — and the third would have been written a third time. `jpsyx`
reached the same point across a dozen commands and answered it with one module,
and the shape it arrived at is worth taking whole: rows carry a group key and a
detail, the query matches every word against the whole row, and everything
except drawing and reading keys is pure, so the layout and the interaction are
tested without a terminal.

**Consequences.** A new list is a `Vec<Row>` and a set of columns. Adding a
column to the stream is one line, and it is searchable for free. The interactive
half is opt-out rather than opt-in: a listing opens browsable when both ends are
a terminal, and is plain text otherwise, which is what keeps `h log | grep`
working. The one thing it does not do is edit in place; Enter hands the row's
key back to the command, which is why `edit` and `delete` can share it and mean
different things by it.

**Revisit when.** A list needs more than one line per row, or columns that are
not text. Both would change the layout pass rather than the rest of it.

## A receipt is a table, a list is a listing

**Decision.** Two renderers, deliberately: `render::output::table` draws a
bordered receipt for a fixed handful of labelled values (a write's
confirmation, `child list`, `config show`), and `listing` draws a list somebody
scrolls, searches and chooses from.

**Why.** They answer different questions. A receipt is the answer to "what did
that do", is read once, and wants a border and a title so it reads as a block.
A listing is the answer to "which of these", is read down, and wants alignment
and quiet so the eye can travel. Forcing either shape onto the other makes one
of them worse, and the choice is not about how many rows there happen to be.

**Consequences.** Both pick their colours from `src/theme.rs`, so they agree on
what a heading is, and neither knows about the other. A command showing a
handful of labelled values reaches for `context.table`; a command showing rows
somebody might be looking through reaches for `Listing`.

**Revisit when.** A receipt grows long enough to want scrolling, or a listing
short enough that a person misses the border. The first is the more likely, and
it is a `Listing` with a heading rather than a new renderer.

## The bare command is a full-screen shell, and it keeps every command by suspending

**Decision.** `h` with no command opens a ratatui app on stderr
(`src/tui/`) and keeps it open. Choosing a command leaves the alternate screen,
runs the existing handler inline exactly as the command line does, and redraws
the shell afterwards.

**Why.** The menu was already the way this tool is used, but as a stack of
inline prompts it could never show anything: no header saying which child, no
facts above the rows, no fixed place for the keys. A full screen can. The
alternative to suspending was a panel per command before the shell could ship
at all, which would have meant rewriting twenty-odd flows, their questions and
their receipts, in one change, and losing the tests that cover them. Suspending
cost one function and kept all of it.

**Consequences.** Every command works on day one, and panels replace them one
at a time when there is a reason to. Command output appears on the ordinary
screen rather than inside the app, so the "Continue" pause is what holds a
receipt or a failure on screen until it has been read, and a person can scroll
back to it. Two key maps coexist until the prompts become panels: the shell
reads `h` as back, and a suspended prompt still reads `H` as down.

Drawing on stderr rather than stdout is what keeps `h > entries.txt` filling the
file with what the commands printed, and keeps `prompt::available` the right
gate for opening the shell at all. It also means the shell and `dash` can both
be on screen in one session without fighting over a buffer, since `dash` owns
stdout.

Resuming rebuilds the `Terminal` instead of calling `Terminal::clear`, because
clearing asks the terminal where its cursor is and blocks on stdin for the
answer, which is the stdin the menu reads keys from.

**Revisit when.** The facts panel and the per-command panels have replaced
enough of the suspended flows that stepping out of the screen is the unusual
case rather than the normal one. At that point the remaining suspensions are
worth listing and finishing rather than leaving as a seam.

## The tool is designed for one hand in the dark

**Decision.** Every user-facing decision is read back to one person: a parent at
3am, with a baby on one arm and one hand free. The rules that follow from that
are in [`tui.md`](tui.md), and they bind every feature, not only the shell.

**Why.** "Easy to use" is not a constraint anybody can fail, so it never
decides an argument. A named person with a named limitation does: they cannot
see the keyboard, cannot use two hands, and will not remember a shortcut. That
rules things out. A free-text field is out. A default that is merely defensible
rather than usually right is out. A direction key with no letter alias is out,
because the free hand is not always the one near the arrows.

**Consequences.** Menus and presets are the first answer to "how does somebody
supply this value", and typing is the last. Every question opens with the most
likely answer highlighted, so `Enter` alone is a complete interaction. Digits
highlight rather than submit, everywhere, so a misread row cannot log a feed.
There are always three ways out. The keys stay on the screen rather than behind
a `?`.

This costs breadth: a menu of six presets is more code than a text field, and
every new value needs a default worth defending. That is the trade.

**Revisit when.** Never for the principle. The specific mappings are worth
revisiting when the two key maps in the tool are reconciled, which is a
decision about muscle memory rather than about code.

## The shell is a dashboard around the menu, not a menu with a header

**Decision.** The shell's main area holds widgets: the Menu view as the main
panel, and a sidebar of small boxes beside it, each answering one question. The
Now widget is the first, and `h now` is no longer a Home row because the widget
shows it permanently. `r` refreshes every widget at once.

**Why.** The shell is meant to be left running all day, and a screen somebody
leaves running should be worth glancing at. The question that gets asked at 3am
is "when did she last eat", and having to navigate to it is a worse answer than
having it already on the screen. Keeping the row as well as the widget would
have meant two ways to the same fact and one more row to read past.

**Consequences.** The shell now reads from Huckleberry, which the navigation
half never did. That brought three rules with it, all of them from
[`dashboards.md`](dashboards.md) rather than new: the screen says how stale it
is, a failed read keeps the numbers and reports the failure beside them, and
nothing about a baby is painted red.

It also brought a constraint the menu never had. A read that cannot reach
Huckleberry takes a minute to give up, so reads happen on a background task and
the loop polls for keys on a one-second tick. The rows move while a read is in
flight, which also makes the live timers count up on their own.

The widget is only as tall as its facts, leaving the foot of the sidebar empty
for the next widget. That empty space is deliberate: it is where widgets go,
and a box two thirds full of nothing reads as broken rather than as finished.

**Revisit when.** The sidebar has enough widgets that choosing which are on
screen is a decision a parent should make rather than one this file makes for
them. That is a layout the shell does not have and should not grow before it is
needed.

## The parts have fixed names, written down

**Decision.** [`nomenclature.md`](nomenclature.md) fixes what this project
calls its own parts, and AGENTS.md makes reading it part of starting work.

**Why.** "Panel", "widget", "view", "screen" and "dashboard" were all being
used for several things at once, and two of them meant something specific
already: the dashboard is `h dash`, and the shell is what `h` opens. A part
with two names grows two implementations, and a request that says "the panel"
cannot be answered without guessing which one.

**Consequences.** Some pairs stay deliberately interchangeable because both
are natural and neither is ambiguous: widget and panel, the Now widget and the
View Latest widget. The file says which pairs those are rather than pretending
there is one word. New parts get an entry in the same change that names them.

**Revisit when.** Never as a whole; continuously in the small. The file is a
record, so it is only useful if it keeps up.

## "Today" is a setting, because midnight is wrong for everybody

**Decision.** How a family counts a day is configured, not assumed. Two modes:
`continuous`, a rolling twenty-four hours, and `discrete`, from an hour the
family names. A discrete day is not a calendar day, and its night ends where
its day begins.

**Why.** A calendar day is the one answer that is wrong for every family with a
baby. Nobody is awake at midnight thinking of it as a boundary, and a 4am feed
filed under a fresh day is a feed the person who gave it cannot find. Beyond
that the right answer depends on the baby: a newborn's day has no shape at all,
so any boundary is a fiction and the only honest window is the last
twenty-four hours; an older baby has a day with a beginning, and "how much has
she eaten today" means since she woke.

**Consequences.** Screens say which window they mean rather than assuming one:
a rolling family sees `Fed in last 24h`, a discrete family sees `Total fed today` with
the hour it began on it. The setting reaches every screen through one step,
`DayRule::apply_to` on the dataset as it is read, rather than through a
parameter on every renderer. That also means `child show` prints the night this
tool draws with rather than the one Huckleberry stores, which is the honest
thing for it to print and is worth knowing.

The age that picks the default is read once, at setup. It is never re-read and
the mode never changes by itself: a screen that quietly started counting
differently one morning would be worse than one counting a way somebody chose.
Twelve weeks is a judgement, not a clinical boundary, and it decides only which
answer is offered first.

**Revisit when.** A third way of counting turns up that neither mode covers,
or the boundary between them is worth moving. Both are one more arm in
`DayMode`.

## Every command runs through a setup gate

**Decision.** Every command puts first-use setup in place before it acts: an
account when there is none, then the settings that have no default worth
having. The gate lives in `dispatch`, so an explicit invocation, a row chosen
in the shell and a retried view all reach it. `auth`, `config` and `info` are
exempt.

**Why.** A tool that quietly uses the wrong definition of "today" is worse than
one that stops and asks, and the alternative to asking is a default that is
wrong for whoever did not think about it. Putting the gate in `dispatch`
rather than in `run` was what made it true for the shell as well, which
dispatches without going through `run` at all.

The three exemptions are the same exemption: they are how somebody gets out of
an unconfigured state. A gate in front of `config set` would be a locked door
with the key behind it.

**Consequences.** A script or an agent meeting a fresh configuration fails
rather than running, for anything the missing answer would change. That is the
two-audiences rule working as designed and not an exception to it: the refusal
names every missing setting and the `config set` that answers each, in one
message, so one pass fixes all of them.

Each half of the gate asks only for what it is for. An account is wanted by
anything that opens a socket, so not by `--offline`. The day settings are
wanted by the screens whose figures depend on them, so
`h --offline snapshot.json log` draws its list on an unconfigured machine and
`h diaper --pee` does not stop a script to ask about a setting it never reads.
The first cut of this gated everything on every command, which turned the
"demonstrable without an account" path into one that needed configuring first,
and made a diaper wait on a question about arithmetic it does not do.

The list of day-showing commands is the thing to keep right: a screen left off
it gets midnight, quietly. It is named in one place with a comment saying so.

The gate reloads the context after anything is written, so a command never acts
on the settings as they were before setup ran.

**Revisit when.** The list of day-showing commands stops being short enough to
read, or a setting arrives that does not divide the commands the same way.

## The hours a day keeps are always configured; how "today" is counted is separate

**Decision.** `day_start` and `day_end` are asked for on first use whatever
else is chosen, and `day_mode` is a third, independent setting. The night is
not configured at all: it is the stretch from `day_end` to the next
`day_start`.

**Why.** The first cut treated the hours as belonging to discrete days, and
asked for them only when a family chose that mode. That was wrong in both
directions. Every screen with a day on it needs the hours: the summary counts
its rows between them, the stripe chart draws its rows from them, and the night
window on every chart is the gap between them. And a family counting a rolling
day still has a night, still has a summary table, and still wants both drawn to
the hours they keep.

Making the night its own setting was the other mistake. `night_start` and
`day_start` are one boundary each, and an hour that belonged to neither was
always possible. Naming them `day_end` and `day_start` says what they are and
makes the night what is left over, which cannot disagree with itself.

**Consequences.** `night_start` is read as `day_end` through a serde alias, so
a configuration written before this keeps working. Setup asks three questions
rather than one or three depending on an answer, which is also easier to
explain.

`summary` and `trends` now count rows from `day_start`, and `stripes` runs its
rows from the previous `day_end`. The two differ on purpose, and the reason is
in `stripes.rs`: a chart about where sleep lands must not cut a row through the
middle of a night.

`log`, `edit` and `delete` still group under calendar-day headings. Those are
labels rather than arithmetic, so nothing is counted wrongly, but a 4am feed
appears under a different heading there than the row it is counted on.

**Revisit when.** The listing headings start to look like a contradiction
rather than a detail, or a family turns up who wants a night that is not simply
the gap between two days.

## The Now drawer is `h now`, not a second screen about the same data

**Decision.** The shell's Now drawer draws exactly what `h now` prints: same
facts, same wording, same order. `render::now::screen` returns the rows as text
with a role on each piece, `lines` paints them for stdout, and the drawer draws
the same rows as widgets.

**Why.** The first version of the widget had its own wording, tuned for a
narrow column: `Asleep 40m` where the command said `Sleep  currently sleeping
for 40m`, `Longest` where it said `Night of Sun 21 Sep`. Two vocabularies for
one set of facts is two things to keep right, and the one somebody reads at 3am
would have been whichever they happened to open. Sharing the rows makes them
identical by construction rather than by remembering.

**Consequences.** A change to what the drawer shows is a change to `h now`, and
the other way round. There is nowhere to make one without the other, which is
the point. The drawer is taller than a hand-tuned widget would be, which is
what moved it from a column beside the menu to a strip along the bottom.

The one thing the drawer has that the command does not is what to say before a
read has finished or after one has failed. A command in that position simply
fails; a drawer has to keep drawing something.

**Revisit when.** The two genuinely have to differ. That would be a decision to
record here, not a line added to one of them.

## Nothing leaves the shell but `Ctrl-Q`

**Decision.** No menu row ends the session, and no single key does either. A
bare `q` no longer quits. `Ctrl-Q` leaves, and `Ctrl-C` is kept as an escape
hatch.

**Why.** The shell is meant to be left running all day. Leaving it should be a
deliberate two-key act, not something a tired hand does by reflex on its way to
a letter. The Exit row went for the same reason: a row that ends the session
sits in the same list as a row that logs a feed, reachable by the same
keystroke.

**Consequences.** This overrides the earlier rule that the way out should be a
row on the screen as well as a key. The footer names `ctrl-q quit` on every
screen, which is what carries it now. `Ctrl-C` is a deliberate exception rather
than a second way out: a full-screen program that ignores it is one somebody
has to kill from another terminal.

**Revisit when.** Somebody gets stuck. That would mean the footer is not doing
its job, which is a thing to fix rather than a reason to hand `q` back.

## `Ctrl-C` cancels a level; `Ctrl-Q` cancels the session

**Decision.** `Ctrl-C` walks out of one menu level, and ends the session only
when there is no level left to walk out of. `Ctrl-Q` ends it from anywhere.

**Why.** The first version of this made `Ctrl-C` a second quit, which was a
worse answer than either alternative. `Ctrl-C` already means something
everywhere else: cancel the thing you are in. Inside a submenu that thing is
the submenu. Honouring that means the chord a tired hand reaches for never
destroys more than it looks like it will, and somebody holding it down walks
out of the menu rather than losing the screen on the first press.

That leaves `Ctrl-Q` as the one key that does not care where you are, which is
what a force quit should be.

**Consequences.** There is no single key that leaves, by design, and the footer
names `ctrl-q quit` on every screen because that is the one worth advertising.
`Ctrl-C` needs no advertising: it is the key somebody presses without being
told, and it now does the least surprising thing when they do.

**Revisit when.** Somebody wants out of a deep menu in one press and finds
`Ctrl-Q` too far from the home row.

## The family's day governs arithmetic, never timestamps

**Decision.** `day_start`, `day_end` and `day_mode` decide which events are
counted together. They never decide what day a thing is said to have happened
on. `log`, `edit` and `delete` group under ordinary calendar days, midnight to
midnight, and will not be re-based.

**Why.** "When did this happen" has one answer and it is the one on the clock.
A 4am feed happened at 4am on the date the clock said, and a list of what
happened must not argue with a phone, a hospital note or anybody's memory. It
is only when that feed is being *counted* that it matters which day's total it
belongs to, and that is the only question these hours answer.

**Consequences.** Under a 6am day start, a 4am feed appears under today's date
in `log` and is counted on yesterday's row in `summary`. That looks like a
contradiction and is not: the two screens are answering different questions,
and each is answering its own correctly. This was previously recorded here as
an unfinished edge; it is the intended design.

A new screen joins the rule by asking one question: does it aggregate? If it
adds, averages, groups for a total or draws a row that sums, it uses the day
rule. If it lists what happened, it uses the calendar.

**Revisit when.** Never for the principle. A screen that does both at once
would need its two halves labelled, which is a presentation problem rather than
a reason to change which day is which.

## A command runs inside the shell, not instead of it

**Decision.** Choosing a row never takes the screen away. The Menu view becomes
the command's questions, its output and its failures, and the header, the Now
drawer and the keys stay where they are. `dash` is the one exception.

**Why.** The reason to keep the facts on the screen is to be able to look at
them *while* answering a question. How long since the last feed is part of
deciding what to log next, and a screen that hid it the moment somebody started
logging hid it exactly when it was wanted. Suspending also looked like leaving
the program, which is the opposite of what an always-on app should feel like.

**How, without two of everything.** Every interactive loop here is the same
shape: draw some lines, wait for a key, decide, repeat. `prompt::host` lets
something else do the drawing and the waiting, so the menus, the text fields and
the browsable listings are the same code in both places. The alternative was a
panel written per flow, which would have meant two vocabularies for every
question and two places to fix every bug. This is the same rule as the Now
drawer, applied to input instead of output.

Output needed the same treatment, and was already nearly there: `render::print`
and `render::note` are the only two places this tool writes, so taking them was
one change rather than fifty. Everything that printed directly was moved onto
them.

**Consequences.** The command runs on a task and blocks on a channel while a
question is up, so the loop must always answer: output is acknowledged at once
and a frame is answered by the next keystroke. The loop polls every 30ms while
a command is running rather than every second, because a question that arrives
between keystrokes should not wait a second to appear.

`Ctrl-Q` is read before anything reaches a command, so no question can trap
somebody. Every other key belongs to the flow while one is running, and the
footer says so.

The prompts lay themselves out to the panel rather than the terminal, through
`host::size`, so a table drawn in a panel fits the panel.

**Revisit when.** A flow is common enough to deserve better than a stack of
hosted questions. A diaper is four questions in a row where it could be one
screen, and that is a panel worth writing; it would be an addition to this
design rather than a change of it.

## The dashboard is drawn in the panel, so nothing takes the screen

**Decision.** `h dash` inside the shell is a view drawn in the panel where the
menu is, from the reading the shell already has. Nothing suspends the shell any
more, and `Shell::suspend` is what the shell does when it ends.

**Why.** It was left as the one exception because it is a second full-screen
program and a terminal has one alternate screen to give. But "everything runs
inside the shell except one thing" is a rule somebody has to remember, and the
exception was the command most likely to be opened and left open.

Drawing it in the panel turned out to cost one parameter: the dashboard already
splits a frame into a tab bar, a body and a status line, so taking an area
instead of the whole frame was the change. Everything else it needs, the shell
already had.

**Consequences.** It reads no data of its own. `r` refreshes it along with the
widgets, which also means it can never disagree with the drawer above it about
how stale it is. It stays a command in the catalog, because `h dash` is one, and
the shell answers that row with a view instead of a job; the test that walks the
menu asserts it is still reached.

Its own status bar drops its key hints inside the panel, because the shell's
footer names the keys and they are not the same keys: `q` does not leave in
there.

The Now tab and the Now drawer now say the same things twice while it is open.
That is worth fixing and is not fixed here.

**Revisit when.** The shell's own widgets cover enough that the dashboard is
redundant rather than convenient.

## `q` leaves, and `Ctrl-Q` leaves from inside a question too

**Decision.** Both `q` and `Ctrl-Q` end the session. `q` works whenever the
shell itself has the keyboard; `Ctrl-Q` works from anywhere, including from
inside a question somebody is typing an answer into. This supersedes the
earlier decision above that took `q` away.

**Why.** Taking `q` away was solving the wrong problem. The worry was a tired
hand hitting `q` by reflex, but the case where that actually matters is a text
field, where `q` is a letter going into an answer. Everywhere else `q` is what
a hand reaches for to close a full-screen program, and making somebody use a
chord for it was a cost paid on every use to avoid a mistake possible on a few.

Now commands run inside the shell, the distinction is easy to draw. While one
is asking, every ordinary key belongs to it. `Ctrl-Q` is not a letter, so it
gets through regardless, which is exactly what a force quit is for.

**Consequences.** `keys::forces_quit` is the predicate the loop checks before
handing a key to a running command, and it is the chord alone. The footer names
`q quit` on the ordinary screens and `ctrl-q quit` while a command has the
keys, so each says the one that works where it is shown.

A hosted prompt that wants `q` for itself gets it. The browsable listing
already does: `q` leaves the list, and now that is what it does inside the
shell too.

**Revisit when.** A prompt turns up where `q` is neither a letter nor a
shortcut, and somebody expects it to quit.

## Input is queued, and the panel draws the last frame

**Decision.** `Exchange` in `src/tui/job.rs` keeps input that arrives before a
question is ready for it, and the flow panel draws the last frame rather than
the one awaiting an answer.

**Why.** Both were bugs, and both came from the same shape: a command runs on
its own task, so between answering a keystroke and the command's next frame
there is a moment when nothing is waiting for input and nothing is pending to
draw.

Turning input away in that moment lost characters. Typing at ordinary speed
was enough; a dictation tool pasting a phrase was worse, because the whole
burst arrived inside one window. `6:30am` came out as `63a`.

Drawing the pending frame in that moment drew nothing, once per character,
which is what the flicker was.

**Consequences.** Bracketed paste is enabled on the shell's screen, so a paste
is one event rather than one per character: easier to keep together than to put
back together, and a pasted newline can be dropped rather than read as Enter
submitting an answer halfway through. `host::Input` carries either, and each
hosted loop says what it does with a paste: text appends it, a listing types it
into the search box, a menu has nowhere to put it.

`Exchange` is pure, so both rules are asserted rather than observed. That
mattered here: the tests were written against the old behavior first and failed
exactly the way the bug did, which is the only reason there is evidence the
diagnosis was right.

**Revisit when.** A hosted loop wants to know that input is waiting before it
draws, to skip frames under a fast burst. Nothing needs that yet.

## `j` and `k` are the only letters that move

**Decision.** In every menu, list and screen in this tool, `j` is down and `k`
is up and no other letter moves a cursor. `h` and the left arrow mean back:
out of a submenu, out of a list, and out of a question without answering it.
`l` and the right arrow mean forward where there is one.

**Why.** The prompts predate the shell and read `h` as down and `p` as up, so
the same letter moved the cursor inside a question and walked out of the menu
behind it. Two meanings for one letter is a letter nobody can press without
looking, which is the one thing this tool's screens exist not to require. It
was carried as a known rough edge for several changes; it is not one now.

**Consequences.** `p` and `P` no longer move anything. Ctrl-J, Ctrl-K, Ctrl-N
and Ctrl-P still do, because they are chords rather than letters and they keep
working while a search filter has focus, where every letter is text.

The dashboard's Log tab used to read `h` as down so a long list could be
scrolled with it. It does not any more: the dashboard is the only screen here
with a left and a right, `h` and `l` are them on every tab, and `j` and `k`
scroll. `action_for_tab` no longer varies by tab, which is the point of it.

**Revisit when.** A screen turns up with a left and a right that is not the
dashboard, and `h` has to mean one of them there rather than back.

## A running sleep takes two lines, and says when the last one finished

**Decision.** While a sleep is running, `now` draws it on one line and the
previous sleep on another beneath it, muted and indented to the value column.
The note reads `(previous sleep finished 2h 10m ago · slept for 1h 20m)`.

**Why.** It was one line holding two facts, and a tired eye reads that twice to
find where the first one ends. Splitting them costs a row on a screen that has
rows to spare, and the indent keeps the note under the value it is about rather
than under the label, so the column still reads down.

The wording changed with it. It said *was*, which reads as when the sleep
happened; what the number actually measures is from that sleep's **end**, so it
says *finished*. And it now carries how long that sleep ran, which is the
question asked straight after "when did it end".

**Consequences.** `render::now::screen` returns rows rather than one row per
fact, so a fact may take more than one. The shell's Now drawer follows without
changing, which is the point of it drawing the same rows.

**Revisit when.** Another fact wants a note of its own and the pattern needs a
name rather than a helper.

## The dashboard drops its Now tab inside the shell

**Decision.** Drawn in the shell's panel, the dashboard leaves out its Now tab
and opens on Sleep. Run on its own, `h dash` still has all five.

**Why.** The shell's Now drawer is already on the screen above it. The same
facts twice is one of them wasted, on the screen where room is scarcest.

**Consequences.** The tab list is a property of the state rather than a
constant, so the digits count the tabs that are there and wrapping wraps around
them. `Tab::ALL` stays the full set, because the standalone dashboard still
has it; `State::tabs` is what anything drawing or moving should ask.

**Revisit when.** The drawer stops showing what the Now tab shows, or the
dashboard stops being drawn beneath it.

## The typical ranges live in a file, and say where they came from

**Decision.** The age bands are `data/reference.toml`, read in at compile time,
one band per age with a `source` line naming where the range came from. They
run from the first day to five years.

**Why.** They were Rust constants, which made adding an age band a code change
and made checking one a matter of trusting whoever typed it. In a file they can
be read, argued with and corrected by somebody who is not editing the program,
and the `source` line means a future reader can check a number rather than
trust it. The two rules that matter are in the file's own header, where
somebody editing it will see them.

TOML rather than YAML, which is what was asked for: `toml` is already a
dependency of this tool and does the same job, and the rule in
[`rules/rust.md`](rules/rust.md) is to check whether something already present
does it before adding a crate. It is a one-line change if YAML is wanted.

**Consequences.** A band with no ceiling leaves `high` out, and a test asserts
every such band says "or more" in its label, so a line missing by accident
cannot quietly turn a range into a floor. Another test asserts no metric in the
file mentions volume, which is the rule about milk made mechanical.

Where sources disagreed, the band is the wider one. Wet diapers over the second
half of the first year are quoted between four and eight a day depending who is
asked, and the floor here sits under all of them: a band that tells somebody
they are short when every source says they are not is worse than a band that is
merely wide.

**Revisit when.** A band is wanted that is not a simple low/high by age, such
as one that depends on weight.

## A day still going is never called short

**Decision.** On `now`, a figure below its typical range reads as where today
has got to (`3 so far today`), not as a verdict. Over and inside are said
plainly, because they have already happened.

**Why.** The ranges are per day, and for most of the day every figure is under
every one of them: at eight in the morning the day is an hour old. "Today is
under that" would be true and useless, four times over, every morning, on a
screen whose whole job is not to frighten anybody. It is the same reason the
summary's averages skip today and the trend chart marks today "still going".

**Consequences.** Below is painted grey rather than yellow, because it is not a
reading. Zero reads as "nothing yet today" rather than `0s`, the way the column
beside it already says "nothing logged".

The summary still says "this week is under that", and should: a week of
complete days is a thing that can be short.

**Revisit when.** The screen learns what time it is well enough to say "under"
once the day is nearly over, which is a real improvement and a different
change.

## Every number about a baby is adversarially reviewed, and the review is kept

**Decision.** No figure describing a child goes into this tool on one source's
word. The method is [`docs/research/methodology.md`](research/methodology.md):
source tiers, independent reviewers told to disprove rather than confirm, a
separate reviewer auditing the sources without seeing the verdicts on the
numbers, a fixed verdict vocabulary, and an exact quote with a URL for every
claim. Each review is written up in `docs/research/`, including the claims that
came back confirmed.

**Why.** The first review of `data/reference.toml` checked 19 bands and only 4
survived unchanged. Five attributions credited a body with words it had never
published. One floor was the exact value the literature uses to flag inadequate
intake, presented to the parent as the reassuring number. None of that was
visible without going back to the primary sources and trying to break them,
and none of it would have been caught by a reviewer asked merely to check.

**Consequences.** Adding a band is slow now, and deliberately so. Declining to
publish a band is a normal outcome rather than a failure: four were deleted in
the first review because no qualifying source supports a range at that age.
Advocacy organisations, manufacturers and content sites cannot carry a claim on
their own, which cost us several convenient numbers.

**Revisit when.** Never for the method. For any individual band, when a body
revises a guideline, when a source is downgraded, or when somebody reports a
number that felt wrong in use.

## Bands say whether they are describing or recommending

**Decision.** A label opens with "typical" when it describes what was observed
in a population, and "recommended" when it reports what a body advises. A test
enforces one or the other.

**Why.** The sleep bands were recommendations wearing the word "typical". The
difference is not pedantic: observed sleep is far wider than recommended sleep,
so a recommendation labelled "typical" told more than half of a healthy newborn
population that they were short, every night. The same number is sound guidance
and a false alarm depending only on which word introduces it.

**Consequences.** The column mixes two vocabularies, which is a small cost in
tidiness for a large gain in honesty. The rule that a floor sits under every
qualifying source governs "typical" claims; a "recommended" band is judged
against the body that recommends it, because being under advice is a true
statement in a way that being outside normal is not.

**Revisit when.** A metric acquires both an observed range and a recommendation
worth showing together, which would need a second line rather than a second
word.

## Counts are floors, and only sleep has a ceiling

**Decision.** Feeds, wet diapers and dirty diapers are floors with no upper
bound. Sleep keeps both bounds.

**Why.** No qualifying source names a high number of feeds or diapers as
atypical, and several say the opposite: newborns void about twenty times a day
in the first month, cluster feeding can run hourly, and the WHO exempts
breastfed babies by name from its stool-frequency definition of diarrhoea. The
ceilings the file used to carry were therefore inventions, and inventions that
only ever fired on normal babies. One of them called a baby meeting CDC
guidance exactly "above typical".

**Consequences.** "Today is over that" can only ever appear for sleep, which is
worth knowing when reading the rendering tests. A high count is not a thing
this tool remarks on.

**Revisit when.** A source publishes an upper bound for a count, which none
currently does.
