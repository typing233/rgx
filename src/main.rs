#![allow(dead_code)]

mod app;
mod ast;
mod codegen;
mod debugger;
mod engine;
mod explain;
mod filter;
mod ui;

use std::io;
use std::time::Duration;

use clap::{Parser, Subcommand};
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

use app::{App, Focus, Overlay};
use codegen::Language;
use filter::FilterConfig;
use ui::input::{RegexInput, TextInput};
use ui::overlay::{CodeGenPanel, DebugPanel, ExplainPanel};
use ui::results::MatchResultsWidget;
use ui::status::StatusBar;

#[derive(Parser)]
#[command(name = "rgx", version, about = "Interactive regex tester and debugger")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Filter stdin/file lines using interactive regex
    Filter {
        /// File to read (reads stdin if omitted)
        file: Option<String>,
        /// JSON field path to extract for matching (dot-separated)
        #[arg(long)]
        json: Option<String>,
    },
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Filter { file, json }) => {
            let config = FilterConfig {
                json_field: json,
                file_path: file,
            };
            filter::run_filter(config)
        }
        None => run_interactive()
    }
}

fn run_interactive() -> io::Result<()> {
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

            match app.overlay {
                Overlay::Debugger => draw_debugger_view(frame, app, size),
                Overlay::CodeGen => draw_codegen_view(frame, app, size),
                Overlay::None => draw_normal_view(frame, app, size),
            }
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

fn draw_normal_view(
    frame: &mut ratatui::Frame,
    app: &App,
    size: ratatui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // regex input
            Constraint::Length(3),  // test input
            Constraint::Min(5),    // results
            Constraint::Length(3), // explanation
            Constraint::Length(2), // status bar
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

    let explain_widget = ExplainPanel {
        explanation: &app.explanation,
    };
    frame.render_widget(explain_widget, chunks[3]);

    let status_widget = StatusBar {
        engine: app.engine_manager.current_level(),
        manual_override: app.engine_manager.is_manual(),
        flags: &app.flags,
        match_count: app.match_count(),
    };
    frame.render_widget(status_widget, chunks[4]);
}

fn draw_debugger_view(
    frame: &mut ratatui::Frame,
    app: &App,
    size: ratatui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(10),
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

    if let Some(session) = &app.debug_session {
        let debug_widget = DebugPanel {
            session,
            pattern: &app.regex_input,
            text: &app.test_input,
        };
        frame.render_widget(debug_widget, chunks[2]);
    }

    let status_widget = StatusBar {
        engine: app.engine_manager.current_level(),
        manual_override: app.engine_manager.is_manual(),
        flags: &app.flags,
        match_count: app.match_count(),
    };
    frame.render_widget(status_widget, chunks[3]);
}

fn draw_codegen_view(
    frame: &mut ratatui::Frame,
    app: &App,
    size: ratatui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
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

    let lang_names: Vec<&str> = Language::ALL.iter().map(|l| l.name()).collect();
    let code = app.codegen_output.as_deref().unwrap_or("");
    let codegen_widget = CodeGenPanel {
        selected: app.codegen_selected,
        code,
        languages: &lang_names,
    };
    frame.render_widget(codegen_widget, chunks[1]);

    let status_widget = StatusBar {
        engine: app.engine_manager.current_level(),
        manual_override: app.engine_manager.is_manual(),
        flags: &app.flags,
        match_count: app.match_count(),
    };
    frame.render_widget(status_widget, chunks[2]);
}

fn handle_key(app: &mut App, key: KeyEvent) {
    // Handle overlay-specific keys first
    match app.overlay {
        Overlay::Debugger => {
            match (key.modifiers, key.code) {
                (_, KeyCode::Esc) => {
                    app.toggle_debugger();
                    return;
                }
                (_, KeyCode::Left) => {
                    if let Some(session) = &mut app.debug_session {
                        session.step_backward();
                    }
                    return;
                }
                (_, KeyCode::Right) => {
                    if let Some(session) = &mut app.debug_session {
                        session.step_forward();
                    }
                    return;
                }
                (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('h') | KeyCode::Char('H')) => {
                    if let Some(session) = &mut app.debug_session {
                        session.toggle_heatmap();
                    }
                    return;
                }
                _ => {}
            }
        }
        Overlay::CodeGen => {
            match (key.modifiers, key.code) {
                (_, KeyCode::Esc) => {
                    app.toggle_codegen();
                    return;
                }
                (_, KeyCode::Left) => {
                    app.codegen_select_prev();
                    return;
                }
                (_, KeyCode::Right) => {
                    app.codegen_select_next();
                    return;
                }
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    app.copy_codegen();
                    return;
                }
                _ => {}
            }
        }
        Overlay::None => {}
    }

    // Global key bindings
    match (key.modifiers, key.code) {
        (_, KeyCode::Esc) => {
            app.should_quit = true;
        }
        (_, KeyCode::Tab) => {
            app.toggle_focus();
        }
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            app.toggle_debugger();
        }
        (KeyModifiers::CONTROL, KeyCode::Char('g')) => {
            app.toggle_codegen();
        }
        (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
            app.cycle_engine();
        }
        (KeyModifiers::CONTROL, KeyCode::Char('z')) => {
            app.undo();
        }
        (KeyModifiers::CONTROL, KeyCode::Char('y')) => {
            app.redo();
        }
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            app.copy_matches();
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
