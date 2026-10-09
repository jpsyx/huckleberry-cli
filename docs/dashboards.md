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
- **No typical range for milk volume, at any age.** The rule people reach for,
  150 to 200 ml per kilogram per day, is not published as a range by any
  professional body for a term baby, and measured intake in exclusively
  breastfed babies never reaches even its floor at any age. Drawing it would
  quietly tell a frightened first-time parent they are underfeeding their baby,
  every day, for a year. `data/reference.toml` simply has no such metric, so it
  returns nothing by construction rather than by a special case somebody could
  delete, and a test asserts none appears.
- **Counts are floors, and only sleep has a ceiling.** No qualifying source
  calls a high number of feeds or diapers atypical, so nothing can read as over
  the top of those bands. Sleep is the one metric a body puts a top on.
- **A band says what kind of claim it makes.** "typical" describes what was
  observed; "recommended" reports what a body advises. The sleep bands from
  four months on are recommendations, and observed normal is wider than all of
  them.
- **Every range closes with the pediatrician.** Wherever bands are printed, the
  last line says that these ranges are not this baby and that the pediatrician
  is right where they disagree. Both screens print the same constant, so the
  wording cannot drift. See [`research/`](research/) for where the numbers came
  from and how hard they were argued with.
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

Last fed            36m ago · 73 ml of Formula · 8:03 pm
Last diaper         1h 31m ago · wet · 7:09 pm
Last sleep          currently sleeping for 30m
                    (previous sleep finished 2h 10m ago · slept for 1h 20m)
Tonight             nothing finished yet

as of 2m ago

In last 4h
┌────────────────────────┬────────────────────────┐
│ Feed                   │ Sleep                  │
├────────────────────────┼────────────────────────┤
│ 146 ml total · 2 feeds │ 1h 20m total · 1 sleep │
│ 36m ago · 3h 4m ago    │ 2h 10m ago             │
└────────────────────────┴────────────────────────┘

Today
┌────────────────────────────────────────┬───────────────┐
│ Feed                                   │ Sleep         │
├────────────────────────────────────────┼───────────────┤
│ 430 ml total · 6 feeds · since 6:00 am │ 11h 18m total │
└────────────────────────────────────────┴───────────────┘
```

After the last-event facts and the night's sleep, totals are grouped by window.
`In last 4h` has Feed and Sleep columns, a header row, and one data row with
two lines: totals above event times. The daily table follows with the same
columns and a single-line data row. Each table sizes its columns to its own contents to avoid wasting width.
On narrow terminals, long cells wrap within the same data row, without adding
horizontal separators or dropping events.

A running sleep takes two lines: what is happening now, and the one before it
underneath in muted text, indented to line up with the value it is about. One
long line holding both is a line a tired eye reads twice to find where the
first fact ends. The note says *finished* because it measures from that
sleep's end, and it carries how long the sleep ran, which is the question asked
straight after.

The next line relabels itself. Inside the family's night window it reads
`Tonight`; outside it reads `Last night's sleep`. It used to name the date, which
was precise and is not what anybody calls it at 3am. An ambiguous label there is
the one that gets misread, so "tonight" and "last night" stay distinct.

**Every populated total says "total".** The word qualifies the quantity,
so a volume or duration cannot be mistaken for the amount per event.

**Each recent total carries the times behind it.** Event times occupy a
muted second line in the same table cell, with no border separating them
from the total. One event uses that line too; multiple events always use
` · `, including lists of two. The list has no leading verb. Sleep times
measure from the end of each completed sleep, because that is when the baby
woke. A sleep still running is described in the Last sleep fact above.
The daily table keeps the existing total wording and has no event list.

The screen has three sections: Last facts (including freshness), the stacked
recent and daily tables, and typical ranges for the baby's age. Available
**content width**, measured in terminal cells, determines their placement:

| Width | Arrangement |
| --- | --- |
| 96 cells or more | Last facts followed by typical ranges left, tables right |
| Below 96 cells, or unknown width | Last facts, tables, then typical ranges stacked vertically |

Facts and ranges wrap within their columns; table cells wrap inside their
borders. A muted vertical rule separates facts and ranges from tables for the
full height of the block, so neighboring entries are read independently.
Long words wrap at Unicode grapheme boundaries, and column padding counts
terminal cells rather than characters.

The CLI measures the terminal when `h now` runs. The shell recalculates the
layout on every frame, including after resize, using the drawer's inner width
minus its leading padding. Both consume the same rows; narrow widths stack
ranges instead of dropping them. On short screens the drawer still yields
height to the menu, so its lower lines can be clipped.

