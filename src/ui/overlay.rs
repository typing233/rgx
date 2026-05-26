use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::debugger::DebugSession;

pub struct DebugPanel<'a> {
    pub session: &'a DebugSession,
    pub pattern: &'a str,
    pub text: &'a str,
}

impl<'a> Widget for DebugPanel<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta))
            .title(if self.session.heatmap_mode {
                " Debugger [HEATMAP] "
            } else {
                " Step Debugger "
            });

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 4 {
            return;
        }

        let mut lines = Vec::new();

        // Step counter
        lines.push(Line::from(vec![
            Span::styled(
                format!("Step {}/{}", self.session.current_step + 1, self.session.total_steps()),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Backtracks: {}", self.session.backtrack_count()),
                Style::default().fg(Color::Red),
            ),
        ]));
        lines.push(Line::from(""));

        if self.session.heatmap_mode {
            // Heatmap rendering
            lines.push(Line::from(Span::styled(
                "Pattern heatmap (visit frequency):",
                Style::default().fg(Color::Cyan),
            )));

            let max_heat = self.session.max_heat();
            let pat_chars: Vec<char> = self.pattern.chars().collect();
            let mut spans = Vec::new();

            for (i, ch) in pat_chars.iter().enumerate() {
                let heat = self.session.heatmap.get(&i).copied().unwrap_or(0);
                let color = heat_color(heat, max_heat);
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::Black).bg(color),
                ));
            }
            lines.push(Line::from(spans));

            // Legend
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default().bg(Color::Rgb(0, 100, 0))),
                Span::raw(" low  "),
                Span::styled("  ", Style::default().bg(Color::Rgb(200, 200, 0))),
                Span::raw(" med  "),
                Span::styled("  ", Style::default().bg(Color::Rgb(255, 0, 0))),
                Span::raw(" high"),
            ]));
        } else {
            // Step detail
            if let Some(step) = self.session.current() {
                // Pattern with position marker
                lines.push(Line::from(Span::styled(
                    "Pattern:",
                    Style::default().fg(Color::Cyan),
                )));
                let pat_chars: Vec<char> = self.pattern.chars().collect();
                let mut pat_spans = Vec::new();
                for (i, ch) in pat_chars.iter().enumerate() {
                    if i == step.pattern_offset {
                        pat_spans.push(Span::styled(
                            ch.to_string(),
                            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
                        ));
                    } else {
                        pat_spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::White)));
                    }
                }
                if step.pattern_offset >= pat_chars.len() && !pat_chars.is_empty() {
                    pat_spans.push(Span::styled(
                        " ",
                        Style::default().bg(Color::Green),
                    ));
                }
                lines.push(Line::from(pat_spans));

                lines.push(Line::from(""));

                // Text with position marker
                lines.push(Line::from(Span::styled(
                    "Text:",
                    Style::default().fg(Color::Cyan),
                )));
                let text_chars: Vec<char> = self.text.chars().collect();
                let mut text_spans = Vec::new();
                for (i, ch) in text_chars.iter().enumerate() {
                    if i == step.text_offset {
                        text_spans.push(Span::styled(
                            ch.to_string(),
                            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
                        ));
                    } else {
                        text_spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::White)));
                    }
                }
                if step.text_offset >= text_chars.len() && !text_chars.is_empty() {
                    text_spans.push(Span::styled(
                        " ",
                        Style::default().bg(Color::Green),
                    ));
                }
                lines.push(Line::from(text_spans));

                lines.push(Line::from(""));

                // Step description
                let desc_style = if step.is_backtrack {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else if step.matched {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                lines.push(Line::from(Span::styled(&step.description, desc_style)));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Left/Right:step  H:heatmap  Esc:close",
            Style::default().fg(Color::DarkGray),
        )));

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        para.render(inner, buf);
    }
}

pub struct CodeGenPanel<'a> {
    pub selected: usize,
    pub code: &'a str,
    pub languages: &'a [&'static str],
}

impl<'a> Widget for CodeGenPanel<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .title(" Code Generation ");

        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines = Vec::new();

        // Language selector
        let mut lang_spans = Vec::new();
        for (i, lang) in self.languages.iter().enumerate() {
            if i == self.selected {
                lang_spans.push(Span::styled(
                    format!(" [{}] ", lang),
                    Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD),
                ));
            } else {
                lang_spans.push(Span::styled(
                    format!("  {}  ", lang),
                    Style::default().fg(Color::White),
                ));
            }
        }
        lines.push(Line::from(lang_spans));
        lines.push(Line::from(""));

        // Code output
        for code_line in self.code.lines() {
            lines.push(Line::from(Span::styled(
                code_line.to_string(),
                Style::default().fg(Color::LightGreen),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Left/Right:language  Ctrl+C:copy  Esc:close",
            Style::default().fg(Color::DarkGray),
        )));

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        para.render(inner, buf);
    }
}

pub struct ExplainPanel<'a> {
    pub explanation: &'a str,
}

impl<'a> Widget for ExplainPanel<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Explanation ");

        let inner = block.inner(area);
        block.render(area, buf);

        if self.explanation.is_empty() {
            Paragraph::new(Span::styled(
                "Enter a regex to see explanation",
                Style::default().fg(Color::DarkGray),
            )).wrap(Wrap { trim: false })
            .render(inner, buf);
        } else {
            Paragraph::new(Span::styled(
                self.explanation,
                Style::default().fg(Color::White),
            )).wrap(Wrap { trim: false })
            .render(inner, buf);
        }
    }
}

fn heat_color(heat: usize, max: usize) -> Color {
    if max == 0 {
        return Color::Rgb(0, 50, 0);
    }
    let ratio = heat as f64 / max as f64;
    if ratio < 0.33 {
        let g = (100.0 + ratio * 3.0 * 155.0) as u8;
        Color::Rgb(0, g, 0)
    } else if ratio < 0.66 {
        let r = ((ratio - 0.33) * 3.0 * 255.0) as u8;
        let g = 255 - ((ratio - 0.33) * 3.0 * 55.0) as u8;
        Color::Rgb(r, g, 0)
    } else {
        let r = 255;
        let g = (255.0 * (1.0 - ratio)) as u8;
        Color::Rgb(r, g, 0)
    }
}
