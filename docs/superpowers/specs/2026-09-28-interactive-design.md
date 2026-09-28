# One-handed interactive CLI

Status: approved written specification. The conversational design is approved;
this document incorporates Edit above Visualizations and a visible Delete entry.

## Purpose and scope

A parent holding or feeding a baby should be able to reach every existing CLI
capability by running `hb`, moving through numbered lists, and pressing Enter.
Fixed choices must never require typing their names or numbers. Times, notes,
measurements, credentials, new names, search terms, and paths can require text.
The same selection controls apply to prompts reached through explicit commands.

The implementation changes the CLI's entry flow, shared prompts, and list
navigation. It reuses existing command handlers and API operations. It does not
add new tracking capabilities or change stored data semantics. All current flags,
scriptable commands, aliases, and machine output remain available.

## Entry and navigation

With terminal stdin and stderr, a missing subcommand opens the home menu. This
also supports global flags, such as `hb --offline snapshot.json`. Without a usable
interactive terminal, a missing subcommand prints help and exits without waiting.
Explicit `--help` and `--version` retain their normal immediate behavior.

Home menu, in order:

1. Log a diaper
2. Log a feed
3. Log sleep
4. Edit
5. Delete
6. Visualizations
7. View logs
8. Other logging
9. More
10. Exit

Log a diaper is initially highlighted. Submenus offer Back. Completed commands
return to the home menu; leaving a dashboard or list returns to its parent menu.
Static results remain readable until Enter returns to the menu. A failure is
shown with its context and returns control to the menu, without retrying a write.

Every list shows numbers and a visible selection marker, including logs, edit,
delete, foods, and child selection. Moving changes the highlight; only Enter
selects. Navigation stops at the first and last entries rather than wrapping.
Long lists scroll while keeping the selected row visible. Search remains
available where useful, but selecting an existing item never requires search.

| Key | Action |
| --- | --- |
| Down, J/j | Move down |
| Up, K/k | Move up |
| H/h | Move down |
| P/p | Move up |
| Enter | Select highlighted item |
| Esc | Back out of a menu, or cancel the current operation |
| Ctrl-C | Cancel the current operation; exit at the home menu |

H down and P up is the mapping accepted with this specification.
While a text field or search has focus, letters are text. Arrow navigation still
works in searchable lists; Esc first leaves search. Existing paging shortcuts
remain available. On the dashboard, H/P navigate rows when a list has focus;
Left/Right and Tab retain tab navigation so H does not conflict with moving up
or down in a list.

## Defaults, optional values, and editing

Existing configured or remembered defaults are highlighted. A required choice
without an existing default starts on the first choice, except destructive or
conflict choices, which start on a non-mutating Back/Cancel entry. A highlighted
value is not accepted until Enter is pressed.

An optional choice includes Skip. With no existing value, Skip is highlighted.
Skip means omit the field, not an empty string saved as data. Required choices
never offer Skip. Yes/no prompts become lists and preserve their current default;
delete confirmation defaults to No and describes the selected record.

Editing preserves the existing value by default. Where a value can be removed,
Keep current value and Clear value are distinct choices. Missing optional values
start on Skip. Unknown stored enum values can be kept without forcing a known
replacement. These controls preserve exact stored timestamps and numeric values
when a displayed default is accepted.

Optional text fields offer Skip and Enter text; existing text adds Keep and
Clear. Numeric fields retain existing defaults and allow custom input; optional
measurements offer Skip. Time prompts retain Enter for now or the exact existing
time and allow typed clock/relative answers. Ambiguous AM/PM is a list.

Multi-food selection supports choosing another food, removing a selected food,
and Done, using only navigation and Enter. Enter a new food name is available
when the desired food is absent. Free-form solids amounts offer the existing
`some` default and a Custom option, without restricting valid text amounts.

## Command and option coverage

Every leaf command has a Run action and an Options action. Run is highlighted
and uses the existing defaults and missing-value prompts. Options allows less
common adjustments without adding those questions to every ordinary recording.
Returning from Options retains the pending choices until Run or Back. Required
activity questions keep time first. Pre-supplied options are not asked again.

The table is the coverage contract for canonical commands. Aliases select the
same route; they do not need duplicate menu entries.

| Route | Commands | Reachable options or questions |
| --- | --- | --- |
| Log a diaper | `diaper` | at, mode, pee, poo, color, consistency, rash, notes |
| Log a feed / Bottle | `feed bottle` | at, amount, type, units, notes |
| Log a feed / Solids | `feed solids` | at, repeated food choices, amount, reaction, notes |
| Log a feed / Nursing | `feed nursing start`, `pause`, `resume`, `switch`, `stop`, `cancel`, `status` | start/at as applicable; side for start and resume |
| Log sleep | `sleep start`, `manual`, `pause`, `resume`, `stop` (`end`), `cancel`, `status` | start, end, at as applicable; manual overlap resolution |
| Edit | `edit` | entry picker or explicit id, field edits equivalent to repeated set, list-only mode, days, limit |
| Delete | `delete` | entry picker or explicit id, tracker, list-only mode, days, limit, confirmation choice corresponding to yes |
| Visualizations / Current status | `now` | readable output or JSON |
| Visualizations / Dashboard | `dash` | days, refresh |
| Visualizations / Trends | `trends` | every metric, days |
| Visualizations / Summary | `summary` | days, readable output or JSON |
| Visualizations / Sleep stripes | `stripes` | days |
| View logs | `log` | kind or all, search, days, limit |
| Other logging / Potty | `potty` | at, mode, how, color, consistency, notes |
| Other logging / Growth | `growth` | at, weight, height, head, units |
| More / Foods | `foods list`, `foods add` | all/custom/curated, search, archived; new name |
| More / Children | `child list`, `child use`, `child show` | choose child by label; explicit id alternative |
| More / Account | `auth login`, `auth status`, `auth logout` | email, masked password, timezone |
| More / Settings | `config show`, `config set`, `config path` | every key and value, with fixed choices where applicable |
| More / Export | `export` | days, output file or stdout |
| More / Session options | global flags | verbose, config path, child override, offline snapshot or live data |
| More / Help and version | help, version, `info` | command-specific help and build information |

