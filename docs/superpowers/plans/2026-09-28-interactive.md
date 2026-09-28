# One-handed Interactive CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every existing command and option reachable from bare `hb`, with numbered menus and one-handed selection throughout the app.

**Architecture:** Shared prompt controls own selection, text entry, and terminal cleanup. An interactive command tree collects arguments and resolves them through clap into the same typed commands used by explicit invocations. Both paths share command dispatch; a session loop reloads context between operations.

**Tech Stack:** Rust edition 2024, clap derive, crossterm, ratatui, anyhow, serde/TOML; existing dependencies only.

**Spec:** [Approved interactive design](../specs/2026-09-28-interactive-design.md).

## Global Constraints

- Work only in `feat/interactive` at the existing worktree; load `using-wt` before worktree operations. Do not merge or push this feature without a new request.
- Read repository instructions, `docs/architecture.md`, `docs/cli.md`, `docs/dashboards.md`, `docs/rules/rust.md`, and `docs/rules/cli-ux.md`. Run `npx skills list` before implementation.
- No new dependency is required. API crate and storage semantics stay unchanged.
- Fixed choices must never require typing their names or numbers.
- Every list shows numbers and a visible selection marker, including logs, edit, delete, foods, and child selection.
- Menu and prompt output uses stderr. Explicit JSON, exports, receipts, and other command data retain their stdout formats.
- Required activity questions keep time first. Pre-supplied options are not asked again.
- Existing configured or remembered defaults are highlighted. Optional empty fields highlight Skip. Keep and Clear are distinct.
- J/H/Down move down; K/P/Up move up, including uppercase letters. Letters remain text in a text editor or search.
- Pure domain and rendering functions do not read the clock or perform terminal/network I/O. Functions stay at most 45 lines; split cohesive modules when production files exceed roughly 400 lines.
- Follow red/green TDD. Each task runs its regression tests and full workspace tests before its commit. Update the relevant current-behavior docs with that task.
- Bump CLI version and matching lock entry in each implementation commit: first user-visible addition from 0.11.1 to 0.12.0, later additions increment minor and internal fixes increment patch, per repository rules. Do not bump API version.

## Review Focus

1. Empty, filtered, or very long lists: no invalid selection, lost cursor, or unnumbered rows (Tasks 1 and 3).
2. Esc, Ctrl-C, EOF, key-release events, and I/O failure during mixed text/menu flows: no unintended submission and restored terminal state (Tasks 2 and 8).
3. Unknown stored enum values and rounded defaults: Keep preserves original data without converting or clearing it (Task 4).
4. Account, child, configuration, or offline source changes between operations: the next operation uses fresh context (Task 8).
5. Repeated arguments, mutually exclusive flags, masked secrets, and cancellation during argument collection: no invalid command dispatch, leaked password, or accidental write (Tasks 5 through 8).

## Files and boundaries

| Files | Responsibility |
| --- | --- |
| `src/prompt.rs` to `src/prompt/mod.rs`, `question.rs`, `text.rs`, `secret.rs`, `terminal.rs` | Preserve public prompt API while separating data, text editing, passwords, and terminal ownership; keep `prompt/time.rs` |
| New `src/prompt/select/{mod,model,draw}.rs` | Pure choice state and rendering, thin stderr terminal loop |
| `src/listing/{state,layout,mod}.rs`, `src/dashboard/{state,draw/panels}.rs`, `src/commands/dash.rs` | Shared key contract, visible numbering, focus-sensitive dashboard navigation |
| `src/commands/{feed,settings,delete}.rs`, `src/commands/edit/form.rs` | Fix free-text finite choices, multi-food selection, Keep/Clear, raw tracker picker |
| New `src/commands/feed/{mod,bottle,nursing,solids}.rs` | Split the existing oversized feed file by activity as its forms change, retaining public entry points |
| New `src/interactive/{mod,catalog,draft,options,session}.rs` | Menu tree, pending arguments, input widgets, loop and context reload |
| New `src/interactive/options/{fields,edit,session}.rs` if needed | Split option widgets by responsibility rather than letting options.rs grow beyond the file limit |
| `src/cli/mod.rs`, `src/commands/mod.rs`, `src/lib.rs` | Optional subcommand and shared dispatch |
| New `tests/{menu_choices,interactive_catalog,interactive_options,interactive_entry}.rs` | Pure behavior, actual clap coverage, typed command construction, nonterminal entry |
| New `tests/support/interactive_pty.py` | Standard-library PTY smoke driver with synthetic/offline fixtures and bounded waits |
| Existing `tests/{event_times,friendly_output,public_api,logging_order}.rs` | Preserve existing parser, output, and prompt-order contracts |
| `README.md`, `docs/{cli,architecture,dashboards}.md`, `docs/rules/cli-ux.md` | Implemented user-facing behavior and architectural boundaries |

