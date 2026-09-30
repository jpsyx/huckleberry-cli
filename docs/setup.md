# Setup, and how a family counts a day

Two things this tool cannot guess: who you are, and what you mean by "today".
It asks for both the first time it needs them, and never asks twice.

## What it asks, and when

Every command goes through the gate in `src/setup/`. `--help` and `--version`
never reach it, because clap answers those and exits before this program runs.

1. **An account.** With no usable credentials, `auth login` runs first: email,
   password, timezone, and the child if there is only one on the account.
2. **When a day begins** (`day_start`) and **when it ends** (`day_end`).
   Always, whichever way totals are counted: every screen with a day on it is
   drawn between these two hours.
3. **How "today" is counted** for the running totals (`day_mode`), which
   decides only whether those totals use the hours or ignore them.

Three families of command are exempt from all of it, and all for the same
reason: they are how somebody gets out of an unconfigured state, and a gate in
front of them would be a locked door with the key behind it.

| Exempt | Why |
| --- | --- |
| `auth …` | it is how you sign in |
| `config …` | it is how you set the settings |
| `info` | it says where both of those live |

### Each half asks only for what it is for

The two halves of the gate are asked for separately, because they answer
different questions.

| | Asked for |
| --- | --- |
| **an account** | every command that is not exempt, unless `--offline` is reading a snapshot, which opens no socket and has nobody to be signed in as |
| **the day settings** | only the commands whose output depends on them: `now`, `dash`, `summary`, `trends`, `stripes`, and the shell, whose Now widget shows the same figures |

The two are independent on purpose. The hours are what a day *is*, and the
summary table, the trend bars, the stripe chart and the night window all need
them. The mode is a separate question about one thing only: what the running
totals on the 3am screen cover.

So `h --offline snapshot.json log` draws its list on a machine nobody has
configured, and `h diaper --pee` records a diaper without stopping a script to
ask about a setting it will never read. `h --offline snapshot.json now` still
asks, because it would otherwise print `Total fed today` against midnight, quietly,
and midnight is the one boundary that is wrong for everybody.

`shows_a_day` in `src/setup/mod.rs` is the list. **A command joins it the
moment it starts reading `Config::day_rule`**, and a screen left off it gets
midnight without saying so.

## Both audiences, as everywhere else

A person at a terminal is asked. An agent, a pipe or a CI job is not: it fails
naming every setting that is missing and the command that answers each, all in
one message rather than one per run.

```text
error: 2 settings are not configured: day_start, night_start. On a terminal
this is asked for; here, set it:
  h config set day_start <VALUE>    # when a day begins, e.g. 6:00 or 6am
  h config set night_start <VALUE>  # when night begins, e.g. 19:30 or 7:30pm
```

So a script sets them once and never meets the gate again:

```sh
h config set day_start 6am
h config set day_end 7:30pm
h config set day_mode discrete   # or continuous
```

## The two ways to count a day

### `continuous`: a rolling twenty-four hours

"Today" is the last twenty-four hours, ending now, and it moves with the clock.
The screens say `in last 24h` rather than `today`, because for a family counting
this way there is no today and a word doing the opposite of its job at 3am is
the word that gets misread.

### `discrete`: from an hour each morning

"Today" runs from `day_start` to now. **It is not a calendar day.** With a day
starting at 6am, a 4am feed belongs to the day before, which is where the
person who gave it will look for it. Midnight is the one boundary that is wrong
for everybody: nobody with a baby is awake at midnight thinking of it as one.

The night is simply the stretch from `day_end` to the next `day_start`, which
is why there is no third hour to configure: the night ends where the day
begins, and a family that had said both separately could have had an hour
belonging to neither.

## Which one you are offered

Setup reads the baby's age off the profile and offers the fitting one first.
Either answer is one keystroke, and `config set day_mode` changes it later.

| Age when setup runs | Offered |
| --- | --- |
| 12 weeks or under | `continuous` |
| over 12 weeks | `discrete` |
| unknown | `discrete` |

A newborn's day has no shape: feeds and sleeps are scattered round the clock,
and the only honest window is the last twenty-four hours. By twelve weeks most
babies have a day with a beginning, and "how much has she eaten today" starts
to mean since she woke. An unknown age gets `discrete`, which is what the word
means to anybody who has not thought about it.

The age is read once, to choose which answer is offered first. It is never
re-read and the mode never changes on its own: a screen that quietly started
counting differently one morning would be worse than one that counts a way you
chose.

## What the setting changes

| Where | What it does |
| --- | --- |
| `now`, and the shell's Now widget | `day_mode` decides what the feed and sleep totals cover, and whether they read `Total fed today` / `Total slept today` or `Fed in last 24h` / `Slept in last 24h` |
| `summary` and `trends` | each row is one of this family's days, `day_start` to `day_start`, so a 4am feed is counted on the row before |
| `stripes` | each row runs from the previous `day_end` to this one, so a night lands whole on one row |
| every screen with a night on it | `day_end` and `day_start` replace the profile's night, applied to the dataset once as it is read |

`child show` prints the night this tool uses, which is the configured one when
there is one. That is deliberate: it is the night the screens are drawn with.

## Why the stripe chart opens on the night

The summary counts its rows from `day_start`, because a day's totals are the
totals of a waking day. The stripe chart does not: its rows run from one
`day_end` to the next, so a row reads as *the night leading into this day, and
then this day*.

That is because of what the chart is for. Midnight, and `day_start` too, both
fall inside the longest sleep there is. Cutting a row at either puts half a
night at the right-hand end of one row and half at the left-hand end of the
next, which is the one thing a picture of where sleep lands exists not to do.
Opening on the night puts each night whole, at the left, where a week of them
can be compared at a glance.

The ruler above the chart still carries wall-clock hours (`00`, `06`, `12`,
`18`) placed where they actually fall, rather than hours counted from the left
edge. Looking up when a sleep happened should not be a sum.

## The rule: arithmetic uses your day, timestamps use the calendar

**`day_start`, `day_end` and `day_mode` decide which events are counted
together. They never decide what day a thing is said to have happened on.**

| Kind of screen | Counts by | Examples |
| --- | --- | --- |
| anything that aggregates | this family's day | `summary`, `trends`, `stripes`, the running totals on `now` and in the Now drawer |
| anything that reports an entry | the calendar, midnight to midnight | `log`, `edit`, `delete` |

`log`, `edit` and `delete` group under plain calendar dates and always will.
They are not being re-based, and that is the right answer rather than an
unfinished one: "when did this happen" has one answer and it is the one on the
clock. A 4am feed happened at 4am on the date the clock said, and a list of
what happened must not argue with a phone, a hospital note or anybody's memory.

It is only when that feed is being *counted* that it matters which day's total
it belongs to, and that is the question these hours answer. So under a 6am day
start, a 4am feed appears under today's date in `log` and is counted on
yesterday's row in `summary`. Both are correct, and they are answering
different questions.

A new screen joins this rule by asking one question: **does it aggregate?** If
it adds, averages, groups for a total or draws a row that sums, it uses the day
rule. If it lists what happened, it uses the calendar.

## Where the settings live

In `config.toml` beside every other setting, as
[`cli.md`](cli.md) describes. Times are stored as 24-hour `HH:MM` and read back
as a person says them, so `06:00` is confirmed as `6:00 am` and a mis-picked pm
is visible at once. A bare `6` is refused rather than guessed at: it is two
different times, and this is a setting read at 3am.
