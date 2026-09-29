# The command line

Every action has a flag or a subcommand, and every value a person can omit is
asked for when there is a terminal and refused by name when there is not. Both
audiences are first class; see [`rules/cli-ux.md`](rules/cli-ux.md) for why.

## Reading the output

On a terminal, commands show labelled tables with a small emoji heading and
semantic colours. Successful recordings have a receipt: sleep shows its start,
end and duration; nursing shows the total and each side; bottles, meals,
diapers, potty trips and growth show what was recorded. Status, child profiles,
food lists, settings, account details and edit results use the same presentation.
Long tables become stacked, wrapped records on narrow terminals.

Times in receipts include the local date, year and timezone abbreviation, so a
sleep across midnight or a repeated hour at the end of daylight saving time is
unambiguous. Durations read `2h 30m`, including sleep totals in summaries and
stripe charts; a short session reads `35s`. Measurements carry their units.
Child profiles show the date of a weight measurement and honour the configured
measurement system. IDs are labelled wherever they are needed for another command.

For example, an overnight manual sleep produces:

```text
✅ 😴 Sleep recorded

┌──────────┬────────────────────────────┐
│ Detail   │ Value                      │
├──────────┼────────────────────────────┤
│ Started  │ Sep 27, 2026, 11:00 pm EDT │
│ Ended    │ Sep 28, 2026, 1:30 am EDT  │
│ Duration │ 2h 30m                     │
└──────────┴────────────────────────────┘
```

Redirecting stdout preserves the existing tab-separated or `key=value` payloads,
including numeric timestamps and seconds. Explicit JSON and export output also
retain their schemas. `NO_COLOR` removes colour without removing readable labels,
tables or emoji. Each stream decides independently whether it can use colour, so
redirected stdout stays free of ANSI escapes even when stderr is a terminal.

## Getting started

The installed command is `hb`. `./install.sh --name <other>` changes it, and
everything below works the same under whatever you pick.

```sh
hb auth login          # asks for email, password and timezone
hb child list          # who is on the account
hb child use           # pick one, and remember it
hb now                 # the screen this tool exists for
```

`auth login` saves one child automatically when the account has only one. The
password prompt draws one asterisk per character: the characters stay hidden,
and the length is visible, which is what catches a mis-paste before a failed
sign-in does. Backspace and Ctrl-U erase; Ctrl-C and Esc abandon.

## Global flags

| Flag | Environment | What it does |
| --- | --- | --- |
| `--child <CID>` | `HUCKLEBERRY_CHILD` | act on this child for one run |
| `--config <PATH>` | `HUCKLEBERRY_CONFIG` | use this settings file, and the credentials beside it |
| `--offline <PATH>` | | read a snapshot instead of Huckleberry |
| `--verbose` | `HUCKLEBERRY_VERBOSE` | detail on stderr |

## Reading

| Command | Shows |
| --- | --- |
| `now` | last feed, last diaper, asleep or awake, the night's longest stretch |
| `summary` | one row per day: feeds, milk, sleep, diapers, and the typical ranges |
| `stripes` | a 24-hour chart of where sleep lands, one row per day |
| `trends --metric <M>` | one number over time, as bars |
| `log` | everything, newest first: browsable, and searchable with `/` |
| `edit --list` | the same, with the name each entry answers to |
| `dash` | all of the above, full screen, live |
| `export` | the lot, as JSON |

`now` leads its Sleep row with the current session, for example
`currently sleeping for 30m (previous sleep was 2h 10m ago)`. The parenthetical
is muted when colour is enabled and omitted if no previous sleep is recorded.
When no sleep is running, the row reads `2h 10m ago · slept for 1h 20m`.
Previous-sleep elapsed time always starts at that sleep's **end**, not its
start. The longest-night-stretch row and `now --json` retain their existing
meanings.

