# Retained Form Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Back return one view while retaining all form answers, and make
Escape abandon the draft and return to Home throughout bare `h`.

**Architecture:** A pure form controller retains typed answers and editable
controls by stable view identity. The shell owns an active or suspended prepared
command, while collection and execution are separate phases. Existing prompt
models and renderers serve both hosted forms and standalone prompt loops.

**Tech Stack:** Rust edition 2024, clap, ratatui, crossterm, Tokio, jiff and the
existing `huckleberry-api` client; no new dependencies.

**Spec:** [Approved design](../specs/2026-10-11-menu-history-design.md).

## Global Constraints

- "Moving the cursor never pops answers."
- "Forward always displays the next applicable view, including one that has already been answered; it does not silently skip through remembered answers."
- "Back never validates a field or forces an answer."
- "Time answers retain their resolved instants once accepted."
- "A paused form retains data, not a blocked prompt thread or a live reply channel."
- "No second set of prompt renderers or API-crate changes is needed. No new dependency is planned."
- "Existing non-atomic API behavior is unchanged: if an execution phase partly succeeds, report its actual failure and never restart that phase through navigation."
- Preserve flag behavior, unknown stored values, Now wording and calendar rules.
- Keep the API crate independent; all changes are in the CLI and its docs/tests.
- Work in `refactor/menu-history`; do not push, merge, publish or create a PR.
- Use red/green TDD, documented public interfaces, short functions and the
  existing semantic colours. Do not add unsafe code or em dashes.

## Review Focus

1. A pasted note followed by rapid Back/Enter must retain the complete buffer
   and never turn a pasted newline into submission (Tasks 2 and 13).
2. Reopening a picker after its row disappeared must preserve selection by
   identity where possible and never silently target a different entry (Task 10).
3. A retained time crossing midnight or a DST boundary must keep its exact
   instant, even when the visible minute no longer uniquely identifies it (Task 3).
4. Escape during a slow or partly successful write must not detach its result,
   retry it, or deliver queued keys into a new command (Tasks 4 and 8).
5. Missing settings or expired credentials discovered during preparation must
   retain the requested command across setup without replaying login or writes
   when navigating within the next form (Task 11).

## File and interface map

| Files | Responsibility |
| --- | --- |
| `src/prompt/flow/{mod,model,view,runner}.rs` | Retained form state, control ownership, pure navigation and shared input driver |
| `src/prompt/{text,text_model}.rs`, `src/prompt/select/`, `src/prompt/dial/` | Reusable editable controls, acceptance and navigation outcomes |
| `src/commands/prepared/{mod,forms,execute}.rs` | Prepare command inputs, own the typed form variants, execute a completed input once |
| `src/tui/{job,state,keys,mod}.rs` and new `src/tui/forms.rs` | Job lifecycle, one suspended form, Home versus Back and menu selection memory |
| New form files beside each affected command | Typed answers, applicable routes, defaults and validation for that command family |
| `src/listing/`, `src/commands/edit/pick.rs` | Reusable listing state and entry selection inside forms |
| `src/setup/run.rs`, new `src/setup/form.rs`, `src/session.rs` | Prerequisite forms and their explicit completion boundaries |
| `src/commands/prepared/tests/`, `tests/form_*.rs`, existing regression suites | Pure journeys, hosted transport tests and local request assertions |

Keep existing public command entry-point signatures. Each migrated handler uses
the same family form and completed-input executor as the shell; only the input
driver differs. Do not maintain a second imperative list of questions for CLI
and shell, and do not serialize completed forms back into clap arguments.

### Shared contracts established by Tasks 1 through 4

Names below are the interfaces between tasks. Supporting private helpers belong
beside their owner. `Result` is `anyhow::Result`; `Input` is `prompt::host::Input`.

