# Nomenclature

The words this project uses for its own parts, so a request, a commit message
and a module name all mean the same thing by them.

**Add to this file whenever a new part gets a name.** A part with two names
grows two implementations.

## The screens

| Term | Means |
| --- | --- |
| **the shell** | The full-screen app `h` opens with no command, and keeps open. `src/tui/`. Not the dashboard. |
| **the dashboard** | The separate full-screen program `h dash` opens: five tabs over the same data, and a program you run rather than one you leave running. `src/dashboard/`. |
| **a one-shot command** | Anything that prints and exits: `h now`, `h summary`, `h log`. |
| **a listing** | The searchable, scrollable list behind `h log`, `h edit` and `h delete`. `src/listing/`. |

The shell and the dashboard are different programs. When a sentence could mean
either, say which.

## Inside the shell

| Term | Means |
| --- | --- |
| **widget**, **panel** | The same thing, interchangeably: one bordered box showing one thing. Prefer **widget** in new prose; **panel** appears in older text and in the dashboard's code. |
| **view** | A named arrangement that fills the shell's main area. The Menu view is the only one so far. |
| **the Menu view** | The main panel: the `What would you like to do?` rows. `src/tui/draw/menu.rs`. |
| **the Now widget**, **the View Latest widget** | The same widget, interchangeably: the one that permanently shows what `h now` prints. `src/tui/draw/now.rs`. |
| **the sidebar** | The column beside the Menu view where widgets stack. Holds the Now widget today. |
| **the header** | The top line: the tool, the child, and the breadcrumb. |
| **the footer** | The bottom line: what is happening, what is wrong, and the keys. |
| **the breadcrumb** | `Home › Log a feed`, on the right of the header. |
| **a row** | One line of the Menu view. |
| **the cursor** | The bar (`▌`) down the left of the highlighted row. |
| **a motion** | What one keystroke asks for, before anything knows what is on screen. `src/tui/keys.rs`. |
| **an intent** | What the shell must do that the state cannot do for itself: run a command, refresh, quit. `src/tui/state.rs`. |
| **a refresh** | Re-reading everything every widget shows. `r`, and after any command. |
| **a reading** | The result of one refresh: a dataset, the calendar its days are counted in, and the units it is shown in. |
| **suspend** / **resume** | Stepping out of the full screen so a command can ask its questions and print its receipt, and stepping back in afterwards. |
| **stacked** / **side by side** | The two layouts. Side by side when the terminal is wide enough for the sidebar; stacked, facts on top, when it is not. |

## The data

| Term | Means |
| --- | --- |
| **a dataset** | Everything read about one child over one window. `src/domain/types.rs`. |
| **a snapshot** | A dataset written to a file by `h export` and read back with `--offline`. |
| **the window** | How many days back a read covers. |
| **live state** | What the trackers say is happening right now: a running sleep, a running nursing session. It is not history, and history cannot know about it. |
| **an entry**, **a row of history** | One recorded thing: a feed, a sleep, a diaper. |
| **a tracker** | One kind of thing Huckleberry records. Sleep, feeding, diapers, pumping, milestones. |
| **as of** | How stale what is on screen is. Every screen carries one. |

## Days, nights and windows

| Term | Means |
| --- | --- |
| **today** | Whatever this family counts as one, which is a setting and not a calendar day. Never assume midnight. |
| **the day rule** | How a family counts a day: the mode, and the hours it turns on. `DayRule` in `src/domain/today.rs`. |
| **continuous** | A day mode: a rolling twenty-four hours ending now. Screens say `in 24h`, never `today`. |
| **discrete** | A day mode: from `day_start` each morning to now. |
| **day start** | The hour a discrete day begins at. Anything earlier belongs to the day before. |
| **night start** | The hour night begins at. The night ends where the day begins. |
| **the window** (of a screen) | The stretch `today` resolved to on this screen, as of now. `Window` in `src/domain/today.rs`. Not to be confused with **the read window**, which is how many days back a read covers. |
| **totals** | What a window adds up to: millilitres, nursing seconds, milk feeds, meals, sleep. |
| **the last 3h** | The fixed recent window on the `now` screen, which is a feeding interval rather than a day. |
| **setup** | The first-use gate: an account, then the settings with no default. `src/setup/`. See [`setup.md`](setup.md). |

## The rules these words serve

- [`tui.md`](tui.md) is the design brief for the shell: one hand, in the dark,
  holding a baby.
- [`dashboards.md`](dashboards.md) holds the rules about how a number about a
  baby may be shown. They bind the shell too.
- [`rules/cli-ux.md`](rules/cli-ux.md) holds the rules about how the tool talks.
