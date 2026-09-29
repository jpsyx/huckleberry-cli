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

### 2. Getting out is never an accident, and never a puzzle

`q`, `Ctrl-C`, and the Exit row all end the session. Back at the top level does
nothing at all: `h` and `Esc` are navigation keys somebody presses by reflex,
and a session that ends because a thumb went left one row too far is a session
that has to be reopened in the dark.

Every submenu ends in a Back row, so the way out is a row on the screen as well
as a key. Nothing in this app should require remembering anything.

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

```
 Huckleberry                                            Home › Log a feed
┌ Log a feed ────────────────────────────────────────────────────────────┐
│▌ 1. Bottle                                                             │
│  2. Nursing                                                          › │
│  3. Solids                                                             │
│  4. Back                                                               │
└────────────────────────────────────────────────────────────────────────┘
 ↑/↓ j/k move · ← h back · → l open · Enter select · q quit
```

| Key | Does |
| --- | --- |
| `↓`, `j`, `Ctrl-N` | down one row |
| `↑`, `k`, `Ctrl-P` | up one row |
| `→`, `l`, `Enter`, `Space` | open the row under the cursor |
| `←`, `h`, `Esc`, `Backspace` | back, and nothing at the top level |
| `1`–`9` | put the cursor on that row |
| `g`, `Home` / `G`, `End` | first row, last row |
| `q`, `Ctrl-C` | leave |

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

## Where this is going

The shell is the always-on app, so the panel above the menu is where the facts
belong: how long since the last feed, whether anybody is asleep right now, what
the night looks like so far. The seam is already there: `draw.rs` lays out a
header, a body and a footer, and the body is a menu today because that is all
there is to put in it.

The order things are likely to arrive in:

1. Facts on the same screen as the menu, refreshed on a timer.
2. Panels for the commands that are asked most, so the common ones stop
   suspending the screen at all.
3. Search and history in the shell rather than through the suspended listing.

## Known rough edge

Inside a suspended prompt, `J`/`H` move **down** and `K`/`P` move up: `h` means
down there and back here. That mapping predates this screen and is documented in
[`cli.md`](cli.md). Reconciling the two is worth doing before the prompts become
panels, and it is a decision about muscle memory rather than about code.
