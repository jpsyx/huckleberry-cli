use super::*;
use crate::domain::fixtures::{bottle, dataset, diaper, nursing, sleep};

fn date(text: &str) -> Date {
    text.parse().expect("a date")
}

fn request<'a>(dataset: &'a Dataset, calendar: &'a Calendar, metric: Metric) -> Request<'a> {
    Request {
        dataset,
        calendar,
        rule: DayRule::continuous(6.0, 20.0),
        now: calendar.at(date("2025-09-22"), 18, 0),
        selected: date("2025-09-22"),
        measure: Measure {
            metric,
            period: Period::All,
        },
        coverage: (f64::NEG_INFINITY, f64::INFINITY),
    }
}

fn last_value(points: &[Point]) -> f64 {
    points.last().expect("a curve").value
}

#[test]
fn bottles_step_at_the_exact_timestamp_and_today_stops_at_now() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.feeds = vec![bottle(calendar.at(date("2025-09-22"), 6, 7) + 13.0, 90.0)];
    let result = build(&request(&dataset, &calendar, Metric::Milk));
    assert_eq!(
        result.today.points,
        vec![
            Point {
                seconds: 0.0,
                value: 0.0
            },
            Point {
                seconds: 433.0,
                value: 0.0
            },
            Point {
                seconds: 433.0,
                value: 90.0
            },
            Point {
                seconds: 43_200.0,
                value: 90.0
            },
        ]
    );
    assert_eq!(
        result.today.markers[0].point,
        Point {
            seconds: 433.0,
            value: 90.0
        }
    );
    assert!(result.today.is_partial);
    assert!(result.selected.is_none());
    assert!((result.today.duration - 86_400.0).abs() < 0.001);
}

#[test]
fn a_wet_and_dirty_diaper_counts_once_for_each_requested_quantity() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.diapers = vec![
        diaper(calendar.at(date("2025-09-22"), 7, 0), true, true),
        diaper(calendar.at(date("2025-09-22"), 8, 0), true, false),
    ];
    assert!(
        (last_value(
            &build(&request(&dataset, &calendar, Metric::Wet))
                .today
                .points
        ) - 2.0)
            .abs()
            < 0.001
    );
    assert!(
        (last_value(
            &build(&request(&dataset, &calendar, Metric::Dirty))
                .today
                .points
        ) - 1.0)
            .abs()
            < 0.001
    );
}

#[test]
fn sleep_union_crosses_night_and_day_boundaries_without_double_counting() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.sleep = vec![
        sleep(calendar.at(date("2025-09-21"), 19, 30), 7_200.0),
        sleep(calendar.at(date("2025-09-21"), 21, 0), 3_600.0),
        sleep(calendar.at(date("2025-09-22"), 5, 30), 3_600.0),
    ];
    let mut request = request(&dataset, &calendar, Metric::Sleep);
    request.selected = date("2025-09-21");
    request.measure.period = Period::Night;
    let result = build(&request);
    let selected = result.selected.expect("a historical day");
    assert!((last_value(&selected.points) - 9_000.0).abs() < 0.001);
    assert!(selected.points.contains(&Point {
        seconds: 0.0,
        value: 0.0
    }));
    assert!(selected.points.contains(&Point {
        seconds: 7_200.0,
        value: 7_200.0
    }));
    assert!(
        !selected
            .markers
            .iter()
            .any(|marker| marker.point.seconds > 36_000.0)
    );
}

#[test]
fn paused_live_sleep_stops_growing_without_a_completed_end_marker() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.live.sleep_active = true;
    dataset.live.sleep_start = Some(calendar.at(date("2025-09-22"), 16, 0));
    dataset.live.sleep_paused = true;
    dataset.live.sleep_paused_at = Some(calendar.at(date("2025-09-22"), 17, 0));
    let result = build(&request(&dataset, &calendar, Metric::Sleep));
    assert!((last_value(&result.today.points) - 3_600.0).abs() < 0.001);
    assert_eq!(
        result
            .today
            .markers
            .iter()
            .map(|marker| marker.symbol)
            .collect::<Vec<_>>(),
        vec!['o']
    );
    assert!(result.today.points.contains(&Point {
        seconds: 39_600.0,
        value: 3_600.0
    }));
}

