//! Recording a bottle.
//!
//! An instant event: there is no bottle timer. Two writes, because the app
//! keeps the last bottle's amount and kind as the defaults it offers next
//! time, and a parent who always gives 90 ml of formula should not have to
//! say so twice.

use serde_json::json;

use crate::client::{Huckleberry, now_seconds};
use crate::error::Result;
use crate::ids;
use crate::models::common::Number;
use crate::models::feed::{BottleFeedInterval, BottleType, FeedInterval, LastBottle, VolumeUnits};
use crate::models::{to_fields, to_json};
use crate::paths;

impl Huckleberry {
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
}
