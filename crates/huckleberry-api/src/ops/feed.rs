//! Nursing sessions and bottles.
//!
//! The nursing timer banks seconds per side. `leftDuration` and
//! `rightDuration` hold what is already finished, and the side running now is
//! measured from `timerStartTime`, which resets on every switch and every
//! resume. [`nursing_totals`] is the one place that adds the two together, and
//! every operation here goes through it so that pausing, switching and
//! finishing cannot disagree about the total.

use serde_json::json;

use super::TimerChange;
use crate::client::{Huckleberry, now_seconds};
use crate::error::Result;
use crate::firestore::FieldUpdate;
use crate::ids;
use crate::models::common::{Number, Timestamp};
use crate::models::feed::{
    BottleFeedInterval, BottleType, BreastFeedInterval, FeedDocument, FeedInterval, FeedSide,
    FeedTimer, LastBottle, LastNursing, LastSide, VolumeUnits,
};
use crate::models::{to_fields, to_json};
use crate::paths;

/// What a finished nursing session amounts to.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletedNursing {
    /// When the feed started, in seconds.
    pub start: f64,
    /// Seconds on the left.
    pub left_seconds: f64,
    /// Seconds on the right.
    pub right_seconds: f64,
    /// Which side to record it as having ended on.
    pub last_side: FeedSide,
}

impl CompletedNursing {
    /// Both sides together, in seconds.
    #[must_use]
    pub fn total_seconds(&self) -> f64 {
        self.left_seconds + self.right_seconds
    }
}

/// Seconds on each side at `now`, counting the side running now.
#[must_use]
pub fn nursing_totals(timer: &FeedTimer, now: f64) -> (f64, f64) {
    timer.totals(now)
}

/// The side a switch moves to.
///
/// `none` is a transition marker rather than a side, so it switches to the
/// right, which is what the app does when it has lost track.
#[must_use]
pub const fn opposite_side(side: &FeedSide) -> FeedSide {
    match side {
        FeedSide::Left => FeedSide::Right,
        _ => FeedSide::Left,
    }
}

/// Turns a running timer into the feed it represents.
///
/// The recorded side falls back the way the app's own history does: the active
/// side, then the last side, and if that is the `none` marker, whichever side
/// actually got more time.
#[must_use]
pub fn completed_nursing(timer: &FeedTimer, now: f64) -> Option<CompletedNursing> {
    if !timer.active {
        return None;
    }
    let started = timer.timer_start_time.map(Number::as_f64)?;
    let (left_seconds, right_seconds) = nursing_totals(timer, now);
    let mut last_side = timer
        .active_side
        .clone()
        .or_else(|| timer.last_side.clone())
        .unwrap_or(FeedSide::Right);
    if last_side == FeedSide::None {
        last_side = if right_seconds >= left_seconds {
            FeedSide::Right
        } else {
            FeedSide::Left
        };
    }
    Some(CompletedNursing {
        start: timer.feed_start_time.map_or(started, Number::as_f64),
        left_seconds,
        right_seconds,
        last_side,
    })
}

impl Huckleberry {
    /// Starts a nursing session on one side.
    ///
    /// # Errors
    ///
    /// As every write: a refused or unreachable Firestore.
    pub async fn start_nursing(&self, cid: &str, side: FeedSide) -> Result<()> {
        let now = now_seconds();
        let document = FeedDocument {
            timer: Some(FeedTimer {
                active: true,
                paused: false,
                timestamp: Some(Timestamp::at(now)),
                local_timestamp: Some(Number::Float(now)),
                feed_start_time: Some(Number::Float(now)),
                timer_start_time: Some(Number::Float(now)),
                uuid: ids::session_id(),
                left_duration: Some(Number::Float(0.0)),
                right_duration: Some(Number::Float(0.0)),
                last_side: Some(FeedSide::Left),
                active_side: Some(side),
            }),
            prefs: None,
        };
        let token = self.token().await?;
        self.firestore()
            .merge(
                &token,
                &paths::tracker(paths::FEED, cid),
                &to_fields(&document)?,
                "starting a nursing session",
            )
            .await
    }

