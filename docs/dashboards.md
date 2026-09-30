# The screens

Six views over the same data, and one set of rules about how a number is
allowed to be shown.

The shell `h` opens with no command is a screen too, and the rules below apply
to it. What goes on it, and the one hand it is built for, are in
[`tui.md`](tui.md).

## The rules

These came from the parent the original dashboard was built for, and they are
enforced by tests rather than by good intentions.

- **Nothing is ever painted red, flagged, or alarmed.** A figure outside a
  typical range is yellow, which means look again, not something is wrong. Red
  is a verdict and this tool is in no position to deliver one. A test asserts
  that the day table emits no `error` tone at all, and `Tone::Attention`
  exists separately from `Tone::Warning` so that "outside the usual" and "the
  tool has a problem" cannot drift into the same colour.
- **Today is the brightest row, never the dimmest.** Wherever several days are
  shown, today is what somebody is looking for, so it is bold bright white
  while the other days stay fully legible in plain bright white. This holds in
  `summary`, `trends`, `stripes`, `log` and every dashboard table. Brightening
  one row is the rule; dimming the rest is not.
- **A figure with no band to judge it stays grey.** Unjudged is not the same as
  approved, which is why daily milk never turns green.
- **Colour is never the only carrier.** Each band line says where the week sits
  in words as well, so the meaning survives a pipe, a screenshot and colour
  blindness.
- **No typical range for milk volume, at any age.** The obvious rule, 150 to
  200 ml per kilogram per day, describes established feeding from about two
  weeks on, not the first week when intake is still ramping. Drawing it for a
  seven-day-old would quietly tell a frightened first-time parent they are
  underfeeding their baby, every day, in a chart. `data/reference.toml` simply
  has no such metric, so it returns nothing by construction rather than by a
  special case somebody could delete, and a test asserts none appears.
- **A day still going is never called short.** Every figure is under every
  range at eight in the morning, because the day is an hour old. `now` says
  where today has reached instead (`3 so far today`) and leaves the reading to
  the reader. Over, and inside, are verdicts a partial day can support, because
  they have already happened.
- **A number nobody recorded prints as a dash.** `0 ml` is a claim about the
  baby; `—` is a claim about the record. A bottle with no amount counts as a
  feed and contributes no volume.
- **Every "time ago" carries an honest "as of".** A stale "last fed two hours
  ago" can send somebody to wake a sleeping baby, so the screen always says
  when it last looked.
- **Averages skip today and skip empty days.** Today is half finished. A day
  before the parents started logging is not a day the baby had no wet diapers,
  and averaging it in shows a number under the typical range for no reason but
  a gap in the record.

## `now`

The 3am screen: four facts, in the order they get asked.

```
Wren

Last fed       36m ago · 73 ml of Formula · 8:03 pm
Diaper          1h 31m ago · wet · 7:09 pm
Sleep          currently sleeping for 30m
               (previous sleep finished 2h 10m ago for 1h 20m)
Tonight        nothing finished yet
Last 3h        73 ml · 1 feed
Fed today      430 ml · 6 feeds · since 6:00 am
Slept today    11h 18m

as of 2m ago
```

A running sleep takes two lines: what is happening now, and the one before it
underneath in muted text, indented to line up with the value it is about. One
long line holding both is a line a tired eye reads twice to find where the
first fact ends. The note says *finished* because it measures from that
sleep's end, and it carries how long the sleep ran, which is the question asked
straight after.

The next line relabels itself. Inside the family's night window it reads
`Tonight`; outside it reads `Night of Sun 21 Sep` and names the night it means.
An ambiguous label there is the one that gets misread at 3am.

On a screen wide enough, the typical ranges for this baby's age go in a second
column beside the facts, each judged against **today** rather than against the
week the summary judges. Below that width they follow underneath instead, and
in the shell's drawer, which cannot grow without taking room from the menu,
they are simply left out. The ranges themselves live in
[`data/reference.toml`](../data/reference.toml); see [`setup.md`](setup.md) for
what a day means here.

The three running totals answer the second question rather than the first: not
"when did she last eat" but "has she had enough". Three hours because that is
the interval a newborn feeds on, so the question the line answers is whether
one is due.

