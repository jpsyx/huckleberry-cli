# Retained form navigation in the shell

Status: approved on 2026-10-11. Implemented on `refactor/menu-history`; final review findings fixed and verification complete.

## Purpose and success criteria

Bare `h` presents questions one view at a time. A parent must be able to go
back to correct an earlier answer and then move forward through the answers
already entered. Going back must not cancel the command or lose later values.
Escape is the deliberate way to abandon the draft and return to Home.

The motivating journey is a diaper entry: answer the time, contents and
details, go back several views, change the time, then advance. Each subsequent
view opens with its previous selection or text, and Enter keeps that answer.
Nothing is recorded until the applicable questions have been completed.

This applies throughout the shell's existing command flows, not just diapers.
The terminal remains the interface, and the existing prompt implementations
continue to serve both the shell and explicit CLI commands. Flags, API schemas,
the Now drawer and family-day arithmetic retain their existing semantics.

## Original behavior and cause

- `prompt/select/model.rs` gives Left, `h`, `a` and Escape the same Cancel
  result. `prompt/select/terminal.rs` turns it into `prompt::Cancelled`.
- Command handlers collect answers in ordinary local variables. That error
  unwinds the handler, losing the preceding answers. For example,
  `commands/diaper.rs` asks for time and contents before iterating the details.
- `tui/job.rs` maps cancellation to one landing result. `tui/mod.rs` returns
  to the launching menu, which is Home for Log a diaper. There is no distinct
  result for returning one question versus returning to Home.
- `tui/state.rs` already has a menu stack, but it contains menu levels, not
  command questions. Extending that stack alone cannot retain form answers.
- Dials currently use horizontal movement for columns. Hosted text fields
  accept letters and do not use Left to leave the field.

The host's frame exchange is rendering and input transport. Its frames can be
individual keystrokes or animation pictures, so they are not suitable units of
form history.

## Interaction contract

| Context | Back: Left / `h` / `a` | Escape | Forward |
| --- | --- | --- | --- |
| Home | No action | No action | Open selected row |
| Submenu | Return one menu level | Return to Home | Open selected row |
| Choice question | Return one view, retaining selection | Discard draft and return to Home | Accept selected answer and open the next applicable view |
| Dial | Move one column left; from the first column, return one view | Discard draft and return to Home | Enter accepts the value; Right / `l` / `d` still moves columns |
| Text or password field | Left returns one view; `h` and `a` remain text | Discard draft and return to Home | Enter accepts the field |
| Entry picker or read-only view | Return to the containing view | Return to Home | Preserve the view's existing selection or detail action |

Uppercase directional aliases retain the same meanings. Existing choice-menu
forward aliases remain supported, and digits only highlight a row. Dials keep
their current default column, including minutes for relative time; therefore
Back from that default column takes two leftward movements to leave the dial.
Tab continues to switch the time dial between relative and clock modes.
Backspace edits text as before; its existing menu behavior remains Back.
Search filters retain letters as text and their existing cursor controls;
Back navigation applies once the filter editor has been left.

Back never validates a field or forces an answer. The current buffer, selection
or dial position is retained even if it has not been submitted or is invalid.
Forward validates before advancing. On revisiting an answered field, its saved
value replaces the original default. An explicit Skip or cleared value is an
answer and must not revert to the initial default.

Nested questions are views too. Back from a custom value returns to its preset
menu; Back from an edit field returns to Fields to change with all other edits
retained. The existing Edit menu remains an editor with Done, rather than
becoming a sequential logging form.

Back from the first command question returns to its launching menu, retaining
one suspended draft. Reopening the same command restores its first question.
Walking through menus retains that draft; choosing another command, pressing
Escape, completing the command or leaving the shell discards it. There is no
collection of multiple suspended command drafts or persistence across sessions.
Menu selections are remembered by menu identity when a submenu is reopened.

Ctrl-C retains its cancellation role: it abandons the current command to the
launching menu, or backs out of a submenu, and exits at Home. Ctrl-Q exits from
anywhere; ordinary `q` remains text while a question owns the keyboard. Footer
hints distinguish Back from Escape/Home on every affected hosted view.

