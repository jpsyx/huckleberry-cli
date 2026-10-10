# Automatic releases and the Version modal

## Intent and approved direction

Pushing to `main` should be the only release action a contributor performs.
The repository chooses a version, records a matching tag, and publishes a
GitHub release with a useful description. Versions remain below 1.0.
The command line and shell show the installed version, and the shell can
compare it with the latest GitHub release without blocking interaction.

The approved approach is a repository-owned GitHub Actions workflow with
testable release helpers, conventional-commit version selection, and a
centered Version modal. A release-management framework was considered, but
the repository-owned workflow directly expresses the every-push and pre-1.0
rules without another release configuration system.

## Starting point

- The CLI package and installed binary are `0.47.2` at main commit
  `9406a8fc512f2e202573526818fadd53137d9720`.
- `Cargo.toml` supplies the compiled version; `Cargo.lock` records it too.
- The independent `huckleberry-api` crate is `0.6.0`.
- There are no existing Git tags, GitHub releases, or Actions workflows.
- `-v` currently enables verbose diagnostics. `--version` and `-V` are
  clap's existing version flags.
- More > Version currently places the version in the shell footer.
- The repository is public, Actions can receive explicit write permissions,
  and main currently has no branch protection preventing a bot version commit.

## Version policy

One ordinary push to main produces one release after its checks pass, even
when it contains only documentation, tests, or internal maintenance. A push
containing multiple commits still produces one release.

Inspect the full messages of commits introduced by the push, using its Git
`before..after` range rather than the possibly truncated event commit list.
For merges, include the newly reachable commits. Generated version commits
are excluded from classification and descriptions.

| New commit messages | Bump | Example from 0.47.2 |
| --- | --- | --- |
| Any `feat:` or `feat(scope):` | minor | 0.48.0 |
| Any conventional `!` breaking-change marker, or `BREAKING CHANGE:` / `BREAKING-CHANGE:` footer | minor | 0.48.0 |
| Everything else, including unclassified messages and empty commits | patch | 0.47.3 |

Minor takes precedence over patch. Major remains zero. Automatic classification
uses commit messages; contributors do not edit version files or select a
release version. Ordinary messages still release through the patch fallback.
Changing the major-version policy is a future explicit repository change.

The initial baseline is the existing CLI version in the triggering source
manifest (`0.47.2` when this design was written); historical commits
before the first triggering push do not create retrospective releases. With a
`feat:` commit for this feature and no intervening releases, the first release
would be `v0.48.0` from that baseline. Main advanced to `0.48.1` during
implementation, so the expected first release is now `v0.49.0`. After bootstrap, release tags are the allocation ledger,
including a reserved tag whose GitHub release still needs publication.

Only the CLI package version and its matching lockfile package entry are
updated. The API crate keeps its independent version. The current manual-bump
rule is replaced as part of implementation; preparatory design commits do not
manually increment the baseline.

## Release workflow

### Trigger, validation, and authority

