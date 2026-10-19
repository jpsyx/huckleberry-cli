//! Conservative chronological eligibility, independent of display windows.
use crate::RowRef;
use serde_json::Value as Json;

/// Unknown active state cannot prove that a timer is idle.
pub(super) fn active(document: &Json) -> bool {
    document
        .get("timer")
        .filter(|timer| !timer.is_null())
        .is_some_and(|timer| {
            timer
                .get("active")
                .is_none_or(|active| active != &Json::Bool(false))
        })
}

/// Last event time for an instant event or a measured interval.
pub(super) fn endpoint(row: &Json) -> Option<f64> {
    let start = row
        .get("start")?
        .as_f64()
        .filter(|start| start.is_finite() && *start >= 0.0)?;
    let duration = if let Some(duration) = row.get("duration") {
        duration.as_f64()?
    } else if row.get("leftDuration").is_some() || row.get("rightDuration").is_some() {
        row.get("leftDuration")?.as_f64()? + row.get("rightDuration")?.as_f64()?
    } else {
        0.0
    };
    let end = start + duration;
    (duration >= 0.0 && end.is_finite()).then_some(end)
}

/// An unreadable event is not evidence that nothing happened later.
pub(super) fn blocks(row: &Json, end: f64) -> bool {
    endpoint(row).is_none_or(|event_end| event_end > end)
}

/// Equal latest starts are ambiguous; invalid sleep data never qualifies.
pub(super) fn candidate(rows: &[(RowRef, Json)], now: f64) -> Option<&(RowRef, Json)> {
    if rows.iter().any(|(_, row)| endpoint(row).is_none()) {
        return None;
    }
    let latest = rows.iter().max_by(|(_, left), (_, right)| {
        left["start"]
            .as_f64()
            .unwrap()
            .total_cmp(&right["start"].as_f64().unwrap())
    })?;
    let start = latest.1["start"].as_f64()?;
    let duration = latest.1.get("duration")?.as_f64()?;
    let end = endpoint(&latest.1)?;
    if duration <= 0.0
        || end > now
        || rows.iter().any(|(at, row)| {
            at != &latest.0
                && (row["start"].as_f64().is_none_or(|other| other >= start) || blocks(row, end))
        })
    {
        return None;
    }
    Some(latest)
}
