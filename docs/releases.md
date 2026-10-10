# Releases

Every successful push to main produces one version, tag, and GitHub release.
Contributors push their changes without editing the CLI version themselves.
`Cargo.toml` remains the build version source and `Cargo.lock` moves with it.

The workflow reads all new commit messages in the push. `feat:` (with an
optional scope), a conventional `!` header, or a `BREAKING CHANGE:` /
`BREAKING-CHANGE:` footer increments the minor version. All other pushes,
including documentation and unclassified messages, increment the patch.
Major stays zero. One push containing several commits receives one bump,
with minor taking precedence. The initial baseline is 0.47.2.

The reusable API crate has its own version and is not bumped by CLI releases.
Release development tests use Python 3.9 or newer, with no extra packages:
`python3 -B -m unittest discover -s scripts/release/tests -v` (also `just release-test`).
`just check` includes these alongside Rust format, lint, and tests. Building and
running the app itself still requires only the Rust toolchain.
