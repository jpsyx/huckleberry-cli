# Rust rules

How Rust is written in this repository. The lint configuration in
`Cargo.toml` enforces the first two mechanically; the rest is on you and on
review.

## No `unsafe`

`[lints.rust] unsafe_code = "forbid"` is set, and it stays set. A problem that
seems to need `unsafe` is a problem that needs a different crate or a different
design. Raise it before reaching for the escape hatch.

## Keep clippy clean

`pedantic` and `nursery` are on at `warn`, and `just lint`
(`cargo clippy --all-targets -- -D warnings`) turns every warning into a
failure. Do not land new warnings, and do not silence one with a bare
`#[allow(...)]`. When a lint is genuinely wrong for this codebase, allow it in
`Cargo.toml` with a comment saying why, so the exception is visible in one
place instead of scattered through the source.

## Split the pure from the impure

Decision logic goes in a pure function that takes values and returns values:
which key means what, how a line is formatted, what a parsed argument implies.
Those functions get the tests. The shells that touch the terminal, the
filesystem, the network, or a child process stay thin enough to read in one
screen, and hold no decisions worth asserting. When a shell grows an `if` that
matters, that `if` belongs in a pure function next to it.

Write tests red first: a failing test that names the behavior, then the
smallest code that turns it green, then the cleanup with the bar green.

## Errors carry context

Return `anyhow::Result` from anything a person can trigger, and attach
`.context("what was being attempted")` at every layer that knows something the
message would otherwise lose. The binary prints the whole chain, so each
context line is one step of the story: what failed, while doing what.

## One module per file, and keep files small

Prefer many small single-responsibility modules over a few large ones. A `.rs`
file that passes roughly **400 lines of production code** is a smell: split it
into a directory of submodules (`foo.rs` becomes `foo/mod.rs` plus
`foo/<part>.rs`), and let the directory tree mirror the ownership in the code.
Inline `#[cfg(test)]` blocks do not count toward that budget, but they are held
to the same standard: split a large or multi-purpose suite by behavior, and
build fixtures from small composable helpers rather than one catch-all harness.
`mod.rs` stays a thin glue and re-export layer. Split along a real seam, never
at an arbitrary line, and do not split a file that is already cohesive just to
hit a number.

## Comment the why, not the what

The function name and its doc comment cover what it does. A comment earns its
place when the reason is not obvious from the code: a surprising API, an
ordering that matters, a workaround and the thing it works around. Document
every exported item with `///`.

## Do not add dependencies casually

The dependency set is small on purpose, and every crate in it is one more
thing to audit, update, and explain. Before adding one, check whether the
standard library or a crate already present does the job. When it is genuinely
worth it, add it with a comment in `Cargo.toml` saying what it is for, and
justify it in `docs/architecture.md`.

## `docs/` is the durable record

Code says what the program does today; `docs/` says how it fits together and
why. Update the affected document in the same change that changes behavior.
Design rationale belongs in `docs/decisions.md`: one section per decision, what
was chosen, what it rules out, and what would make it worth revisiting.

## Bump the crate version for every committed change

`Cargo.toml` is the single source of the version and `Cargo.lock` moves with
it. Before 1.0, a user-visible addition bumps the minor version and a fix or an
internal change bumps the patch version. Pick the bump yourself rather than
asking.
