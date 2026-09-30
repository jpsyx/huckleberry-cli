# CLI experience rules

This is a tool people type at, and a tool agents script. Both are first-class,
and the rules below are what keeps them from pulling the design apart.

## Stdout is data, stderr is the conversation

Stdout carries only intentional output: the answer the command was asked for,
in a shape something else can consume (`key=value` lines, a path, a JSON
document on a pipe; a readable table on a terminal). Progress, prompts, warnings,
failures, and the argument parser's own errors go to stderr. Anyone may pipe this tool into
another, so a diagnostic on stdout is a bug, and so is a decorated table on a
pipe where a machine expected a value.
Terminal receipts and status tables use friendly labels, dates with timezones,
and durations with units. JSON and export remain machine-readable even on a
terminal.

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
`value`, `muted`, `success`, `warning`, `error`, `info`, `prompt`, `today`,
`good`, `attention`. They live in `src/theme.rs`, and that is the only file
allowed to know what a role looks like. Style by meaning (a failure is `error`,
a command name is `accent`, a hint is `muted`), and be sparing: color guides
the eye, it does not paint everything.

Three roles carry rules of their own, because they are about somebody's baby
rather than about the tool:

- **`today` is brighter, and the other days are not dimmer.** Wherever several
  days appear, today is the row being looked for. Brighten it; leave the rest
  fully legible.
- **`attention` is yellow and is never red.** It means a figure sits outside
  what is typical for this age, which is worth a second look and is not an
  emergency. `error` and `warning` stay for the tool's own problems, and a test
  fails if a screen about a baby paints with either.
- **Color never carries a meaning on its own.** Say it in words too, so it
  survives a pipe, a screenshot, and colour blindness.

Terminals do not reliably report whether they are light or dark, so the palette
assumes **dark** and uses the bright half of the ANSI palette. A dark
foreground on a dark background is unreadable, and a test in `src/theme.rs`
fails if a tone drifts out of that range. Each stream emits colour only when that stream is a terminal and `NO_COLOR` is
unset. The stdout palette is independent of the prompt palette on stderr, so
piped output is always plain text. Small emoji headings accompany terminal
receipts; meaning remains explicit in the accompanying words.

Fixed-choice prompts are numbered menus: `↓`/`j` move down and `↑`/`k` move up,
and `←`/`h` backs out. **`j` and `k` are the only letters that move**, in every
menu, list and screen in this tool: a letter that moves the cursor in one place
and leaves in another is a letter nobody can press without looking. 1-9
highlight the corresponding item in every menu. Enter accepts the
highlighted item. Zero and numbers beyond the menu's length are ignored; items
10 and later remain reachable with navigation keys. Optional choices include Skip.
Optional text offers Skip or Enter text, and existing values can be kept or
cleared. Esc and Ctrl-C cancel the current question without accepting it.