The ranges are judged against **today**, using the configured daily window.
They live in [`data/reference.toml`](../data/reference.toml); see
[`setup.md`](setup.md) for what a day means here.

**Each range names its subject**: `typical sleep at this age`, `typical feed in
the first weeks`, `typical diapers from day 3`. A label that opened with
`typical at this age` left the reader to work out what of, which is fine in a
table with a header and not fine in a column that has none. The age heads the
column with a blank line under it, so it reads as the heading it is rather than
as the first range.

The tables show the last four hours followed by the family's daily window.
A family counting a rolling day sees `In last 24h`; a family counting from
an hour sees `Today`, with `since 6:00 am` in the Feed cell. What `today`
covers is a setting, and [`setup.md`](setup.md) is what asks. A sleep in
progress counts towards the daily Sleep total.

A window with nothing in it says `nothing logged` rather than `0 ml`, for the
reason every other figure here does: zero is a claim about the baby and an
absent record is a claim about the record.

The live timers win over history: a sleep in progress has not been written to
the intervals collection yet, so history cannot know about it.

## `summary`

A day table with four merged category headings above its numeric columns:

| Category | Columns |
| --- | --- |
| feed | feeds, milk, formula, breast, nursed, daytime milk, night milk, daytime nursed, night nursed, milk ml/feed (or milk oz/feed), nurse/feed |
| sleep | sleep, night, longest, avg sleep |
| wake time | wake time, night wake, avg wake, longest |
| diaper | wet, dirty |

Volume headings follow the configured `ml` or `oz` unit. Milk/feed includes
formula and expressed breast milk, divided by the number of bottles with a
recorded amount. Unknown amounts are not zero measurements. Nurse/feed divides
both sides' duration by nursing sessions only. Neither average uses all feeds.
A missing denominator prints a dash and becomes `null` in JSON.

On a terminal, `summary` opens an interactive table. It runs through the prompt
host in the shell, keeping the Now drawer in place. Tab and Shift-Tab, left/right,
H/L, and A/D cycle every numeric column, wrapping at the ends. Day is fixed and
cannot be selected. Selection has brackets as well as colour. Numeric columns
shift into view only when the selection passes a visible edge. Reversing direction
moves the selection within the existing window until it passes the opposite edge.
This is the default behavior, with no scrolling-mode toggle.

A bar chart below the table shows the selected metric across the entire requested
window, oldest to newest, with a blank column between bars. Durations use hours;
the table keeps hours and minutes. Missing values leave empty slots. The dotted
horizontal line is the mean of that metric's recorded daily values in the most
recent seven complete family days. Today never contributes, and older days never
replace gaps. The dots are magenta against cyan bars. Where a dot crosses a bar,
its background retains the cyan bar color so the line overlays the bar without
cutting a dark hole through it. Every occupied bar cell also has a cyan background,
including fractional top glyphs, so the bar and overlapping dots fill equal cell
heights. Colored bars therefore display at whole-cell height resolution. Empty
slots and gaps keep the terminal background.
The legend shows the value and the number of usable days out of
seven; no usable days means no average line. Shorter requested windows use the
available complete days and say how many. The notes explain the dots even in
panels too short for the legend. If the window cannot fit separated bars, the
chart asks for a wider panel or fewer days instead of merging days together.
Up/down, J/K, W/S, PageUp/PageDown, and Home/End scroll the rows and notes while
the graph stays visible. Short panels omit the title to retain a data row.
Esc or Q returns directly to the menu. Piped output stays a plain table;
`--json` always prints numeric data and never opens an interactive screen.

Each row covers `day_start` to the next `day_start` in the family's timezone.
The default is today plus seven complete family days: eight rows and eight bar
slots. `--days N` selects N complete days plus today. The live read starts at the
oldest requested day's beginning, including across daylight saving changes.
Sleep reads also retain records beginning earlier but ending on or after that
boundary, so overnight carry-over and the first waking gap remain available.
Rows marked `~` are partial: today, or days not fully covered by an offline
snapshot's recorded read window. Older snapshots may lack the new eighth day's
first hours; those rows remain visible but never enter averages. Export nine
days (`h export --days 9`) to cover the default summary at the export time,
including the extra hour at a fall daylight-saving transition.
Today is partial and bold; completed-day averages continue to skip it and empty
rows. Existing reference bands and the pediatrician note retain their meanings.
No new reference ranges are attached to the new metrics.

