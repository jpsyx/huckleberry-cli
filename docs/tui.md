# The shell, and the one hand it is built for

`h` with no command opens a full-screen app and keeps it open. It is meant to
be left running in a terminal all day: somewhere to glance at, and somewhere to
act from.

This file is the design brief for that app. **Every feature in this repository
is held to it**, not only the ones inside `src/tui/`, because a prompt that
demands typing undoes a menu that did not.

## The north star

> A parent at 3am, in the dark, with a baby on one arm and one hand free.

That person is tired, cannot see the keyboard, and cannot use two hands. Read
every design question back to them. They are not impressed by a shortcut they
have to remember, and they are not helped by a screen that needs a sentence
typed into it.

Everything else on this page follows from that one line.

## The rules

These are the ones that get argued about, so they are written down and, where
they can be, asserted in `tests/tui_shell.rs`.

### 1. Arrows, hjkl and WASD mean the same directions

| Direction | Arrow | Vim | WASD |
| --- | --- | --- | --- |
| down | `↓` | `j` | `s` |
| up | `↑` | `k` | `w` |
| back | `←` | `h` | `a` |
| forward, open | `→` | `l` | `d` |

The hand that is free is not always the one near the arrows, so all three sets
work throughout navigation, in either case. `Enter` and `Space` also open,
because a thumb finds the space bar without looking.

`h` and `a` mean **back** here: in a tree of menus, back is what left means.
Text fields and search filters still accept these keys as letters. Shifted
`W`/`S` leap on dials just like shifted `K`/`J`.

### 2. `q` leaves, and `Ctrl-Q` leaves from anywhere

**No menu row ends the session.** Leaving is a key, and there are two of them,
which differ only in how far each one reaches.

| Key | Does |
| --- | --- |
| `q` | ends the session, whenever the shell itself has the keyboard |
| `Ctrl-Q` | ends the session from anywhere at all, including from inside a question |
| `Ctrl-C` | cancels what you are in: one menu level, or the session when you are already at the top |

While a command is asking something, every ordinary key belongs to it, `q`
included: `q` is a letter, and a shell that quit when somebody typed the word
"quiet" into a note is a shell nobody trusts with a text field. `Ctrl-Q` is not
a letter, so it always gets through. That is the whole of the difference, and
it is why both exist.

`Ctrl-C` means what it means everywhere else, which is why it is not simply a
third quit. Inside a submenu the thing being cancelled is the submenu; at the
top level there is nothing left to cancel, so it is the session. The reflex
chord never destroys more than it looks like it will, and somebody who holds it
down walks out rather than losing the screen in one press.

Back at the top level does nothing at all: `h`, `a` and `Esc` are navigation keys,
and a session that ends because a thumb went left one row too far is a session
that has to be reopened in the dark.

Every submenu still ends in a Back row, so walking out of one is a row on the
screen as well as a key. Home ends in nothing, because there is nothing at Home
to walk out to.

Leaving View logs, Foods List, or the Edit/Delete entry picker with Escape,
left, `h`, `a`, or `q` returns straight to the menu. That key is the whole
navigation action; there is no "Anything else?" or Continue confirmation
afterward. Empty results and entry details still remain visible until the
reader leaves them. Enter on a browsed row without details, such as a food,
keeps the list and its selection in place.

### 3. Menus first, typing last

A fixed set of answers is a menu. A number is a preset plus "something else". A
sentence is the only thing that should ever need the keyboard, and it should
always be skippable.

When a new value has to be collected, the order to try is: a default that is
already right, a preset, a menu, a number nudged up and down, and only then free
text.

### 4. Enter alone is always a valid answer

Every question opens with the most likely answer already highlighted, so the
whole interaction is `Enter`. Times default to now. Edits default to the value
that is already stored. A default that is right nine times out of ten is worth
more than a shortcut that saves a keystroke on the tenth.

### 5. Nothing is ever only available on the command line