## Chosen state model

Use a form draft, stable view identities and a current-view cursor. The draft
outlives individual prompt invocations. Moving the cursor never pops answers.

| Part | Responsibility |
| --- | --- |
| Form draft | Retain typed accepted answers and the editable state of each visited view, including unfinished values |
| View identity | Identify a question by its purpose and owner, independent of its label or position |
| Active route | Identify the applicable views and their order under the current answers |
| View cursor | Identify the view currently being edited and its containing view |
| Form controller | Apply Back/Forward, validate answers, and return a completed typed input to the command |
| Shell navigation | Retain the launching menu and distinguish return to parent, return to Home, and quit |

The reusable navigation machinery belongs in `prompt/flow/`. Command-owned
collectors define their typed values, conditions and validation. The shell owns
the active or suspended command form through the job lifecycle. Presentation
state is held in the form and passed to the existing selection, text and dial
controls; those controls report acceptance, Back or Home distinctly.

Answers and presentation state have separate roles. A partly typed value can
be retained for editing without becoming a valid submission value. Stable
identities include the command/form scope and field, with entry or item identity
where a form repeats questions. Repeated labels such as "How much?" cannot
share state accidentally. Reopening an editor for another history entry cannot
reuse the previous entry's draft.

Navigation and route decisions are pure. Filesystem reads, account reads,
terminal input and writes remain in the surrounding command or host. Collection
must be resumable at view boundaries instead of re-entering an entire command.
The state model stores values and identities, never a recording of keystrokes.

## Conditional views and dependent values

Accepted changes recompute the active route. Retained state is keyed by view
identity, so removing a view from that route does not erase its answer or move
another answer into its slot. Forward always displays the next applicable
view, including one that has already been answered; it does not silently skip
through remembered answers.

For example, changing a diaper from dirty to wet hides dirty amount, colour
and consistency. Those answers remain in the draft but do not enter the wet
submission. Returning to dirty restores them. New views open with their normal
defaults. Draft construction uses the current applicable fields and supplied
flags, rather than serializing every value the draft happens to remember.

Dependencies are validated against the current draft. Moving a sleep start
past a remembered end retains that end for correction and prevents saving it.
Changing a unit preserves the physical quantity using the existing conversions;
it never reinterprets the old number in new units. Pump total and separate-side
answers have distinct identities, and only the selected mode is submitted.
Confirmations tied to an entry or timer must be checked against that target.

Time answers retain their resolved instants once accepted. Returning later and
pressing Enter must not reparse "20 min ago" against a later clock or turn an
accepted "now" into a new timestamp. The dial derives its display from that
instant and the current supplied clock. An explicit edit or `n` reset produces
a new answer. Existing timestamp precision and calendar-day rules remain intact.

## Collection and save boundaries

Each command separates preparation, form collection and execution. Preparation
loads records and suggested defaults. Collection produces typed command inputs;
the existing command operation performs its mutations only after collection
completes. Returning to earlier views does not rerun authentication, setup,
timer changes, saves, exports or command dispatch.

Some existing flows have prerequisite operations. Authentication completes
before setup settings or the requested command can run, and remains a separate
completed operation. The setup settings questions collect one draft before
saving those settings, so Back among those questions does not need to undo
incremental writes. Setup's command continues only after setup succeeds.
Account/session maintenance remains outside form navigation.

Sleep overlap choices must finish collection before cancelling/completing the
active sleep or writing the manual sleep. Existing non-atomic API behavior is
unchanged: if an execution phase partly succeeds, report its actual failure
and never restart that phase through navigation.

On final acceptance the form enters execution and its editable history closes.
Keys arriving during execution cannot trigger another submission. Escape can
request a Home landing without implying that a dispatched write was undone;
the operation's result is still collected and reported. Completed receipts and
failure screens have navigation but cannot reopen a submitted draft. Existing
explicit retry behavior for read-only commands remains separate from forms.