Sleep totals merge overlapping intervals and split them at both the day and
night boundaries, using actual elapsed seconds through daylight saving changes.
Completed sleep counts and the longest sleep belong to the day the sleep began.
Average sleep is the mean full duration of completed sleeps that began on the
row's family day, including both daytime and nighttime starts. Unfinished live
sleeps do not contribute. The older `average_nap_seconds` JSON field remains
available with its daytime-only meaning.

Daytime/night milk and nursing assign each feed's amount or full nursing duration
by its start time, matching the existing daily feeding totals. Daytime runs from
`day_start` up to `day_end`; night starts at `day_end`. Inverted hours work too.
The splits always add back to the daily totals.

Summary rows are rebuilt on opening from the latest settings and raw history.
After any shell command, retained Now/dashboard history receives the current day
rule immediately, before the background refresh returns. Changing hours or mode
therefore updates calculations even if that refresh fails; derived totals are not
cached.

Wake totals estimate elapsed time outside recorded sleep. They subtract sleep
coverage from the elapsed family day, or from its elapsed night for night wake.
They stop at now for today and remain absent without sleep records. A running
sleep contributes up to now, a paused one up to its pause timestamp. Older
snapshots that omit that timestamp leave affected wake estimates absent.
These are estimates from the log, not independent measurements of waking.

Average and longest wake use complete gaps from one sleep's end to the next
sleep's start, after overlaps are merged. Both belong to the day waking began,
even if the gap crosses a day boundary. The unfinished gap after the last sleep
is excluded; so is any unknown gap before the first recorded sleep.

The muted note below the rows names the configured night start and end, and the
timezone. Its parenthetical shows `h config set day_end HH:MM` to change night
start and `h config set day_start HH:MM` to change night end, using current values
as editable examples. The same settings are reachable through the shell's Config
menu. See [`setup.md`](setup.md).

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
| `Tab`, `→`, `l`, `d` | next screen |
| `Shift-Tab`, `←`, `h`, `a` | previous screen |
| `1`–`5` | go straight to a screen |
| `j`, `k`, `w`, `s`, `↑`, `↓` | scroll the log |
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
↑/↓ j/k w/s move · / searches · enter opens one · h/a or q leaves
```

| Key | Does |
| --- | --- |
| `j`, `k`, `w`, `s`, `↑`, `↓`, `Ctrl-J`, `Ctrl-K` | move |
| `u`, `PgUp`, `PgDn`, `Ctrl-F` | move a half screen |
| `g`, `G`, `Home`, `End` | first, last |
| `/` | search: type and the list narrows |
| `Enter` | open what is under the cursor |
| `←`, `h`, `a`, `q`, `Esc`, `Ctrl-C` | leave |

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
assert!(text.contains("Last night's sleep"), "{text}");
assert!(!text.contains("Tonight"), "an ambiguous label is the bug: {text}");
```

## Duration labels

Daily sleep and night-sleep totals, their averages, and stripe-chart totals use
hours and minutes (for example `2h 30m`). Nursing side durations use the same
units as their total. JSON still exposes numeric seconds for calculations.

Interactive lists number every visible row and accept `j`/`s` for down and `k`/`w` for
up, in either case, as well as the arrows. `h`/`a` and the left arrow leave the
list, as Escape does. Search treats every letter as text.

The dashboard is the one screen here with a left and a right, so `h`/`a` and `l`/`d`
move between tabs, on every tab including the Log; `j`/`s` and `k`/`w` scroll.
Inside the shell, left/back closes the Dashboard view; Shift-Tab still selects
the previous tab. Lists reserve `d` to match `l` (no action), with PageDown or
Ctrl-F paging forward instead.

## Mixed diapers and day boundaries

A mixed diaper is stored as `both`. Normalization marks it wet and dirty, so
Summary, its selected-column chart, Trends, and the dashboard Diapers tab count
one wet and one dirty while the total remains one diaper. The dashboard wet
sparkline uses that same wet count. Now and the Now drawer use both flags for
running totals and show `wet + dirty` when the latest diaper is mixed. Logs
show `mixed` with both recorded sizes; Stripes draws one tick for the event.

Summary and Trends always group by `day_start`, including in continuous mode.
With a 7am day start, a mixed diaper at 3:42am is one wet and one dirty on the
previous day's row. The calendar-day log lists it on the date it happened.
Continuous mode makes Now's totals cover the last 24 hours; it does not change
Summary's daily boundaries. See [`setup.md`](setup.md).
