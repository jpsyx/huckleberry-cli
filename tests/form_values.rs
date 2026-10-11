//! A remembered instant or quantity must not change merely by being revisited.
use app::cli::Units;
use app::domain::Calendar;
use app::prompt::{
    dial::time::{Mode, Opening},
    flow::{ControlEvent, time::TimeState, volume::VolumeState},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn clock(at: f64, now: f64, mode: Mode) -> TimeState {
    TimeState::create_time(
        Calendar::new("America/New_York").unwrap(),
        Opening {
            at,
            mode,
            not_before: None,
        },
        now,
    )
}

#[test]
fn accepted_relative_time_does_not_move() {
    let mut time = clock(8800.0, 10000.0, Mode::Relative);
    assert_eq!(time.accept(10000.0).unwrap().to_bits(), 8800_f64.to_bits());
    assert_eq!(time.accept(10300.0).unwrap().to_bits(), 8800_f64.to_bits());
}

#[test]
fn untouched_time_keeps_precision() {
    // The two distinct 01:30 instants at the fall-back, plus local midnight.
    for instant in [1_762_061_400.125, 1_762_065_000.125, 1_762_056_000.125] {
        let mut time = clock(instant, instant + 7200.0, Mode::Absolute);
        assert_eq!(
            time.accept(instant + 10800.0).unwrap().to_bits(),
            instant.to_bits()
        );
        time.apply(press(KeyCode::Tab), instant + 10800.0);
        time.apply(press(KeyCode::Tab), instant + 10800.0);
        assert_eq!(
            time.accept(instant + 10860.0).unwrap().to_bits(),
            instant.to_bits()
        );
    }
}

#[test]
fn explicit_now_updates_time() {
    let mut time = clock(8800.0, 10000.0, Mode::Relative);
    time.accept(10000.0).unwrap();
    time.apply(press(KeyCode::Char('n')), 10300.0);
    assert_eq!(time.accept(10300.0).unwrap().to_bits(), 10300_f64.to_bits());
}

#[test]
fn explicit_now_replaces_a_matching_clock_minute_and_its_date() {
    let now = 1_762_065_000.125;
    for instant in [now - 86_400.0, now - 3600.0, now - 0.125] {
        let mut time = clock(instant, now, Mode::Absolute);
        time.apply(press(KeyCode::Char('n')), now);
        assert_eq!(time.accept(now).unwrap().to_bits(), now.to_bits());
    }
    let now = 100_020.125;
    let mut time = clock(now - 86_400.0, now, Mode::Absolute);
    time.apply(press(KeyCode::Char('n')), now);
    time.apply(press(KeyCode::Right), now);
    time.apply(press(KeyCode::Up), now);
    assert_eq!(
        time.accept(now).unwrap().to_bits(),
        (now - 60.125).to_bits()
    );
    let mut time = clock(now - 0.125, now, Mode::Relative);
    time.accept(now).unwrap();
    time.apply(press(KeyCode::Char('n')), now);
    assert_eq!(time.accept(now).unwrap().to_bits(), now.to_bits());
}

#[test]
fn clock_switch_keeps_instant() {
    let mut time = clock(8800.125, 10000.0, Mode::Relative);
    time.apply(press(KeyCode::Tab), 10300.0);
    time.apply(press(KeyCode::Right), 10300.0);
    time.apply(press(KeyCode::Tab), 10300.0);
    assert_eq!(
        time.accept(10300.0).unwrap().to_bits(),
        8800.125_f64.to_bits()
    );
}

#[test]
fn dial_back_leaves_only_column_zero() {
    for code in [
        KeyCode::Left,
        KeyCode::Char('h'),
        KeyCode::Char('a'),
        KeyCode::Char('H'),
        KeyCode::Char('A'),
    ] {
        let mut time = clock(8800.0, 10000.0, Mode::Relative);
        assert_eq!(time.apply(press(code), 10000.0), ControlEvent::Stay);
        assert_eq!(time.apply(press(code), 10000.0), ControlEvent::Back);
    }
}

#[test]
fn units_preserve_quantity() {
    let mut volume = VolumeState::create_volume(Units::Ml, Some(90.125), 1.0);
    volume.apply(press(KeyCode::Right));
    volume.apply(press(KeyCode::Up));
    let (units, amount) = volume.value();
    assert_eq!(units, Units::Oz);
    assert!((amount - 90.125 / 29.573_529_562_5).abs() < 1e-9);
    volume.apply(press(KeyCode::Down));
    let (units, amount) = volume.value();
    assert_eq!(units, Units::Ml);
    assert!((amount - 90.125).abs() < 1e-9);
}

#[test]
fn untouched_ounce_amount_keeps_its_original_bits() {
    let original = 0.07_f64;
    let mut volume = VolumeState::create_volume(Units::Oz, Some(original), 0.0);
    assert_eq!(volume.value().1.to_bits(), original.to_bits());
    volume.apply(press(KeyCode::Right));
    volume.apply(press(KeyCode::Up));
    volume.apply(press(KeyCode::Down));
    assert_eq!(volume.value().0, Units::Oz);
    assert_eq!(volume.value().1.to_bits(), original.to_bits());
}
