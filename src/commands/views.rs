//! The read-only screens: now, summary, trends, stripes and log.
//!
//! All five load the same dataset and hand it to a renderer, so they live
//! together: the differences between them are a line each.

use anyhow::Result;
use huckleberry_api::client::now_seconds;

use crate::cli::{LogKind, TrendMetric, Units};
use crate::domain::{log, now, stripes, summaries};
use crate::listing::Listing;
use crate::prompt::{self, Choice, Question};
use crate::render;
use crate::render::format;
use crate::session::Context;

/// The 3am screen.
pub async fn now(context: &Context, json: bool) -> Result<()> {
    let (dataset, calendar) = super::load(context, Some(1)).await?;
    let at = now_seconds();
    let view = now::build(&dataset, &calendar, context.config.day_rule(), at);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&as_json(&view, &dataset, at))?
        );
        return Ok(());
    }
    render::print(&render::now::lines(
        &view,
        &dataset,
        &calendar,
        context.output_theme(),
        units(context),
        at,
    ));
    Ok(())
}

/// The day table.
pub async fn summary(context: &Context, days: Option<u32>, json: bool) -> Result<()> {
    let (dataset, calendar) = super::load(context, days).await?;
    let at = now_seconds();
    let window = context.days(days) as usize;
    let rows = summaries::build(&dataset, &calendar, at, window);

    if json {
        println!("{}", serde_json::to_string_pretty(&rows_as_json(&rows))?);
        return Ok(());
    }
    render::print(&render::summary::lines(
        &rows,
        &dataset,
        &calendar,
        context.output_theme(),
        units(context),
        at,
    ));
    Ok(())
}

/// One number over time.
pub async fn trends(
    context: &Context,
    metric: Option<TrendMetric>,
    days: Option<u32>,
) -> Result<()> {
    let metric = match metric {
        Some(chosen) => chosen,
        None => ask_for_metric(context)?,
    };
    let (dataset, calendar) = super::load(context, days).await?;
    let window = context.days(days) as usize;
    let rows = summaries::build(&dataset, &calendar, now_seconds(), window);
    render::print(&render::trends::lines(
        &rows,
        metric,
        context.output_theme(),
        units(context),
    ));
    Ok(())
}

/// The stripe chart.
pub async fn stripes(context: &Context, days: Option<u32>) -> Result<()> {
    let (dataset, calendar) = super::load(context, days).await?;
    let window = context.days(days) as usize;
    let rows = stripes::build(&dataset, &calendar, now_seconds(), window);
    render::print(&render::stripes::lines(&rows, context.output_theme()));
    Ok(())
}

/// The merged stream.
///
/// A list to look around in rather than a wall of text: on a terminal it opens
/// the browsable view, where `/` searches and Enter opens one entry. Piped, it
/// is the same rows as plain lines.
pub async fn log(
    context: &Context,
    kind: Option<LogKind>,
    days: Option<u32>,
    limit: usize,
    search: Option<&str>,
) -> Result<()> {
    let (dataset, calendar) = super::load(context, days).await?;
    let entries: Vec<_> = render::log::only(
        log::build(&dataset, |amount| format::volume(amount, units(context))),
        kind.map(LogKind::to_domain),
    )
    .into_iter()
    .take(limit)
    .collect();
    let now = now_seconds();
    let today = format::day_short(calendar.day_of(now));
    Listing::new(&dataset.child.name, "entries", &render::log::COLUMNS)
        .rows(render::log::rows(&entries, &calendar, now, &|_| None))
        .today(|heading| heading == today)
        .empty("nothing logged in this window")
        .query(search)
        .verb("j/k or ↑/↓ move · / searches · enter opens one · q leaves")
        .show(context.output_theme())
}

/// Which volume unit to show, from the configuration.
fn units(context: &Context) -> Units {
    Units::from_setting(&context.config.units)
}

/// Offers the metrics when `--metric` was left out.
fn ask_for_metric(context: &Context) -> Result<TrendMetric> {
    let choices: Vec<Choice<'_>> = TrendMetric::ALL
        .iter()
        .map(|metric| Choice {
            value: metric.as_str(),
            hint: metric.title(),
        })
        .collect();
    let question = Question::new("metric to chart", "Which number?", "--metric <METRIC>")
        .with_choices(&choices);
    let answer = prompt::ask(&question, context.theme)?;
    TrendMetric::parse(&answer)
        .ok_or_else(|| anyhow::anyhow!("`{answer}` is not a metric this tool charts"))
}