The workflow runs on every push to main with no path filters. It uses the
repository's generated token with explicit `contents: write` and Actions read
access for coordination. No personal access token or contributor-managed
release secret is required. GitHub documents that pushes made with this token
do not recursively start push workflows, which prevents the generated version
commit from causing another release. [Token behavior](https://docs.github.com/en/actions/concepts/security/github_token)

Validate the triggering source with formatting, clippy, the full Rust test
suite, and the release-helper tests. Failed checks prevent a new tag or
release. A workflow failure is visible in Actions and can be retried; routine
successful pushes need no approval or manual release step.

### Publication and source identity

1. Retain the event's before/after SHAs as the push identity and description
   range. Check out the exact after SHA with history available.
2. Wait for older unfinished main-push runs before entering the publication
   queue. Perform this wait outside the publication concurrency group so an
   earlier run cannot be trapped behind the run waiting for it.
3. Serialize publication with a dedicated concurrency group and `queue: max`.
   The default single-pending queue cancels intervening runs; it is unsuitable
   here. GitHub's queue has a 100-pending-run limit and orders waiting time,
   so predecessor coordination remains necessary. [Concurrency behavior](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency)
4. Inside the publication lock, fetch current release tags, resolve any
   existing reservation for this push, and choose the next version once.
5. Produce a version-only commit whose parent is the triggering source SHA.
   Validate that its diff changes only the CLI version in the two Cargo files,
   and verify its compiled version. An annotated `v0.MINOR.PATCH` tag points
   at that commit and records the source SHA, before SHA, and workflow run ID.
6. Push the tag and publish its GitHub release. Use GitHub-generated notes,
   supplemented with linked subjects for the exact push's non-generated
   commits so direct pushes also have useful descriptions. Record the source
   SHA and compare link. Publish as a normal release, even though major is 0.
   [Release API](https://docs.github.com/en/rest/releases/releases)
7. Synchronize the published CLI version back to main. When main still equals
   the triggering source, fast-forward it to the version commit. If another
   push has advanced main, create a separate version-only commit on current
   main, preserving all of its source changes. Retry ordinary non-fast-forward
   races by refetching and reapplying only the version fields. Never force-push.

The tag therefore always builds the source from its own push with the correct
version, even if main moves during validation. In that race the tag's version
commit can be outside main's ancestry; the metadata records its source, and
main receives the same version fields without importing the older source tree.
The bot's source updates are expected to appear in normal pulls.

### Recovery

Use the triggering source SHA as the durable idempotency key. A retry that
finds its tag verifies the tag metadata and finishes the missing release or
main synchronization instead of allocating another version. Existing tags
are never moved. A mismatched reservation is an error, not permission to
overwrite someone else's tag.

Only complete earlier runs cease blocking publication. If an earlier run
failed before reserving a version and a newer source has since released, an
old retry reports that it was superseded instead of publishing stale code as
the newest version. A retry of an already reserved older version may complete
its missing release, but must not change GitHub's latest pointer or lower
main's version. Partial publication remains visible as a failed workflow
until recovery succeeds. Transient network and main-update races receive
bounded retries; permanent errors do not silently count as success.

Force-pushing or recreating main is outside the ordinary release contract.
The helper detects a rewritten source history and fails with an explanation
instead of inventing a range or rewriting published history.

## CLI version flags

`h -v`, `h -V`, and `h --version` print the same existing version line and
exit successfully before setup or network activity. Verbose diagnostics remain
available through `--verbose` and `HUCKLEBERRY_VERBOSE`. Update help, tests,
and documentation that currently describe `-v` as verbose.

The repository requires every shell capability to be scriptable. Expose the
same optional GitHub check through `h info --check-update`, using the same
client and comparison logic as the modal. Plain `h info` keeps its existing
output and performs no network request. With the flag, append `latest_version`
(empty when unknown) and `update_status` to its existing machine facts, and
corresponding human labels on a terminal. Status values are `up_to_date`,
`update_available`, `ahead`, `offline`, `no_release`, and `unavailable`.
A failed request reports a concise diagnostic and exits unsuccessfully;
offline and no-release results are successfully reported states. This check
needs no Huckleberry login or child selection.

## Version modal

Choosing More > Version opens a bordered modal centered in the terminal,
drawn above the existing shell. It shows the installed CLI version immediately
and retains the underlying menu and cursor when closed. Its geometry is clamped
to the current terminal size; resize redraws it without a panic.

The modal uses the existing semantic theme roles and contains a visible Close
action selected by default. Enter, Space, right/open aliases, Back/Escape,
left/back aliases, and Ctrl-C close it. `q` and Ctrl-Q retain the shell's quit
semantics. `r` retries the version check. Other navigation keys cannot move
the menu behind the modal. Its help line shows close, retry, and quit keys.

| State | Visible information |
| --- | --- |
| Checking | Installed version; `Checking GitHub...` |
| Installed equals latest | Installed and latest versions; `Up to date` |
| Installed is older | Installed and latest versions; `Update available` |
| Installed is newer | Installed and latest versions; `Ahead of the latest release` |
| Offline mode | Installed version; `Offline: update check disabled` |
| No published release | Installed version; `No published release found` |
| Network, rate-limit, or malformed-response error | Installed version; `Unable to check GitHub` with a concise reason |

Opening the modal starts one background request to
`https://api.github.com/repos/jpsyx/huckleberry-cli/releases/latest`, with an
application User-Agent and a five-second total timeout. The public request
uses no Huckleberry credentials and sends no child or tracking data. Reopening
starts a fresh check; repeated retry keys do not create concurrent requests.
Closing cancels its pending task, and stale results cannot update a later
modal instance. `--offline` performs no request, including when retry is used.

Parse the returned tag as a semantic version with an optional leading `v`.
Compare numerically, including standard prerelease precedence and ignoring
build metadata. Invalid data never produces an up-to-date claim. The modal
checks GitHub's latest published release rather than the newest arbitrary tag.
It displays the published version even if the repository eventually leaves
major zero.

## Component boundaries

- Repository release helpers own version classification, tag metadata,
  manifest updates, and repeat-run decisions; thin orchestration owns Git,
  GitHub, and subprocess execution. Use standard-library Python helpers for
  workflow scripting and tests, keeping the Rust app free of release tooling.
- A CLI-side version module owns pure semantic-version comparison and a thin
  GitHub client. Reuse the existing reqwest dependency and use the established
  semver crate for comparison, with dependency rationale documented.
- Shell state owns modal visibility and the explicit check result. Drawing
  only consumes that state. The event loop starts, polls, and cancels the
  background task, following the existing reading-task pattern.
- The API crate, baby-domain arithmetic, and Now renderer are unchanged.

## Verification and documentation

Use red/green tests for version classification across multi-commit pushes,
scoped features, breaking markers, fallback messages, and the major-zero rule.
Temporary Git repositories exercise bootstrap, exact source tagging, main
advancing during a release, retries after tag/release publication, and recovery
of an older reserved release without making it latest. GitHub interactions use
test doubles locally; tests must never publish a real release.

Rust tests cover all version flag aliases, retained verbose handling, the
optional scriptable info check, semantic comparison boundaries, HTTP failures
against a local stub, offline behavior,
modal input isolation, closing and reopening, and centered/clamped drawing.
Verify a slow check does not block the shell. Run the full format, lint, and
test checks before landing.

Update `docs/cli.md`, `docs/tui.md`, `docs/nomenclature.md`,
`docs/architecture.md`, `docs/rules/rust.md`, and release documentation alongside
the implementation. Explain the commit-message bump policy and the automatic
version commits. Release assets and automatic installation are outside this
request; the release includes GitHub's normal source archives.

After the authorized push to main, verify the actual workflow, generated tag,
release notes, synchronized Cargo version, and GitHub latest endpoint. Pull the
bot version commit before cleaning up the merged development worktree. A
local passing test suite alone does not establish that release automation works.
