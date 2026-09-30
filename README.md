# Huckleberry CLI

This is a CLI for the incredible baby-tracking app [Huckleberry](https://huckleberrycare.com).
They didn't pay me to say the app is incredible; I'm just a fan.

This CLI is for parents that can never find their phone but always have their computer
close to them. Use this to log your child's sleep, feeds, diapers,
pumping, health, milestones, and see your baby's most recent data instantly.

The CLI UX has been optimized for single-hand usage in the middle of the night when you are
sitting down, baby on your lab, bottle-feeding in one hand, and you only have one
hand to type. Just run `h` and then navigate the interactive menu
with up/down arrows and press Enter to make your selections.

**Huge** props goes to [Woyken](https://github.com/Woyken) for having built the [py-huckleberry-api](https://github.com/Woyken/py-huckleberry-api)
in the first place (see [Credit](#credit)). This repo is a direct port of his repo into Rust. All ported API code is in `crates/huckleberry-api`. This CLI is a wrapper around the Rust API.

_**Ugh, did you vibe code this entire CLI?**_

Yes, I did. Because I have a newborn to take care of. It would be more concerning
if I _didn't_ vibe code this app.

_**I'm not sure I can trust this CLI if it's fully vibe coded**_

I literally do not care, I'm just trying to feed my baby.

_**How can I contribute to this repo?**_

Please don't. I have no time to review your code. This is open source, so just
click the "Fork" button and vibe code
to your heart's content to add any features you want.

_**Can this CLI be operated by agents?**_

This CLI was intended for human babies with human parents using their human fingers to type commands.
The UX design centers around interactive menus and quick inputs, rather than
memorizing long complicated commands.

_However_, the CLI is built such that everything you can do interactively
can also be done through a single command by passing in different options.
This means that, yes, if you wanted to make this even more ridiculously
overengineered, you can have the CLI be driven by your Claude or Codex or
whatever LLM you swear fealty to.

## Getting started

A Rust toolchain is the only prerequisite ([rustup.rs](https://rustup.rs)).

```sh
./install.sh            # builds a release binary into ~/.local/bin
h auth login           # asks for email, password and timezone
h now
```

The command is `h`, short for huckleberry: this is a tool you reach for at 3am
one-handed. `./install.sh --name <something-else>` installs it under another
name, and `BIN_DIR` chooses somewhere other than `~/.local/bin`.

## View your most recent data

```console
$ h now
Ada

Last fed       36m ago · 73 ml of Formula · 8:03 pm
Diaper         1h 31m ago · wet · 7:09 pm
Sleep          28m ago · slept for 1h 40m
Night of Sun 27 Sep   longest 3h 16m · from 2:00 am
```

## Log new activity

```console
$ h diaper
$ h feed
$ h pump
```

Each of these commands will enter an interactive mode from which you submit the activity. No need to remember a bunch of annoying options and flags.

## Log sleep

Start and stop a sleep session:

```console
$ h sleep start
$ h sleep stop
```

Or view the status of your baby's current sleep session:

```
$ h sleep status
```

Or manually enter a sleep session in the past that you may have missed

```
$ h sleep manual
```

## View your data in new and mostly useless ways

````

```console
$ h stripes
Where sleep lands

            00          06          12          18
Mon 21 Sep │██▼◦████▼◦███▼ ◦ ▼ █◦█▼  █◦▼    █◦▼   ◦▼███◦·█▼█│ 13.0h
Tue 22 Sep │███▼█████▼████▼ ◦▼ ██◦▼  █◦▼█  ██◦▼  ◦██▼██◦·█▼█│ 14.1h
Wed 23 Sep │██▼█◦███▼◦███ ▼◦  ▼█◦ ▼ █◦██▼   ◦▼█   ◦▼████◦█▼█│ 13.9h

            █ asleep   ▼ feed   ◦ diaper   · night
````

```console
$ h summary
day          feeds  milk ml  formula ml  breast ml  nursed  sleep  night  longest  wet  dirty
Sun 27 Sep       8      373         223        150     56m   11.3    7.9   2h 36m    7      4
Sat 26 Sep       9      443         299        144   1h 1m   13.1    9.9   2h 38m    8      3

average over 6 complete days: 9.0 feeds · 441 ml milk · 13.8h sleep · 8.0 wet · 3.5 dirty
Wren is 21 days old
  typical at this age: 8 to 12 feeds a day
  typical from day 5: 6 or more wet diapers a day
```

And `h dash` for all of it at once, full screen and live.

## Everything it can do

|                 |                                                                            |
| --------------- | -------------------------------------------------------------------------- |
| **Read**        | `now`, `summary`, `stripes`, `trends`, `log`, `dash`, `export`             |
| **Sleep**       | `sleep start / pause / resume / stop / cancel / status`, `sleep manual`    |
| **Feeding**     | `feed bottle`, `feed nursing start / switch / pause / stop`, `feed solids` |
| **Diapers**     | `diaper`, `potty`                                                          |
| **Corrections** | `edit`, `delete`                                                           |
| **Health**      | `growth`                                                                   |
| **Foods**       | `foods list`, `foods add`                                                  |
| **Setup**       | `auth login / status / logout`, `child list / use / show`, `config`        |

Lists browse and search: `log`, `edit`, `delete` and `foods list` open a
scrollable view on a terminal where `/` filters as you type, and print plain
aligned rows when piped.

[`docs/cli.md`](docs/cli.md) has the full surface;
[`docs/dashboards.md`](docs/dashboards.md) describes each screen and the rules
they follow; [`docs/setup.md`](docs/setup.md) covers first use and the two ways
a family can count a day; [`docs/tui.md`](docs/tui.md) is the design brief for
the shell `h` opens, and for the one hand it is built for;
[`docs/nomenclature.md`](docs/nomenclature.md) is what this project calls its
own parts.

## Your credentials

`config.toml` holds settings and no secret, so it is safe to show somebody. The
email, password and session live in `credentials.toml` beside it, written with
mode `0600`.

`HUCKLEBERRY_EMAIL` and `HUCKLEBERRY_PASSWORD` override the file, and a
password supplied that way is never written to disk:

```sh
export HUCKLEBERRY_PASSWORD="$(pass huckleberry)"
h now
```

`h auth logout` deletes the file.

## Trying it without an account

`export` writes a snapshot and `--offline` reads one back. Nothing under
`--offline` opens a socket, so every read-only screen works from a file:

```sh
h export --out snapshot.json
h --offline snapshot.json dash
```

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

## Credit

**`crates/huckleberry-api` is a Rust port of
[py-huckleberry-api](https://github.com/Woyken/py-huckleberry-api) by
[Woyken](https://github.com/Woyken).**

That project did the hard part. Huckleberry publishes no API and no
documentation for one. Working out that it is a Firebase application, which
collection holds what, what every field is called, which units each one is in,
and what sequence of writes the app expects for each operation is original
research, and all of it is Woyken's. This repository translates that knowledge
into Rust and builds a terminal client on top. It does not originate it.

Where the port deliberately differs from the original,
[`docs/api.md`](docs/api.md) says so and says why. Everything else is a
translation, field for field.

If this is useful to you, the upstream project is the one to star.

## Licence

MIT. See [`LICENSE`](LICENSE).

py-huckleberry-api is also MIT, and its copyright notice travels with this port
in [`NOTICE`](NOTICE), as that licence requires.

Huckleberry is a product of Huckleberry Labs, Inc. This project is an
unofficial client, is not affiliated with or endorsed by them, and uses their
name only to say what it talks to.