```rust
// prompt/flow/model.rs
pub struct ViewId { pub field: &'static str, pub owner: Option<String> }
pub enum FormValue {
    Choice(String), Text(String), Skipped, Instant(f64),
    Volume { units: crate::cli::Units, amount: f64 }, Picked(String),
}
pub enum FormEvent { Stay, Suspend, Home, Cancel, Ready }
pub trait FormModel {
    type Output;
    fn route(&self) -> Vec<ViewId>;
    fn create_view(&self, id: &ViewId, now: f64) -> Result<View>;
    fn accept(&mut self, id: &ViewId, value: FormValue, now: f64) -> Result<()>;
    fn forward(&self, id: &ViewId) -> Option<ViewId>;
    fn complete(&self) -> Result<Self::Output>;
}
pub struct FormState<Form: FormModel> { /* model, views, current view, errors */ }
impl<Form: FormModel> FormState<Form> {
    pub fn create_form(model: Form, now: f64) -> Result<Self>;
    pub fn apply(&mut self, input: Input, now: f64, size: (u16, u16)) -> FormEvent;
    pub fn render(&self, now: f64, size: (u16, u16), theme: Theme) -> Vec<String>;
    pub fn complete(&self) -> Result<Form::Output>;
}
// commands/prepared/mod.rs; structs/enums own their data and are Send.
pub async fn prepare(context: Context, command: Command, now: f64) -> Result<PreparedCommand>;
impl PreparedCommand {
    pub fn apply(&mut self, input: Input, now: f64, size: (u16, u16)) -> FormEvent;
    pub fn render(&self, now: f64, size: (u16, u16), theme: Theme) -> Vec<String>;
    pub async fn execute(self) -> Result<CommandProgress>;
}
pub enum CommandProgress {
    Finished { home: bool },
    Continue(Box<PreparedCommand>), // completed prerequisite, next form
}
```

`View` in `flow/view.rs` owns label/help and one `Control`: selection, text,
time, volume or listing. It provides `apply(&mut self, input: Input, now: f64,
size: (u16, u16)) -> ControlEvent`, `value(&self, now: f64) -> Result<FormValue>`
and `render(&self, now: f64, size: (u16, u16), theme: Theme) -> Vec<String>`.
`ControlEvent` is `Stay`, `Accept`, `Back`, `Home` or `Cancel`. `Control` is an
enum of retained states, not callback closures borrowing command locals.

`route` is the current ordered route including containing views. Back selects
its predecessor without deleting state; Back at its first view emits Suspend.
`forward` normally selects the following route element; editors override it to
return from a field to their field menu. `None` requests completion. Rebuild
labels and available choices after acceptance while preserving compatible
control state by identity. Invalid acceptance keeps the view and its raw buffer.

`PreparedCommand` owns Context, resolved client/child when needed, a typed form
variant and any pending prerequisite continuation. It is never a stored future
waiting for prompt input. Its execution consumes the prepared operation, calls
the existing client methods and receipts, and cannot restart collection.

## Implementation tasks

Test commands must execute the named tests, not merely exit successfully with
zero selected cases. Alongside each family integration suite, run its request
tests with `cargo test -p huckleberry-cli --lib commands::prepared::tests::<module>`:
Task 4 uses `lifecycle`, Tasks 5-11 use `diaper`, `feed`, `pump`, `sleep`,
`measurements`, `entries` and `setup`, respectively; Task 13 uses `catalog`.
Use the existing API test stub as read-only support via a path module, with
synthetic child IDs and temporary settings. Register each test module in
`src/commands/prepared/tests/mod.rs` as it is added.

### Task 1: Retained state and conditional routes

**Files:** Create `src/prompt/flow/{mod,model,view}.rs` and
`tests/form_state.rs`; modify `src/prompt/mod.rs` and `docs/nomenclature.md`.
**Interfaces:** Produce `ViewId`, `FormValue`, `FormEvent`, `FormModel`,
`FormState`, `View` and `ControlEvent` from the shared contracts. Start with
selection/text controls; subsequent tasks add their other variants.