The host keeps queuing input within a live flow and drawing its last frame as
it does today. When a flow ends, its pending input and replies belong to that
flow and cannot leak into Home or the next command. A paused form retains data,
not a blocked prompt thread or a live reply channel.

## Alternatives considered

| Approach | Trade-off |
| --- | --- |
| Retained draft with stable view identities and cursor (chosen) | Supports conditional paths and explicit save boundaries; requires separating collection in existing handlers |
| Stack of rendered views or snapshots | Simple for menus, but popping loses forward state and snapshots cannot reliably identify conditional fields |
| Restart commands and replay previous input | Requires fewer local changes, but can repeat writes, change relative timestamps and route old input to the wrong question |

## Implementation scope

The implementation includes shared prompt controls and the shell navigation/job
boundary, then every existing question-driven command flow reachable from the
shell: diaper/potty, bottle/nursing/solids, sleep, pumping, growth, edit/delete,
configuration, child selection and sign-in/setup. Single-question commands use
the same Back/Home contract. Existing interactive pickers and helper questions
participate at their containing view, including food selection, custom values,
confirmation and edit-field menus.

Expected source areas are `src/prompt/`, `src/tui/`, `src/interactive/`, the
affected collectors under `src/commands/`, and the existing prompt owners in
`src/session.rs`, `src/setup/` and `src/commands/edit/pick.rs`. Listing, summary
and dashboard integration changes are limited to routing Escape/Home and
returning to their containing view. No second set of prompt renderers or
API-crate changes is needed. No new dependency is planned.

Document the final behavior in `docs/tui.md`, `docs/architecture.md`,
`docs/nomenclature.md`, `docs/rules/cli-ux.md` and the relevant command/setup
documentation. Distinguish a form view from a prompt animation step, and a form
draft from `interactive::CommandDraft`, which represents CLI arguments.

## Verification and acceptance

Write failing behavior tests before implementation. Use pure navigation tests,
the hosted prompt driver and local request stubs; no real account writes are
needed.

1. A diaper journey goes back twice, changes the time, then revisits each
   following view with its prior answer. The final request has the edited time
   and exactly the retained applicable details, and is submitted once.
2. Switching dirty to wet omits dirty fields; switching back restores them.
   Repeated labels and conditional insertion/removal cannot exchange values.
3. Back preserves unfinished text, Skip, cleared values, selected choices and
   dial state. Nested custom input returns to its actual containing view.
4. Left, `h`, `a` and their uppercase aliases agree where they navigate. Text
   accepts letters; dials leave only from the first column. Hints match the
   behavior. Escape returns Home from every menu depth and hosted prompt kind.
5. Back at a command's first view returns to its launching menu. Reopening
   restores the draft; selecting a different command, Escape or completion
   clears it. Home Back/Escape do not quit.
6. Advancing the clock while revisiting an accepted relative time preserves the
   accepted instant. Explicit changes and reset-to-now produce new instants.
7. Invalid dependent values remain editable and cannot be submitted. Pump unit
   and mode changes preserve the appropriate values. Edit retains explicit
   field changes and the existing unknown-wire-value preservation guarantees.
8. Delete/edit return to their picker without losing its selection. Choosing a
   different entry isolates its draft. Setup settings do not write on Back.
9. Escaping before final submission produces no entry/configuration mutation.
   Back/Enter during or after execution cannot duplicate a write. Queued keys
   cannot act on Home after the flow exits, and failed writes never replay.
10. Existing CLI flags, noninteractive errors, single prompt implementations,
    menu coverage, input queuing and nonblank drawing continue to pass their
    regression suites. Include hosted journeys for each migrated command family
    so completion cannot be claimed from diaper coverage alone.

After focused red/green cycles, run the repository's full `just check`
(formatting, clippy, Rust tests and release-script tests). Review the final
command inventory against the shell catalog and verify representative flows
through the terminal harness.

## Related documentation

- [Shell design requirements](../../tui.md)
- [Architecture and command boundaries](../../architecture.md)
- [Names of shell parts](../../nomenclature.md)
- [Setup and date rules](../../setup.md)
- [CLI interaction rules](../../rules/cli-ux.md)