    /// Pauses the running nursing session, banking the side that was running.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_nursing`].
    pub async fn pause_nursing(&self, cid: &str) -> Result<TimerChange> {
        let Some(timer) = self.feed_timer(cid).await? else {
            return Ok(TimerChange::NotRunning);
        };
        if timer.paused {
            return Ok(TimerChange::Unchanged);
        }
        let now = now_seconds();
        let (left, right) = nursing_totals(&timer, now);
        let side = timer.current_side();
        self.update_feed(
            cid,
            &[
                FieldUpdate::set("timer.paused", json!(true)),
                FieldUpdate::set("timer.active", json!(true)),
                FieldUpdate::set("timer.timestamp", json!({ "seconds": now })),
                FieldUpdate::set("timer.local_timestamp", json!(now)),
                FieldUpdate::set("timer.leftDuration", json!(left)),
                FieldUpdate::set("timer.rightDuration", json!(right)),
                FieldUpdate::set("timer.lastSide", json!(side.as_str())),
                // Nothing is running, so there is no active side. Leaving one
                // would make the paused timer read as still accruing.
                FieldUpdate::delete("timer.activeSide"),
            ],
            "pausing the nursing session",
        )
        .await
    }

    /// Resumes a paused nursing session, on `side` or on whichever was last.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_nursing`].
    pub async fn resume_nursing(&self, cid: &str, side: Option<FeedSide>) -> Result<TimerChange> {
        let Some(timer) = self.feed_timer(cid).await? else {
            return Ok(TimerChange::NotRunning);
        };
        if !timer.paused {
            return Ok(TimerChange::Unchanged);
        }
        let now = now_seconds();
        let side = side
            .or_else(|| timer.last_side.clone())
            .unwrap_or(FeedSide::Left);
        self.update_feed(
            cid,
            &[
                FieldUpdate::set("timer.paused", json!(false)),
                FieldUpdate::set("timer.active", json!(true)),
                FieldUpdate::set("timer.timestamp", json!({ "seconds": now })),
                FieldUpdate::set("timer.local_timestamp", json!(now)),
                // The side starts accruing from now; what it had is banked.
                FieldUpdate::set("timer.timerStartTime", json!(now)),
                FieldUpdate::set("timer.activeSide", json!(side.as_str())),
                FieldUpdate::set("timer.lastSide", json!(FeedSide::None.as_str())),
            ],
            "resuming the nursing session",
        )
        .await
    }

    /// Switches sides, banking whatever the current one has run for.
    ///
    /// Switching always resumes: a paused session that is switched is a
    /// session the parent has picked back up.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_nursing`].
    pub async fn switch_nursing_side(&self, cid: &str) -> Result<TimerChange> {
        let Some(timer) = self.feed_timer(cid).await? else {
            return Ok(TimerChange::NotRunning);
        };
        let now = now_seconds();
        let (left, right) = nursing_totals(&timer, now);
        let next = opposite_side(&timer.current_side());
        self.update_feed(
            cid,
            &[
                FieldUpdate::set("timer.paused", json!(false)),
                FieldUpdate::set("timer.lastSide", json!(FeedSide::None.as_str())),
                FieldUpdate::set("timer.timestamp", json!({ "seconds": now })),
                FieldUpdate::set("timer.local_timestamp", json!(now)),
                FieldUpdate::set("timer.timerStartTime", json!(now)),
                FieldUpdate::set("timer.activeSide", json!(next.as_str())),
                FieldUpdate::set("timer.leftDuration", json!(left)),
                FieldUpdate::set("timer.rightDuration", json!(right)),
            ],
            "switching sides",
        )
        .await
    }

    /// Throws away the running nursing session without recording it.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_nursing`].
    pub async fn cancel_nursing(&self, cid: &str) -> Result<TimerChange> {
        let existing = self.feed_document(cid).await?;
        let session_id = existing
            .and_then(|document| document.timer)
            .map_or_else(ids::session_id, |timer| timer.uuid);
        let now = now_seconds();
        self.update_feed(
            cid,
            &[FieldUpdate::set(
                "timer",
                json!({
                    "active": false,
                    "paused": false,
                    "timestamp": { "seconds": now },
                    "timerStartTime": null,
                    "uuid": session_id,
                    "local_timestamp": now,
                    "leftDuration": 0.0,
                    "rightDuration": 0.0,
                    "lastSide": "left",
                }),
            )],
            "cancelling the nursing session",
        )
        .await
    }