Config keys are child, timezone, days, units, measurements, refresh, and verbose.
Fixed-value options come from the existing enums or settings definitions, rather
than a partial hand-maintained subset. Tracker selection offers the known
supported collections and custom input where the existing command accepts it.
Lists display child names and entry descriptions; IDs remain available as
secondary detail or explicit input, rather than being required knowledge.

Edit field menus expose every currently supported field for the selected kind;
repeated changes are applied together through existing edit validation. This
includes time-only edits for kinds without detail forms and live sleep start
corrections. Delete retains its existing ability to access tracker-specific rows.

Persistent settings are changed only through Settings or the existing remember
choices. Session options affect subsequent operations in the current interactive
session. Switching configuration, child, credentials, or offline source refreshes
the context before the next operation so stale settings cannot target the wrong
child. Offline mode retains the existing read-only restrictions and offers a
route to switch back to live data.

## Architecture and implementation boundaries

Use a small `interactive` module for menu structure, option collection, and the
session loop. Its pure menu model maps selections to existing typed commands and
session options. Shared command dispatch accepts an already resolved command and
context, so explicit invocations and menus execute the same handlers without
shelling out or duplicating API calls.

Use a shared prompt selection component with pure key interpretation, cursor
state, default resolution, and rendering. Integrate it into `prompt::ask`,
`ask_optional`, and `confirm` for finite choices. Split prompt responsibilities
into files as needed; text/time/password input and choice selection retain clear
boundaries. Cancellation is a typed outcome distinguishable from a failed
operation; never match error strings to decide whether to leave a menu.

Extend the existing listing controls and numbering to use the same navigation
contract. Reuse crossterm, ratatui, and semantic theme roles already installed.
No new dependency is required. Menu and prompt output uses stderr. Explicit JSON,
exports, receipts, and other command data retain their stdout formats.

Raw mode, cursor visibility, and any alternate screen are restored on selection,
cancellation, I/O failure, or transition to another terminal view. Only one view
owns terminal input at a time. Menus release terminal control before invoking
existing dashboards, listings, or text prompts. Narrow terminals and NO_COLOR
retain usable numbering, wrapping, and a non-color selection marker.

Cancellation before submission does not save the pending activity or edit.
Previously completed commands remain saved. An interrupted or failed network
write is not automatically replayed; preserve the handler's error and let the
parent inspect current state before deciding what to do next.

## Alternatives considered

| Approach | Consequence |
| --- | --- |
| Shared choice controls plus a typed menu tree (selected) | Consistent direct-command prompts and complete interactive access while reusing handlers |
| Only a root launcher | Leaves typed selections and inaccessible options inside commands |
| Separate full-screen application for every form | Duplicates command behavior and increases synchronization and terminal-state complexity |

## Acceptance and verification

- Bare interactive `hb` opens the ordered home menu, with Edit before
  Visualizations and Delete reachable directly from home.
- Every command and user-facing argument in clap's command tree has a route,
  field control, or equivalent interaction listed above. A coverage test compares
  actual clap metadata with the menu's routes and argument bindings; missing new
  commands or options fail it. Help/version and aliases have explicit treatment.
- Tests construct commands from representative menu paths, including repeated
  food/edit values, conflicting food filters, global overrides, JSON, exports,
  and tracker-specific deletion. These exercise real routing and validation.
- Pure tests cover every key mapping, uppercase letters, list boundaries,
  scrolling, selected defaults, Skip/Keep/Clear, and cancellation.
- Direct-command tests cover diaper, bottle, nursing side, sleep overlap, child,
  settings, confirmations, and edit choices using the shared selection controls.
- Deterministic terminal tests or local PTY smoke checks cover arrows and Enter,
  letter navigation, text transitions, Back, terminal restoration, and return to
  menus after a command. Use offline/synthetic fixtures, not real account writes.
- Nonterminal tests confirm no blocking prompts, existing flags and aliases,
  and clean stdout for JSON and other machine output.
- Update docs/cli.md, docs/architecture.md, docs/dashboards.md, and
  docs/rules/cli-ux.md with the implemented behavior. Update the README's bare
  invocation guidance if it describes the old help-only behavior.
- Before completion run cargo fmt --check, cargo clippy --all-targets -- -D
  warnings, and cargo test for the full workspace, then review the diff.

## Delivery state

The earlier sleep changes are already merged into main as 8adc94b. This feature
is isolated on feat/interactive. This document describes proposed behavior, not
features already implemented. After specification approval, write the detailed
implementation plan and obtain its review before implementing with red/green
regression tests. Merge or publication of this new feature is a separate action.
