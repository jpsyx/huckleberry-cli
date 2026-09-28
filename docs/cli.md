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
| `now` | last feed, last nappy, asleep or awake, the night's longest stretch |
| `summary` | one row per day: feeds, milk, sleep, nappies, and the typical ranges |
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
hb sleep start
hb sleep pause
hb sleep resume
hb sleep stop        # records it
hb sleep cancel      # throws it away
hb sleep status
```

`sleep start` refuses to start a second sleep over a running one, because that
would leave the first unrecorded.

### Feeding

```sh
hb feed bottle --amount 90 --type formula
hb feed bottle                       # asks, offering the last amount

hb feed nursing start --side left
hb feed nursing switch
hb feed nursing pause
hb feed nursing stop
hb feed nursing status

hb feed solids --food Avocado --reaction loved
```

`--type` takes `formula`, `breast-milk`, `cow-milk`, `goat-milk`, `soy-milk`,
`tube-feeding` or `other`. `--units` takes `ml` or `oz` and defaults to the
`units` setting. Asked interactively, the bottle prompt offers the last amount
and the last kind as defaults, which at 3am is most of the work.

`feed nursing start` with no `--side` offers the side opposite the last feed,
which is what the app suggests.

### Nappies, potty and growth

```sh
hb diaper --mode both --poo medium --color yellow --consistency loose
hb diaper                        # asks what was in it
hb potty --mode pee --how went-potty
hb growth --weight 3.6
```

The nappy prompt asks what was in it and then asks every question that answer
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
`color`, `consistency`, `rash` and `notes` on a nappy; `mode`, `how`, `color`,
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
