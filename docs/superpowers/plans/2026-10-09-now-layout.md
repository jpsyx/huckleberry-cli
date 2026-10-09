# Responsive Now layout implementation plan

**Goal:** Lay out Last facts, recent/daily tables, and typical ranges according
to the available terminal or drawer width.

**Architecture:** `render::now::screen` owns all three sections and their
placement. A pure layout module wraps styled text and joins columns; the CLI
and Ratatui drawer consume exactly the same rows at the same content width.

**Spec:** The requested wide layout is facts, tables, ranges. The two-column
layout stacks facts and ranges on the left, with tables on the right. The
single-column layout stacks facts, tables, ranges. Keep the configured daily
heading (`Today` or `In last 24h`) and existing reference wording.

**Constraints:** No new dependencies, network calls, or clinical claims.
Unknown width uses the single-column layout. Never discard typical ranges
because the width is narrow. Preserve semantic colors and table event times.

## Implementation

- [x] Add renderer tests for wide, medium, narrow, and unknown widths, missing
  birthdate, long facts, Unicode text, and the configured daily heading.
- [x] Run `cargo test -q render::now::layout_tests` and confirm failures from
  the existing two-column arrangement and unwrapped facts.
- [x] Add `src/render/now/layout.rs`: choose three columns from 144 cells,
  two from 96, and one below 96. Share width fairly after three-cell gutters.
  Wrap facts and ranges without changing their text or tone.
- [x] Assemble the three sections in `src/render/now.rs`; make both `lines`
  and `src/tui/draw/now.rs` call `screen`. Remove the ranges-dropping path.
- [x] Add drawer tests for equal-width parity and resizing. Update old layout
  expectations to the requested arrangement; preserve content assertions.
- [x] Update `docs/dashboards.md`, `docs/tui.md`, and `docs/architecture.md`.
  Bump the CLI minor version to 0.44.0.
- [x] Run format, full tests, and clippy; inspect rendered fixture layouts.
- [x] Request independent code review and resolve material findings.

## Review focus

Width transitions must not drop sections. Wrapping must retain timestamps
and semantic colors. Wide Unicode names must count terminal cells. Missing
age data must not create an empty third column. Live resizing must use the
drawer content width after its borders and padding.

## Verification record

The placement/width tests failed against the original layout, then passed.
A separate failing regression confirmed that narrow fact wrapping split the
feed amount from its milk type; wrapping now keeps that detail together when
it fits. The full workspace suite passed 1,141 tests; format and clippy checks
passed. Independent review found no issues. Fixture output was inspected at
80, 120, and 180 cells, and the live resize test returns through all layouts.

The drawer's existing height policy remains unchanged: the menu retains its
minimum usable height, and short screens may clip the drawer's lower rows.
Scrolling and height-policy changes are outside this width-layout change.
