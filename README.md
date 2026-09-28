# huckleberry-cli

A terminal client and dashboard for [Huckleberry](https://huckleberrycare.com),
the baby tracking app. Read a child's sleep, feeds, nappies, pumping, health
and milestones, log new entries, and see the whole week at a glance without
opening your phone.

Two packages in one repository:

- **[`crates/huckleberry-api`](crates/huckleberry-api)**, a reusable Rust
  client for Huckleberry. It depends on nothing else here and is publishable on
  its own.
- **`src/`**, the command-line tool that consumes it.

Both are MIT licensed.

## The screen this exists for

```console
$ hb now
Wren

Last fed       36m ago · 73 ml of Formula · 8:03 pm
Nappy          1h 31m ago · wet · 7:09 pm
Sleep          asleep 40m
Tonight        nothing finished yet

as of 2m ago
```

```console
$ hb stripes
Where sleep lands

            00          06          12          18
Mon 21 Sep │██▼◦████▼◦███▼ ◦ ▼ █◦█▼  █◦▼    █◦▼   ◦▼███◦·█▼█│ 13.0h
Tue 22 Sep │███▼█████▼████▼ ◦▼ ██◦▼  █◦▼█  ██◦▼  ◦██▼██◦·█▼█│ 14.1h
Wed 23 Sep │██▼█◦███▼◦███ ▼◦  ▼█◦ ▼ █◦██▼   ◦▼█   ◦▼████◦█▼█│ 13.9h

            █ asleep   ▼ feed   ◦ nappy   · night
```

```console
$ hb summary
day          feeds  milk ml  formula ml  breast ml  nursed  sleep  night  longest  wet  dirty
Sun 27 Sep       8      373         223        150     56m   11.3    7.9   2h 36m    7      4
Sat 26 Sep       9      443         299        144   1h 1m   13.1    9.9   2h 38m    8      3

average over 6 complete days: 9.0 feeds · 441 ml milk · 13.8h sleep · 8.0 wet · 3.5 dirty
Wren is 21 days old
  typical at this age: 8 to 12 feeds a day
  typical from day 5: 6 or more wet nappies a day
```

And `hb dash` for all of it at once, full screen and live.

## Getting started

A Rust toolchain is the only prerequisite ([rustup.rs](https://rustup.rs)).

```sh
./install.sh            # builds a release binary into ~/.local/bin
hb auth login           # asks for email, password and timezone
hb now
```

The command is `hb`, short for huckleberry: this is a tool you reach for at 3am
one-handed. `./install.sh --name <something-else>` installs it under another
name, and `BIN_DIR` chooses somewhere other than `~/.local/bin`.

## What it can do

| | |
| --- | --- |
| **Read** | `now`, `summary`, `stripes`, `trends`, `log`, `dash`, `export` |
| **Sleep** | `sleep start / pause / resume / stop / cancel / status` |
| **Feeding** | `feed bottle`, `feed nursing start / switch / pause / stop`, `feed solids` |
| **Nappies** | `diaper`, `potty` |
| **Health** | `growth` |
| **Foods** | `foods list`, `foods add` |
| **Setup** | `auth login / status / logout`, `child list / use / show`, `config` |

[`docs/cli.md`](docs/cli.md) has the full surface;
[`docs/dashboards.md`](docs/dashboards.md) describes each screen and the rules
they follow.

## Two audiences, both first class

**A person who omits a value is asked for it.** `hb feed bottle`
asks how much, offering the last amount as the default, and what was in it,
offering the last kind. `hb diaper` asks what was in it. Nobody
has to read `--help` to do the obvious thing.

**An agent or a script drives everything with flags.** No action is reachable
only by answering a prompt, and no command blocks on input it was given. With
stdin redirected, a missing value is a failure naming the flag rather than a
prompt nobody will see. Stdout is data and stderr is the conversation, so
piping anywhere yields plain text.

```sh
hb now --json | jq -r '.last_feed.ago_seconds'
hb feed bottle --amount 90 --type formula
hb summary --days 30 --json > month.json
```

## Your credentials

`config.toml` holds settings and no secret, so it is safe to show somebody. The
email, password and session live in `credentials.toml` beside it, written with
mode `0600`.

`HUCKLEBERRY_EMAIL` and `HUCKLEBERRY_PASSWORD` override the file, and a
password supplied that way is never written to disk:

```sh
export HUCKLEBERRY_PASSWORD="$(pass huckleberry)"
hb now
```

`hb auth logout` deletes the file.

## Trying it without an account

`export` writes a snapshot and `--offline` reads one back. Nothing under
`--offline` opens a socket, so every read-only screen works from a file:

```sh
hb export --out snapshot.json
hb --offline snapshot.json dash
```

## Using the client from your own code

```toml
[dependencies]
huckleberry-api = { path = "crates/huckleberry-api" }
```

```rust,no_run
use huckleberry_api::{Credentials, Huckleberry, Window, client::now_seconds};

# async fn example() -> Result<(), huckleberry_api::Error> {
let client = Huckleberry::new(
    Credentials::new("parent@example.com", "hunter2"),
    "America/New_York",
)?;
let user = client.user().await?;
let child = user.first_child().expect("a child on the account");

let week = Window::last_days(now_seconds(), 7);
println!("{} feeds", client.feed_intervals(&child.cid, week).await?.len());
# Ok(())
# }
```

See [`crates/huckleberry-api/README.md`](crates/huckleberry-api/README.md) and
[`docs/api.md`](docs/api.md), which maps every method to its Python original
and records where the two deliberately differ.

## Development

`just` is optional (`cargo install just`): every recipe is one command you can
also type by hand.

```sh
cargo test                                    # just test
cargo clippy --all-targets -- -D warnings     # just lint
cargo fmt                                     # just fmt
just check                                    # everything that has to pass
```

Conventions live in [`AGENTS.md`](AGENTS.md) and
[`docs/rules/`](docs/rules). Architecture is in
[`docs/architecture.md`](docs/architecture.md) and the decisions behind it in
[`docs/decisions.md`](docs/decisions.md).

## Agent skills

Agent skills are not tracked in git, but `skills-lock.json` is.
`just skills-install` restores them after a clone. See
[`docs/skills.md`](docs/skills.md).

## Licence

MIT. See [`LICENSE`](LICENSE).

Huckleberry is a trademark of its owners; this project is not affiliated with
or endorsed by them.