The last two say which window they mean rather than assuming one.
A family counting a rolling day sees `Fed in 24h` and `Slept in 24h`; a family
counting from an hour sees `Fed today` and `Slept today`, with `since 6:00 am`
on the first of them so both are anchored without saying it twice. What
`today` covers is a setting, and [`setup.md`](setup.md) is what asks. A sleep
in progress counts towards `Slept today`, because a baby asleep right now has
slept that time today whatever the intervals collection says.

A window with nothing in it says `nothing logged` rather than `0 ml`, for the
reason every other figure here does: zero is a claim about the baby and an
absent record is a claim about the record.

The live timers win over history: a sleep in progress has not been written to
the intervals collection yet, so history cannot know about it.

## `summary`

The table a pediatrician asks for.

```
day          feeds  milk ml  formula ml  breast ml    nursed     sleep     night  longest  wet  dirty
Sun 27 Sep       8      373         223        150       56m   11h 18m    7h 54m   2h 36m    7      4
Sat 26 Sep       9      443         299        144     1h 1m    13h 6m    9h 54m   2h 38m    8      3
…

average over 6 complete days: 9.0 feeds · 441 ml milk · 13h 48m sleep · 8.0 wet · 3.5 dirty
Wren is 21 days old
  typical at this age: 8 to 12 feeds a day (this week is in that range)
  typical at this age: 14 to 17 hours in 24 (this week is under that)
  typical from day 5: 6 or more wet diapers a day (this week is in that range)
  typical at this age: 3 or more dirty diapers a day (this week is in that range)
```

Today's row is bold bright white. In the block underneath, each figure and each
band line is green when the week is inside the range and yellow when it is not,
with milk grey because it has no range. It is not faint: this is the part
somebody reads to find out whether anything is off, and faint made it the
hardest thing on the screen to read. It is secondary by where it sits, not by
being hard to see.

Each row is one of this family's days, `day_start` to `day_start`, not a
calendar day. Under a day starting at 6am a 4am feed is counted on the row
before, which is where the person who gave it will look for it. See
[`setup.md`](setup.md).

One asymmetry is worth knowing about. Sleep **seconds** are split at the hour
the day starts, so a sleep across that boundary contributes to both rows. Sleep
**counts** and the longest stretch belong to the day the sleep began. Splitting
the counts would turn one sleep into two; not splitting the seconds would lose
part of it off one row. Most nights now fall inside a single row rather than
being divided at all, which is the point of counting days this way.

## `stripes`

The one picture that answers a question the numbers do not: *where* is the
sleep landing.

```
            00          06          12          18
Mon 21 Sep │██▼◦████▼◦███▼ ◦ ▼ █◦█▼  █◦▼    █◦▼   ◦▼███◦·█▼█│  13h 0m
Tue 22 Sep │███▼█████▼████▼ ◦▼ ██◦▼  █◦▼█  ██◦▼  ◦██▼██◦·█▼█│  14h 6m
…
            █ asleep   ▼ feed   ◦ diaper   · night
```

**A row opens at the previous day's end, not at midnight.** Midnight falls in
the middle of the longest sleep there is, and cutting the row there puts half a
night at the right-hand end of one row and half at the left-hand end of the
next, which is the one thing a picture of where sleep lands exists not to do.
Each row reads as *the night leading into this day, and then this day*, which
is how a night gets talked about anyway.

The ruler still carries wall-clock hours, placed where they actually fall
rather than counted from the left edge. Looking up when a sleep happened should
not be a sum.

Positions are fractions of each row's own length, not of a hardcoded 86400, so
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
diapers into a crisis and a recovery.

Today is marked "still going" so its short bar is not read as a drop.

## `log`

Everything, newest first, grouped by day. The screen for when the totals look
wrong and a feed is suspected of having gone unlogged, so it loses nothing:
every row of every collection appears exactly once.

Every displayed bottle and pumping volume uses the configured `units`, including
left and right pumping amounts. This also applies to entry details, edit/delete
lists and confirmations, and the dashboard's Log tab. Source records may mix
millilitres and ounces; normalization keeps the quantities in millilitres and
presentation converts them once for the reader.

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