`log` takes `--search <TEXT>` as well, which is the same filter the `/` key
types interactively. On a terminal it opens a list you can scroll, search and
open an entry from; piped, it is the same rows as plain text. See
[`dashboards.md`](dashboards.md) for the keys.

`now` and `summary` take `--json`. `summary`, `stripes`, `trends`, `log` and
`export` take `--days`. `log` takes `--kind` and `--limit`.

`trends --metric` accepts `milk`, `feeds`, `sleep`, `night-sleep`,
`longest-sleep`, `wet`, `dirty`, `nursing` and `pumped`. Left out on a
terminal, you are offered the list.

## Writing

Every new bottle, meal, diaper, potty trip and growth measurement asks **when**
after its other questions. Enter accepts `now`. Nursing starts ask when they
started; sleep and nursing pause, resume and stop actions, plus nursing side
switches, also ask when. Cancel and status do not record an event and need no time.

Every **When?** prompt, including history and ongoing-timer edits, shows
`E.g. '1:23 pm' or '123pm' or '32 min ago' are all valid` beneath the question
in the muted hint colour. The helper remains readable as plain text when
colour is disabled. Timer-start and both manual-sleep questions use the same
helper, and parsing errors repeat these examples.

All these questions share the sleep-start parser: `358 am`, `3:58 a.m.`,
`0358`, `21:30`, `9pm`, or whole relative minutes such as `32 mins ago`.
An ambiguous clock time prompts for AM or PM. Clock times mean their most recent
occurrence in the configured timezone; relative minutes count elapsed time across
midnight and daylight-saving changes. Invalid answers are asked again.

Time is the first activity question, before amounts, units, sides or notes.
Accepting `now` records the instant that question is answered, even if the
remaining questions take longer.

Use `--at <TIME>` to answer directly, or `--start <TIME>` for sleep and nursing
starts (`--at` is also an alias for nursing starts). Flag values must resolve
without an AM/PM follow-up. Without a terminal, omitted event times default to
now, preserving scripts. A timer transition before its current segment is
refused before writing. Stopping a paused sleep uses its pause time; stopping
paused nursing records only the already banked durations. Sleep resume retains
the existing continuous-sleep semantics, so its pause is included in the duration.

```sh
hb feed bottle --at "32 mins ago"
hb feed solids --at "358 am"
hb diaper --at now
hb potty --at 21:30
hb growth --at "10 minutes ago"
hb feed nursing start --start "20 mins ago"
hb feed nursing switch --at "5 mins ago"
hb sleep stop --at "3 mins ago"
```

Receipts for instant events include the chosen local date and time. Backdated
entries go into history without replacing a newer last-entry summary.

### Sleep

```sh
hb sleep start                               # asks when it started; Enter means now
hb sleep start --start "28m ago"
hb sleep start --start "358 am"
hb sleep pause
hb sleep resume
hb sleep end         # asks when it ended, then records it (alias for sleep stop)
hb sleep cancel      # throws it away
hb sleep status

hb sleep manual                              # asks when it began and ended
hb sleep manual --start 11:30pm --end 1:15am
hb sleep manual --start "120 mins ago" --end "30 mins ago"
hb sleep manual --start "32 min ago" --end now
```

`sleep end` (also `sleep stop`) asks **When did it end?** using the same
time reader, dimmed examples and `now` default as `sleep start`. Before the
question, it shows the active sleep's start date and local time (with timezone),
plus how long ago it started, for example `2h 30m ago`. This context goes to
stderr, including when `--at` is supplied, and counts time spent paused.
Pass `--at "32 min ago"` or `--at "1:23 pm"` to supply the end without a prompt.
Without a terminal, an omitted time defaults to now. A paused sleep still
ends at its recorded pause time.

Clock and relative time inputs accept trailing sentence punctuation
(`.`, `,`, `!`, `?`, `;`, or `…`), so `40 minutes ago.`, `now!`, and
`1:23 pm,` work in prompts and flags. Signs and punctuation inside relative
durations are not removed: negative or fractional minutes remain invalid.

