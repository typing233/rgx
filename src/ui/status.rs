use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::engine::{EngineLevel, RegexFlags};

pub struct StatusBar<'a> {
    pub engine: EngineLevel,
    pub manual_override: bool,
    pub flags: &'a RegexFlags,
    pub match_count: usize,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        let engine_label = if self.manual_override {
            format!(" Engine: {} (manual) ", self.engine)
        } else {
            format!(" Engine: {} (auto) ", self.engine)
        };

        let flags_str = format_flags(self.flags);

        let spans = vec![
            Span::styled(
                engine_label,
                Style::default().fg(Color::Black).bg(engine_color(self.engine)).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                flags_str,
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(" │ "),
            Span::styled(
                format!("{} matches", self.match_count),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" │ "),
            Span::styled(
                "Tab:focus C-e:engine C-d:debug C-g:code C-z/y:undo/redo C-c:copy Esc:quit",
                Style::default().fg(Color::DarkGray),
            ),
        ];

        Paragraph::new(Line::from(spans)).render(inner, buf);
    }
}

fn engine_color(engine: EngineLevel) -> Color {
    match engine {
        EngineLevel::Basic => Color::Green,
        EngineLevel::Fancy => Color::Yellow,
        EngineLevel::Pcre2 => Color::Magenta,
    }
}

fn format_flags(flags: &RegexFlags) -> String {
    let mut parts = Vec::new();
    if flags.case_insensitive { parts.push("i"); }
    if flags.multiline { parts.push("m"); }
    if flags.dot_matches_newline { parts.push("s"); }
    if flags.unicode { parts.push("u"); }
    if flags.extended { parts.push("x"); }
    if parts.is_empty() {
        "flags: none".to_string()
    } else {
        format!("flags: {}", parts.join(""))
    }
}