Every command, flag and correction is reachable and editable from the shell.
This is not aspirational: `tests/tui_shell.rs` walks the menu tree with motions
and asserts it reaches every command `clap` knows about, and
`tests/interactive_catalog.rs` asserts the menu tree covers the command tree.
A feature that ships with a flag and no route through the shell is unfinished.

The reverse also holds, and is the rule in
[`rules/cli-ux.md`](rules/cli-ux.md): everything here stays drivable by an agent
with flags alone.

### 6. Digits highlight; only Enter acts

`1`–`9` move the cursor to that row and stop. It is the same rule in every menu
and prompt in the tool. A number that submits is a number that logs a feed when
somebody meant to scroll.

### 7. The frame never moves

The header is the name and where you are; the middle is the panel; the last line
is the keys. A screen that is glanced at is read by position long before it is
read by word, so the parts stay where they were yesterday.

### 8. The keys are on the screen, always

There is no `?` to press. The line along the bottom says what the keys do, on
every screen, because the person reading it has not memorised anything and
should not have to.

### 9. Colour is semantic, and the rules about babies still apply

Colours come from `src/theme.rs` through one function, so this screen and every
other one agree on what a heading looks like. Everything in
[`dashboards.md`](dashboards.md) holds here too: nothing about a baby is ever
painted red, today is the brightest row, and colour never carries a meaning on
its own.

### 10. Decisions are pure; only the shell touches the terminal

`keys.rs` says what a keystroke means, `state.rs` says what it does to what is
on screen, `draw.rs` turns that into widgets, and `shell.rs` owns the alternate
screen. The first three are tested; the fourth holds nothing worth asserting.
This is the same split as `src/dashboard/`, for the same reason.

## What is on the screen today

The words for the parts are in [`nomenclature.md`](nomenclature.md). The short
version: the **Menu view** is the main panel, the **Now widget** is the one
widget beside it, and **widget** and **panel** mean the same thing.

```
 Huckleberry · Wren                                             Home
┌ Now ───────────────────────────────────────────────────────────────────┐
│ Wren                                                                   │
│                                                                        │
│ Last fed            36m ago · 73 ml of Formula · 8:03 pm               │
│ Last diaper         1h 31m ago · wet · 7:09 pm                         │
│ Last sleep          currently sleeping for 30m                         │
│                     (previous sleep finished 2h 10m ago · slept for 1h 20m) │
│ Tonight             nothing finished yet                               │
│                                                                        │
│ In last 4h                                                             │
│ ┌────────────────────────┬────────────────────────┐                    │
│ │ Feed                   │ Sleep                  │                    │
│ ├────────────────────────┼────────────────────────┤                    │
│ │ 146 ml total · 2 feeds │ 1h 20m total · 1 sleep │                    │
│ │ 36m ago · 3h 4m ago    │ 2h 10m ago             │                    │
│ └────────────────────────┴────────────────────────┘                    │
│                                                                        │
│ Today                                                                  │
│ ┌────────────────────────────────────────┬───────────────┐             │
│ │ Feed                                   │ Sleep         │             │
│ ├────────────────────────────────────────┼───────────────┤             │
│ │ 430 ml total · 6 feeds · since 6:00 am │ 11h 18m total │             │
│ └────────────────────────────────────────┴───────────────┘             │
│                                                                        │
│ as of 2m ago                                                           │
└────────────────────────────────────────────────────────────────────────┘
┌ What would you like to do? ────────────────────────────────────────────┐
│▌ 1. Log a diaper                                                       │
│  2. Log a feed                                                       › │
│  3. Log sleep                                                        › │
│  4. Edit                                                               │
│  5. Visualizations                                                   › │
│  6. View logs                                                          │
│  7. Other logging                                                    › │
│  8. Delete                                                             │
│  9. More                                                             › │
└────────────────────────────────────────────────────────────────────────┘
 ↑/↓ j/k w/s · ← h/a back · → l/d open · Enter select · r refresh · q quit
```