`sleep start` refuses to start a second sleep over a running one, because that
would leave the first unrecorded. On a terminal it asks when the sleep started,
with `now` as the default. It uses the same clock reader as manual entries,
including `358 am` and the AM/PM question for ambiguous times. Clock times
mean their most recent occurrence in the configured timezone. Relative starts
accept whole minutes: `10 minutes ago`, `20 mins ago`, `28m ago`, and the
singular `minute` and `min` forms. Relative minutes are elapsed time, including
across midnight or a daylight-saving transition. Invalid answers are asked
again before anything is written.

`--start <TIME>` supplies the same value without prompting; ambiguous flag
values must include AM or PM. `--start now` starts immediately, and without a
terminal an omitted `--start` keeps the existing default of now.

`sleep manual` records a sleep that has already happened, and asks two
questions because that is all a sleep is. Both accept `now`, relative minutes
and flexible clock times, in prompts and flags. Relative answers count elapsed
minutes back from when each answer is read, preserving their date and seconds
across midnight and daylight-saving changes. Clock and relative answers can be
mixed. Both times are required when there is no terminal.

A bare start clock time is the most recent one, so at 3am `11:30pm` is last night,
and a clock-only end is
the first such time after the start, so a sleep across midnight needs nobody to
say so. A time is read the way it is said: `3:57am`, `3:57 AM`, `3:57 a.m.`,
`357am`, `0357`, `03:57`, `21:30`, `2130`, `9pm`, `7`. A bare twelve-hour time
like `3:57` is either half of the day, so you are asked which; with `--start`
it is refused, naming both spellings. The end must be after the start and no
later than now; relative end times are never rolled forward to repair an invalid
interval. During a repeated daylight-saving hour, a clock-only end uses the
first matching occurrence at or after the start, including the second occurrence
of that hour. Relative offsets that reach before the Unix epoch are rejected for
every activity-time input.

A sleep in progress is not in the way, because the entry is history and the
timer is a timer. They meet only when the entry runs into the running sleep,
and then `--overlap` says which to keep. Left out on a terminal you are asked,
in yellow, with no default: all three answers throw something away.

| `--overlap` | Does |
| --- | --- |
| `discard-sleep` | throws the sleep in progress away, and keeps the entry |
| `discard-manual` | keeps the sleep in progress, and throws the entry away |
| `record-sleep` | finishes and records the sleep in progress, and throws the entry away |

### Feeding

```sh
hb feed bottle --amount 90 --type formula
hb feed bottle                       # asks when, units, amount, milk, and notes

hb feed nursing start --side left
hb feed nursing switch
hb feed nursing pause
hb feed nursing stop
hb feed nursing status

hb feed solids --food Avocado --reaction loved
```

`--type` takes `formula`, `breast-milk`, `cow-milk`, `goat-milk`, `soy-milk`,
`tube-feeding` or `other`. `--units` takes `ml` or `oz`.

Asked interactively, a bottle is five questions: when, which units, how much,
what was in it, and anything to note. Every one of them arrives with an answer
already in it, so the fast path is five presses of Enter. The `units` setting
is what the units question offers, not what the command assumes: somebody who
mostly records in ounces still gives the odd bottle in millilitres, and
`hb config set units oz` changes what Enter takes. The amount offered is the
last bottle's, **converted into the units being recorded in**: the app stores
it in whatever units it was entered in, so offering it as it stands turned a
last bottle of 1.15 oz into an offer of "1.15" under a question reading "How
much, in ml?".

A meal asks when, then the food, how much, how it went, and anything to note.

`feed nursing start` with no `--side` offers the side opposite the last feed,
which is what the app suggests.

### Diapers, potty and growth

```sh
hb diaper --mode both --poo medium --color yellow --consistency loose
hb diaper                        # asks when, then what was in it
hb potty --mode pee --how went-potty
hb growth --weight 3.6
hb growth                        # asks when, the system, the weight, then the rest
```