- [x] Write `back_keeps_forward_answers`, `hidden_answers_return_to_their_own_fields`,
  `skip_is_an_answer` and `back_does_not_validate`. Use a synthetic form with
  stable IDs `at`, `mode`, `colour`, `notes`, and assert this journey:
  ```text
  accept at=1000; mode=dirty; colour=brown; move to notes
  Back -> colour=brown; Back -> mode=dirty
  accept mode=wet -> notes; accepted output has no colour
  Back -> mode; accept dirty -> colour=brown
  ```
  Also assert two views with the same display label retain different answers.
- [x] Run `cargo test --test form_state`; observe the expected missing-interface
  failure first, then a behavior failure once the minimal types compile.
- [x] Implement the contracts using a map keyed by ViewId and a current ID.
  Maintain accepted values in the typed model and unfinished state in views.
  Recompute routes only from accepted values. Validate completion before Ready.
  Document form view, form draft and current-view cursor in nomenclature.
- [x] Run `cargo test --test form_state`; all cases pass. Inspect the tests to
  ensure they drive transitions and outputs, not internal map layout.
- [x] Commit the task files: `feat: retain form answers across view navigation`.

### Task 2: Shared controls distinguish Back, Home and cancellation

**Files:** Create `src/prompt/text_model.rs`, `src/prompt/flow/runner.rs` and
`tests/form_controls.rs`; modify `src/prompt/{mod,text,secret}.rs`,
`src/prompt/select/{model,terminal,draw,mod}.rs`,
`src/prompt/dial/{model,terminal,draw}.rs`, `src/prompt/flow/view.rs`,
`docs/tui.md` and `docs/rules/cli-ux.md`.
**Interfaces:** Consume Task 1. Produce `TextState`, plus
`flow::runner::collect<Form: FormModel>(form: &mut FormState<Form>, theme: Theme)
-> Result<Form::Output>` for standalone collection. Use existing renderers and
the same View methods in both transport paths.

- [x] Write `choice_back_and_home_are_distinct`, `dial_back_leaves_only_column_zero`,
  `text_back_keeps_letters_and_paste` and `reopened_secret_is_masked`.
  Table-test Left/h/a/H/A; at dial column 1 expect Stay/column 0, then Back.
  In text, insert `haha quiet`, paste `first\nsecond`, press Left, reopen and
  assert the buffer is `haha quietfirstsecond`. Escape emits Home, Ctrl-C Cancel.
- [x] Run `cargo test --test form_controls`; confirm existing cancellation or
  fresh-buffer behavior fails the assertions before changing the controls.
- [x] Extract pure text state and reuse current selection/dial state and renderers.
  Retain raw buffers and dial positions on Back; never substitute an old default
  for an explicitly cleared field. Keep secret state out of Debug/log output.
  Make digit shortcuts highlight only; keep shifted aliases and Tab semantics.
  Update hosted hints to distinguish `back` and `Esc home`; standalone adapters
  keep existing cancellation behavior and terminal restoration.
- [x] Run `cargo test --test form_controls --test menu_choices --test one_hand_forms`.
  Check masking, required-text validation, empty optional fields and rendered hints.
- [x] Commit: `feat: share retained prompt controls with explicit navigation`.

### Task 3: Resolved time and volume controls

**Files:** Create `src/prompt/flow/{time,volume}.rs` and `tests/form_values.rs`;
modify `src/prompt/{time,mod}.rs`, `src/prompt/dial/{time,volume}.rs`,
`src/prompt/flow/{mod,view}.rs` and `docs/architecture.md`.
**Interfaces:** Produce `TimeState::create_time(calendar: Calendar,
opening: dial::time::Opening, now: f64) -> Self`,
`TimeState::accept(&mut self, now: f64) -> Result<f64>` and
`VolumeState::create_volume(units: Units, amount: Option<f64>, minimum: f64)
-> Self`; integrate them as Control variants. Retain existing public readers.

- [x] Write `accepted_relative_time_does_not_move`, `untouched_time_keeps_precision`,
  `explicit_now_updates_time`, `clock_switch_keeps_instant` and
  `units_preserve_quantity`. At now=10_000 accept 20 minutes ago as 8_800;
  revisit at 10_300 and assert 8_800, then press `n` and assert 10_300.
  Repeat with a `.125` instant, local midnight and both occurrences of the
  America/New_York DST fallback hour. Tab/column focus must not dirty a value.