## Every list: `log`, `edit`, `delete`, `foods list`

One module draws them all (`src/listing/`), because a list somebody scrolls to
find last night's diaper is the same list whichever command they came to run.
Ported from the listing view in this author's `jpsyx` CLI, which arrived at
this shape over a dozen commands.

An entry from the last five hours carries how long ago it was beside the clock
time, because that is the question being asked of the top of the list; older
rows show the time alone. The column is padded to its widest cell either way,
so the kinds stay in one line down the screen.

```
Auggie 40 entries

  when                  what     detail
Sun 27 Sep
› 11:00 pm (12m ago)    Sleep    slept 2h 30m
  10:41 pm (31m ago)    Bottle   37 ml of Formula
  10:32 pm (40m ago)    Diaper   pee · big
  7:47 pm (3h 25m ago)  Diaper   mixed · little pee · medium poop (brown, loose) · rash noted
  5:21 pm               Pumping  59 ml (L 30 ml, R 30 ml)
j/k or ↑/↓ move · / searches · enter opens one · q leaves
```

| Key | Does |
| --- | --- |
| `j`, `k`, `↑`, `↓`, `Ctrl-J`, `Ctrl-K` | move |
| `u`, `d`, `PgUp`, `PgDn` | move a half screen |
| `g`, `G`, `Home`, `End` | first, last |
| `/` | search: type and the list narrows |
| `Enter` | open what is under the cursor |
| `q`, `Esc`, `Ctrl-C` | leave |

The search matches every whitespace-separated word against the whole row — its
cells, its heading and its note — so `diaper mixed` narrows rather than widens.
A group keeps its heading while one of its rows survives, because the heading
is context for the rows and not one of them. `Esc` clears the filter before it
leaves the list, so backing out of a search never costs the screen.

What `Enter` opens depends on the command: `edit` fills the entry's form again,
`delete` asks to confirm, and a listing nobody is choosing from prints what is
known about the row.

A row the command cannot act on is listed and drawn back rather than left out,
because the stream is the stream. Pressing Enter on one says why on the line at
the foot instead of doing nothing at a keystroke somebody meant.

Each tracker has a colour of its own — sleep blue, feeding cyan, diapers
magenta, pumping green, milestones yellow — so forty entries can be read by
shape before they are read by word. A kind shares its hue with a role it never
appears beside, and means nothing of that role: in a list of kinds a colour is
a category, not a verdict, and every row says its kind in words as well. The
row under the cursor is bold white, brighter than any of them, as today is
everywhere else in this tool.

A diaper says what was in it in the words the app's own buttons use, with the
sizes that were recorded: `mixed · little pee · big poop`. A size nobody
recorded is left out rather than guessed at, and a diaper with one thing in it
needs no label on its size (`pee · big`), because there is nothing else it
could be the size of.

Columns are laid out once across every group, so the rows under one heading
line up with the rows under every other. When the terminal is too narrow for
that, the columns share out what there is: narrow ones keep their natural width
and the greedy ones split the rest, so one long description cannot push the
time and the kind off the screen.

**A listing is only browsable when both ends are a terminal.** Piped output and
anything an agent runs are the same rows as plain, aligned text, so a listing
stays parseable; and naming a filter (`--search`, `edit --list`) says "just show
me", which prints and gets out of the way.

## Testing a screen

Every renderer takes values and returns `Vec<String>`, and the dashboard is
drawn into `ratatui`'s test backend. So the tests read like descriptions of
what the screen says:

```rust
assert!(text.contains("Night of Sun 21 Sep"), "{text}");
assert!(!text.contains("Tonight"), "an ambiguous label is the bug: {text}");
```

## Duration labels

Daily sleep and night-sleep totals, their averages, and stripe-chart totals use
hours and minutes (for example `2h 30m`). Nursing side durations use the same
units as their total. JSON still exposes numeric seconds for calculations.

Interactive lists number every visible row and accept `j` for down and `k` for
up, in either case, as well as the arrows. `h` and the left arrow leave the
list, as Escape does. Search treats every letter as text.

The dashboard is the one screen here with a left and a right, so `h` and `l`
move between tabs, on every tab including the Log; `j` and `k` scroll.
