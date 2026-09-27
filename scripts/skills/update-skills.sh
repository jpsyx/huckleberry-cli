#!/usr/bin/env bash
#
# Updates every agent skill this project has, whatever installed it.
#
# Two managers own the skills here and neither knows about the other:
#
# - `npx skills` owns everything in `skills-lock.json`.
# - A handful of skills ship their own CLI and install themselves. They are
#   never in the lock, so they are updated through that CLI instead. The list
#   below was written when this project was scaffolded, from the capability
#   manifest the scaffolder keeps; an empty list is normal.
#
# This script is the one place a skill update happens, so a project updates its
# skills the same way whatever language it is written in.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Space separated npm package names, written by the scaffolder.
SELF_INSTALLING_SKILLS="impeccable"

# Updates impeccable, which keeps a copy per agent frontend plus its hook
# manifests. `update` refreshes an existing install; a clone that has never
# installed it (the skill directories are gitignored) gets the full install.
update_impeccable() {
  if [ -d ".agents/skills/impeccable" ]; then
    npx --yes impeccable update </dev/null
  else
    npx --yes impeccable install --yes --scope=project \
      --providers=claude,codex,cursor,opencode </dev/null
  fi
}

echo "Updating the skills in skills-lock.json..."
npx --yes skills update --project --yes </dev/null

for skill_name in $SELF_INSTALLING_SKILLS; do
  echo "Updating $skill_name..."
  case "$skill_name" in
  impeccable)
    update_impeccable
    ;;
  *)
    echo "No updater is known for '$skill_name'. Update it by hand." >&2
    ;;
  esac
done

echo "Agent skills are up to date."