- [x] Run `cargo test --test form_values`; confirm relative reparsing and
  rounding would fail before adding retained resolved values.
- [x] Keep canonical instants/quantities separate from display wheels. Reuse
  existing date rules and unit conversions with explicit now. A real value
  edit replaces the canonical value; navigation and redraw do not. New bounds
  mark a retained answer invalid rather than silently clamping it. Bottle
  retains its positive minimum; pumping allows zero. Document the state split.
- [x] Run `cargo test --test form_values --test event_times --test volume_units`.
- [x] Commit: `fix: preserve accepted times and quantities while revisiting forms`.

### Task 4: Shell lifecycle, suspension and one-time execution

**Files:** Create `src/tui/forms.rs`, `src/commands/prepared/{mod,forms,execute}.rs`,
`src/commands/prepared/tests/{mod,lifecycle}.rs` and `tests/form_lifecycle.rs`;
modify `src/tui/{job,state,keys,mod,version}.rs`, `src/tui/draw/{mod,flow,version}.rs`,
`src/interactive/{mod,operation}.rs`, `src/commands/mod.rs`,
`src/prompt/host.rs`, `tests/tui_shell.rs` and `docs/architecture.md`.
**Interfaces:** Consume Tasks 1-3; produce PreparedCommand/CommandProgress.
`tui::forms::Forms` owns one active/suspended PreparedCommand, its CommandPath,
and execution phase. Add `Motion::Home` for Escape, distinct from Back/Cancel.

- [x] Write `first_view_suspends_without_a_waiter`, `same_command_restores_draft`,
  `another_command_clears_draft`, `escape_goes_home_from_every_depth`,
  `menu_selection_survives_reentry`, `ready_can_execute_only_once` and
  `escaping_a_write_waits_for_its_result`. Use a synthetic prepared form and
  a controlled execution future; assert its call count is exactly one despite
  queued Enter/Back. No read or write reruns when a suspended form is resumed.
- [x] Run `cargo test --test form_lifecycle` and `cargo test --test tui_shell`;
  confirm failures distinguish current menu-only navigation from the new flow.
- [x] Introduce Preparing, Collecting, Suspended, Executing and Result phases.
  Form controls run from the shell's input/tick loop without a waiting worker.
  Background tasks prepare or execute; existing host transport handles output
  and read-only viewers. Reuse Exchange queuing/nonblank-frame behavior, with
  queue ownership ending at form termination. Home during execution records a
  landing intent, consumes no new submission and still collects the result.
  Preserve selections keyed by MenuId and clear the suspended draft on Home,
  cancellation, different command, completion or quit. Document the boundary.
- [x] Run `cargo test --test form_lifecycle`, `cargo test --test tui_shell` and existing
  `tui::job` tests. Verify no disconnected reply causes an error receipt or hang.
- [x] Commit: `refactor: separate shell form collection from command execution`.

### Task 5: Diaper and potty forms, including the reported journey

**Files:** Create `src/commands/diaper_form.rs`, `tests/form_diaper.rs` and
`src/commands/prepared/tests/diaper.rs`; modify `src/commands/{mod,diaper}.rs`,
`src/commands/prepared/{forms,execute}.rs` and `docs/cli.md`.
**Interfaces:** Produce `DiaperForm: FormModel<Output = DiaperSubmission>`.
Submission owns `at: f64`, `mode: DiaperKind`, `details: DiaperDetails` and
`potty: Option<PottyOutcome>`. Existing diaper/potty entry points share this form.

- [x] Write `diaper_back_edits_time_and_prefills_later_views`,
  `wet_submission_omits_remembered_dirty_details`, `dirty_restores_hidden_answers`
  and `potty_preserves_outcome_without_rash`. Assert the approved journey with
  changed at=8_800 and colour=brown; inspect the actual local-stub request and
  assert one entry mutation. Back or Escape before completion must produce zero.
- [x] Run `cargo test --test form_diaper`; confirm the old flow cancels on Back before
  converting its collection code. Keep that regression as the acceptance anchor.