No production code or dependencies are added during this planning stage.

## Task 1: Pure numbered choice model

**Files:** Create `src/prompt/select/{mod,model,draw}.rs`, `tests/menu_choices.rs`; modify `src/prompt.rs` to expose the module.

**Interfaces:** In `prompt::select`, define `MenuItem { label: String, detail: Option<String> }`, `Selection { cursor: usize, top: usize }`, `SelectionAction::{Stay, Submit(usize), Cancel}`. Export `Selection::new(count: usize, default: usize) -> Self`, `Selection::apply(&mut self, key: KeyEvent, count: usize, height: usize) -> SelectionAction`, and `render(label: &str, items: &[MenuItem], state: &Selection, width: u16, height: u16, theme: Theme) -> Vec<String>`. Re-export from select/mod.rs.

- [ ] Write failing tests `menu_keys_select_only_on_enter`, `empty_menu_cannot_submit`, `selection_stops_at_boundaries`, `narrow_menu_keeps_number_and_cursor`, and `release_events_do_not_select`. Key expectations:

```rust
assert_eq!(state.apply(press('H'), 3, 2), SelectionAction::Stay);
assert_eq!(state.cursor, 1);
assert_eq!(state.apply(enter(), 3, 2), SelectionAction::Submit(1));
assert_eq!(empty.apply(enter(), 0, 2), SelectionAction::Stay);
```

