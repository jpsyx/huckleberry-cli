# The command line

Every action has a flag or a subcommand, and every value a person can omit is
asked for when there is a terminal and refused by name when there is not. Both
audiences are first class; see [`rules/cli-ux.md`](rules/cli-ux.md) for why.

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
| `log` | everything, newest first |
| `edit --list` | the same, with the name each entry answers to |
| `dash` | all of the above, full screen, live |
| `export` | the lot, as JSON |

`now` and `summary` take `--json`. `summary`, `stripes`, `trends`, `log` and
`export` take `--days`. `log` takes `--kind` and `--limit`.

`trends --metric` accepts `milk`, `feeds`, `sleep`, `night-sleep`,
`longest-sleep`, `wet`, `dirty`, `nursing` and `pumped`. Left out on a
terminal, you are offered the list.

## Writing

### Sleep

```sh
hb sleep start                               # asks when it started; Enter means now
hb sleep start --start "28m ago"
hb sleep start --start "358 am"
hb sleep pause
hb sleep resume
hb sleep stop        # records it
hb sleep cancel      # throws it away
hb sleep status

hb sleep manual                              # asks when it began and ended
hb sleep manual --start 11:30pm --end 1:15am
```

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
questions because that is all a sleep is. Neither time carries a date: a bare
time is the most recent one, so at 3am `11:30pm` is last night, and the end is
the first such time after the start, so a sleep across midnight needs nobody to
say so. A time is read the way it is said: `3:57am`, `3:57 AM`, `3:57 a.m.`,
`357am`, `0357`, `03:57`, `21:30`, `2130`, `9pm`, `7`. A bare twelve-hour time
like `3:57` is either half of the day, so you are asked which; with `--start`
it is refused, naming both spellings.

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
hb feed bottle                       # asks which units, how much, what, and notes

hb feed nursing start --side left
hb feed nursing switch
hb feed nursing pause
hb feed nursing stop
hb feed nursing status

hb feed solids --food Avocado --reaction loved
```

`--type` takes `formula`, `breast-milk`, `cow-milk`, `goat-milk`, `soy-milk`,
`tube-feeding` or `other`. `--units` takes `ml` or `oz`.

Asked interactively, a bottle is four questions: which units, how much, what
was in it, and anything to note. Every one of them arrives with an answer
already in it, so the fast path is four presses of Enter. The `units` setting
is what the first question offers, not what the command assumes: somebody who
mostly records in ounces still gives the odd bottle in millilitres, and
`hb config set units oz` changes what Enter takes. The amount offered is the
last bottle's, **converted into the units being recorded in**: the app stores
it in whatever units it was entered in, so offering it as it stands turned a
last bottle of 1.15 oz into an offer of "1.15" under a question reading "How
much, in ml?".

A meal asks for the food, how much, how it went and anything to note.

`feed nursing start` with no `--side` offers the side opposite the last feed,
which is what the app suggests.

### Diapers, potty and growth

```sh
hb diaper --mode both --poo medium --color yellow --consistency loose
hb diaper                        # asks what was in it
hb potty --mode pee --how went-potty
hb growth --weight 3.6
hb growth                        # asks the system, the weight, then the rest
```

`growth` asks which system the numbers are in, offering the `measurements`
setting, then the weight, the length and the head circumference; only the
weight has to be answered, and only when no measurement was passed at all.

The diaper prompt asks what was in it and then asks every question that answer
implies: how much wet, how much dirty, the colour, the consistency, whether
there was a rash, and anything to note. Each one takes Enter for "leave it
out", so the fast path is still a few keystrokes, and a flag answers its own
question only: `--pee big` is not a statement about the colour. A potty trip is
asked the same questions minus the rash, which the app has no field for.

### Foods

```sh
hb foods list --search avocado
hb foods list --custom
hb foods add "Sweet potato"
```

`foods list` shows the family's own foods and Huckleberry's curated database,
the latter flagged for common allergens and choking hazards.

### Correcting an entry

```sh
hb edit                                   # pick one off a list, then answer
hb edit --list                            # what there is, and what each is called
hb edit --id diaper/1758572400000-3f2a --set pee=big
```

`edit` is the submission process again, with one difference: every question
arrives with what is already recorded as its answer, so Enter keeps it and only
what is typed changes. A field that can be empty takes `-` for "leave it out".

| Key | Does |
| --- | --- |
| `j`, `k`, `↑`, `↓` | move |
| `PgUp`, `PgDn`, `Ctrl-U`, `Ctrl-D` | move ten |
| `g`, `G`, `Home`, `End` | first, last |
| `Enter` | edit this one |
| `q`, `Esc`, `Ctrl-C` | leave, changing nothing |

`--set` takes the fields of whichever entry it is: `mode`, `pee`, `poo`,
`color`, `consistency`, `rash` and `notes` on a diaper; `mode`, `how`, `color`,
`consistency` and `notes` on a potty trip; `amount`, `type`, `units` and
`notes` on a bottle; `left`, `right` and `notes` on a nursing session (in
minutes); `foods`, `amount`, `reaction` and `notes` on a meal; `duration` (in
minutes) and `notes` on a sleep. An empty value clears the field, as in
`--set color=`. Naming a field the entry does not have fails with the ones it
does.

Two things `edit` will not do. It does not move an entry in time: a row's id in
Huckleberry leads with its own millisecond timestamp, so changing when
something happened would leave history sorted by a time the row no longer
claims. And it changes only what this tool can log, so a pumping session and a
milestone are listed but read-only; `--list` says which is which in its own
column.

### Removing an entry

```sh
hb delete                                  # pick one off the list, then confirm
hb delete --list                           # what there is, and what each is called
hb delete --id feed/1758572400000-3f2a --yes
hb delete --tracker health --list          # the rows the stream does not show
```

The same list `edit` shows, with the same keys. It removes anything that came
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
  asked for goes to stdout as `key\tvalue` or `key=value` lines or as JSON;
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
