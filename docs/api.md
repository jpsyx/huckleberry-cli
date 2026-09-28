# The Huckleberry API, in Rust

`crates/huckleberry-api` is a port of
[`py-huckleberry-api`](https://github.com/Woyken/py-huckleberry-api). It covers
everything the Python client covers, in the same shapes, and it is a package in
its own right: nothing in it mentions this command-line tool.

## How Huckleberry works

Huckleberry is a Firebase application called `simpleintervals`.

- **Signing in** is Firebase Identity Toolkit: an email and password are
  exchanged for an ID token (one hour), a refresh token, and the user's uid.
- **The data** is Cloud Firestore, one document per tracker per child, with the
  history in a subcollection underneath.
- **The curated food database** is not Firestore at all. It is a single JSON
  object in Firebase Storage, keyed by food id, which the app downloads whole.

The Firebase web API key is a public project identifier rather than a secret;
every request still carries a signed-in user's token.

### The collections

| Path | Holds |
| --- | --- |
| `users/{uid}` | the account, and the children on it |
| `childs/{cid}` | the profile, including the two hours that define a night |
| `sleep/{cid}` | the running timer, and the last sleep |
| `sleep/{cid}/intervals` | sleep history |
| `feed/{cid}` | the nursing timer, and the last feed of each kind |
| `feed/{cid}/intervals` | nursing, bottles and solids, in one collection |
| `diaper/{cid}` | the last nappy and the last potty trip |
| `diaper/{cid}/intervals` | nappies and potty trips, in one collection |
| `health/{cid}` | the last growth, medication and temperature entries |
| `health/{cid}/data` | health history. **`data`, not `intervals`** |
| `pump/{cid}/intervals` | pumping sessions |
| `milestones/{cid}/intervals` | the firsts |
| `types/{cid}/custom` | the family's own solids foods |

## Why REST rather than gRPC

The Python client uses `google-cloud-firestore`, which speaks gRPC and expects
Google credentials. No Rust crate takes a bare Firebase ID token, and pulling a
gRPC stack in to make six kinds of request would be the largest thing in the
dependency tree by a wide margin. So this crate speaks the Firestore REST API,
which wants nothing but an HTTP client and a bearer token.

Everything is one of five requests: `GET` a document, `POST … :runQuery`, `GET`
a collection page, `PATCH` a document, and `PATCH` with an `updateMask`.

## The four deliberate differences

Everything else is a faithful port. These four are choices, not omissions.

### 1. Listeners become polling

`setup_sleep_listener` and its three siblings use Firestore's `Listen`, a
bidirectional gRPC stream with no REST equivalent. `ops::watch` replaces them
with polling: read the document, hand it to the caller when it differs, wait,
repeat.

A listener is pushed within a second of a write; a poll is late by up to its
interval and costs one read per interval per watcher. For a dashboard
refreshing every few seconds that trade is fine, and it is the only one
available without a gRPC stack.

### 2. A field this crate cannot read becomes absent, not fatal

The rule, in one line: **a required field must be right, and an optional field
this crate cannot read becomes absent rather than fatal.**

Two mechanisms enforce it.

*Unknown enum values are carried.* The Python models use `Literal[...]`, which
is strict: a poo colour Huckleberry adds next year makes the whole read fail
validation. Here every such type has an `Unknown(String)` variant, so an
unfamiliar value round-trips unchanged. `FromStr` is the strict door, and it is
the one a command-line argument comes through.

*Optional fields of a surprising type degrade to `None`.* Every `Option<T>`
field on every model deserializes through `models::lenient`, which tries the
type and gives up on the field rather than on the document.

This is not defensive programming for its own sake. The schema is
Huckleberry's, and this crate shipped with `subscription.free_trial_expiration`
typed as text where the app stores a timestamp. The consequence, before this
rule existed, was that an account with a trial on it could not be read at all:
one field nothing in this crate looks at, and `user()` returned
`invalid type: integer 1789657330, expected a string`.

Required fields stay strict on purpose. A sleep with no `start` is not a sleep,
and keeping it would be worse than dropping it.

### 3. Failures travel rather than being logged

The Python `list_*` methods catch their exceptions, log them, and return
whatever they had. This crate returns `Result` and lets the caller decide;
`Error::is_permission_denied` is there so a caller pulling many collections can
carry on past the ones this account cannot read. `src/dataset.rs` in the CLI is
that caller, and it records each refusal beside the data.

A row that does not *deserialize* is still skipped rather than fatal, for the
same reason as (2). `collection_rows` returns everything untyped for a caller
who wants to see what was dropped.

### 4. Reads are windowed, merged and sorted

`list_sleep_intervals` and friends become `sleep_intervals(cid, window)`.
They still make the same two queries the Python client makes, and for the same
structural reason: when a subcollection gets long, Huckleberry packs older rows
into a single document with `multi: true` and the rows nested under `data`.
Firestore indexes fields and not map entries, so those nested rows cannot be
filtered by a `where` clause. The loose rows are filtered properly and the
batches are fetched whole and filtered in memory.

The two results are merged and sorted by `start`. The Python client returns
them in query order, which is neither.

## Name mapping

The models drop the `Firebase` prefix, because the crate name is the namespace.

| Python | Rust |
| --- | --- |
| `HuckleberryAPI` | `Huckleberry` |
| `FirebaseUserDocument` | `models::user::UserDocument` |
| `FirebaseChildDocument` | `models::child::ChildDocument` |
| `FirebaseSleepDocumentData` | `models::sleep::SleepDocument` |
| `FirebaseSleepIntervalData` | `models::sleep::SleepInterval` |
| `FirebaseSleepTimerData` | `models::sleep::SleepTimer` |
| `FirebaseFeedIntervalData` | `models::feed::FeedInterval` |
| `FirebaseBottleFeedIntervalData` | `models::feed::BottleFeedInterval` |
| `FirebaseBreastFeedIntervalData` | `models::feed::BreastFeedInterval` |
| `FirebaseSolidsFeedIntervalData` | `models::feed::SolidsFeedInterval` |
| `FirebaseDiaperData` | `models::diaper::DiaperEntry` |
| `HealthDataEntry` | `models::health::HealthEntry` |
| `FirebaseGrowthData` | `models::health::GrowthEntry` |
| `FirebasePumpIntervalData` | `models::pump::PumpInterval` |
| `FirebaseCustomFoodTypeDocument` | `models::solids::CustomFood` |
| `FirebaseCuratedFoodDocument` | `models::solids::CuratedFood` |
| `SolidsFoodReference` | `models::solids::FoodReference` |
| `Firebase*MultiContainer` | `models::common::MultiContainer<T>` |

| Python method | Rust method |
| --- | --- |
| `authenticate` | `authenticate` |
| `refresh_session_token` | automatic; `auth::refresh` to force it |
| `get_user` | `user` |
| `get_child` | `child` |
| `get_latest_growth` | `latest_growth` |
| `list_sleep_intervals` | `sleep_intervals` |
| `list_feed_intervals` | `feed_intervals` |
| `list_diaper_intervals` | `diaper_intervals` |
| `list_health_entries` | `health_entries` |
| `start_sleep` … `complete_sleep` | the same names |
| `start_nursing` … `complete_nursing` | the same names |
| `log_bottle`, `log_diaper`, `log_potty`, `log_growth`, `log_solids` | the same names |
| `list_solids_curated_foods` | `curated_foods` |
| `list_solids_custom_foods` | `custom_foods` |
| `create_solids_custom_food` | `create_custom_food` |
| `setup_*_listener` | `watch_*` (polling; see above) |
| — | `pump_intervals`, `milestones`, `collection_rows` (new) |

## Four things that catch people out

These are the ones that cost real time, and each has a method whose whole job
is to stop you having to remember it.

1. **The sleep timer's `timerStartTime` is in milliseconds. The feed timer's is
   in seconds.** Read one as the other and the baby has been asleep since 1970.
   `SleepTimer::started_at` and `FeedTimer::totals` do the conversion.

2. **`active: false` is not a paused session.** A completed nursing session is
   left as `{ active: false, paused: true }`, which reads at a glance like a
   feed in progress. `FeedDocument::running_timer` and
   `SleepDocument::running_timer` filter on `active`.

3. **The timezone offset's sign is inverted.** Huckleberry stores what
   JavaScript's `getTimezoneOffset` returns: minutes to *add* to local time to
   reach UTC. New York in summer stores `240`, Berlin stores `-120`. It is
   computed per instant, which is why an interval carries both `offset` and
   `end_offset`.

4. **An update mask is the whole of the semantics.** A field in the mask and in
   the body is written; a field in the mask and not in the body is *deleted*; a
   field in neither is left alone. A merge names every leaf of the payload,
   which is what leaves the sibling keys alone. And a mask entry that is not a
   plain identifier has to be backtick-quoted, which the sleep condition key
   `10-20_minutes` is not.

## Testing

The pure parts (the value codec, masks, queries, every piece of timer
arithmetic) are tested inline. The request shapes are tested from the other end
of a socket: `tests/firestore_requests.rs` drives the real client against a
stub Firestore on loopback and asserts the method, the URL, the query
parameters and the decoded body of every write.

That is not belt and braces. It is what caught the interval rows going out
without their `mode` discriminator, which no unit test could have seen and
which Huckleberry would have accepted and then failed to display.