- [x] Move question definitions and typed answers into the form. Reuse
  `cli::actions::unanswered` for applicability, preserve explicit flags, and
  construct details from applicable fields. Register preparation and execution;
  preserve receipt content and use the existing API methods. Update CLI docs.
- [x] Run `cargo test --test form_diaper` and
  `cargo test --test mixed_diapers --test logging_order --test event_times`.
- [x] Commit: `fix: retain diaper and potty answers when navigating back`.

### Task 6: Bottle and nursing forms

**Files:** Create `src/commands/feed/{bottle_form,nursing_form}.rs`,
`tests/form_feed.rs` and `src/commands/prepared/tests/feed.rs`; modify
`src/commands/feed/{mod,bottle,nursing}.rs`,
`src/commands/prepared/{forms,execute}.rs` and `docs/cli.md`.
**Interfaces:** Produce BottleForm/BottleSubmission and NursingForm/NursingSubmission
using FormModel. Bottle submission owns at, units, amount, bottle kind and notes;
nursing submission owns action, resolved at where applicable and side.

- [ ] Write `bottle_back_retains_volume_kind_and_notes` and
  `nursing_back_retains_time_and_side`. Assert a time correction preserves an
  entered amount and kind; resuming does not refetch the suggested defaults.
  Drive start/pause/resume/switch/stop separately against the local stub and
  assert no timer mutation until Ready, then one invocation of the operation.
- [ ] Run `cargo test --test form_feed`; observe the navigation failures first.
- [ ] Prepare suggested bottle values and nursing side before collection.
  Reuse the canonical time/volume controls; keep default suggestions distinct
  from accepted answers. Move only collection and its executor seam, retaining
  existing API calls, timer outcomes and receipts. Update docs and dispatch.
- [ ] Run `cargo test --test form_feed` and
  `cargo test --test volume_units --test logging_order --test event_times`.
- [ ] Commit: `refactor: retain bottle and nursing form drafts`.

### Task 7: Pumping forms and custom duration views

**Files:** Create `src/commands/pump/form.rs`, `tests/form_pump.rs` and
`src/commands/prepared/tests/pump.rs`; modify `src/commands/pump/{mod,logging,prompts,timer}.rs`,
`src/commands/prepared/{forms,execute}.rs`, `docs/tui.md` and `docs/cli.md`.
**Interfaces:** Produce PumpForm/PumpSubmission. Submission owns action, resolved
at, PumpAmounts, units, optional duration in seconds and notes. Reuse Task 3 controls.

- [ ] Write `pump_modes_keep_separate_answers`, `pump_zero_side_survives_back`,
  `custom_duration_returns_to_presets` and `stop_keeps_timer_start`.
  Enter total=90 ml; switch to sides left=0/right=60; switch back and assert 90.
  Final submission includes only the selected mode. Back from custom duration
  preserves its buffer while returning to presets. Timer stop retains its API
  start/duration semantics after editing the end time.
- [ ] Run `cargo test --test form_pump`; confirm existing cancellation loses this state.
- [ ] Replace local collection with a typed form using distinct total/left/right
  IDs. Keep the optional custom duration as a child view. Register all pumping
  actions, with no artificial questions for actions needing none. Update docs.
- [ ] Run `cargo test --test form_pump` and
  `cargo test --test pump_cli --test pump_edit`, plus existing `commands::pump` tests.
- [ ] Commit: `refactor: retain pumping modes amounts and duration views`.

### Task 8: Sleep collection and overlap execution boundary

**Files:** Create `src/commands/sleep_form.rs`, `tests/form_sleep.rs` and
`src/commands/prepared/tests/sleep.rs`; modify `src/commands/{mod,sleep}.rs`,
`src/commands/prepared/{forms,execute}.rs` and `docs/cli.md`.
**Interfaces:** Produce SleepForm/SleepSubmission, owning action, resolved
start/end where applicable and the existing overlap decision.

