use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::ast::highlight::group_match_color;
use crate::engine::{MatchResult, MatchSpan};

pub struct MatchResultsWidget<'a> {
    pub test_text: &'a str,
    pub result: Option<&'a MatchResult>,
    pub error: Option<&'a str>,
}

impl<'a> Widget for MatchResultsWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Matches ");

        let inner = block.inner(area);
        block.render(area, buf);

        if let Some(err) = self.error {
            let err_para = Paragraph::new(Span::styled(
                err,
                Style::default().fg(Color::Red),
            ))
            .wrap(Wrap { trim: false });
            err_para.render(inner, buf);
            return;
        }

        let result = match self.result {
            Some(r) => r,
            None => {
                Paragraph::new(Span::styled(
                    "Enter a regex pattern to begin matching",
                    Style::default().fg(Color::DarkGray),
                ))
                .render(inner, buf);
                return;
            }
        };

        let mut lines = Vec::new();

        // Match summary
        lines.push(Line::from(Span::styled(
            format!("{} match(es) found", result.full_matches.len()),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        // Detailed matches with group info
        for (idx, full) in result.full_matches.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("Match #{}: ", idx + 1),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!("\"{}\"", &full.text),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" [{}-{}]", full.start, full.end),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            if idx < result.group_matches.len() {
                for (gi, group) in result.group_matches[idx].iter().enumerate() {
                    let group_color = group_match_color(gi);
                    match group {
                        Some(g) => {
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(
                                    format!("Group {}: ", gi + 1),
                                    Style::default().fg(group_color),
                                ),
                                Span::styled(
                                    format!("\"{}\"", &g.text),
                                    Style::default().fg(group_color).add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    format!(" [{}-{}]", g.start, g.end),
                                    Style::default().fg(Color::DarkGray),
                                ),
                            ]));
                        }
                        None => {
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(
                                    format!("Group {}: ", gi + 1),
                                    Style::default().fg(group_color),
                                ),
                                Span::styled(
                                    "<no match>",
                                    Style::default().fg(Color::DarkGray),
                                ),
                            ]));
                        }
                    }
                }
            }
        }

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        para.render(inner, buf);
    }
}

pub fn match_spans_for_text(result: &MatchResult) -> &[MatchSpan] {
    &result.full_matches
}
