# Nomenclature

The words this project uses for its own parts, so a request, a commit message
and a module name all mean the same thing by them.

**Add to this file whenever a new part gets a name.** A part with two names
grows two implementations.

## The screens

| Term | Means |
| --- | --- |
| **the shell** | The full-screen app `h` opens with no command, and keeps open. `src/tui/`. Not the dashboard. |
| **the dashboard** | Five tabs over the same data. `src/dashboard/`. It is both a program of its own (`h dash`) and a view inside the shell; say which when it could be either. |
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
| **the flow panel** | The same area while a command is running: its questions, its output and its failures. `src/tui/draw/flow.rs`. |
| **a job** | One command running inside the shell, and everything it has said so far. `src/tui/job.rs`. |
| **the host** | Whatever is drawing a prompt and feeding it keys. The shell installs itself as one while a job runs; with none installed, prompts own the terminal. `src/prompt/host.rs`. |
| **a frame** (of a prompt) | One pass of a hosted loop: the lines to draw, and the key that answers them. Not to be confused with a ratatui frame. |
| **the Now widget**, **the View Latest widget**, **the Now drawer** | The same thing, interchangeably: the full-width strip along the bottom that permanently shows what `h now` prints. `src/tui/draw/now.rs`. It shows exactly what the command prints and nothing else, so changing one means changing both; see [`tui.md`](tui.md). |
| **the drawer** | Where a widget sits: a full-width strip across the top of the shell, above the main panel. |
| **the Dashboard view** | `h dash` drawn in the shell's panel, from the reading the shell already has. Back closes it. It is a view rather than a job: nothing about it runs as a command. |
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
| **day start** | The hour a day begins at. Anything earlier belongs to the day before. Always configured. |
| **day end** | The hour a day ends at, which is where night begins. Always configured. Was called **night start**, which is the same hour under a name that made it sound like a third setting. |
| **the night** | The stretch from `day_end` to the next `day_start`. Not configured separately: it is what is left over. |
| **a day row** | One row of `summary` or `trends`: `day_start` to `day_start`. |
| **a calendar day** | Midnight to midnight, in the family's timezone. What `log`, `edit` and `delete` group under, and what a timestamp is always reported against. Never used for arithmetic. |
| **a stripe row** | One row of `stripes`: the previous `day_end` to this one, so a night lands whole on it. |
| **the window** (of a screen) | The stretch `today` resolved to on this screen, as of now. `Window` in `src/domain/today.rs`. Not to be confused with **the read window**, which is how many days back a read covers. |
| **totals** | What a window adds up to: millilitres, nursing seconds, milk feeds, meals, sleep. |
| **the last 4h** | The fixed recent window on the `now` screen, which is a feeding interval rather than a day. |
| **setup** | The first-use gate: an account, then the settings with no default. `src/setup/`. See [`setup.md`](setup.md). |

## The rules these words serve

- [`tui.md`](tui.md) is the design brief for the shell: one hand, in the dark,
  holding a baby.
- [`dashboards.md`](dashboards.md) holds the rules about how a number about a
  baby may be shown. They bind the shell too.
- [`rules/cli-ux.md`](rules/cli-ux.md) holds the rules about how the tool talks.