`growth` asks when, then which system the numbers are in, offering the
`measurements` setting, then the weight, the length and the head circumference; only the
weight has to be answered, and only when no measurement was passed at all.

The diaper prompt asks when, then what was in it, and every question that answer
implies: how much wet, how much dirty, the colour, the consistency, whether
there was a rash, and anything to note. Optional details take Enter for
"leave it out", and the time takes Enter for now, so the fast path is still a few keystrokes, and a flag answers its own
question only: `--pee big` is not a statement about the colour. A potty trip is
asked the same questions minus the rash, which the app has no field for.

### Foods

```sh
hb foods list --search avocado
hb foods list --custom
hb foods add "Sweet potato"
```

`foods list` shows the family's own foods and Huckleberry's curated database,
the latter flagged for common allergens and choking hazards. Two hundred foods
is the listing this tool most needs a search in, so it opens browsable: press
`/` and type. `--search` is the same filter for a script.

### Correcting an entry

```sh
hb edit                                   # pick one off a list, then answer
hb edit --list                            # what there is, and what each is called
hb edit --id diaper/1758572400000-3f2a --set pee=big
```

`edit` selects an entry, then offers its editable fields. Each line shows the
value that field would be saved with, and a field changed during this form is
marked `(changed)`; Done keeps everything as listed. A field with fixed
alternatives offers Keep and, when optional, Clear. Times are asked outright,
because their own default already means keep.

A sleep is edited as the two times it spans: **Edit start** and **Edit stop**,
not a stored instant and a length. Both read like every other time question,
with the dimmed examples and the same flexible parser, and the stop is saved as
the duration between them. Moving the start keeps a stop already chosen where it
is; a start moved past that stop drops it and says so.

Selecting a time field opens **When?** (**When did it end?** for a stop) with
the stored date and time as its default, for example `2026-09-27 8:00 AM`. Keep
and Enter preserve the exact instant, including fractional seconds. A clock-only
answer such as `8am` keeps the entry's local date; a dated answer such as
`2026-09-27 08:00` can move days. A clock-only stop lands on the first such time
after the start, so an overnight sleep needs no date, and a stop that is not
after its start is refused. Relative minutes resolve when answered. Fixed
alternatives use lists; only notes, custom amounts and numeric measurements
require typing.

An active sleep appears first as **Ongoing sleep**, separately from saved
history. Selecting it asks for a new start using the same flexible clock and
relative-minute parser as `sleep start`. Enter keeps the exact existing start;
`now`, `358 am`, and `32 mins ago` replace it. The same operation is scriptable:

```sh
hb edit --id sleep/current --set "start=32 mins ago"
hb edit --id sleep/current                  # asks for a new start
```

`edit --list` includes `sleep/current` while a sleep is active, even when paused.
This edit changes only its start and synchronization timestamps: it preserves
the existing session, details, and running or paused state. It does not finish
the sleep or create a history entry. Future starts, starts after a paused
sleep's endpoint, and sessions stopped or replaced while the prompt was open
are refused. A concurrent update during the write is also refused; select the
current sleep again to retry. Scripts must supply `--set start=<TIME>`.

| Key | Does |
| --- | --- |
| `j`/`h`, `k`/`p`, `↑`, `↓` (either case) | move |
| `PgUp`, `PgDn`, `Ctrl-U`, `Ctrl-D` | move ten |
| `g`, `G`, `Home`, `End` | first, last |
| `Enter` | edit this one |
| `q`, `Esc`, `Ctrl-C` | leave, changing nothing |

Every list built from the stream (`log`, `edit`, `delete`) shows the clock time
of each entry, and for anything from the last five hours how long ago it was,
as in `11:28 am (2h 32m ago)`. Older rows show the time alone. The column is
padded to its widest cell, so the kinds stay aligned down the screen.

