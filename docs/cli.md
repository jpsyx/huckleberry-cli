# The command line

Every action has a flag or a subcommand, and every value a person can omit is
asked for when there is a terminal and refused by name when there is not. Both
audiences are first class; see [`rules/cli-ux.md`](rules/cli-ux.md) for why.

## Getting started

```sh
huckleberry-cli auth login          # asks for email, password and timezone
huckleberry-cli child list          # who is on the account
huckleberry-cli child use           # pick one, and remember it
huckleberry-cli now                 # the screen this tool exists for
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
huckleberry-cli sleep start
huckleberry-cli sleep pause
huckleberry-cli sleep resume
huckleberry-cli sleep stop        # records it
huckleberry-cli sleep cancel      # throws it away
huckleberry-cli sleep status
```

`sleep start` refuses to start a second sleep over a running one, because that
would leave the first unrecorded.

### Feeding

```sh
huckleberry-cli feed bottle --amount 90 --type formula
huckleberry-cli feed bottle                       # asks, offering the last amount

huckleberry-cli feed nursing start --side left
huckleberry-cli feed nursing switch
huckleberry-cli feed nursing pause
huckleberry-cli feed nursing stop
huckleberry-cli feed nursing status

huckleberry-cli feed solids --food Avocado --reaction loved
```

`--type` takes `formula`, `breast-milk`, `cow-milk`, `goat-milk`, `soy-milk`,
`tube-feeding` or `other`. `--units` takes `ml` or `oz` and defaults to the
`units` setting. Asked interactively, the bottle prompt offers the last amount
and the last kind as defaults, which at 3am is most of the work.

`feed nursing start` with no `--side` offers the side opposite the last feed,
which is what the app suggests.

### Nappies, potty and growth

```sh
huckleberry-cli diaper --mode both --poo medium --color yellow --consistency loose
huckleberry-cli diaper                        # asks what was in it
huckleberry-cli potty --mode pee --how went-potty
huckleberry-cli growth --weight 3.6
```

The nappy prompt asks what was in it and stops there. It offers the colour and
consistency only when the answer was dirty, only when no detail flag was
passed, and only as a yes-or-no first.

### Foods

```sh
huckleberry-cli foods list --search avocado
huckleberry-cli foods list --custom
huckleberry-cli foods add "Sweet potato"
```

`foods list` shows the family's own foods and Huckleberry's curated database,
the latter flagged for common allergens and choking hazards.

## Settings

```sh
huckleberry-cli config show
huckleberry-cli config set days 14
huckleberry-cli config set          # asks which, and what
huckleberry-cli config path
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
huckleberry-cli now --json
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
huckleberry-cli now --json | jq -r '.last_feed.ago_seconds'
huckleberry-cli summary --days 30 --json > month.json
huckleberry-cli export --days 90 --out snapshot.json
huckleberry-cli --offline snapshot.json stripes
```

## Without an account

`export` writes a snapshot and `--offline` reads one back. Nothing under
`--offline` opens a socket, so every read-only screen works from a file:

```sh
huckleberry-cli export --out snapshot.json
huckleberry-cli --offline snapshot.json now
huckleberry-cli --offline snapshot.json dash
```

Write commands refuse `--offline` rather than pretending.
