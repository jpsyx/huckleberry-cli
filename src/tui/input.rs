//! Routes input to the focused panel before a command can consume it.

use crate::prompt::host::Input;

use super::{
    App, Intent, Motion,
    job::Job,
    keys::{forces_quit, motion_for},
    state::Panel,
};

/// Routes input while discarding a retained command only when navigation abandons it.
pub fn route_with_draft(
    app: &mut App,
    job: Option<&mut Job>,
    suspended: &mut Option<super::forms::Forms>,
    input: Input,
    height: usize,
    now: f64,
) -> Intent {
    let should_discard = match &input {
        Input::Key(key) => match motion_for(*key) {
            Motion::Home => true,
            Motion::Cancel => app.focus == Panel::Menu && app.version.is_none(),
            _ => false,
        },
        Input::Pasted(_) => false,
    };
    if should_discard && !app.summary.state.is_comparing {
        *suspended = None;
    }
    route(app, job, input, height, now)
}

/// Applies one input to its owner, returning work for the shell's event loop.
pub fn route(
    app: &mut App,
    job: Option<&mut Job>,
    input: Input,
    height: usize,
    now: f64,
) -> Intent {
    if let Input::Key(key) = &input {
        if forces_quit(*key) {
            return Intent::Quit;
        }
        if app.summary.state.is_comparing {
            app.summary.state.apply(*key);
            return Intent::Stay;
        }
        let motion = motion_for(*key);
        if app.version.is_some() || matches!(motion, Motion::FocusMenu | Motion::FocusSummary) {
            return app.apply(motion, height);
        }
    }
    if app.focus == Panel::Summary {
        return summary_input(app, job, &input, height, now);
    }
    if let Some(job) = job {
        job.feed(input, now);
        return Intent::Stay;
    }
    let Input::Key(key) = input else {
        return Intent::Stay;
    };
    if app.dashboard.is_some() {
        return dashboard_key(app, key);
    }
    app.apply(motion_for(key), height)
}

fn summary_input(
    app: &mut App,
    job: Option<&mut Job>,
    input: &Input,
    height: usize,
    now: f64,
) -> Intent {
    let Input::Key(key) = *input else {
        return Intent::Stay;
    };
    if key.kind != crossterm::event::KeyEventKind::Press {
        return Intent::Stay;
    }
    match motion_for(key) {
        Motion::Quit => Intent::Quit,
        Motion::Refresh => Intent::Refresh,
        Motion::Home => {
            app.focus = Panel::Menu;
            job.map_or_else(
                || app.apply(Motion::Home, height),
                |job| {
                    job.feed(Input::Key(key), now);
                    Intent::Stay
                },
            )
        }
        Motion::Cancel => {
            app.focus = Panel::Menu;
            Intent::Stay
        }
        _ => {
            if app.summary.state.apply(key) {
                app.focus = Panel::Menu;
            }
            Intent::Stay
        }
    }
}

/// The dashboard occupies the left panel and shares the shell's refresh.
fn dashboard_key(app: &mut App, key: crossterm::event::KeyEvent) -> Intent {
    match motion_for(key) {
        Motion::Quit => return Intent::Quit,
        Motion::Home => app.go_home(),
        Motion::Back | Motion::Cancel => {
            app.close_dashboard();
        }
        Motion::Refresh => return Intent::Refresh,
        _ => {
            if let Some(dashboard) = app.dashboard.as_mut() {
                dashboard.apply(key);
            }
        }
    }
    Intent::Stay
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    fn press(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Intent {
        route(
            app,
            None,
            Input::Key(KeyEvent::new(code, modifiers)),
            20,
            0.0,
        )
    }

    #[test]
    fn every_focus_alias_preserves_both_panel_selections() {
        for (left, right) in [
            (KeyCode::Left, KeyCode::Right),
            (KeyCode::Char('h'), KeyCode::Char('l')),
            (KeyCode::Char('a'), KeyCode::Char('d')),
            (KeyCode::Char('H'), KeyCode::Char('L')),
            (KeyCode::Char('A'), KeyCode::Char('D')),
        ] {
            let mut app = App::new();
            press(&mut app, KeyCode::Down, KeyModifiers::NONE);
            press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
            assert_eq!(app.breadcrumb(), "Home › Log a feed");
            press(&mut app, right, KeyModifiers::ALT);
            press(&mut app, KeyCode::Right, KeyModifiers::NONE);
            press(&mut app, KeyCode::Down, KeyModifiers::NONE);
            assert_eq!(app.focus, Panel::Summary);
            assert_eq!(app.summary.state.selected, 1);
            assert_eq!(app.cursor(), 0);
            press(&mut app, left, KeyModifiers::ALT);
            assert_eq!(app.focus, Panel::Menu);
            assert_eq!(app.breadcrumb(), "Home › Log a feed");
            press(&mut app, KeyCode::Down, KeyModifiers::NONE);
            assert_eq!(app.cursor(), 1);
            assert_eq!(app.summary.state.selected, 1);
        }
    }

    #[test]
    fn modal_and_release_keys_cannot_change_panel_focus() {
        let mut app = App::new();
        route(
            &mut app,
            None,
            Input::Key(KeyEvent::new_with_kind(
                KeyCode::Right,
                KeyModifiers::ALT,
                KeyEventKind::Release,
            )),
            20,
            0.0,
        );
        assert_eq!(app.focus, Panel::Menu);
        app.open_version(true);
        press(&mut app, KeyCode::Right, KeyModifiers::ALT);
        assert_eq!(app.focus, Panel::Menu);
        assert!(app.version.is_some());
    }

    #[test]
    fn summary_can_refresh_quit_and_return_without_closing() {
        let mut app = App::new();
        press(&mut app, KeyCode::Right, KeyModifiers::ALT);
        assert_eq!(
            press(&mut app, KeyCode::Char('r'), KeyModifiers::NONE),
            Intent::Refresh
        );
        assert_eq!(
            press(&mut app, KeyCode::Char('q'), KeyModifiers::NONE),
            Intent::Quit
        );
        press(&mut app, KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(app.focus, Panel::Menu);
        press(&mut app, KeyCode::Right, KeyModifiers::ALT);
        press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.focus, Panel::Menu);
        assert_eq!(app.breadcrumb(), "Home");
    }
}

#[cfg(test)]
mod comparison_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn comparison_captures_focus_and_escape_without_discarding_navigation() {
        let mut app = App::new();
        app.focus = Panel::Summary;
        app.summary.state.is_comparing = true;
        app.summary.state.selected = 2;
        let key = |code, modifiers| Input::Key(KeyEvent::new(code, modifiers));
        route(
            &mut app,
            None,
            key(KeyCode::Left, KeyModifiers::ALT),
            20,
            0.0,
        );
        assert_eq!(app.focus, Panel::Summary);
        assert!(app.summary.state.is_comparing);
        route(
            &mut app,
            None,
            key(KeyCode::Esc, KeyModifiers::NONE),
            20,
            0.0,
        );
        assert!(!app.summary.state.is_comparing);
        assert_eq!(app.focus, Panel::Summary);
        assert_eq!(app.summary.state.selected, 2);
    }
}
