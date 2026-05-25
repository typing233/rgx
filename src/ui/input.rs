use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::ast::highlight::{style_for_token, tokenize};
use crate::engine::MatchSpan;

pub struct RegexInput<'a> {
    pub content: &'a str,
    pub cursor: usize,
    pub focused: bool,
    pub title: &'a str,
    pub highlight_syntax: bool,
}

impl<'a> Widget for RegexInput<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(self.title);

        let inner = block.inner(area);
        block.render(area, buf);

        let width = inner.width as usize;
        let cursor_display_pos = UnicodeWidthStr::width(&self.content[..self.cursor]);
        let scroll_offset = compute_scroll(cursor_display_pos, width);

        if self.highlight_syntax && !self.content.is_empty() {
            let tokens = tokenize(self.content);
            let mut spans = Vec::new();
            let mut last_end = 0;

            for token in &tokens {
                if token.start > last_end {
                    spans.push(Span::styled(
                        self.content[last_end..token.start].to_string(),
                        Style::default().fg(Color::White),
                    ));
                }
                spans.push(Span::styled(
                    self.content[token.start..token.end].to_string(),
                    style_for_token(token),
                ));
                last_end = token.end;
            }
            if last_end < self.content.len() {
                spans.push(Span::styled(
                    self.content[last_end..].to_string(),
                    Style::default().fg(Color::White),
                ));
            }

            let line = Line::from(spans);
            Paragraph::new(line)
                .scroll((0, scroll_offset as u16))
                .render(inner, buf);
        } else {
            Paragraph::new(self.content)
                .scroll((0, scroll_offset as u16))
                .render(inner, buf);
        }

        if self.focused {
            let cx = inner.x + (cursor_display_pos - scroll_offset) as u16;
            if cx >= inner.x && cx < inner.x + inner.width {
                if let Some(cell) = buf.cell_mut((cx, inner.y)) {
                    cell.set_style(Style::default().bg(Color::White).fg(Color::Black));
                }
            }
        }
    }
}

pub struct TextInput<'a> {
    pub content: &'a str,
    pub cursor: usize,
    pub focused: bool,
    pub title: &'a str,
    pub matches: Option<&'a [MatchSpan]>,
}

impl<'a> Widget for TextInput<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(self.title);

        let inner = block.inner(area);
        block.render(area, buf);

        let width = inner.width as usize;
        let cursor_display_pos = UnicodeWidthStr::width(&self.content[..self.cursor]);
        let scroll_offset = compute_scroll(cursor_display_pos, width);

        let line = match self.matches {
            Some(matches) if !matches.is_empty() => build_highlighted_line(self.content, matches),
            _ => Line::from(Span::styled(
                self.content.to_string(),
                Style::default().fg(Color::White),
            )),
        };

        Paragraph::new(line)
            .scroll((0, scroll_offset as u16))
            .render(inner, buf);

        if self.focused {
            let cx = inner.x + (cursor_display_pos - scroll_offset) as u16;
            if cx >= inner.x && cx < inner.x + inner.width {
                if let Some(cell) = buf.cell_mut((cx, inner.y)) {
                    cell.set_style(Style::default().bg(Color::White).fg(Color::Black));
                }
            }
        }
    }
}

fn build_highlighted_line(text: &str, matches: &[MatchSpan]) -> Line<'static> {
    let mut spans = Vec::new();
    let mut last_end = 0;

    for m in matches {
        if m.start > last_end {
            spans.push(Span::styled(
                text[last_end..m.start].to_string(),
                Style::default().fg(Color::White),
            ));
        }
        spans.push(Span::styled(
            text[m.start..m.end].to_string(),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        last_end = m.end;
    }

    if last_end < text.len() {
        spans.push(Span::styled(
            text[last_end..].to_string(),
            Style::default().fg(Color::White),
        ));
    }

    Line::from(spans)
}

fn compute_scroll(cursor_pos: usize, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    if cursor_pos >= width {
        cursor_pos - width + 1
    } else {
        0
    }
}
