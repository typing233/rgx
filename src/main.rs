#![allow(dead_code)]

mod app;
mod ast;
mod engine;
mod ui;

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    Terminal,
};

use app::{App, Focus};
use ui::input::{RegexInput, TextInput};
use ui::results::MatchResultsWidget;
use ui::status::StatusBar;

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|frame| {
            let size = frame.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(8),
                    Constraint::Length(2),
                ])
                .split(size);

            let regex_widget = RegexInput {
                content: &app.regex_input,
                cursor: app.regex_cursor,
                focused: app.focus == Focus::Regex,
                title: " Regex Pattern ",
                highlight_syntax: true,
            };
            frame.render_widget(regex_widget, chunks[0]);

            let match_spans = app.last_result.as_ref().map(|r| r.full_matches.as_slice());
            let text_widget = TextInput {
                content: &app.test_input,
                cursor: app.test_cursor,
                focused: app.focus == Focus::TestString,
                title: " Test String ",
                matches: match_spans,
            };
            frame.render_widget(text_widget, chunks[1]);

            let results_widget = MatchResultsWidget {
                test_text: &app.test_input,
                result: app.last_result.as_ref(),
                error: app.last_error.as_deref(),
            };
            frame.render_widget(results_widget, chunks[2]);

            let status_widget = StatusBar {
                engine: app.engine_manager.current_level(),
                manual_override: app.engine_manager.is_manual(),
                flags: &app.flags,
                match_count: app.match_count(),
            };
            frame.render_widget(status_widget, chunks[3]);
        })?;

        if app.should_quit {
            return Ok(());
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                handle_key(app, key);
            }
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
    match (key.modifiers, key.code) {
        (_, KeyCode::Esc) => {
            app.should_quit = true;
        }
        (_, KeyCode::Tab) => {
            app.toggle_focus();
        }
        (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
            app.cycle_engine();
        }
        (KeyModifiers::ALT, KeyCode::Char('i')) => {
            app.toggle_flag_case_insensitive();
        }
        (KeyModifiers::ALT, KeyCode::Char('m')) => {
            app.toggle_flag_multiline();
        }
        (KeyModifiers::ALT, KeyCode::Char('s')) => {
            app.toggle_flag_dotall();
        }
        (KeyModifiers::ALT, KeyCode::Char('u')) => {
            app.toggle_flag_unicode();
        }
        (KeyModifiers::ALT, KeyCode::Char('x')) => {
            app.toggle_flag_extended();
        }
        (_, KeyCode::Backspace) => {
            app.delete_char();
        }
        (_, KeyCode::Delete) => {
            app.delete_forward();
        }
        (_, KeyCode::Left) => {
            app.move_cursor_left();
        }
        (_, KeyCode::Right) => {
            app.move_cursor_right();
        }
        (_, KeyCode::Home) => {
            app.move_cursor_home();
        }
        (_, KeyCode::End) => {
            app.move_cursor_end();
        }
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
            app.insert_char(c);
        }
        _ => {}
    }
}