#[test]
fn nursing_duration_grows_linearly_at_exact_session_boundaries() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.feeds = vec![nursing(calendar.at(date("2025-09-22"), 6, 7), 600.0, 300.0)];
    let result = build(&request(&dataset, &calendar, Metric::Nursing));
    assert_eq!(
        result.today.points,
        vec![
            Point {
                seconds: 0.0,
                value: 0.0
            },
            Point {
                seconds: 420.0,
                value: 0.0
            },
            Point {
                seconds: 1_320.0,
                value: 900.0
            },
            Point {
                seconds: 43_200.0,
                value: 900.0
            },
        ]
    );
}

#[test]
fn missing_amount_is_unknown_but_a_recorded_zero_is_a_measurement() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    let mut feed = bottle(calendar.at(date("2025-09-22"), 7, 0), 0.0);
    if let crate::domain::types::FeedEvent::Bottle { amount_ml, .. } = &mut feed {
        *amount_ml = None;
    }
    dataset.feeds = vec![feed];
    let missing = build(&request(&dataset, &calendar, Metric::Milk));
    assert!(!missing.today.has_data);
    assert!(missing.today.points.is_empty());
    dataset.feeds = vec![bottle(calendar.at(date("2025-09-22"), 7, 0), 0.0)];
    let zero = build(&request(&dataset, &calendar, Metric::Milk));
    assert!(zero.today.has_data);
    assert!(last_value(&zero.today.points).abs() < 0.001);
}

#[test]
fn average_is_anchored_to_today_and_excludes_partial_and_unrecorded_days() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.feeds = vec![
        bottle(calendar.at(date("2025-09-10"), 7, 0), 900.0),
        bottle(calendar.at(date("2025-09-15"), 7, 0), 800.0),
        bottle(calendar.at(date("2025-09-16"), 7, 0), 100.0),
        bottle(calendar.at(date("2025-09-21"), 7, 0), 200.0),
        bottle(calendar.at(date("2025-09-22"), 7, 0), 600.0),
    ];
    let mut request = request(&dataset, &calendar, Metric::Milk);
    request.coverage.0 = calendar.at(date("2025-09-15"), 8, 0);
    request.selected = date("2025-09-10");
    let result = build(&request);
    assert_eq!(result.average_days, 2);
    assert!((last_value(&result.average) - 150.0).abs() < 0.001);
    assert!(!result.selected.expect("selected day").has_data);
    assert!(
        result
            .average
            .windows(2)
            .all(|pair| pair[1].value >= pair[0].value)
    );
    assert!(result.average.len() > 97);
}

#[test]
fn daylight_saving_days_keep_their_actual_endpoints_and_average_common_span() {
    let calendar = Calendar::new("America/New_York").expect("timezone");
    let mut dataset = dataset();
    dataset.feeds = vec![
        bottle(calendar.at(date("2025-03-08"), 8, 0), 80.0),
        bottle(calendar.at(date("2025-03-09"), 8, 0), 100.0),
    ];
    let mut request = request(&dataset, &calendar, Metric::Milk);
    request.now = calendar.at(date("2025-03-10"), 12, 0);
    request.selected = date("2025-03-08");
    let result = build(&request);
    assert!((result.selected.expect("selected day").duration - 82_800.0).abs() < 0.001);
    assert!((result.average.last().expect("average").seconds - 82_800.0).abs() < 0.001);
    assert_eq!(result.average_days, 2);
}

