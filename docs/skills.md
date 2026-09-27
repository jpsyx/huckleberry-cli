# Agent skills

This project ships a curated set of agent skills so every coding agent working
here starts with the same tooling. This document explains how they get onto
disk and who owns what.

## Two managers, one wrapper

Skills come from two tools that know nothing about each other:

- **`npx skills`** installs everything listed in `skills-lock.json`. It writes
  the real skill directories to `.agents/skills/`, symlinks them into
  `.claude/skills/` for Claude Code, and relies on Cursor, OpenCode, and Codex
  reading `.agents/skills/` natively.
- **`npx impeccable`** installs the `impeccable` skill. It ships its own
  installer and writes a copy per frontend (`.agents`, `.claude`, `.cursor`,
  `.opencode`) plus its hook manifests, so it is never in `skills-lock.json`.

One wrapper and one script cover both, so there is a single place to ask what
is installed and a single place to bring it up to date:

- `npx skills list`: merged listing of every skill, flagging any that are missing.
- `just skills-install`: install the locked skills that are not on disk yet.
- `just skills-update`: update every skill, from both managers.

The first two run this project's own skills tooling under `scripts/skills`.
`just skills-update` runs `scripts/skills/update-skills.sh`, which is a
plain shell script on purpose: updating skills is the one thing every project
does the same way whatever it is written in, so it does not go through the
language's own tooling. The script updates everything in the lock with
`npx skills update`, then updates each self-installing skill through its own
CLI. That list is at the top of the script, written when the project was
scaffolded:

```sh
SELF_INSTALLING_SKILLS="impeccable"
```

Never create the per-frontend symlinks or copies by hand. Each manager owns the
layout its own frontends expect, and hand-made links drift the moment either
tool changes.

## What git tracks

**Only `skills-lock.json`.** The installed directories (`.agents/`,
`.claude/skills/`, `.cursor/skills/`, `.opencode/`, `.codex/`) are gitignored:
together they are around 14MB and 600+ files of vendored content, most of it
impeccable's four copies of itself.

The lock is therefore the manifest of what this project wants, and installing
is what makes the working tree match it:

- `just skills-install` restores them after a clone.
  Installing adds whatever the lock asks for that is not already on disk and
  leaves everything else untouched. A complete project makes no network calls
  at all, so it stays fast.
- Install and update are separate for the same reason a package manager keeps
  them separate: `just skills-install` materializes what is locked without
  upgrading anything, and `just skills-update` is the deliberate "go get
  the latest" step.
- Where the restore runs automatically, `CI=true` or `SKIP_SKILLS_INSTALL=1`
  turns it off. No agent frontend runs in CI, so the network cost there buys
  nothing.

## Changing the skill set

Add or remove skills with the manager, not by editing directories:

```sh
npx skills add <owner>/<repo> --skill <name> --agent claude-code cursor opencode codex
npx skills remove <name>
```

Both commands update `skills-lock.json`. Commit that change: it is what every
other clone and CI checkout installs from.

The lock was not written by hand. The scaffolder that created this project ran
`npx skills add` for each skill its manifest selects for a project of this
shape, and those installs wrote `skills-lock.json`. That is why a fresh project
starts with a curated list rather than an empty one; from here on the lock is
this repository's own, and the commands above are what change it.
