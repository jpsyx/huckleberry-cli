# The screens

Six views over the same data, and one set of rules about how a number is
allowed to be shown.

## The rules

These came from the parent the original dashboard was built for, and they are
enforced by tests rather than by good intentions.

- **Nothing is ever painted red, flagged, or alarmed.** Typical ranges appear
  as grey text. A number outside a band is visibly outside it, and the decision
  about what to do is the reader's. A test asserts that the day table emits no
  `error` or `warning` tone at all.
- **No typical range for milk volume, at any age.** The obvious rule, 150 to
  200 ml per kilogram per day, describes established feeding from about two
  weeks on, not the first week when intake is still ramping. Drawing it for a
  seven-day-old would quietly tell a frightened first-time parent they are
  underfeeding their baby, every day, in a chart. `domain::reference` simply
  has no such metric, so it returns nothing by construction rather than by a
  special case somebody could delete.
- **A number nobody recorded prints as a dash.** `0 ml` is a claim about the
  baby; `—` is a claim about the record. A bottle with no amount counts as a
  feed and contributes no volume.
- **Every "time ago" carries an honest "as of".** A stale "last fed two hours
  ago" can send somebody to wake a sleeping baby, so the screen always says
  when it last looked.
- **Averages skip today and skip empty days.** Today is half finished. A day
  before the parents started logging is not a day the baby had no wet nappies,
  and averaging it in shows a number under the typical range for no reason but
  a gap in the record.

## `now`

The 3am screen: four facts, in the order they get asked.

```
Wren

Last fed       36m ago · 73 ml of Formula · 8:03 pm
Nappy          1h 31m ago · wet · 7:09 pm
Sleep          asleep 40m
Tonight        nothing finished yet

as of 2m ago
```

The fourth line relabels itself. Inside the family's night window it reads
`Tonight`; outside it reads `Night of Sun 21 Sep` and names the night it means.
An ambiguous label there is the one that gets misread at 3am.

The live timers win over history: a sleep in progress has not been written to
the intervals collection yet, so history cannot know about it.

## `summary`

The table a pediatrician asks for.

```
day          feeds  milk ml  formula ml  breast ml  nursed  sleep  night  longest  wet  dirty
Sun 27 Sep       8      373         223        150     56m   11.3    7.9   2h 36m    7      4
Sat 26 Sep       9      443         299        144   1h 1m   13.1    9.9   2h 38m    8      3
…

average over 6 complete days: 9.0 feeds · 441 ml milk · 13.8h sleep · 8.0 wet · 3.5 dirty
Wren is 21 days old
  typical at this age: 8 to 12 feeds a day
  typical from day 5: 6 or more wet nappies a day
  typical at this age: 14 to 17 hours in 24 (this week is under that)
```

One asymmetry is worth knowing about. Sleep **seconds** are split across
midnight, so a sleep beginning at 23:46 contributes to both days. Sleep
**counts** and the longest stretch belong to the day the sleep began. Splitting
the counts would turn one overnight into two sleeps; not splitting the seconds
would lose the small hours off every night, which for a newborn is most of the
sleep there is.

## `stripes`

The one picture that answers a question the numbers do not: *where* is the
sleep landing.

```
            00          06          12          18
Mon 21 Sep │██▼◦████▼◦███▼ ◦ ▼ █◦█▼  █◦▼    █◦▼   ◦▼███◦·█▼█│ 13.0h
Tue 22 Sep │███▼█████▼████▼ ◦▼ ██◦▼  █◦▼█  ██◦▼  ◦██▼██◦·█▼█│ 14.1h
…
            █ asleep   ▼ feed   ◦ nappy   · night
```

Positions are fractions of each day's own length, not of a hardcoded 86400, so
a 23-hour or 25-hour day still fills the strip exactly. A feed is drawn over a
sleep block on purpose: a feed during a sleep is the thing somebody opening
this is looking for.

## `trends`

One number over time, as bars.

```
Milk by bottle

Mon 21 Sep    461 ml ██████████████████████████████████
Tue 22 Sep    436 ml ████████████████████████████████▎
…
Sun 27 Sep    373 ml ███████████████████████████▋  (today, still going)

average 441 ml · peak 461 ml
            ███▇██▇
```

Bars rather than a plotted line, because a terminal row is wide and a terminal
cell is coarse. Partial cells are drawn at eighth resolution so two days a few
percent apart look different. The scale always starts at zero: a chart of daily
totals scaled from its own minimum turns eleven, twelve and thirteen wet
nappies into a crisis and a recovery.

Today is marked "still going" so its short bar is not read as a drop.

## `log`

Everything, newest first, grouped by day. The screen for when the totals look
wrong and a feed is suspected of having gone unlogged, so it loses nothing:
every row of every collection appears exactly once.

## `dash`

The full-screen version, five tabs, redrawing every second so the live timers
count up, and re-reading on the `refresh` setting.

| Key | Does |
| --- | --- |
| `q`, `Esc`, `Ctrl-C` | leave |
| `Tab`, `→`, `l` | next screen |
| `Shift-Tab`, `←`, `h` | previous screen |
| `1`–`5` | go straight to a screen |
| `j`, `k`, `↑`, `↓` | scroll the log |
| `r` | re-read now |

Three keys leave, because a full-screen program that traps somebody's terminal
for guessing wrong is a bad program.

A failed re-read never takes the screen away. The numbers that were there stay
there and the failure goes on the status bar, because a dashboard that blanks
itself when the wifi drops is worse than one that admits it is showing
something from a minute ago.

Colours come from `src/theme.rs` through one function, so the dashboard and the
one-shot commands agree on what a heading looks like, and there is still
exactly one file that decides what a role means. They are indexed rather than
RGB, so the dashboard inherits the palette the person has chosen for their
terminal.

## Testing a screen

Every renderer takes values and returns `Vec<String>`, and the dashboard is
drawn into `ratatui`'s test backend. So the tests read like descriptions of
what the screen says:

```rust
assert!(text.contains("Night of Sun 21 Sep"), "{text}");
assert!(!text.contains("Tonight"), "an ambiguous label is the bug: {text}");
```