#[test]
fn a_newly_started_live_sleep_has_a_known_zero_and_start_marker() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.live.sleep_active = true;
    dataset.live.sleep_start = Some(calendar.at(date("2025-09-22"), 18, 0));
    let result = build(&request(&dataset, &calendar, Metric::Sleep));
    assert!(result.today.has_data);
    assert!(last_value(&result.today.points).abs() < 0.001);
    assert_eq!(
        result.today.markers,
        vec![Marker {
            point: Point {
                seconds: 43_200.0,
                value: 0.0
            },
            symbol: 'o',
        }]
    );
}

#[test]
fn completed_nap_count_is_known_zero_when_an_ongoing_sleep_is_recorded() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.live.sleep_active = true;
    dataset.live.sleep_start = Some(calendar.at(date("2025-09-22"), 16, 0));
    let result = build(&request(&dataset, &calendar, Metric::Naps));
    assert!(result.today.has_data);
    assert!(last_value(&result.today.points).abs() < 0.001);
    assert!(
        !result
            .today
            .markers
            .iter()
            .any(|marker| marker.symbol == 'x')
    );
}

#[test]
fn completed_nap_count_retains_its_actual_end_marker() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.sleep = vec![sleep(calendar.at(date("2025-09-22"), 7, 0), 1_800.0)];
    let result = build(&request(&dataset, &calendar, Metric::Naps));
    assert_eq!(
        result.today.markers,
        vec![
            Marker {
                point: Point {
                    seconds: 3_600.0,
                    value: 1.0
                },
                symbol: 'o'
            },
            Marker {
                point: Point {
                    seconds: 5_400.0,
                    value: 1.0
                },
                symbol: 'x'
            },
        ]
    );
}

#[test]
fn advancing_online_time_does_not_complete_history_fetched_before_day_end() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.fetched_at = calendar.at(date("2025-09-23"), 5, 59);
    dataset.feeds = vec![bottle(calendar.at(date("2025-09-22"), 7, 0), 90.0)];
    let mut request = request(&dataset, &calendar, Metric::Milk);
    request.now = calendar.at(date("2025-09-23"), 6, 15);
    request.coverage.1 = request.now;
    request.selected = date("2025-09-22");
    let result = build(&request);
    assert_eq!(result.average_days, 0);
    let selected = result.selected.expect("yesterday");
    assert!(selected.is_partial);
    assert!((selected.observed - 86_340.0).abs() < 0.001);
}

#[test]
fn a_day_nap_graph_excludes_its_end_after_night_begins() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.sleep = vec![sleep(calendar.at(date("2025-09-22"), 19, 30), 5_400.0)];
    let mut request = request(&dataset, &calendar, Metric::Naps);
    request.now = calendar.at(date("2025-09-22"), 22, 0);
    request.measure.period = Period::Day;
    let result = build(&request);
    let markers: Vec<_> = result
        .today
        .markers
        .iter()
        .map(|marker| {
            (
                marker.point.seconds as i64,
                marker.point.value as i64,
                marker.symbol,
            )
        })
        .collect();
    assert_eq!(markers, vec![(48_600, 1, 'o')]);
}

#[test]
fn a_night_nap_graph_has_no_end_marker_for_a_daytime_nap() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.sleep = vec![sleep(calendar.at(date("2025-09-22"), 19, 30), 5_400.0)];
    let mut request = request(&dataset, &calendar, Metric::Naps);
    request.now = calendar.at(date("2025-09-22"), 22, 0);
    request.measure.period = Period::Night;
    let result = build(&request);
    assert!(result.today.has_data);
    assert!(last_value(&result.today.points).abs() < 0.001);
    assert!(result.today.markers.is_empty());
}

fn typed_bottle(start: f64, amount: f64, kind: &str) -> crate::domain::types::FeedEvent {
    let mut feed = bottle(start, amount);
    if let crate::domain::types::FeedEvent::Bottle { bottle_type, .. } = &mut feed {
        *bottle_type = Some(kind.to_owned());
    }
    feed
}