| Key | Does |
| --- | --- |
| `↓`, `j`, `s`, `Ctrl-N` | down one row |
| `↑`, `k`, `w`, `Ctrl-P` | up one row |
| `→`, `l`, `d`, `Enter`, `Space` | open the row under the cursor |
| `←`, `h`, `a`, `Esc`, `Backspace` | back, and nothing at the top level |
| `1`–`9` | put the cursor on that row |
| `g`, `Home` / `G`, `End` | first row, last row |
| `r` | refresh every widget |
| `q` | leave, whenever the shell has the keyboard |
| `Ctrl-Q` | leave, from anywhere, including from inside a question |
| `Ctrl-C` | leave this menu, or the session at the top level |

A chevron (`›`) marks a row that opens another menu, so nothing is a surprise.
The numbers are right-aligned as a column, so every label starts in the same
place whether its row is 9 or 10.

## How it runs a command

### 17. The shell is the container. A command happens inside it

**Choosing a row never takes the screen away.** The Menu view becomes the
command's questions, its receipt and its failures; the header, the Now drawer
and the keys stay exactly where they were. The shell is not something you leave
to do a thing and come back to. It is where the thing happens.

That matters because of what the other panels are for. The reason to keep the
facts on the screen is to be able to look at them *while* answering a question:
how long since the last feed is part of deciding what to log next, and a screen
that hides it the moment you start logging has hidden it exactly when it was
wanted.

A command runs on its own task and the loop keeps drawing the whole time, so
the clocks tick and the widgets refresh while a question is on the screen.

### 18. Nothing typed is ever dropped, and the screen never blinks

A command runs on its own task, so there is a window after every keystroke,
while it works out what to draw next, when nothing is waiting for input.
**Anything typed in that window is kept until the question that wants it turns
up.** Without that, typing faster than the command answers loses characters,
and a dictated phrase arrives in pieces: `6:30am` came out as `63a`.

For the same reason a paste arrives whole rather than as one key per character.
Bracketed paste is on, so a dictation tool's burst is one event to keep
together rather than a dozen to put back together, and a pasted newline is
dropped rather than read as Enter submitting the answer halfway through.

**And what is drawn is the last frame, not the pending one.** Answering leaves
nothing waiting until the command sends its next frame; drawing the pending one
meant drawing nothing for that moment, once per character, which reads as a
flicker under the hand.

Both live in `Exchange` in `src/tui/job.rs`, which is pure and is the only
place either rule is kept.

### 19. There is one implementation of every question

The questions in the panel are the same questions the command line asks. Not
a second set written for the shell: literally the same code, drawing through
whatever owns the screen.

Every interactive loop in this tool is the same shape: draw some lines, wait
for a key, decide, repeat. `src/prompt/host.rs` lets something else do the
drawing and the waiting. When the shell installs itself as the host, the menus,
the text fields and the browsable listings hand their lines over and get keys
back; when nothing is hosting, they own the terminal as before.

So a prompt improved for the command line is improved in the shell, and a
question can never word itself one way in one place and another way in the
other. It is the same rule as the Now drawer, applied to input instead of
output.

### 20. A value is turned, not typed

Times and volumes in the shell are answered on a **dial**: a row of columns,
each showing the two values either side of the one chosen so it reads as a
wheel rather than a number. A time has three columns, hour, minute and the
half of the day; a volume has two, the amount and its units.

```
When?

  11 : 42
  12 : 43   am
   1 : 44   pm  [now]
   2 : 45
   3 : 46
  ──
  ←/→ h/l a/d column · ↑/↓ j/k w/s turn · shift leaps
  n [now] · Enter set · Esc back
```

The `[now]` marks the row where **the column being turned** is showing its
present value: the row to turn back to. It follows the cursor, so moving from
the hour to the minutes moves the mark to wherever the current minute sits,
and it disappears when that value has scrolled out of sight rather than
pointing off the screen.

The mark recedes with the row it has moved to, muted one row out and fainter
two: left bright on a row whose numbers have gone quiet it would read as the
loudest thing on the screen while being the least important thing on it.

