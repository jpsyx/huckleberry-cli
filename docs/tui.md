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

Back at the top level does nothing at all: `h` and `Esc` are navigation keys,
and a session that ends because a thumb went left one row too far is a session
that has to be reopened in the dark.

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
┌ Now ───────────────────────────────────────────────────────────────────┐
│ Wren                                                                   │
│                                                                        │
│ Last fed           36m ago · 73 ml of Formula · 8:03 pm                │
│ Diaper             1h 31m ago · wet · 7:09 pm                          │
│ Sleep              currently sleeping for 40m                          │
│                    (previous sleep finished 2h ago · slept for 1h 20m) │
│ Tonight            nothing finished yet                                │
│ Fed in last 4h     73 ml · 1 feed                                      │
│ Total fed today    430 ml · 6 feeds · since 6:00 am                    │
│ Total slept today  11h 18m                                             │
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
 ↑/↓ j/k move · ← h back · → l open · Enter select · r refresh · q quit
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

### Where the output goes

The shell draws on **stderr**, not stdout, so `h > entries.txt` still fills the
file with what the commands printed when they are run from the command line.
Inside the shell there is no stdout to write to that would not scribble across
the panels, so `render::print` and `render::note` hand their lines to the host
instead. Those two functions are the only places this tool writes output, which
is what makes that one change rather than fifty.

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

**`j` and `k` are the only letters that move.** In the shell, in every prompt
it opens, and in every list. `h` and the left arrow mean back: out of a
submenu, out of a list, and out of a question without answering it, which is
what Escape does.

That was not always true. The prompts predate the shell and read `h` as down
and `p` as up, so the same letter moved the cursor in a question and walked out
of the menu behind it. A letter that means two things is a letter nobody can
press without looking, which is the one thing this screen is built not to
require.

The dashboard is the only screen here with a left and a right, and there `h`
and `l` are them, on every tab. Its Log tab used to read `h` as down so a long
list could be scrolled with it; `j` and `k` do that now, as everywhere else.