#[test]
fn bottle_category_markers_identify_only_the_selected_milk_type() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.feeds = vec![
        typed_bottle(calendar.at(date("2025-09-22"), 7, 0), 60.0, "Breast Milk"),
        bottle(calendar.at(date("2025-09-22"), 8, 0), 90.0),
    ];
    for (metric, seconds, value) in [
        (Metric::Formula, 7_200.0, 90.0),
        (Metric::BreastMilk, 3_600.0, 60.0),
    ] {
        let result = build(&request(&dataset, &calendar, metric));
        assert_eq!(
            result.today.markers,
            vec![Marker {
                point: Point { seconds, value },
                symbol: 'o',
            }]
        );
        assert!((last_value(&result.today.points) - value).abs() < 0.001);
    }
}

#[test]
fn the_other_milk_type_is_known_zero_for_averages_without_an_event_marker() {
    let calendar = Calendar::utc();
    for (metric, kind) in [
        (Metric::Formula, "Breast Milk"),
        (Metric::BreastMilk, "Formula"),
    ] {
        let mut dataset = dataset();
        dataset.feeds = vec![typed_bottle(
            calendar.at(date("2025-09-21"), 7, 0),
            90.0,
            kind,
        )];
        let mut request = request(&dataset, &calendar, metric);
        request.selected = date("2025-09-21");
        let result = build(&request);
        let selected = result.selected.expect("the recorded day");
        assert!(selected.has_data);
        assert!(selected.markers.is_empty());
        assert!(last_value(&selected.points).abs() < 0.001);
        assert_eq!(result.average_days, 1);
        assert!(last_value(&result.average).abs() < 0.001);
    }
}

#[test]
fn matching_bottles_with_a_recorded_zero_keep_their_event_marker() {
    let calendar = Calendar::utc();
    for (metric, kind) in [
        (Metric::Formula, "Formula"),
        (Metric::BreastMilk, "Breast Milk"),
    ] {
        let mut dataset = dataset();
        dataset.feeds = vec![typed_bottle(
            calendar.at(date("2025-09-22"), 7, 0),
            0.0,
            kind,
        )];
        let result = build(&request(&dataset, &calendar, metric));
        assert!(result.today.has_data);
        assert_eq!(
            result.today.markers,
            vec![Marker {
                point: Point {
                    seconds: 3_600.0,
                    value: 0.0
                },
                symbol: 'o',
            }]
        );
    }
}

#[test]
fn period_curves_start_at_zero_and_stop_at_their_own_boundary() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.sleep = vec![sleep(calendar.at(date("2025-09-21"), 19, 0), 10_800.0)];
    for (period, duration, value) in [
        (Period::Day, 50_400.0, 3_600.0),
        (Period::Night, 36_000.0, 7_200.0),
    ] {
        let mut request = request(&dataset, &calendar, Metric::Sleep);
        request.selected = date("2025-09-21");
        request.measure.period = period;
        let result = build(&request);
        let selected = result.selected.unwrap();
        assert!((selected.duration - duration).abs() < 0.001);
        assert_eq!(
            selected.points.first(),
            Some(&Point {
                seconds: 0.0,
                value: 0.0
            })
        );
        assert_eq!(
            selected.points.last(),
            Some(&Point {
                seconds: duration,
                value
            })
        );
        assert_eq!(
            result.average.last(),
            Some(&Point {
                seconds: duration,
                value
            })
        );
        assert!(
            selected
                .markers
                .iter()
                .all(|marker| marker.point.seconds <= duration)
        );
    }
}

#[test]
fn wake_markers_describe_waking_instead_of_sleeping() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.sleep = vec![
        sleep(calendar.at(date("2025-09-22"), 5, 0), 7_200.0),
        sleep(calendar.at(date("2025-09-22"), 9, 0), 3_600.0),
    ];
    let result = build(&request(&dataset, &calendar, Metric::Wake));
    let markers: Vec<_> = result
        .today
        .markers
        .iter()
        .map(|marker| (marker.point.seconds as i64, marker.symbol))
        .collect();
    assert_eq!(markers, vec![(3_600, 'o'), (10_800, 'x'), (14_400, 'o')]);
}