`--set` takes the fields of whichever entry it is: `mode`, `pee`, `poo`,
`color`, `consistency`, `rash` and `notes` on a diaper; `mode`, `how`, `color`,
`consistency` and `notes` on a potty trip; `amount`, `type`, `units` and
`notes` on a bottle; `left`, `right` and `notes` on a nursing session (in
minutes); `foods`, `amount`, `reaction` and `notes` on a meal; `duration` (in
minutes) and `notes` on a sleep, where the form asks for that duration as a stop
time. An empty value clears the field, as in
`--set color=`. Naming a field the entry does not have fails with the ones it
does.

Every saved entry accepts `--set at=<TIME>` (`start` is an alias), including
pumping, milestones, and growth, medication and temperature history. These
additional entries currently offer the time question only; their other fields
are preserved. An explicit `--id` can correct a time outside the picker window.

```sh
hb edit --id feed/1758572400000-3f2a --set "at=8am"
hb edit --id feed/1758572400000-3f2a --set "at=2026-09-27 08:00"
```

Changing a history time preserves duration and unmodeled fields. The API updates
timestamp-based IDs and affected last-entry summaries together, leaving batch
neighbors and active timers intact. The receipt prints the resulting entry ID,
which can change when the timestamp changes. Concurrent storage changes cause
the time correction to fail rather than overwrite newer data. Detail edits are
saved before the time correction; if that correction fails, the error says that
preceding detail changes have already been saved.

### Removing an entry

```sh
hb delete                                  # pick one off the list, then confirm
hb delete --list                           # what there is, and what each is called
hb delete --id feed/1758572400000-3f2a --yes
hb delete --tracker health --list          # the rows the stream does not show
```

The same recorded history `edit` shows, with the same keys; live timers are
excluded. It removes anything that came
from Huckleberry, not only what this tool can log, because taking a row away
needs no knowledge of what is in it.

Deleting is the one thing here that cannot be undone, so it is the one thing
that asks twice: the entry is described back before the question, and with no
terminal the failure names `--yes` rather than assuming consent.

`--tracker <NAME>` works on one tracker's own rows instead of the merged
stream, which is how a growth measurement or a temperature is reached at all:
`--list` there prints each row as it is stored.

A delete also puts the tracker's own shortcuts back. Every tracker keeps a copy
of its most recent entry (`prefs.lastDiaper`, `prefs.lastBottle`,
`prefs.lastGrowthEntry`) so the app can draw a home screen without reading
history; removing the row one of those describes would leave the app showing an
entry that is no longer there, so it is rewritten from whatever is now the most
recent of that kind, or taken away when there is none.

## Settings

```sh
hb config show
hb config set days 14
hb config set          # asks which, and what
hb config path
```

| Setting | Default | What it does |
| --- | --- | --- |
| `child` | — | which child commands act on |
| `timezone` | `UTC` | the family's IANA timezone |
| `days` | `7` | the default history window |
| `units` | `ml` | volume units, for display and for `--amount` |
| `measurements` | `metric` | growth units |
| `refresh` | `20` | dashboard poll interval, in seconds |
| `verbose` | `false` | detail without `--verbose` |

`timezone` is the one that matters most: every day boundary, night window and
stored offset is computed in it. It is validated when it is set, so a typo
fails then rather than quietly moving every day in the tool.

## Credentials

The email, the password and the session live in `credentials.toml` beside
`config.toml`, written with mode `0600`. `config.toml` holds no secret.

`HUCKLEBERRY_EMAIL` and `HUCKLEBERRY_PASSWORD` override the file, and a
password supplied that way is never written to disk. That is the way to drive
this from CI or from a password manager:

```sh
export HUCKLEBERRY_PASSWORD="$(pass huckleberry)"
hb now --json
```

`auth logout` deletes the file.

## Driving it from a script

