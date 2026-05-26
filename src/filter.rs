use std::io::{self, BufRead, Write};

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

pub struct FilterConfig {
    pub json_field: Option<String>,
    pub file_path: Option<String>,
}

pub fn run_filter(config: FilterConfig) -> io::Result<()> {
    let lines = read_input_lines(&config)?;

    if !atty::is(atty::Stream::Stdout) {
        // stdout is not a TTY: read regex from stdin if possible, otherwise output all
        // Actually in pipe mode we skip TUI and just output matching lines
        // But we need regex from somewhere - in pipe mode just pass through
        // Better: if stdout is not TTY, we assume stdin has data and no interactive input
        // Just output all lines (no filter in non-interactive mode without regex arg)
        for line in &lines {
            writeln!(io::stdout(), "{}", line)?;
        }
        return Ok(());
    }

    run_filter_tui(lines, config.json_field)
}

fn read_input_lines(config: &FilterConfig) -> io::Result<Vec<String>> {
    if let Some(path) = &config.file_path {
        let content = std::fs::read_to_string(path)?;
        Ok(content.lines().map(|l| l.to_string()).collect())
    } else {
        let stdin = io::stdin();
        let lines: Vec<String> = stdin.lock().lines().collect::<io::Result<Vec<_>>>()?;
        Ok(lines)
    }
}

struct FilterApp {
    pattern: String,
    cursor: usize,
    lines: Vec<String>,
    filtered: Vec<usize>,
    scroll: usize,
    json_field: Option<String>,
    error: Option<String>,
}

impl FilterApp {
    fn new(lines: Vec<String>, json_field: Option<String>) -> Self {
        let filtered: Vec<usize> = (0..lines.len()).collect();
        Self {
            pattern: String::new(),
            cursor: 0,
            lines,
            filtered,
            scroll: 0,
            json_field,
            error: None,
        }
    }

    fn update_filter(&mut self) {
        if self.pattern.is_empty() {
            self.filtered = (0..self.lines.len()).collect();
            self.error = None;
            self.scroll = 0;
            return;
        }

        match regex::Regex::new(&self.pattern) {
            Ok(re) => {
                self.error = None;
                self.filtered = self.lines.iter().enumerate()
                    .filter(|(_, line)| {
                        let target = self.extract_target(line);
                        re.is_match(&target)
                    })
                    .map(|(i, _)| i)
                    .collect();
                self.scroll = 0;
            }
            Err(e) => {
                self.error = Some(format!("Regex error: {}", e));
            }
        }
    }

    fn extract_target(&self, line: &str) -> String {
        if let Some(field) = &self.json_field {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                return extract_json_field(&val, field)
                    .unwrap_or_default();
            }
        }
        line.to_string()
    }

    fn insert_char(&mut self, c: char) {
        self.pattern.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.update_filter();
    }

    fn delete_char(&mut self) {
        if self.cursor > 0 {
            let prev = prev_char_boundary(&self.pattern, self.cursor);
            self.pattern.drain(prev..self.cursor);
            self.cursor = prev;
            self.update_filter();
        }
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = prev_char_boundary(&self.pattern, self.cursor);
        }
    }

    fn move_right(&mut self) {
        if self.cursor < self.pattern.len() {
            self.cursor = next_char_boundary(&self.pattern, self.cursor);
        }
    }

    fn output_results(&self) -> io::Result<()> {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        for &idx in &self.filtered {
            writeln!(out, "{}", self.lines[idx])?;
        }
        Ok(())
    }
}

fn run_filter_tui(lines: Vec<String>, json_field: Option<String>) -> io::Result<()> {
    enable_raw_mode()?;
    io::stderr().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stderr());
    let mut terminal = Terminal::new(backend)?;

    let mut app = FilterApp::new(lines, json_field);
    let mut should_output = false;

    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(1),
                    Constraint::Min(5),
                ])
                .split(frame.area());

            // Pattern input
            let input_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Filter Pattern ");
            let input_para = Paragraph::new(app.pattern.as_str())
                .block(input_block);
            frame.render_widget(input_para, chunks[0]);

            // Status
            let status_text = if let Some(err) = &app.error {
                Line::from(Span::styled(err.as_str(), Style::default().fg(Color::Red)))
            } else {
                Line::from(vec![
                    Span::styled(
                        format!(" {}/{} lines ", app.filtered.len(), app.lines.len()),
                        Style::default().fg(Color::Green),
                    ),
                    Span::raw(" │ "),
                    Span::styled(
                        "Enter:output  Esc:cancel",
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            };
            frame.render_widget(Paragraph::new(status_text), chunks[1]);

            // Filtered lines
            let visible_height = chunks[2].height as usize;
            let items: Vec<ListItem> = app.filtered.iter()
                .skip(app.scroll)
                .take(visible_height)
                .map(|&idx| {
                    let line = &app.lines[idx];
                    ListItem::new(Line::from(Span::raw(line.to_string())))
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(" Results "));
            frame.render_widget(list, chunks[2]);

            // Render cursor
            let inner_x = chunks[0].x + 1 + app.cursor as u16;
            let inner_y = chunks[0].y + 1;
            frame.set_cursor_position((inner_x, inner_y));
        })?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match (key.modifiers, key.code) {
                    (_, KeyCode::Esc) => break,
                    (_, KeyCode::Enter) => {
                        should_output = true;
                        break;
                    }
                    (_, KeyCode::Backspace) => app.delete_char(),
                    (_, KeyCode::Left) => app.move_left(),
                    (_, KeyCode::Right) => app.move_right(),
                    (_, KeyCode::Down) => {
                        let max_scroll = app.filtered.len().saturating_sub(1);
                        if app.scroll < max_scroll {
                            app.scroll += 1;
                        }
                    }
                    (_, KeyCode::Up) => {
                        if app.scroll > 0 {
                            app.scroll -= 1;
                        }
                    }
                    (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                        app.insert_char(c);
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    io::stderr().execute(LeaveAlternateScreen)?;

    if should_output {
        app.output_results()?;
    }

    Ok(())
}

fn extract_json_field(value: &serde_json::Value, path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;

    for part in parts {
        match current {
            serde_json::Value::Object(map) => {
                current = map.get(part)?;
            }
            serde_json::Value::Array(arr) => {
                let idx: usize = part.parse().ok()?;
                current = arr.get(idx)?;
            }
            _ => return None,
        }
    }

    match current {
        serde_json::Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

fn prev_char_boundary(s: &str, pos: usize) -> usize {
    let mut p = pos.saturating_sub(1);
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

fn next_char_boundary(s: &str, pos: usize) -> usize {
    let mut p = pos + 1;
    while p < s.len() && !s.is_char_boundary(p) {
        p += 1;
    }
    p
}
