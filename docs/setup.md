# Setup, and how a family counts a day

Two things this tool cannot guess: who you are, and what you mean by "today".
It asks for both the first time it needs them, and never asks twice.

## What it asks, and when

Every command goes through the gate in `src/setup/`. `--help` and `--version`
never reach it, because clap answers those and exits before this program runs.

1. **An account.** With no usable credentials, `auth login` runs first: email,
   password, timezone, and the child if there is only one on the account.
2. **How a day is counted** (`day_mode`), which has no default worth having.
3. **When a day begins** (`day_start`) and **when night begins**
   (`night_start`), for a family counting discrete days. A rolling day has no
   hour to begin at, so neither is asked.

Three families of command are exempt, all for the same reason: they are how
somebody gets out of an unconfigured state, and a gate in front of them would
be a locked door with the key behind it.

| Exempt | Why |
| --- | --- |
| `auth …` | it is how you sign in |
| `config …` | it is how you set the settings |
| `info` | it says where both of those live |

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
h config set day_mode discrete
h config set day_start 6am
h config set night_start 7:30pm
```

## The two ways to count a day

### `continuous`: a rolling twenty-four hours

"Today" is the last twenty-four hours, ending now, and it moves with the clock.
The screens say `in 24h` rather than `today`, because for a family counting
this way there is no today and a word doing the opposite of its job at 3am is
the word that gets misread.

### `discrete`: from an hour each morning

"Today" runs from `day_start` to now. **It is not a calendar day.** With a day
starting at 6am, a 4am feed belongs to the day before, which is where the
person who gave it will look for it. Midnight is the one boundary that is wrong
for everybody: nobody with a baby is awake at midnight thinking of it as one.

The night runs from `night_start` to `day_start`. The night ends where the day
begins, because a family that had said both and had them disagree would have an
hour belonging to neither.

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
| `now`, and the shell's Now widget | what `Fed today` and `Slept today` cover, and whether they are labelled `today` or `in 24h` |
| every screen with a night on it | `night_start` and `day_start` replace the profile's night, applied to the dataset once as it is read |
| `summary`, `stripes`, `dash` | the night window they shade and total |

`child show` prints the night this tool uses, which is the configured one when
there is one. That is deliberate: it is the night the screens are drawn with.

## Where the settings live

In `config.toml` beside every other setting, as
[`cli.md`](cli.md) describes. Times are stored as 24-hour `HH:MM` and read back
as a person says them, so `06:00` is confirmed as `6:00 am` and a mis-picked pm
is visible at once. A bare `6` is refused rather than guessed at: it is two
different times, and this is a setting read at 3am.
