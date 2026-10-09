# huckleberry-api

An async Rust client for [Huckleberry](https://huckleberrycare.com), the baby
tracking app. Read a child's sleep, feeds, diapers, pumping, health entries and
milestones, and write new entries back.

> ### Credit
>
> **This crate is a Rust port of
> [py-huckleberry-api](https://github.com/Woyken/py-huckleberry-api) by
> [Woyken](https://github.com/Woyken), and it exists because that project
> exists.**
>
> Huckleberry publishes no API and no documentation for one. Working out that
> it is a Firebase application, which collection holds what, what every field
> is called, which units each one is in, and what sequence of writes the app
> expects for each operation is original research, and all of it is Woyken's.
> This crate translates that knowledge into Rust. It does not originate it.
>
> py-huckleberry-api is MIT licensed, and its copyright notice travels with
> this port in [NOTICE](NOTICE). If this crate is useful to you, the upstream
> project is the one to star.

The supported operations retain the shapes the app itself writes. Pump
logging and timers are ported from upstream commit
`6d273804e993243e876d3d6fb109d8a4f0082750`; `docs/api.md` in the repository
records the method mapping and deliberate differences.

```toml
[dependencies]
huckleberry-api = "0.6"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust,no_run
use huckleberry_api::{Credentials, Huckleberry, Window, client::now_seconds};

#[tokio::main]
async fn main() -> Result<(), huckleberry_api::Error> {
    let client = Huckleberry::new(
        Credentials::new("parent@example.com", "hunter2"),
        "America/New_York",
    )?;

    let user = client.user().await?;
    let child = user.first_child().expect("a child on the account");

    let week = Window::last_days(now_seconds(), 7);
    for feed in client.feed_intervals(&child.cid, week).await? {
        println!("{}", feed.start());
    }

    client.start_sleep(&child.cid).await?;
    Ok(())
}
```

## What it does

| Area | Methods |
| --- | --- |
| Account | `user`, `child`, `authenticate`, `session`, `sign_out` |
| Sleep | `start_sleep`, `pause_sleep`, `resume_sleep`, `cancel_sleep`, `complete_sleep`, `sleep_document`, `sleep_intervals` |
| Nursing | `start_nursing`, `pause_nursing`, `resume_nursing`, `switch_nursing_side`, `cancel_nursing`, `complete_nursing` |
| Bottles | `log_bottle`, `feed_intervals`, `feed_document` |
| Solids | `curated_foods`, `custom_foods`, `create_custom_food`, `log_solids` |
| Diapers | `log_diaper`, `log_potty`, `diaper_intervals`, `diaper_document` |
| Health | `log_growth`, `latest_growth`, `health_entries`, `health_document` |
| Pumping | `log_pump`, `log_pump_at`, `start_pump`, `pause_pump`, `resume_pump`, `cancel_pump`, `complete_pump`, `pump_document`, `latest_pump`, `pump_intervals`, `update_pump_entry` |
| Milestones | `milestones` |
| Anything else | `collection_rows` |
| Watching | `watch_sleep`, `watch_feed`, `watch_diaper`, `watch_health`, `watch_pump` |

Pump start, pause, resume and completion also have `_at` variants taking a
Unix timestamp in seconds. `PumpEntry` carries amounts as either
`PumpAmounts::Total` or `PumpAmounts::LeftRight`, volume units, an optional
duration in seconds and an optional note. Total amounts are stored as equal
halves in the two side fields, as researched by Woyken. `PumpTimer::started_at`
and `elapsed_seconds` convert the timer's millisecond fields for callers.

## How it talks to Huckleberry

Huckleberry is a Firebase application. Signing in is Firebase Identity
Toolkit; the data is Cloud Firestore, reached here over its **REST** API rather
than gRPC. That keeps the dependency tree to an HTTP client and serde.

The cost is real-time listeners, which REST does not offer: the Python client's
`setup_*_listener` becomes polling here (`watch_*`). Everything else is
feature-complete.

## Three things that catch people out

1. **The sleep timer is in milliseconds, the feed timer is in seconds.**
   `SleepTimer::started_at` and `FeedTimer::totals` do the conversion.
2. **`active: false` is not a paused session.** A finished feed is left as
   `{ active: false, paused: true }`. Use `FeedDocument::running_timer`.
3. **A windowed read is two queries.** Older rows are packed into batched
   documents Firestore cannot filter inside, so both are fetched and merged.
   That is handled for you; it is written down because it explains the read
   count.

## Safety

`unsafe_code` is forbidden. Every write this crate performs is one the app
performs too, in the same shape; nothing here deletes a child, an account, or
history.

## Licence

MIT, Copyright (c) 2026 Juan Pablo Sarmiento.

Derived from py-huckleberry-api, MIT, Copyright (c) 2025 Woyken. The full
upstream notice is in [NOTICE](NOTICE).

Huckleberry is a product of Huckleberry Labs, Inc. This crate is an unofficial
client and is not affiliated with or endorsed by them.
