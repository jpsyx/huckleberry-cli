# CLI experience rules

This is a tool people type at, and a tool agents script. Both are first-class,
and the rules below are what keeps them from pulling the design apart.

## Stdout is data, stderr is the conversation

Stdout carries only intentional output: the answer the command was asked for,
in a shape something else can consume (`key=value` lines, a path, a JSON
document). Everything else goes to stderr: progress, prompts, warnings,
failures, and the argument parser's own errors. Anyone may pipe this tool into
another, so a diagnostic on stdout is a bug, and so is a decorated,
hand-aligned table where a machine expected a value.

## Two audiences, both first-class

- **An agent must be able to drive everything non-interactively.** Every action
  has a subcommand or a flag. No action is reachable only by answering a
  prompt, and no command blocks on input it was given on the command line.
- **A human who omits a value gets asked, not rejected.** When a required value
  is missing and there is a terminal to ask on, ask for it with a themed
  prompt. Never make somebody read `--help` to do the obvious thing. With no
  terminal (a pipe, a CI job, an agent), fail with a message that names the
  flag that would have answered the question.

Both paths come from one place: `src/prompt.rs`. Describe the missing value as
a `Question` (what it is, the flag that answers it, the choices if there is a
fixed set) and call `prompt::ask`. A command that reads from stdin itself is a
command that will eventually hang on a pipe.

A value that a person would otherwise pass every day belongs in the
configuration file as well, so the order is: the flag, then the configuration,
then the question, then the failure that names the flag.

Adding a command means providing both paths in the same change.

## Narrate by default, detail under `--verbose`

If a command may spend noticeable time (reading many files, touching the
network, waiting on a child process), print a short line on stderr before each
phase saying what it is about to do. A person should never wonder whether the
tool is hung. `--verbose` is for detail and debugging, not for basic
reassurance, and the narration stays factual: it is a progress trace, not a
log dump.

## All color is semantic, and the terminal is dark

Every color decision names a role, never an escape code: `heading`, `accent`,
`value`, `muted`, `success`, `warning`, `error`, `info`, `prompt`. They live in
`src/theme.rs`, and that is the only file allowed to know what a role looks
like. Style by meaning (a failure is `error`, a command name is `accent`, a
hint is `muted`), and be sparing: color guides the eye, it does not paint
everything.

Terminals do not reliably report whether they are light or dark, so the palette
assumes **dark** and uses the bright half of the ANSI palette. A dark
foreground on a dark background is unreadable, and a test in `src/theme.rs`
fails if a tone drifts out of that range. Color is emitted only when stderr is
a terminal and `NO_COLOR` is unset, so piped output is always plain text.