#[test]
fn nursing_marks_both_actual_endpoints() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.feeds = vec![nursing(calendar.at(date("2025-09-22"), 7, 0), 600.0, 300.0)];
    let result = build(&request(&dataset, &calendar, Metric::Nursing));
    let markers: Vec<_> = result
        .today
        .markers
        .iter()
        .map(|marker| (marker.point.seconds as i64, marker.symbol))
        .collect();
    assert_eq!(markers, vec![(3_600, 'o'), (4_500, 'x')]);
}

#[test]
fn a_future_night_has_no_today_points_or_markers() {
    let calendar = Calendar::utc();
    let mut dataset = dataset();
    dataset.sleep = vec![sleep(calendar.at(date("2025-09-22"), 7, 0), 3_600.0)];
    let mut request = request(&dataset, &calendar, Metric::Sleep);
    request.measure.period = Period::Night;
    let result = build(&request);
    assert!(result.today.observed.abs() < 0.001);
    assert!(result.today.points.is_empty());
    assert!(result.today.markers.is_empty());
}

#[test]
fn night_curves_retain_dst_elapsed_lengths_and_a_fixed_average_denominator() {
    let calendar = Calendar::new("America/New_York").unwrap();
    for (night, following, now, expected_seconds) in [
        ("2025-03-08", "2025-03-09", "2025-03-10", 32_400.0),
        ("2025-11-01", "2025-11-02", "2025-11-03", 39_600.0),
    ] {
        let mut data = dataset();
        data.sleep = vec![
            sleep(calendar.at(date(night), 20, 0), expected_seconds),
            sleep(calendar.at(date(following), 20, 0), 36_000.0),
        ];
        data.fetched_at = calendar.at(date(now), 18, 0);
        let mut request = request(&data, &calendar, Metric::Sleep);
        request.now = data.fetched_at;
        request.selected = date(night);
        request.measure.period = Period::Night;
        let result = build(&request);
        let selected = result.selected.unwrap();
        assert!((selected.duration - expected_seconds).abs() < 0.001);
        assert!((last_value(&selected.points) - expected_seconds).abs() < 0.001);
        assert!((result.clock_duration - 36_000.0).abs() < 0.001);
        assert_eq!(result.average_days, 2);
        let average = result.average.last().unwrap();
        let common_seconds = expected_seconds.min(36_000.0);
        assert!((average.seconds - common_seconds).abs() < 0.001);
        assert!((average.value - common_seconds).abs() < 0.001);
    }
}

#[test]
fn wake_ends_at_period_boundaries_keep_their_marker_for_saved_and_live_sleep() {
    let calendar = Calendar::utc();
    for (period, day, hour, elapsed) in [
        (Period::Day, "2025-09-21", 20, 50_400),
        (Period::Night, "2025-09-22", 6, 36_000),
    ] {
        for is_live in [false, true] {
            let mut data = dataset();
            let end = calendar.at(date(day), hour, 0);
            data.sleep = vec![sleep(end - 10_800.0, 3_600.0)];
            if is_live {
                data.live.sleep_active = true;
                data.live.sleep_start = Some(end);
            } else {
                data.sleep.push(sleep(end, 3_600.0));
            }
            let mut request = request(&data, &calendar, Metric::Wake);
            request.selected = date("2025-09-21");
            request.measure.period = period;
            let result = build(&request);
            let selected = result.selected.unwrap();
            let markers: Vec<_> = selected
                .markers
                .iter()
                .map(|marker| (marker.point.seconds as i64, marker.symbol))
                .collect();
            assert_eq!(
                markers,
                vec![
                    (elapsed - 10_800, 'x'),
                    (elapsed - 7_200, 'o'),
                    (elapsed, 'x')
                ]
            );
        }
    }
}