/// The 3am facts as JSON, for a script or a status bar.
fn as_json(
    view: &now::NowView,
    dataset: &crate::domain::types::Dataset,
    at: f64,
) -> serde_json::Value {
    serde_json::json!({
        "child": dataset.child.name,
        "as_of": dataset.fetched_at,
        "now": at,
        "last_feed": view.last_feed.as_ref().map(|last| serde_json::json!({
            "at": last.start,
            "ago_seconds": last.ago_seconds,
            "ml": last.feed.millilitres(),
            "nursing_seconds": last.feed.nursing_seconds(),
        })),
        "last_diaper": view.last_diaper.as_ref().map(|last| serde_json::json!({
            "at": last.start,
            "ago_seconds": last.ago_seconds,
            "wet": last.wet,
            "dirty": last.dirty,
        })),
        "sleep": {
            "asleep": view.sleep_state.asleep,
            "paused": view.sleep_state.paused,
            "asleep_seconds": view.sleep_state.asleep_seconds,
            "awake_seconds": view.sleep_state.awake_seconds,
        },
        "longest_stretch": {
            "tonight": view.longest_stretch.tonight,
            "night_of": view.longest_stretch.night_of.to_string(),
            "seconds": view.longest_stretch.seconds,
        },
        "nursing_now": view.nursing_now.as_ref().map(|nursing| serde_json::json!({
            "side": nursing.side,
            "elapsed_seconds": nursing.elapsed_seconds,
            "paused": nursing.paused,
        })),
        "recent": totals_as_json(&view.recent),
        "today": totals_as_json(&view.today),
        "today_window": {
            "mode": view.today_window.mode.key(),
            "start": view.today_window.start,
            "end": view.today_window.end,
            "began_at_hour": view.today_window.began_at_hour,
        },
        "recent_hours": crate::domain::now::RECENT_HOURS,
        "notes": dataset.notes,
    })
}

/// What a window amounts to, as numbers a script can add up.
fn totals_as_json(totals: &crate::domain::today::Totals) -> serde_json::Value {
    serde_json::json!({
        "ml": totals.millilitres,
        "nursing_seconds": totals.nursing_seconds,
        "milk_feeds": totals.milk_feeds,
        "solids": totals.solids,
        "sleep_seconds": totals.sleep_seconds,
    })
}

/// The day table as JSON, for a spreadsheet or a script.
fn rows_as_json(rows: &[summaries::DaySummary]) -> serde_json::Value {
    serde_json::Value::Array(
        rows.iter()
            .map(|row| {
                serde_json::json!({
                    "day": row.day.to_string(),
                    "partial": row.partial,
                    "has_data": row.has_data,
                    "feeds": row.feed_count,
                    "bottles": row.bottle_count,
                    "nursing": row.nursing_count,
                    "solids": row.solids_count,
                    "milk_ml": row.total_ml,
                    "formula_ml": row.formula_ml,
                    "breast_milk_ml": row.breast_milk_ml,
                    "nursing_seconds": row.nursing_seconds,
                    "left_seconds": row.left_seconds,
                    "right_seconds": row.right_seconds,
                    "average_feed_gap_seconds": row.average_feed_gap_seconds,
                    "longest_feed_gap_seconds": row.longest_feed_gap_seconds,
                    "wet": row.wet_count,
                    "dirty": row.dirty_count,
                    "diapers": row.diaper_count,
                    "rashes": row.rash_count,
                    "sleep_seconds": row.sleep_seconds,
                    "night_sleep_seconds": row.night_sleep_seconds,
                    "day_sleep_seconds": row.day_sleep_seconds,
                    "sleeps": row.sleep_count,
                    "longest_sleep_seconds": row.longest_sleep_seconds,
                    "pumps": row.pump_count,
                    "pumped_ml": row.pumped_ml,
                    "milestones": row.milestone_count,
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Calendar;
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset};
    use crate::domain::today::DayRule;

    #[test]
    fn the_json_facts_carry_every_number_the_screen_shows() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3_600.0, 90.0)];
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        let view = now::build(&data, &calendar, DayRule::discrete_default(), AFTERNOON);
        let json = as_json(&view, &data, AFTERNOON);
        assert_eq!(json["last_feed"]["ml"], serde_json::json!(90.0));
        assert_eq!(json["today"]["ml"], serde_json::json!(90.0));
        assert_eq!(json["today"]["milk_feeds"], serde_json::json!(1));
        assert_eq!(json["today_window"]["mode"], serde_json::json!("discrete"));
        assert_eq!(json["recent"]["milk_feeds"], serde_json::json!(1));
        assert_eq!(json["last_feed"]["ago_seconds"], serde_json::json!(3600.0));
        assert_eq!(json["child"], serde_json::json!("Bear"));
    }

    #[test]
    fn nothing_logged_is_null_in_the_json_rather_than_a_zero() {
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        let data = dataset();
        let view = now::build(&data, &calendar, DayRule::discrete_default(), AFTERNOON);
        let json = as_json(&view, &data, AFTERNOON);
        assert_eq!(json["last_feed"], serde_json::Value::Null);
        assert_eq!(json["sleep"]["asleep_seconds"], serde_json::Value::Null);
    }

    #[test]
    fn the_day_table_json_has_one_object_per_day() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3_600.0, 90.0)];
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        let rows = summaries::build(&data, &calendar, AFTERNOON, 3);
        let json = rows_as_json(&rows);
        assert_eq!(json.as_array().expect("an array").len(), 3);
        assert_eq!(json[0]["day"], serde_json::json!("2025-09-22"));
        assert_eq!(json[0]["milk_ml"], serde_json::json!(90.0));
    }
}