    /// Finishes the running nursing session and writes it to history.
    ///
    /// `None` when nothing was running.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_nursing`].
    pub async fn complete_nursing(&self, cid: &str) -> Result<Option<CompletedNursing>> {
        let Some(timer) = self.feed_timer(cid).await? else {
            return Ok(None);
        };
        let now = now_seconds();
        let Some(completed) = completed_nursing(&timer, now) else {
            return Ok(None);
        };

        let offset = self.zone().offset_minutes(now);
        // Written through `FeedInterval` rather than as a bare row: the enum
        // is what puts `mode` on the document, and the app reads `mode` to
        // decide what kind of feed it is looking at.
        let interval = FeedInterval::Breast(BreastFeedInterval {
            start: Number::Float(completed.start),
            last_side: completed.last_side.clone(),
            last_updated: Some(Number::Float(now)),
            left_duration: Number::Float(completed.left_seconds),
            right_duration: Number::Float(completed.right_seconds),
            offset: Number::Float(offset),
            end_offset: Some(Number::Float(offset)),
            notes: None,
        });
        let token = self.token().await?;
        self.firestore()
            .set(
                &token,
                &paths::history_row(paths::FEED, cid, &ids::interval_id(now)),
                &to_fields(&interval)?,
                "writing the feed to history",
            )
            .await?;

        let last_nursing = LastNursing {
            mode: Some("breast".to_owned()),
            start: Some(Number::Float(completed.start)),
            duration: Some(Number::Float(completed.total_seconds())),
            left_duration: Some(Number::Float(completed.left_seconds)),
            right_duration: Some(Number::Float(completed.right_seconds)),
            offset: Some(Number::Float(offset)),
        };
        let last_side = LastSide {
            start: Number::Float(completed.start),
            last_side: completed.last_side.clone(),
        };
        self.update_feed(
            cid,
            &[
                FieldUpdate::set("timer.active", json!(false)),
                FieldUpdate::set("timer.paused", json!(true)),
                FieldUpdate::set("timer.timestamp", json!({ "seconds": now })),
                FieldUpdate::set("timer.local_timestamp", json!(now)),
                FieldUpdate::set("timer.lastSide", json!(completed.last_side.as_str())),
                // The banked seconds belong to the history row now; leaving
                // them on the timer would double them into the next session.
                FieldUpdate::delete("timer.leftDuration"),
                FieldUpdate::delete("timer.rightDuration"),
                FieldUpdate::delete("timer.activeSide"),
                FieldUpdate::set("prefs.lastNursing", to_json(&last_nursing)?),
                FieldUpdate::set("prefs.lastSide", to_json(&last_side)?),
                FieldUpdate::set("prefs.timestamp", json!({ "seconds": now })),
                FieldUpdate::set("prefs.local_timestamp", json!(now)),
            ],
            "finishing the nursing session",
        )
        .await?;
        Ok(Some(completed))
    }

    /// Records a bottle. An instant event: there is no bottle timer.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_nursing`].
    pub async fn log_bottle(
        &self,
        cid: &str,
        amount: f64,
        bottle_type: BottleType,
        units: VolumeUnits,
    ) -> Result<()> {
        let now = now_seconds();
        let offset = self.zone().offset_minutes(now);
        let interval = FeedInterval::Bottle(BottleFeedInterval {
            start: Number::Float(now),
            last_updated: Some(Number::Float(now)),
            bottle_type: bottle_type.clone(),
            amount: Number::Float(amount),
            units: units.clone(),
            offset: Number::Float(offset),
            end_offset: Some(Number::Float(offset)),
            notes: None,
        });
        let token = self.token().await?;
        self.firestore()
            .set(
                &token,
                &paths::history_row(paths::FEED, cid, &ids::interval_id(now)),
                &to_fields(&interval)?,
                "recording the bottle",
            )
            .await?;

        let last_bottle = LastBottle {
            mode: Some("bottle".to_owned()),
            start: Some(Number::Float(now)),
            bottle_type: Some(bottle_type.clone()),
            bottle_amount: Some(Number::Float(amount)),
            bottle_units: Some(units.clone()),
            offset: Some(Number::Float(offset)),
        };
        // A merge rather than an update: this also sets the defaults the app
        // offers next time, and the feed document may not exist yet on an
        // account whose first ever entry is a bottle.
        let mut prefs = serde_json::Map::new();
        prefs.insert(
            "prefs".to_owned(),
            json!({
                "lastBottle": to_json(&last_bottle)?,
                "bottleType": bottle_type.as_str(),
                "bottleAmount": amount,
                "bottleUnits": units.as_str(),
                "timestamp": { "seconds": now },
                "local_timestamp": now,
            }),
        );
        let fields = crate::firestore::value::fields_from_json(&prefs);
        self.firestore()
            .merge(
                &token,
                &paths::tracker(paths::FEED, cid),
                &fields,
                "recording the bottle",
            )
            .await
    }

