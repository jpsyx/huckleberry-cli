# Huckleberry CLI

This is a CLI for the incredible baby-tracking app [Huckleberry](https://huckleberrycare.com).
They didn't pay me to say the app is incredible; I'm just a fan.

This CLI is for parents that can never find their phone but always have their computer
close to them. Use this to log your child's sleep, feeds, diapers,
pumping, health, milestones, and see your baby's most recent data instantly.

The CLI UX has been optimized for single-hand usage in the middle of the night when you are
sitting down, baby on your lap, bottle-feeding in one hand, and you only have one
hand to type. Just run `h` and then navigate the interactive menu
with the arrow keys, hjkl, or WASD and press Enter to make your selections.

**Huge** props goes to [Woyken](https://github.com/Woyken) for having built the [py-huckleberry-api](https://github.com/Woyken/py-huckleberry-api)
in the first place (see [Credit](#credit)). This repo is a direct port of his repo into Rust. All ported API code is in `crates/huckleberry-api`.
The CLI is a wrapper around the ported Rust API.

_**Ugh, did you vibe code this entire CLI?**_

Yes, I did. Because I have a newborn to take care of. It would be more concerning
if I _didn't_ vibe code this app.

_**I'm not sure I can trust this CLI if it's fully vibe coded**_

I truly do not care, I'm just trying to feed my baby.

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
overengineered, the CLI can be driven by your Claude or Codex or
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

Last fed            36m ago · 73 ml of Formula · 8:03 pm
Diaper              1h 31m ago · wet · 7:09 pm
Sleep               28m ago · slept for 1h 40m
Last night's sleep  longest 3h 16m · from 2:00 am
Sleep in last 4h    1h 40m total · 1 sleep 28m ago
Sleep today         11h 18m total
Fed in last 4h      146 ml total · 2 feeds
                    fed 36m ago and 3h 4m ago
Fed today           430 ml total · 6 feeds · since 6:00 am
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

`h summary` opens a day table grouped into feed, sleep, diaper and wake time.
Tab/Shift-Tab or left/right (H/L, A/D) selects a numeric column and draws its
line graph underneath. Up/down scrolls the rows and notes; Esc or Q leaves.
Milk/feed averages only bottles with recorded amounts, and nurse/feed averages
only nursing sessions. The night note shows the configured hours and the commands
that change them. The same view is in the shell under Visualizations > Summary.

```sh
h summary --days 7
h summary --days 30 --json > month.json
```

Piped output stays a plain table. [Screen details](docs/dashboards.md#summary)
explain the nap and wake calculations, missing values, and existing age-aware
reference ranges.

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

## About the typical ranges, and a disclaimer

This tool prints "typical ranges" next to your baby's figures: how much sleep,
how many feeds, how many diapers are usual at a given age. Please read this
before you rely on any of them.

**Those ranges were researched by a large language model.** They were not
written, reviewed, or approved by a doctor. The research was done carefully and
adversarially: every number is traced to a named source, the sources were
tiered so that advocacy groups and manufacturers could not stand in for
professional bodies, independent reviewers were sent to try to disprove each
number, and the whole method and its findings are written down in
[`docs/research/`](docs/research/) so you can check the work rather than take
it on faith. The ranges themselves live in
[`data/reference.toml`](data/reference.toml), one band per age, each naming
where it came from.

That makes the work **checkable. It does not make it authoritative.** An LLM
can misread a study, cite a body that said something subtly different, or miss
the guideline that supersedes the one it found. Treat nothing here as fact.

**Your pediatrician knows your baby. This file does not.** These are general
ranges describing populations, and no range describes any particular child.
Where this tool and your pediatrician disagree, your pediatrician is right and
this tool is wrong. If something about your baby worries you, call them, and do
not let a green number on a terminal talk you out of it.

This software is not a medical device, and nothing it prints is medical advice,
a diagnosis, or a reason to delay care.
