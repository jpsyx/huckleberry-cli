# Releases

Each successful main push normally produces one version, tag, and GitHub release.
The late-run exception is described below.
Contributors push their changes without editing the CLI version themselves.
`Cargo.toml` remains the build version source and `Cargo.lock` moves with it.

The workflow reads all new commit messages in the push. `feat:` (with an
optional scope), a conventional `!` header, or a `BREAKING CHANGE:` /
`BREAKING-CHANGE:` footer increments the minor version. All other pushes,
including documentation and unclassified messages, increment the patch.
Major stays zero. One push containing several commits receives one bump,
with minor taking precedence. The first release starts from the CLI version in that push’s Cargo manifest,
so changes made before automation is enabled cannot cause a version rollback.

The reusable API crate has its own version and is not bumped by CLI releases.
Release development tests use Python 3.9 or newer, with no extra packages:
`python3 -B -m unittest discover -s scripts/release/tests -v` (also `just release-test`).
`just check` includes these alongside Rust format, lint, and tests. Building and
running the app itself still requires only the Rust toolchain.

## What gets published

The workflow makes a version-only commit on the source SHA from the push and
tags it as `v0.MINOR.PATCH`. The annotated tag records the original source and
push range. It publishes a normal GitHub release with generated release notes,
linked commit subjects, and a compare link. GitHub provides source archives;
this workflow does not build installers or publish the API crate.

It then synchronizes the CLI version back to main. If another push advanced
main during validation, only the version fields are applied to that newer
tree. The release tag still contains its original source. Generated version
commits appear in normal pulls, and the workflow's GitHub token prevents them
from triggering another push release.

## Recovery

The source SHA identifies an already reserved release. Rerunning a failed
workflow completes a missing release or main update using the same tag and
version. Tags are never moved and main is never force-pushed. An unreserved run
overtaken by a newer released source reports that it is superseded, including
a rare original run that reaches GitHub’s queue late. Its changes are already
in the newer release; publishing stale code as latest would be misleading. An older reserved release can be recovered without
replacing the latest release or lowering main's version.

Failed validation publishes nothing. Network failures and concurrent main
updates receive bounded retries; persistent failures stay visible in Actions.
An annotated tag with incompatible metadata, or rewritten main history,
requires investigation rather than overwriting published history.

## GitHub Actions

`.github/workflows/release.yml` validates each main push at its exact source
SHA. Format, clippy, Rust tests, and release tests all have to pass. Both jobs
use the same pinned Rust 1.95.0 toolchain, matching local verification, so a
new upstream lint cannot unexpectedly stop otherwise unchanged releases. The entire
workflow enters a concurrency queue before any job allocates a runner. Waiting
runs cannot occupy the runners needed to finish an earlier release. The queue
retains up to GitHub’s 100 pending runs and does not replace earlier pending
pushes. GitHub orders runs by queue entry, not dispatch time, so an unusually
late original run can be superseded as described above.

The publish job uses the repository's built-in `GITHUB_TOKEN` with contents
write permission. No personal token or release approval is needed. Releases
appear at [GitHub Releases](https://github.com/jpsyx/huckleberry-cli/releases).
If a workflow fails, its Actions log identifies the step; rerunning that run
uses the same push identity and safely resumes reserved publication.