- [ ] Write `moving_start_revalidates_retained_end`, `back_from_overlap_is_read_only`
  and `partial_overlap_failure_is_never_replayed`. Use start=1_000/end=1_600;
  change start to 1_700 and verify the retained 1_600 is shown as invalid until
  corrected. A stub fails manual logging after the overlap timer operation;
  subsequent Back/Enter must not issue either operation again.
- [ ] Run `cargo test --test form_sleep`; verify the old cancellation behavior and
  missing retained end handling fail for the expected reasons.
- [ ] Load overlap context before offering its choice. Collect all questions
  before calling resolve_overlap or log_sleep. Keep existing live-state checks,
  typed API conflicts and partial-failure reporting; no new rollback policy.
  Route timer actions through the same resolved-time form. Update docs.
- [ ] Run `cargo test --test form_sleep` and
  `cargo test --test event_times --test edit_preservation --test logging_order`.
- [ ] Commit: `refactor: retain sleep forms before executing timer changes`.

### Task 9: Solids and growth, including nested and repeated questions

**Files:** Create `src/commands/feed/solids_form.rs`, `src/commands/growth_form.rs`,
`tests/form_measurements.rs` and `src/commands/prepared/tests/measurements.rs`;
modify `src/commands/feed/{mod,solids}.rs`, `src/commands/{mod,growth}.rs`,
`src/commands/prepared/{forms,execute}.rs` and `docs/cli.md`.
**Interfaces:** Produce SolidsForm/SolidsSubmission and GrowthForm/GrowthSubmission.
Solids owns at, selected FoodReferences, amount, reaction and notes; growth owns
at, GrowthMeasurements and System. Use distinct owned view IDs for food items.

- [ ] Write `food_custom_entry_returns_to_selection`, `food_selection_retains_items`,
  `growth_units_preserve_measurements` and `optional_measurements_stay_cleared`.
  Select Avocado and Pear, revisit the custom food editor, and assert each item
  stays associated with its value. For growth, change units and return; compare
  physical quantities within 1e-9, preserving Skip: 2.54 cm becomes 1 inch,
  and 0.45359237 kg becomes 1 pound; returning restores the canonical values.
- [ ] Run `cargo test --test form_measurements`; observe missing nested navigation.
- [ ] Prepare food catalogs once; make list, custom-name and custom-helping views
  part of the same form. Retain the existing FoodSelection semantics. Canonical
  growth values are held in kg/cm, with the exact conversions above and rounding
  only for display. Unfinished invalid numeric text keeps its original unit
  context and requires correction before acceptance in another unit. Retain
  configured defaults and flag omission behavior. Register execution and docs.
- [ ] Run `cargo test --test form_measurements`, existing `commands::feed` tests and
  `cargo test --test one_hand_forms --test logging_order`.
- [ ] Commit: `refactor: retain food and growth inputs across nested views`.

### Task 10: Entry pickers, history edits and deletion

**Files:** Create `src/prompt/flow/listing.rs`, `src/commands/edit/retained.rs`,
`src/commands/delete_form.rs`, `tests/form_entries.rs` and
`src/commands/prepared/tests/entries.rs`; modify `src/listing/{mod,state}.rs`,
`src/prompt/flow/{mod,view}.rs`, `src/commands/{mod,delete}.rs`,
`src/commands/edit/{mod,pick,form,form_pump,history,live}.rs`,
`src/commands/prepared/{forms,execute}.rs` and `docs/architecture.md`.
**Interfaces:** Produce ListingControl over existing Listing/State rendering,
EditForm/EditSubmission and DeleteForm/DeleteSubmission. Edit submission owns
the selected token, original target identity and explicit field changes; delete
owns RowRef, started_at and an accepted confirmation tied to that RowRef.

- [ ] Write `back_from_field_keeps_other_changes`, `back_to_picker_keeps_selection`,
  `another_entry_has_another_draft`, `deleted_row_is_not_retargeted` and
  `delete_confirmation_is_bound_to_entry`. Select entry A, change notes, return
  to picker and choose B: B must show B's data and no accepted A confirmation.
  Remove A from refreshed rows and assert no automatic acceptance of B.