It is about one column, not the whole row. With the hour turned one on from
1:44 pm, the marked row reads `1 : 43 am [now]`, which is not a time anybody
would call now: the `1` is what the mark is about, because the hour is what
is being turned, and the rest of that row is wherever the other columns
happen to be sitting. The alternative was to mark only a row that is the
current time in every column, which is almost never any row once a dial has
been touched, and a mark that is never there answers nothing.

**`n` puts every column back on now at once**, not just the one being turned.
Following the mark by hand is a column at a time, and the dial is most often
wanted exactly where it opened: somebody turns it to check what time something
was, then wants now again. The key is offered only where there is a mark to
land on, and it is named after that mark so the two read as one idea. A jump
redraws rather than animating, because nothing about a reset is worth watching
travel.

```

**A time has two ways of being said, and Tab moves between them.** A dial
opens on how long ago, because that is how somebody says it out loud: twenty
minutes, not 1:24. Tab gets the clock for the times that are easier said that
way.

```
When?

   2h   02m
   1h   01m
   0h   00m ago  [now]
        59m
        58m
        ───

←/→ h/l a/d column · ↑/↓ j/k w/s turn · shift leaps
tab clock · n [now] · Enter set · Esc back
```

It opens on the minutes rather than the hours, because most corrections are
minutes: the thing was twenty minutes ago, not two hours. The column somebody
came to turn is already under the cursor, and the hours are one key to the
left when they are wanted. The clock still opens on the hour, which is where
reading one starts.

Both columns start at nought, which is now, and the mark says so. The word
`ago` sits on the answering row, because two numbers with a letter each say
which numbers they are and not what they mean. Both relative columns increase
upward: `↑`/`k`/`w` takes `0h` to `1h` or `00m` to `01m`. `↓`/`j`/`s` decreases
the selected column, with minutes wrapping from `00m` to `59m`.
Minutes turn one and leap five with Shift, and they still come round within
the selected hour. Hours move one per press, including with Shift. The clock
keeps its existing order.

**The hours hold at nought rather than coming round.** Nothing is less than no
hours ago, and a column that came round would log yesterday on one press of
the wrong key. Nothing is drawn below it either, so the end of the column
looks like one rather than like a value that failed to load. Even when only
`0h` and `1h` are allowed, `1h` stays above `0h`. Whatever is on the dial comes
across when Tab is pressed, so switching never costs the answer somebody has
already turned: an hour and six minutes ago becomes 12:38 pm and back again.

**A correction opens on the clock instead.** Editing an entry starts from what
is recorded, and a relative dial there would have Enter quietly move the entry
to now. Logging something that just happened opens on how long ago; changing
something already written down opens on the time it says.

**Where a start is already fixed, the dial cannot reach back past it.** A
sleep's end may not land before its beginning, so the hours column stops at
the hour that start falls in and the minutes column is rebuilt to just the
minutes still left in it: bounded to ninety minutes, the dial offers `0h` and
`1h`, and at `1h` the minutes run `00m` to `30m` and no further. Refusing
afterwards would be a worse answer to the same problem, because it makes
somebody undo a turn they were never meant to be able to make.

```
How much?

   1.50
   1.75   ml
   2.00   oz
   2.25
   2.50
  ─────
