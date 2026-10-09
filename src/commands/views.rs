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
        crate::render::print(&[serde_json::to_string_pretty(&as_json(&view, &dataset, at))?]);
        return Ok(());
    }
    render::print(&render::now::lines(
        &view,
        &dataset,
        &calendar,
        context.output_theme(),
        units(context),
        at,
        context.screen_width(),
    ));
    Ok(())
}

/// The day table.
pub async fn summary(context: &Context, days: Option<u32>, json: bool) -> Result<()> {
    let (dataset, calendar) = super::load(context, days).await?;
    let at = now_seconds();
    let window = context.days(days) as usize;
    let rows = summaries::build(&dataset, &calendar, context.config.day_rule(), at, window);

    if json {
        crate::render::print(&[serde_json::to_string_pretty(&rows_as_json(&rows))?]);
        return Ok(());
    }
    if crate::summary::interactive() {
        return crate::summary::show(
            &render::summary::view::View {
                rows: &rows,
                dataset: &dataset,
                calendar: &calendar,
                units: units(context),
                now: at,
            },
            context.output_theme(),
        );
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
    let rows = summaries::build(
        &dataset,
        &calendar,
        context.config.day_rule(),
        now_seconds(),
        window,
    );
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
    let rows = stripes::build(
        &dataset,
        &calendar,
        context.config.day_rule(),
        now_seconds(),
        window,
    );
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
        .verb("↑/↓ j/k w/s move · / searches · enter opens one · h/a or q leaves")
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
                    "average_milk_ml": row.average_milk_ml(),
                    "average_nursing_seconds": row.average_nursing_seconds(),
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
                    "average_nap_seconds": row.average_nap_seconds,
                    "wake_seconds": row.wake_seconds,
                    "night_wake_seconds": row.night_wake_seconds,
                    "average_wake_seconds": row.average_wake_seconds,
                    "longest_wake_seconds": row.longest_wake_seconds,
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
        let view = now::build(&data, &calendar, DayRule::assumed(), AFTERNOON);
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
        let view = now::build(&data, &calendar, DayRule::assumed(), AFTERNOON);
        let json = as_json(&view, &data, AFTERNOON);
        assert_eq!(json["last_feed"], serde_json::Value::Null);
        assert_eq!(json["sleep"]["asleep_seconds"], serde_json::Value::Null);
    }

    #[test]
    fn the_day_table_json_has_one_object_per_day() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3_600.0, 90.0)];
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 3);
        let json = rows_as_json(&rows);
        assert_eq!(json.as_array().expect("an array").len(), 3);
        assert_eq!(json[0]["day"], serde_json::json!("2025-09-22"));
        assert_eq!(json[0]["milk_ml"], serde_json::json!(90.0));
    }

    fn summary_json(
        data: &crate::domain::types::Dataset,
        at: f64,
        days: usize,
    ) -> serde_json::Value {
        let calendar = Calendar::new("America/New_York").unwrap();
        rows_as_json(&summaries::build(
            data,
            &calendar,
            DayRule::discrete(7.0, 20.0),
            at,
            days,
        ))
    }

    fn at(date: &str, hour: i8, minute: i8) -> f64 {
        Calendar::new("America/New_York")
            .unwrap()
            .at(date.parse().unwrap(), hour, minute)
    }

    #[test]
    fn summary_averages_use_only_measured_bottles_and_nursing_sessions() {
        use crate::domain::fixtures::nursing;
        let mut data = dataset();
        let mut breast = bottle(AFTERNOON - 7200.0, 60.0);
        if let crate::domain::types::FeedEvent::Bottle { bottle_type, .. } = &mut breast {
            *bottle_type = Some("Breast Milk".into());
        }
        data.feeds = vec![
            bottle(AFTERNOON, 120.0),
            breast,
            nursing(AFTERNOON - 3600.0, 300.0, 300.0),
            nursing(AFTERNOON - 1800.0, 600.0, 600.0),
        ];
        let json = summary_json(&data, AFTERNOON, 1);
        assert_eq!(json[0]["average_milk_ml"], 90.0);
        assert_eq!(json[0]["average_nursing_seconds"], 900.0);
    }

    #[test]
    fn summary_missing_bottle_amounts_do_not_become_zero_measurements() {
        let mut data = dataset();
        let mut unknown = bottle(AFTERNOON, 0.0);
        if let crate::domain::types::FeedEvent::Bottle { amount_ml, .. } = &mut unknown {
            *amount_ml = None;
        }
        data.feeds = vec![unknown, bottle(AFTERNOON - 3600.0, 120.0)];
        let json = summary_json(&data, AFTERNOON, 1);
        assert_eq!(json[0]["bottles"], 2);
        assert_eq!(json[0]["average_milk_ml"], 120.0);
        assert!(json[0]["average_nursing_seconds"].is_null());
    }

    #[test]
    fn summary_naps_exclude_night_sleeps_and_night_totals_split_at_day_end() {
        use crate::domain::fixtures::sleep;
        let mut data = dataset();
        data.sleep = vec![
            sleep(at("2025-09-21", 10, 0), 1800.0),
            sleep(at("2025-09-21", 19, 30), 3600.0),
            sleep(at("2025-09-21", 22, 0), 7200.0),
        ];
        let json = summary_json(&data, AFTERNOON, 2);
        assert_eq!(json[1]["average_nap_seconds"], 2700.0);
        assert_eq!(json[1]["night_sleep_seconds"], 9000.0);
        assert_eq!(json[1]["day_sleep_seconds"], 3600.0);
        assert_eq!(json[1]["wake_seconds"], 73800.0);
        assert_eq!(json[1]["night_wake_seconds"], 30600.0);
    }

    #[test]
    fn summary_wake_gaps_run_from_sleep_end_to_next_start() {
        use crate::domain::fixtures::sleep;
        let mut data = dataset();
        data.sleep = vec![
            sleep(at("2025-09-22", 12, 0), 3600.0),
            sleep(at("2025-09-22", 8, 0), 3600.0),
            sleep(at("2025-09-22", 10, 0), 1800.0),
        ];
        let json = summary_json(&data, AFTERNOON, 1);
        assert_eq!(json[0]["average_wake_seconds"], 4500.0);
        assert_eq!(json[0]["longest_wake_seconds"], 5400.0);
        assert_eq!(json[0]["wake_seconds"], 16200.0);
        assert!(
            json[0]["night_wake_seconds"].is_null(),
            "night has not started"
        );
    }

    #[test]
    fn summary_overlapping_sleep_does_not_double_count_or_make_negative_gaps() {
        use crate::domain::fixtures::sleep;
        let mut data = dataset();
        data.sleep = vec![
            sleep(at("2025-09-22", 8, 0), 7200.0),
            sleep(at("2025-09-22", 9, 0), 7200.0),
            sleep(at("2025-09-22", 12, 0), 3600.0),
        ];
        let json = summary_json(&data, AFTERNOON, 1);
        assert_eq!(json[0]["sleep_seconds"], 14400.0);
        assert_eq!(json[0]["wake_seconds"], 10800.0);
        assert_eq!(json[0]["average_wake_seconds"], 3600.0);
    }

    #[test]
    fn summary_dst_wake_totals_use_actual_elapsed_seconds() {
        use crate::domain::fixtures::sleep;
        let mut data = dataset();
        data.sleep = vec![sleep(at("2025-11-01", 22, 0), 7200.0)];
        let json = summary_json(&data, at("2025-11-02", 14, 0), 2);
        assert_eq!(json[1]["wake_seconds"], 82800.0, "25h day minus 2h sleep");
        assert_eq!(
            json[1]["night_wake_seconds"], 36000.0,
            "12h night minus 2h sleep"
        );
    }

    #[test]
    fn summary_empty_days_have_no_wake_estimates_or_averages() {
        let json = summary_json(&dataset(), AFTERNOON, 1);
        for field in [
            "wake_seconds",
            "night_wake_seconds",
            "average_wake_seconds",
            "longest_wake_seconds",
            "average_nap_seconds",
            "average_milk_ml",
            "average_nursing_seconds",
        ] {
            assert!(json[0].get(field).is_some(), "missing {field}");
            assert!(json[0][field].is_null(), "{field} should be absent");
        }
    }

    #[test]
    fn summary_live_sleep_is_not_mistaken_for_awake_time_or_a_finished_nap() {
        let mut data = dataset();
        data.live.sleep_active = true;
        data.live.sleep_start = Some(at("2025-09-22", 12, 0));
        let json = summary_json(&data, AFTERNOON, 1);
        assert_eq!(json[0]["sleep_seconds"], 7200.0);
        assert_eq!(json[0]["wake_seconds"], 18000.0);
        assert!(json[0]["average_nap_seconds"].is_null());
    }
    #[test]
    fn summary_paused_sleep_retains_elapsed_sleep_without_counting_the_pause() {
        use crate::domain::fixtures::sleep;
        let mut data = dataset();
        data.sleep = vec![sleep(at("2025-09-22", 8, 0), 3600.0)];
        let document = serde_json::from_value(serde_json::json!({"timer": {
            "active": true, "paused": true, "uuid": "paused-sleep",
            "timerStartTime": at("2025-09-22", 11, 0) * 1000.0,
            "timerEndTime": at("2025-09-22", 13, 0) * 1000.0
        }}))
        .unwrap();
        data.live = crate::domain::normalize::live(Some(&document), None);
        assert!(data.live.sleep_active, "the fixture must retain its timer");
        let json = summary_json(&data, AFTERNOON, 1);
        assert_eq!(json[0]["sleep_seconds"], 10800.0);
        assert_eq!(json[0]["wake_seconds"], 14400.0);
        assert_eq!(json[0]["average_nap_seconds"], 3600.0);
    }

    #[test]
    fn summary_old_paused_snapshot_does_not_turn_unknown_sleep_into_waking() {
        use crate::domain::fixtures::sleep;
        let mut data = dataset();
        data.sleep = vec![sleep(at("2025-09-22", 8, 0), 3600.0)];
        data.live = serde_json::from_value(serde_json::json!({
            "sleep_active": true, "sleep_paused": true,
            "sleep_start": at("2025-09-22", 11, 0)
        }))
        .unwrap();
        let json = summary_json(&data, AFTERNOON, 1);
        assert!(json[0]["wake_seconds"].is_null());
    }
    #[test]
    fn summary_naps_follow_the_same_boundaries_as_daytime_sleep_for_inverted_hours() {
        let mut data = dataset();
        data.sleep = vec![crate::domain::fixtures::sleep(
            at("2025-09-21", 22, 0),
            3600.0,
        )];
        let calendar = Calendar::new("America/New_York").unwrap();
        let rows = summaries::build(&data, &calendar, DayRule::discrete(20.0, 7.0), AFTERNOON, 1);
        let json = rows_as_json(&rows);
        assert_eq!(json[0]["day_sleep_seconds"], 3600.0);
        assert_eq!(json[0]["average_nap_seconds"], 3600.0);
    }
    #[test]
    fn summary_unknown_pause_cannot_start_an_invented_wake_gap() {
        use crate::domain::fixtures::sleep;
        let mut data = dataset();
        data.sleep = vec![
            sleep(at("2025-09-22", 8, 0), 3600.0),
            sleep(at("2025-09-22", 12, 0), 3600.0),
        ];
        data.live.sleep_active = true;
        data.live.sleep_paused = true;
        data.live.sleep_start = Some(at("2025-09-22", 11, 0));
        let json = summary_json(&data, AFTERNOON, 1);
        assert_eq!(json[0]["average_wake_seconds"], 7200.0);
        assert_eq!(json[0]["longest_wake_seconds"], 7200.0);
    }
}