- [ ] Run `cargo test --test form_entries`; confirm current field/picker cancellation
  loses the containing view before changing those paths.
- [ ] Extract an owned listing control reusing the existing state/layout. Keep
  query, selected token and scroll on return. Edit's route is picker, field menu,
  chosen field, nested field view; field acceptance returns to the field menu.
  Reuse explicit-change construction, stored enum preservation and live sleep
  conflict checks. Deletion executes only after its current target is confirmed.
  Register collectors/executors and document the ownership boundary.
- [ ] Run `cargo test --test form_entries` and
  `cargo test --test edit_preservation --test pump_edit --test interactive_options`.
- [ ] Commit: `refactor: retain entry pickers and field edits until submission`.

### Task 11: Utility forms, authentication and setup continuations

**Files:** Create `src/commands/utility_form.rs`, `src/setup/form.rs`,
`tests/form_setup.rs` and `src/commands/prepared/tests/setup.rs`; modify
`src/commands/{mod,auth,child,settings,foods}.rs`, `src/setup/{mod,run}.rs`,
`src/session.rs`, `src/commands/prepared/{mod,forms,execute}.rs`,
`docs/setup.md` and `docs/cli.md`.
**Interfaces:** Produce utility FormModel variants for login, settings, child
selection and food creation. Export has no question and stays an immediate
operation behind the Task 4 execution guard. SetupForm output
is a validated Config. Continue carries the pending requested Command and the
reloaded Context into its next prepared form after a prerequisite completes.

- [ ] Write `setup_back_does_not_save`, `setup_saves_one_completed_draft`,
  `login_back_keeps_masked_credentials`, `settings_values_are_scoped_by_key`
  `prerequisite_completion_resumes_requested_command_once` and
  `first_use_escape_returns_to_home`. With temporary
  config/credential paths and local auth responses, assert no config mutation
  while going back through day settings; completing settings writes once.
  Count authentication and requested-command execution independently. Bare `h`
  with missing settings opens a hosted setup form; Escape returns to Home with
  no configuration write, and the next command still goes through setup.
- [ ] Run `cargo test --test form_setup`; confirm incremental setup saves and local
  prompt state fail the relevant assertions before modifying implementation.
- [ ] Collect missing settings into a Config copy, validate, save once and
  reload Context. Model authentication as its own completed prerequisite; Back
  within the subsequent form never reruns it. Move bare-command setup collection
  inside the shell instead of calling it before `tui::run`; show missing-data
  widget state until prerequisites are complete. Route missing child/credentials
  through these forms without bypassing dispatch's setup policy. Isolate each
  setting key's retained value; preserve environment-password handling and
  explicit flag behavior. Register remaining utility forms and update docs.
- [ ] Run `cargo test --test form_setup` and
  `cargo test --test interactive_entry --test interactive_catalog --test public_api`.
- [ ] Commit: `refactor: retain utility and setup forms across navigation`.

### Task 12: Home routing for viewers and completed results

**Files:** Modify `src/interactive/{operation,mod}.rs`, `src/tui/{mod,job,version}.rs`,
`src/listing/{mod,state}.rs`, `src/summary.rs`, `src/dashboard/mod.rs`,
`src/tui/draw/{mod,version}.rs`, `src/commands/views.rs` and `docs/tui.md`;
create `tests/form_viewers.rs`.
**Interfaces:** Reuse Motion::Home/FormEvent::Home. Introduce a typed hosted
return-to-Home result recognized before cancellation/error handling, so legacy
read-only loops can unwind to Home without an error or follow-up question.

- [ ] Write `escape_leaves_search_and_goes_home`, `viewer_back_returns_to_parent`,
  `result_navigation_never_reopens_a_submission` and `home_is_not_swallowed_by_read_retry`.
  Open View logs, Foods List, Summary, Dashboard, Help and Version from nested
  menus: Escape reaches Home; Back reaches the containing view. A focused
  search still accepts h/a as letters. No extra Continue question appears.
  Back from a Help topic restores its topic chooser and prior selection.