Test helpers construct real `KeyEvent`s. Cover j/J/h/H, k/K/p/P, arrows, Esc and Ctrl-C, zero-height and one-row viewports, and a 100-row list with a default near its end.
- [ ] Run `cargo test -p huckleberry-cli --test menu_choices`; confirm missing behavior fails.
- [ ] Implement clamped movement, ignored modifiers/release events, row visibility, wrapping that preserves prefixes, and numbered rendering. Reuse terminal-cell measurement already used by render/output.rs.
- [ ] Run the targeted suite, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`; all must pass.
- [ ] Update the prompt architecture note, version/lock, and commit `feat: add numbered choice navigation`.

## Task 2: Shared terminal prompts with defaults and cancellation

**Files:** Split `src/prompt.rs` into the files in the map; preserve `src/prompt/time.rs`. Extend `tests/menu_choices.rs`; add initial PTY harness `tests/support/interactive_pty.py`.

**Interfaces:** Preserve existing `Question`, `Choice`, `ask`, `ask_optional`, `confirm`, `ask_secret`, `interpret`, and time reader signatures through re-exports. Add `prompt::Cancelled`, a concrete error implementing `std::error::Error`; expose `prompt::is_cancelled(error: &anyhow::Error) -> bool`. Add `select::choose(label: &str, items: &[MenuItem], default: usize, theme: Theme) -> anyhow::Result<usize>`. Add `prompt::terminal::TerminalGuard::enter() -> anyhow::Result<Self>` with explicit `restore(&mut self) -> anyhow::Result<()>` and best-effort Drop cleanup. Internal `read_text(question: &Question<'_>, theme: Theme) -> Result<String>` owns free-text input.

- [ ] Add failing tests `default_choice_is_selected`, `optional_empty_highlights_skip`, `required_question_has_no_skip`, `confirmation_preserves_default`, and `typed_cancel_survives_context`. Assert optional Enter returns None, explicit defaults return their exact value, and cancellation remains distinguishable after anyhow context wrapping.
- [ ] Run `cargo test -p huckleberry-cli --test menu_choices`; confirm failure before implementing the selectors.
- [ ] Route finite questions through choose. Represent Skip separately from real values internally, so a real choice named "skip" still works. Required questions with an unlisted current default receive a Keep current value entry. Confirm uses Yes/No and returns the chosen boolean.
- [ ] Route free text through a crossterm editor supporting normal characters, backspace, Ctrl-U, pasted text, Enter, Esc, Ctrl-C, and EOF. Preserve current trim/default semantics. Optional text uses Skip/Enter text or Keep/Enter text/Clear. Password input stays masked, including pasted text; secrets never appear in command previews or diagnostics. Terminal availability requires usable stdin and stderr for menus; nonterminal behavior retains existing defaults/errors.
- [ ] Implement the guard and stderr selection loop without leaving an alternate screen active between prompts. Guard restoration must run on read/write errors as well as normal returns. Add a PTY smoke scenario verifying a menu-to-text transition, cancellation, and terminal echo restoration; give every wait a timeout.
- [ ] Run targeted tests, `python3 tests/support/interactive_pty.py --scenario prompts` after building its test driver, then full checks. The driver exercises production prompt code with synthetic questions, not account credentials.
- [ ] Update docs/cli.md and docs/rules/cli-ux.md, bump version/lock, and commit `feat: use selectable lists for shared prompts`.

## Task 3: Consistent navigation and numbering in existing lists

**Files:** Modify `src/listing/{state,layout,mod}.rs`, `src/dashboard/{state,draw/panels}.rs`, `src/commands/dash.rs`; tests remain next to pure state/layout functions.

**Interfaces:** Keep existing Listing public methods and machine rendering. Add `dashboard::state::action_for_tab(key: KeyEvent, tab: Tab) -> Action`; route the event loop through it. Existing `action_for` can remain a wrapper if tests or other callers need it. Numbering belongs to interactive rendering, not exported row data.

- [ ] Write failing tests `h_and_p_move_list_rows`, `search_keeps_navigation_letters_as_text`, `filtered_rows_have_consecutive_numbers`, and `dashboard_h_scrolls_log_without_switching_tabs`. Assert searching "happy" keeps that text; selecting filtered row 2 returns that row's original key. On Log, H returns ScrollDown and P returns ScrollUp; Left still changes tabs.
- [ ] Run `cargo test -p huckleberry-cli listing` and `cargo test -p huckleberry-cli dashboard`; verify failures.
- [ ] Extend movement aliases and numbering for browsable lists and dashboard log rows. Preserve paging, disabled-row explanations, and noninteractive output. Reuse Task 1's key behavior without interpreting search characters as navigation.
- [ ] Verify state/layout tests, narrow rendering, and full checks. Update docs/dashboards.md and command list hints together.
- [ ] Bump version/lock and commit `feat: unify navigation across browsable lists`.

## Task 4: Complete one-handed activity and edit forms

**Files:** Split `src/commands/feed.rs` by activity; modify `src/commands/edit/form.rs`, `src/commands/settings.rs`, and `src/commands/sleep.rs`; inspect existing diaper/growth consumers and adjust only if their shared prompt usage cannot express the spec. Add behavioral cases to `tests/menu_choices.rs`, `tests/event_times.rs`, and relevant inline suites.

**Interfaces:** Keep `commands::feed::run` and existing public feed helpers available. In `commands::feed::solids`, define `FoodSelection { selected: Vec<String> }`, `FoodAction::{Add(String), Remove(usize), Done}`, and `FoodSelection::apply(&mut self, action: FoodAction) -> bool` (true only for Done with a nonempty selection). `ask_for_foods(context: &Context, client: &Huckleberry, cid: &str) -> Result<Vec<String>>` owns fetching once and navigating the selection. `settings::value_choices(key: &str) -> Vec<Choice<'static>>` supplies finite setting choices; free-form values retain their existing parser.

- [ ] Write failing tests `meal_can_add_remove_and_finish_multiple_foods`, `settings_units_measurements_and_verbose_are_choices`, `keep_preserves_unknown_enum_and_exact_number`, `clearing_optional_edit_is_distinct_from_keep`, and `sleep_overlap_starts_on_cancel`. Keep the existing time-first tests.
- [ ] Run targeted tests to record expected failures.
- [ ] Implement food Add/Remove/Done, known-food choices plus Enter new name, and solids amount Some/Custom. Replace any catch-all fallback from a cancelled food selection with propagation of the typed cancellation.
- [ ] Make edit choice controls preserve exact current values, including values not represented by the current enum. Optional text clearing no longer requires typing a sentinel. Use existing parser/validation for explicit `--set` flags. Offer Cancel as the initial overlap choice rather than selecting a destructive resolution on an unexamined Enter.
- [ ] Make finite settings values selectable and keep child selection by name. Retain numeric/time text entry, existing default amounts, and configured units. Optional unanswered form fields use the Task 2 Skip controls.
- [ ] Run targeted and full checks, update docs/cli.md and docs/architecture.md, bump version/lock, and commit `feat: complete one-handed recording and edit forms`.

## Task 5: Typed menu catalog and complete command coverage

**Files:** Create `src/interactive/{mod,catalog,draft}.rs`, `tests/interactive_catalog.rs`; modify `src/lib.rs`.

**Interfaces:** Define `CommandPath(Vec<String>)`, `MenuTarget::{Menu(MenuId), Command(CommandPath), SessionOptions, Help, Version, Exit}`, `MenuEntry { label: String, target: MenuTarget }`, and `catalog::entries(menu: MenuId) -> Vec<MenuEntry>`. `MenuId::{Home, Feed, Nursing, Sleep, Visualizations, OtherLogging, More, Foods, Children, Account, Settings}` covers the spec tree; SessionOptions and Help open their dedicated views. Define `CommandDraft { path: CommandPath, values: BTreeMap<String, Vec<String>> }`, with `CommandDraft::new(path: CommandPath) -> Self`, `set(&mut self, id: &str, values: Vec<String>)`, `clear(&mut self, id: &str)`, and `resolve(&self, globals: &SessionOptions) -> Result<Cli>`. Define `SessionOptions { config: Option<PathBuf>, child: Option<String>, offline: Option<PathBuf>, verbose: bool }` in session.rs now, with `from_cli(cli: &Cli) -> Self`; Task 7 adds its interactive editor.

- [ ] Write failing `home_order_and_all_commands_are_reachable` with this literal order:

```rust
assert_eq!(home_labels, ["Log a diaper", "Log a feed", "Log sleep", "Edit",
    "Delete", "Visualizations", "View logs", "Other logging", "More", "Exit"]);
```

Walk clap's real command tree and compare canonical leaf paths to reachable catalog routes. Treat generated help/version separately; test `sleep end` resolves identically to `sleep stop`.
- [ ] Run `cargo test -p huckleberry-cli --test interactive_catalog`; verify failure.
- [ ] Implement the exact tree in the spec. Draft values use clap argument IDs, not display labels. Serialize arguments as a Vec<OsString> and use clap's parser to build typed commands; never join a shell command. Respect positional ordering, repeated values, and boolean actions. Do not include secrets in generated error context.
- [ ] Add and run tests that resolve bottle, sleep, edit repeated set, and solids repeated food drafts into their expected Command variants; invalid flags/values must fail validation.
- [ ] Run full checks, document the catalog boundary, bump version/lock, and commit `feat: map interactive menus to typed commands`.

## Task 6: Every command option gets an appropriate control

**Files:** Create `src/interactive/options.rs` or the split modules in the file map, `tests/interactive_options.rs`; extend draft.rs and catalog.rs only for their contracts.

**Interfaces:** Define `OptionControl::{Choice, Boolean, Text, Secret, Repeated, Form}` and `OptionBinding { argument_id: String, control: OptionControl }`; expose `options::bindings(path: &CommandPath) -> Vec<OptionBinding>`, `options::edit(context: &Context, draft: &mut CommandDraft) -> Result<()>`, and `draft::preview(&CommandDraft) -> Vec<(String, String)>` with password redaction. Metadata supplies enum values, help, argument ordering, and conflicts; explicit policies choose domain widgets where metadata cannot.

- [ ] Write failing `every_clap_argument_has_a_control`, walking real leaf arguments including inherited globals, positionals, generated help/version, and aliases. Compare to the controls actually consumed by edit, not a separate test-only allowlist. Assert repeated food and set stay repeated, password uses Secret, and the custom/curated pair cannot both remain selected.
- [ ] Run `cargo test -p huckleberry-cli --test interactive_options`; verify failure.
- [ ] Implement the Run/Options/Back leaf menu and option editors. Bool/enum values are lists. Numeric/text/path values use text entry; reset clearly says Use default or Ask when running. Reset removes a pending override; it must not pretend the eventual activity question has already been answered. Forms provide actual Skip when collecting a recording.
- [ ] Implement a repeated edit-field builder: select an existing entry, choose supported fields for its kind, edit values through the same form controls, remove pending changes, and Apply together. Use the existing edit key validation and `--set` execution path. Live sleep and time-only rows expose only supported fields. Do not require typing field names or `KEY=VALUE` strings.
- [ ] Expose all log filters, days/limits, refresh, output formats, tracker selection, food sources/archived/search, export file/stdout, login timezone, and positional values from the spec table. Preserve validated explicit IDs as an alternative to pickers.
- [ ] Run tests for conflict removal, repeated argument removal, clear/reset behavior, masked password previews and parse errors, and Back abandoning a draft. Run full checks, update docs/cli.md, bump version/lock, and commit `feat: expose all command options through menus`.

## Task 7: Tracker deletion and session options

**Files:** Modify `src/commands/delete.rs`; create/extend `src/interactive/session.rs` and session option controls; tests in `tests/interactive_options.rs` and inline delete tests.

**Interfaces:** `SessionOptions` from Task 5 retains config/child/offline/verbose; add `to_cli(&self, command: Option<Command>) -> Cli` when Task 8 makes command optional. Until then, use an explicit Command argument. `session::edit_options(current: &SessionOptions, context: &Context) -> Result<SessionOptions>` edits a cloned value and publishes it only when confirmed. Add a pure `delete::create_tracker_rows(rows: &[(RowRef, serde_json::Value)], calendar: &Calendar) -> Vec<crate::listing::Row>`; construct the borrowed Listing inside raw using owned rows and static column definitions.

- [ ] Write failing tests `raw_tracker_rows_can_be_selected_without_typing_id`, `session_cancel_preserves_all_overrides`, and `offline_choice_can_return_to_live`. Assert chosen row keys equal canonical edit tokens and machine list-only output retains its prior format.
- [ ] Run targeted tests to verify failures.
- [ ] In raw tracker deletion, fetch located rows and offer the numbered picker if no ID was supplied on a terminal. Reuse existing removal and confirmation logic, with No highlighted. Nonterminal missing-ID errors still name --id.
- [ ] Add session menus for verbose, configuration path, child override, and offline/live source. Child selection uses available names plus explicit-ID input. Configuration switching applies to the next operation and reloads credentials from beside that file. Ordinary option browsing does not persist configuration.
- [ ] Run focused and full checks, update CLI documentation, bump version/lock, and commit `feat: add tracker picking and session option menus`.

## Task 8: Bare invocation and persistent session loop

**Files:** Modify `src/cli/mod.rs`, `src/commands/mod.rs`, `src/session.rs` only as needed; implement `src/interactive/mod.rs`; create `tests/interactive_entry.rs`; adapt pattern matches in existing CLI tests.

**Interfaces:** `Cli.command: Option<Command>`; remove arg_required_else_help. Export `commands::dispatch(context: &Context, command: &Command) -> Result<()>` containing existing arms. `commands::run(cli: &Cli, theme: Theme) -> Result<()>` chooses explicit dispatch, interactive session, or printed help. `interactive::run(cli: &Cli, theme: Theme) -> Result<()>` owns SessionOptions and menu stack. Add pure `interactive::entry_mode(has_command: bool, stdin_terminal: bool, stderr_terminal: bool) -> EntryMode`, where `EntryMode::{Command, Interactive, Help}`, and `session::load_context(options: &SessionOptions, theme: Theme) -> Result<Context>`.

- [ ] Write failing tests `bare_cli_parses_without_command`, `nonterminal_bare_cli_prints_help_and_exits`, `global_only_cli_enters_menu_when_terminal`, `context_reload_uses_new_config_and_child`, and `typed_cancellation_returns_to_menu`. Update existing bare-invocation test to assert the new behavior; explicit-command parsing tests continue asserting their existing fields under Some.
- [ ] Run `cargo test -p huckleberry-cli --test interactive_entry` and CLI parser tests; verify failures.
- [ ] Extract shared dispatch without changing command behavior. Implement the menu stack, leaf Run/Options/Back, help/version display, static-result Enter pause, and Exit. Reopen Context before every command so changed persistent settings, login/logout, and remembered child apply immediately. Keep config-open errors recoverable through session options.
- [ ] Handle typed cancellation as a return to the parent/home flow; show other failures once and never automatically retry writes. Release menu terminal ownership before invoking the handler. Preserve stdout machine data when redirected, and keep prompts on stderr.
- [ ] Finish SessionOptions::to_cli with optional command, adjusting Task 5 draft resolution and Task 7 callers consistently. Verify full clap command/argument coverage again.
- [ ] Run focused and full checks, update README bare invocation plus architecture/CLI docs, bump version/lock, and commit `feat: launch the interactive session from bare hb`.

## Task 9: Terminal journeys, review, and delivery

**Files:** Extend `tests/support/interactive_pty.py` and `tests/interactive_entry.rs`; only fix production files for demonstrated failures within this feature.

**Interfaces:** PTY runner accepts `--scenario prompts|session|all`, builds or locates the debug binary and a test-only prompt driver, creates temporary config/snapshot files, writes keystrokes after expected screen text, and fails on timeout or unexpected process exit. No account credentials or live writes are allowed.

- [ ] Add failing PTY journeys: navigate root through Edit/Delete/Visualizations; select settings units by H/P and Enter; optional Skip/Keep/Clear through the real prompt driver; menu -> typed time -> menu; long list scrolling and filtering; dashboard exit to parent; failed operation then another selection; Ctrl-C at home; restored echo after Esc and synthetic I/O failure.
- [ ] Implement or adjust only the behavior revealed by those failures. All fixture/model code stays under tests/support. Compile `tests/support/prompt_driver.rs` against the built CLI library from the harness; keep the driver entirely in tests/support rather than adding a production command. Its synthetic questions exercise choices, optional values, text, and cancellation.
- [ ] Run `cargo build -p huckleberry-cli`, then `python3 tests/support/interactive_pty.py --scenario all`. Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and full `cargo test`. All must exit zero; record any environment limitation explicitly.
- [ ] Review spec coverage against the actual catalog, form widgets, alias handling, global overrides, and docs. Request a fresh whole-branch code review using requesting-code-review; resolve concrete findings and rerun affected checks.
- [ ] If fixes occurred, update docs/version/lock and commit them after checks. Report verified behavior and remaining limitations. Leave this branch for review; do not merge, push, install, or remove this worktree without the corresponding request.

## Plan self-review

Each spec section maps to Tasks 1-3 (navigation/presentation), 2/4/6 (defaults and forms), 5-7 (complete commands/options), 8 (entry/session/errors), and 9 (terminal journeys and delivery). Review Focus cases are assigned to named regression tests. The API crate is not modified. The chosen H/P mapping, Edit/Delete ordering, flags-only access, complete option reachability, and terminal restoration remain explicit acceptance conditions.

Execution has not started. Next: human review of this plan and selection of native or subagent-driven execution.