```

The reasoning is the north star. A parent at 3am has one hand free and is not
going to type `1:44 pm`, and every character typed is a character that can be
typed wrong. Turning a wheel cannot produce a time that does not exist.

- **`←`/`→`, `h`/`l` and `a`/`d` move between columns, and the ends hold.**
  A dial is a row of columns, so `h` and `a` move left within the control. `Esc` is how somebody
  leaves, and the hint line says so.
- **`↑`/`↓`, `k`/`j` and `w`/`s` turn the column under the cursor one notch**, and
  shift leaps. The small step is the plain one, because the small step is the
  correction somebody came to make; the leap is what you reach for when it is
  further off. What a notch and a leap are is the column's own business:
  minutes turn one and leap five, millilitres turn one and leap five, ounces
  turn a quarter and leap a whole one, which is four of them.
- **Every column cycles.** Past 59 comes 00, past 12 comes 1, and the values
  above and below are always drawn, so no column ever looks like it has run
  out.
- **The half of the day is a toggle and never moves on its own.** Turning the
  hour past twelve does not flip it. A dial that quietly changes the morning
  to the afternoon is a dial that logs a feed twelve hours out.
- **A turn is animated.** Five minutes is drawn as five minutes going past, so
  the turn can be seen to travel and in which direction. A key held down cuts
  the animation short rather than queueing behind it: whoever is holding it is
  already ahead of the screen.
- **Two steps of quiet.** The near neighbours are muted and the far ones
  fainter still, so the column recedes like a wheel turning away. The column
  being turned is lit, and underlined as well, because colour is never the
  only carrier.

It is one component, `src/prompt/dial/`. `wheel.rs` is a column, `model.rs` is
a row of them and what keys do, `draw.rs` turns that into lines, and `time.rs`
and `volume.rs` are the two things we currently count. Adding a third is a
list of labels and two numbers, and a fix to the dial is a fix to all of them.

**A volume is one question, not two.** The units were never a separate thing
to decide: "how much" is a single answer with a number and a unit in it, and
asking twice made somebody confirm a choice they had already made. Turning the
units column changes the wording rather than the quantity, so sixty
millilitres becomes two ounces and the amount wheel is rebuilt underneath it,
standing on whichever notch is nearest. `--set units=oz` still names the field
on the command line, where there is no dial to carry it.

**Typing stays on a bare terminal.** There a keyboard is already under both
hands, and `32 min ago` is quicker than any number of key presses: the dial
cannot say that, and relative answers are most of what gets typed. The dial
answers in the same words somebody would have typed, so every parser and every
date rule beneath it is untouched either way.

Three places deliberately do not use it. Setup's day-start and day-end
questions are a short menu of sensible hours, which already needs no typing
and should not offer 3:47am. The one question that offers to keep a start that
was never recorded has no time to begin a dial at, and Enter on a dial cannot
mean "leave it unrecorded". And growth's weights and lengths are typed, since
they come off a scale with a precision a wheel would have to round away.

### 21. An optional field is a field, not a menu

A question with nothing to choose between is shown as the field itself. The
menu this replaced had two rows, `Skip` and `Enter text`, which is a menu with
nothing to choose: at 3am it is one keypress in the way of the answer. Leaving
the field empty is the skip, and the field says so.

### The dashboard drops its Now tab in here

The shell's Now drawer is already on the screen above it, so the dashboard's
own Now tab would be the same facts twice with one of them wasted. Inside the
panel it opens on Sleep, and its tabs are `1 Sleep`, `2 Feeding`, `3 Diapers`,
`4 Log`. Run on its own, `h dash` still has all five.

### There are no exceptions

The dashboard used to be one: a second full-screen program that the shell
stepped aside for. It does not any more. It is drawn in the panel like
everything else, from the reading the shell already has, so `r` refreshes it
along with the widgets and there is no second read.

`h dash` still opens it on its own from the command line. Inside the shell it
is a view rather than a command, so Back closes it and puts the menu back, and
the footer names its keys while it is open.

### Summary keeps its graph inside the flow panel

Visualizations > Summary opens a browsable table through the prompt host. Its
numeric columns have grouped headings; Day stays fixed. Tab/Shift-Tab and
left/right (H/L, A/D, either case) cycle columns and update the separated bar chart
below. The horizontal window moves only when selection crosses an edge. A dotted
line compares the bars with the previous seven complete days' average, excluding
today. The default table and chart include those seven days plus partial today.
Left is movement within this control, so Esc or Q closes it directly.
Up/down (J/K, W/S) scroll the rows and notes, including configured night hours
and the commands that change them. The selected heading has brackets as well as
colour, and the keys remain visible on narrow and short panels. The Now drawer
stays on screen throughout.

### Where the output goes

The shell draws on **stderr**, not stdout, so `h > entries.txt` still fills the
file with what the commands printed when they are run from the command line.
Inside the shell there is no stdout to write to that would not scribble across
the panels, so `render::print` and `render::note` hand their lines to the host
instead. Those two functions are the only places this tool writes output, which
is what makes that one change rather than fifty.

## The widgets

### 11. A widget shows one thing, and the menu remains usable

The Now drawer grows to fit its facts and tables, while keeping at least five
visible menu rows plus the menu's borders whenever the terminal has room.
The menu scrolls as its cursor moves. The drawer is no longer capped at half
the screen: that cap would hide the daily table even when enough room exists
for both the facts and a usable menu. On a terminal too short for both, the
last drawer lines are clipped so navigation remains available. While a command
or dashboard is open, its panel keeps more than half the available height so
input dials retain their selected values and instructions.

### 12. A widget is only as tall as it has something to say

The drawer ends where its facts end and the menu takes the rest. A box two
thirds full of nothing reads as broken rather than as finished. Height is
measured at the same instant as the displayed facts, since elapsed-time labels
can gain wrapped lines as the clock advances.

### 13. Every widget refreshes together, on `r`

One key re-reads everything on the screen, because a screen where two boxes
disagree about what time it is is worse than one that is uniformly a minute
old. A refresh also happens by itself after any command, since a command may
have logged the very thing a widget is showing.

Refreshing never moves the cursor or changes the menu: `r` is not navigation.

### 14. A read never blocks the keyboard

Reads run in the background. A Huckleberry that cannot be reached takes a full
minute to say so, and a menu that accepts no keys for a minute is broken
exactly when the wifi is. The widget says `reading…`, the rows still move, and
the answer arrives when it arrives.

### 15. A failed read keeps the numbers and says so

This is [`dashboards.md`](dashboards.md)'s rule, and it holds here: the numbers
that were there stay there, the failure goes on the footer, and the `as of`
line tells the truth about how old they are.

## Where the Now drawer sits, and why

It is a full-width strip across the **top**, above the Menu view:

- It is where the eye lands first, and the facts are read without looking past
  anything else to find them.
- Full width means the facts keep the shape they have everywhere else in this
  tool, label and value on one line, rather than being folded to fit a column.
- The Menu view is still the main panel and takes everything left over beneath
  it. The drawer is only as tall as it has something to say, and gives way to
  the menu on a short terminal rather than the other way round: the menu is the
  part being operated, and a drawer short of its last line is still readable
  where a menu with no rows is not.

### 16. The Now drawer is `h now`, and nothing else

**The drawer shows exactly what `h now` prints. Same facts, same wording, same
order, no additions and no abbreviations.** It is that command left on the
screen rather than a second screen about the same data.

`render::now::screen` is the one function that decides what `now` says.
`lines` paints its rows for stdout and the drawer draws the same rows as
widgets, so the two cannot drift: there is no second copy of the wording to
forget to update.

**A request to change what the Now drawer shows is a request to change the
output of `h now`, and the other way round.** Change `render::now::screen` and
both follow. If the two ever have to differ, that is a decision for
[`decisions.md`](decisions.md) rather than a line added in one of them.

## Where this is going

1. More widgets, added one at a time and each answering a question somebody
   would otherwise navigate to ask.
2. Panels that are better than a hosted prompt for the flows asked most: a
   diaper is four questions in a row where it could be one screen.
3. Widgets beside the drawer rather than only under it, once there are enough
   of them to need the room.

## One key map, everywhere

**WASD aliases hjkl wherever those letters navigate.** `j`/`s` move down,
`k`/`w` move up, `h`/`a` move left or back, and `l`/`d` move right or open.
Both cases work. Fixed-choice prompts and lists still require Enter to choose;
their right keys do nothing. Lists use PageDown or Ctrl-F to page forward, keeping `d`
consistent with `l`. Search filters and text fields treat letters as text.

Dials use left and right to select a column; shifted up and down leap.
The standalone dashboard uses left and right to switch tabs and up and down
to scroll. Inside the shell, left/back closes the Dashboard view, and Tab,
right or Shift-Tab navigate its tabs. WASD follows these same boundaries.