- **stdout is data, stderr is the conversation.** Everything the command was
  asked for goes to stdout as readable terminal output, or as `key\tvalue`,
  `key=value` or JSON data when appropriate for scripts;
  prompts, progress and failures go to stderr. Piping the tool anywhere yields
  plain text with no escape sequences.
- **Nothing blocks on input it was given.** With stdin redirected, a missing
  value is a failure naming the flag, never a prompt.
- **A non-zero exit means it did not happen.**

```sh
hb now --json | jq -r '.last_feed.ago_seconds'
hb summary --days 30 --json > month.json
hb export --days 90 --out snapshot.json
hb --offline snapshot.json stripes
```

## Without an account

`export` writes a snapshot and `--offline` reads one back. Nothing under
`--offline` opens a socket, so every read-only screen works from a file:

```sh
hb export --out snapshot.json
hb --offline snapshot.json now
hb --offline snapshot.json dash
```

Write commands refuse `--offline` rather than pretending.

Fixed-choice prompts are numbered menus: arrows or J/H move down and K/P move
up; 1-9 highlight the corresponding item in every menu without accepting it.
Enter accepts the highlighted item. Zero and numbers beyond the menu's length
are ignored; items 10 and later remain reachable with arrows or letter keys.
The highlighted item is bold bright
white, including wrapped label lines, when terminal styling is enabled. Optional
choices include Skip.
Optional text offers Skip or Enter text, and existing values can be kept or
cleared. Esc and Ctrl-C cancel the current question without accepting it.

Meals offer known foods, Enter a new food name, Remove, and Done so multiple
foods can be selected without comma-separated typing. Solids amount offers
Some or a custom value. Sleep overlap resolution begins on Cancel. Units,
measurement system, and verbose settings use finite selection lists.

Optional command overrides are available through CLI flags. History editing
selects the entry and named fields, keeping existing values unless deliberately
changed; Clear is separate from Keep. Unfamiliar stored enum values survive
changes to other details.

Deletion with a named tracker and no ID opens a searchable entry picker on a
terminal. The selected row still requires the usual confirmation, defaulting to
No. Temporary configuration, child, verbosity and live or snapshot source
overrides can be supplied as global flags when starting `hb`.

With no command, `hb` starts a persistent interactive session when stdin and
stderr are terminals. Home starts with View latest (the same command as
`hb now`), followed by diaper, feed, sleep, Edit,
Visualizations, logs, other logging, Delete, More, and Exit. Read-only views run
immediately when selected: latest/current status, Dashboard, Trends, Summary,
Sleep stripes, logs, sleep/nursing status, food lists, child lists/profiles,
authentication status, configuration display/paths, and build information.
Every command starts immediately when selected, including logging, editing,
deleting, settings/account changes, and exports. Required questions and
command-specific confirmations remain in their normal flows; there is no Run,
Options, Change options, or Session options menu item.
After a read result (or leaving a full-screen view), Back is selected by default.
This keeps static results visible without a separate Continue prompt. Trends
still asks for its required metric before displaying a chart. Failed views offer
Back and an explicit Retry; they never retry automatically.
Successful logging returns home; views return to their parent menu. Other static
results remain visible until Continue. Escape cancels unfinished input and returns
to navigation; Escape or Ctrl-C at home exits cleanly. Failed writes return to
navigation after Continue and are never automatically retried. Bare nonterminal
invocations print help and exit successfully. Explicit commands keep their
scriptable behavior.

Long menu labels wrap within the visible terminal area. Completing Edit's field
picker without changing any fields reports that the entry is unchanged. Menu
output remains on stderr even when command results are redirected to a file.

History edits ask which fields to change, each shown with the value it would be
saved with; Done keeps every field as listed. Keep
preserves stored quantities, unknown values, potty outcomes and food metadata;
Clear removes just the chosen optional field. During network waits, Ctrl-C
returns to navigation. A submitted write may already have completed, so the
session asks you to check logs before retrying. Long text input scrolls to keep
the insertion point visible.