- [ ] Run `cargo test --test form_viewers`; confirm Escape currently behaves
  as ordinary Back or only leaves the search editor.
- [ ] Distinguish hosted Escape/Home from search/menu Back without changing
  standalone viewer behavior. Close overlays and cancel their pending reads.
  Retain Help's chooser and rendered topic as two views in `interactive/mod.rs`;
  generate topic text from the same clap metadata as the existing helper.
  Keep submitted and failed forms closed, preserve read-only explicit Retry,
  and report the outcome if Home was requested during execution. Update hints.
- [ ] Run `cargo test --test form_viewers --test version_modal --test tui_shell`
  and existing `interactive::operation` tests.
- [ ] Commit: `fix: route Escape to Home across hosted viewers and results`.

### Task 13: Complete coverage, terminal journeys and final review

**Files:** Create `src/commands/prepared/tests/catalog.rs` and
`tests/form_journeys.rs`; modify `tests/support/{prompt_driver.rs,interactive_pty.py}`,
`docs/{architecture,tui,nomenclature,cli,setup,decisions}.md`,
`docs/rules/cli-ux.md` and this plan's task checkboxes.
**Interfaces:** Consume the completed prepared-command registry and all forms.
Use existing local Firestore stub support without changing the API crate.

- [ ] Write `every_prompting_command_has_a_retained_form` against the clap/menu
  catalog, classifying each leaf as a retained form, immediate action or reader.
  Fail on unclassified new leaves. Add `fast_navigation_keeps_paste_but_clears_exit_queue`
  and a bounded offline/synthetic PTY journey for the diaper correction, text,
  a nested submenu, suspension/resumption and Escape/Home.
- [ ] Run `cargo test --test form_journeys`; confirm failures for any uncovered routing
  and key-queue leaks before fixing them. Run the PTY journey in its new mode
  and verify failures reflect navigation rather than missing credentials.
- [ ] Close only navigation coverage gaps. Update docs to describe the final
  module names, no duplicate prompt implementations, conditional values, fixed
  times and save boundary. Record the retained-draft choice in decisions.
  Mark task checkboxes only for observed completed work and passing checks.
- [ ] Run `just check` and
  `python3 tests/support/interactive_pty.py --scenario all`.
  If just is unavailable, run `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`, and
  `python3 -B -m unittest discover -s scripts/release/tests -v` individually.
  Expected: all commands exit 0; PTY reports `PTY checks passed`.
- [ ] Review the full branch using the execution method selected by the user;
  address findings with reproducing tests. Commit the final tests/docs as
  `test: verify retained form navigation across the shell`. Report checks,
  branch and remaining limitations. Leave the worktree for review, with no push
  or merge and no cleanup of unmerged work.

## Plan review and execution

Coverage audit against the spec's numbered acceptance criteria:

| Spec criteria | Owning tasks |
| --- | --- |
| 1-2: diaper correction and conditional details | 1, 5 |
| 3-4: retained controls, nested views and directional keys | 2, 3, 7, 9, 10, 12 |
| 5: suspension, draft clearing and menu navigation | 4 |
| 6: fixed accepted timestamps | 3 |
| 7: dependencies, units and explicit edit preservation | 7, 8, 9, 10 |
| 8: picker identity and setup collection | 10, 11 |
| 9: mutation boundary, pending input and failure behavior | 4, 5, 8, 11, 13 |
| 10: all command families, shared prompts and CLI compatibility | 5-13 |

Tasks 1-4 establish shared interfaces; Task 5 proves them with the reported
diaper journey. Tasks 6-11 migrate all other prompting command families, Task 12
finishes viewer routing, and Task 13 checks the whole catalog and transport.
Each task includes its docs, red/green tests and a local commit.

Recommended execution: native execution in this session, followed by a fresh
whole-branch review. The tasks share control and lifecycle interfaces closely,
so retaining that context helps catch inconsistencies as each family migrates.
Subagent-driven execution is also available if per-task independent reviews
are preferred. Implementation starts after the user reviews this plan and
selects an execution method, as required by the writing-plans workflow.
