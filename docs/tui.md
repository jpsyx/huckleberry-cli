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

### 1. Every direction has an arrow and a letter, and they mean the same thing

| Direction | Arrow | Letter |
| --- | --- | --- |
| down | `↓` | `j` |
| up | `↑` | `k` |
| back | `←` | `h` |
| forward, open | `→` | `l` |

The hand that is free is not always the one near the arrows, so neither set is
the real one. `Enter` and `Space` also open, because a thumb finds the space bar
without looking.

`h` means **back** here. It is the vim direction, and in a tree of menus back
is what left means.

### 2. Nothing leaves the shell but `Ctrl-Q`

The shell is meant to be left running all day, so leaving it should take a
deliberate two-key act and nothing else. **No menu row ends the session**, and
neither does any single key: `q` used to quit and no longer does, because `q`
is one keystroke away from every letter a tired hand presses by reflex, and the
widgets should still be there afterwards.

`Ctrl-C` leaves too, and is the only exception. It is what every hand reaches
for when a full-screen program will not let go, and a program that ignores it
is one somebody has to kill from another terminal.

Back at the top level does nothing at all, for the same reason: `h` and `Esc`
are navigation keys, and a session that ends because a thumb went left one row
too far is a session that has to be reopened in the dark.

Every submenu still ends in a Back row, so walking out of one is a row on the
screen as well as a key. Home ends in nothing, because there is nothing at Home
to walk out to.

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
 Huckleberry · Wren                                                  Home
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
┌ Now ───────────────────────────────────────────────────────────────────┐
│ Wren                                                                   │
│                                                                        │
│ Last fed       36m ago · 73 ml of Formula · 8:03 pm                    │
│ Diaper         1h 31m ago · wet · 7:09 pm                              │
│ Sleep          currently sleeping for 40m                              │
│ Tonight        nothing finished yet                                    │
│ Last 3h        73 ml · 1 feed                                          │
│ Fed today      430 ml · 6 feeds · since 6:00 am                        │
│ Slept today    11h 18m                                                 │
│                                                                        │
│ as of 2m ago                                                           │
└────────────────────────────────────────────────────────────────────────┘
 ↑/↓ j/k move · ← h back · → l open · Enter select · r refresh · ctrl-q quit
```

| Key | Does |
| --- | --- |
| `↓`, `j`, `Ctrl-N` | down one row |
| `↑`, `k`, `Ctrl-P` | up one row |
| `→`, `l`, `Enter`, `Space` | open the row under the cursor |
| `←`, `h`, `Esc`, `Backspace` | back, and nothing at the top level |
| `1`–`9` | put the cursor on that row |
| `g`, `Home` / `G`, `End` | first row, last row |
| `r` | refresh every widget |
| `Ctrl-Q` | leave. The only way out, with `Ctrl-C` as the escape hatch |

A chevron (`›`) marks a row that opens another menu, so nothing is a surprise.
The numbers are right-aligned as a column, so every label starts in the same
place whether its row is 9 or 10.

## How it runs a command, and why that is temporary

Choosing a command **suspends** the full screen: the shell leaves the alternate
screen, the existing handler asks its questions and prints its receipt exactly
as it does from the command line, and the shell comes back afterwards.

That is deliberate scaffolding. It means the shell shipped with every command
already working and every prompt already tested, instead of waiting for a panel
to be written for each one. Panels replace those one at a time, and nothing has
to wait for them.

Two consequences worth knowing:

- Commands print where a person can scroll back to them, and the "Continue"
  pause is what holds a receipt or a failure on screen until it has been read.
- The shell draws on **stderr**, not stdout, so `h > entries.txt` still fills
  the file with what the commands printed. The menu is the conversation; the
  commands are the data.

## The widgets

### 11. A widget shows one thing, and the Menu view stays the main panel

The shell is a dashboard in the Bloomberg sense: small dense boxes, each
answering one question, around the thing you actually operate. That is a reason
to add widgets carefully rather than a licence to fill the screen. A widget
earns its place by answering a question somebody would otherwise navigate to
ask. The Menu view is always the largest panel and always present.

### 12. A widget is only as tall as it has something to say

The drawer ends where its facts end and the menu takes the rest. A box two
thirds full of nothing reads as broken rather than as finished.

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

It is a full-width strip along the **bottom**, under the Menu view:

- It is where a glance goes and comes back from without leaving the row being
  navigated. The menu keeps the cursor; the drawer keeps the facts.
- Full width means the facts keep the shape they have everywhere else in this
  tool, label and value on one line, rather than being folded to fit a column.
- The Menu view is the main panel and takes everything left over. The drawer is
  only as tall as it has something to say, and gives way to the menu on a short
  terminal rather than the other way round.

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

1. More widgets in the sidebar, added one at a time and each answering a
   question somebody would otherwise navigate to ask.
2. Widgets for the commands that are asked most, so the common ones stop
   suspending the screen at all.
3. Search and history in the shell rather than through the suspended listing.

## Known rough edge

Inside a suspended prompt, `J`/`H` move **down** and `K`/`P` move up: `h` means
down there and back here. That mapping predates this screen and is documented in
[`cli.md`](cli.md). Reconciling the two is worth doing before the prompts become
panels, and it is a decision about muscle memory rather than about code.