    /// The feed tracker's document.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_nursing`].
    pub async fn feed_document(&self, cid: &str) -> Result<Option<FeedDocument>> {
        self.document(&paths::tracker(paths::FEED, cid), "reading the feed timer")
            .await
    }

    async fn feed_timer(&self, cid: &str) -> Result<Option<FeedTimer>> {
        Ok(self
            .feed_document(cid)
            .await?
            .and_then(|document| document.timer)
            .filter(|timer| timer.active))
    }

    async fn update_feed(
        &self,
        cid: &str,
        updates: &[FieldUpdate],
        operation: &str,
    ) -> Result<TimerChange> {
        let token = self.token().await?;
        self.firestore()
            .update(
                &token,
                &paths::tracker(paths::FEED, cid),
                updates,
                operation,
            )
            .await?;
        Ok(TimerChange::Applied)
    }
}

#[cfg(test)]
mod sides {
    use super::*;

    #[test]
    fn a_switch_alternates() {
        assert_eq!(opposite_side(&FeedSide::Left), FeedSide::Right);
        assert_eq!(opposite_side(&FeedSide::Right), FeedSide::Left);
    }

    #[test]
    fn a_switch_from_the_transition_marker_picks_a_real_side() {
        assert_eq!(opposite_side(&FeedSide::None), FeedSide::Left);
    }
}

#[cfg(test)]
mod completion {
    use super::*;

    fn timer(active: bool, paused: bool, banked: (f64, f64), side: Option<FeedSide>) -> FeedTimer {
        FeedTimer {
            active,
            paused,
            timestamp: None,
            local_timestamp: None,
            feed_start_time: Some(Number::Float(1_000.0)),
            timer_start_time: Some(Number::Float(1_300.0)),
            uuid: "0123456789abcdef".to_owned(),
            left_duration: Some(Number::Float(banked.0)),
            right_duration: Some(Number::Float(banked.1)),
            last_side: Some(FeedSide::Left),
            active_side: side,
        }
    }

    #[test]
    fn the_running_side_is_added_to_what_was_banked() {
        let completed = completed_nursing(
            &timer(true, false, (300.0, 0.0), Some(FeedSide::Right)),
            1_420.0,
        )
        .expect("a completed feed");
        assert!((completed.left_seconds - 300.0).abs() < f64::EPSILON);
        assert!((completed.right_seconds - 120.0).abs() < f64::EPSILON);
        assert!((completed.total_seconds() - 420.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_feed_starts_when_the_feed_started_not_when_the_side_did() {
        let completed = completed_nursing(
            &timer(true, false, (0.0, 0.0), Some(FeedSide::Left)),
            1_420.0,
        )
        .expect("a completed feed");
        assert!((completed.start - 1_000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_paused_session_records_only_what_was_banked() {
        let completed = completed_nursing(
            &timer(true, true, (300.0, 120.0), Some(FeedSide::Right)),
            99_999.0,
        )
        .expect("a completed feed");
        assert!((completed.total_seconds() - 420.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_transition_marker_resolves_to_whichever_side_got_more_time() {
        let mut mid_switch = timer(true, true, (100.0, 400.0), None);
        mid_switch.last_side = Some(FeedSide::None);
        let completed = completed_nursing(&mid_switch, 2_000.0).expect("a completed feed");
        assert_eq!(completed.last_side, FeedSide::Right);
    }

    #[test]
    fn an_inactive_timer_completes_nothing() {
        assert!(completed_nursing(&timer(false, false, (300.0, 0.0), None), 2_000.0).is_none());
    }

    #[test]
    fn a_timer_with_no_start_completes_nothing() {
        let mut bare = timer(true, false, (0.0, 0.0), Some(FeedSide::Left));
        bare.timer_start_time = None;
        assert!(completed_nursing(&bare, 2_000.0).is_none());
    }
}
