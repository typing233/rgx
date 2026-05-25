use ratatui::style::{Color, Modifier, Style};
use regex_syntax::ast::{
    parse::Parser, Ast, ClassPerlKind, Group, GroupKind, RepetitionKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Literal,
    Escape,
    Quantifier,
    Anchor,
    CharClass,
    Dot,
    Alternation,
    Flag,
    Lookaround,
    GroupParen(u32),
    NonCapturingParen,
}

#[derive(Debug, Clone)]
pub struct HighlightToken {
    pub start: usize,
    pub end: usize,
    pub kind: TokenKind,
    pub capture_group: Option<u32>,
}

// Bright foreground colors for group content and parens
const GROUP_COLORS: [Color; 8] = [
    Color::Cyan,
    Color::Yellow,
    Color::Green,
    Color::Magenta,
    Color::Blue,
    Color::LightRed,
    Color::LightCyan,
    Color::LightYellow,
];

// Dark background tints per group (RGB) — subtle enough not to drown text
const GROUP_BG_COLORS: [Color; 8] = [
    Color::Rgb(0, 35, 40),
    Color::Rgb(40, 35, 0),
    Color::Rgb(0, 35, 0),
    Color::Rgb(35, 0, 35),
    Color::Rgb(0, 0, 40),
    Color::Rgb(40, 15, 10),
    Color::Rgb(0, 35, 35),
    Color::Rgb(35, 35, 0),
];

pub fn style_for_token(token: &HighlightToken) -> Style {
    let group = token.capture_group;
    let bg = group.map(|idx| group_bg_color(idx));

    match token.kind {
        TokenKind::GroupParen(idx) => {
            let mut s = Style::default()
                .fg(group_color_by_index(idx))
                .add_modifier(Modifier::BOLD);
            if let Some(bg) = bg {
                s = s.bg(bg);
            }
            s
        }
        TokenKind::NonCapturingParen => {
            let mut s = Style::default().fg(Color::DarkGray);
            if let Some(bg) = bg {
                s = s.bg(bg);
            }
            s
        }
        TokenKind::Literal => {
            match group {
                Some(idx) => {
                    let mut s = Style::default().fg(group_color_by_index(idx));
                    s = s.bg(group_bg_color(idx));
                    s
                }
                None => Style::default().fg(Color::White),
            }
        }
        _ => {
            let fg = syntax_color(&token.kind);
            let mut s = Style::default().fg(fg);
            if let Some(bg) = bg {
                s = s.bg(bg);
            }
            s
        }
    }
}

fn syntax_color(kind: &TokenKind) -> Color {
    match kind {
        TokenKind::Escape => Color::Green,
        TokenKind::Quantifier => Color::Yellow,
        TokenKind::Anchor => Color::Magenta,
        TokenKind::CharClass => Color::LightBlue,
        TokenKind::Dot => Color::LightBlue,
        TokenKind::Alternation => Color::Red,
        TokenKind::Flag => Color::Blue,
        TokenKind::Lookaround => Color::LightMagenta,
        _ => Color::White,
    }
}

pub fn group_color_by_index(index: u32) -> Color {
    GROUP_COLORS[((index as usize).wrapping_sub(1)) % GROUP_COLORS.len()]
}

pub fn group_bg_color(index: u32) -> Color {
    GROUP_BG_COLORS[((index as usize).wrapping_sub(1)) % GROUP_BG_COLORS.len()]
}

pub fn group_match_color(group_index: usize) -> Color {
    GROUP_COLORS[group_index % GROUP_COLORS.len()]
}

pub fn tokenize(pattern: &str) -> Vec<HighlightToken> {
    let ast = match Parser::new().parse(pattern) {
        Ok(ast) => ast,
        Err(_) => return tokenize_fallback(pattern),
    };

    let mut tokens = Vec::new();
    walk_ast(&ast, &mut tokens, None);
    tokens.sort_by_key(|t| t.start);

    fill_gaps(pattern, tokens)
}

fn fill_gaps(pattern: &str, tokens: Vec<HighlightToken>) -> Vec<HighlightToken> {
    let mut result = Vec::new();
    let mut pos = 0;

    for token in tokens {
        if token.start > pos {
            result.push(HighlightToken {
                start: pos,
                end: token.start,
                kind: TokenKind::Literal,
                capture_group: None,
            });
        }
        if token.start >= pos {
            result.push(token.clone());
            pos = token.end;
        }
    }

    if pos < pattern.len() {
        result.push(HighlightToken {
            start: pos,
            end: pattern.len(),
            kind: TokenKind::Literal,
            capture_group: None,
        });
    }

    result
}

fn walk_ast(ast: &Ast, tokens: &mut Vec<HighlightToken>, enclosing_group: Option<u32>) {
    match ast {
        Ast::Empty(_) => {}
        Ast::Flags(flags) => {
            let span = &flags.span;
            tokens.push(HighlightToken {
                start: span.start.offset,
                end: span.end.offset,
                kind: TokenKind::Flag,
                capture_group: enclosing_group,
            });
        }
        Ast::Literal(lit) => {
            let span = &lit.span;
            let kind = if lit.kind == regex_syntax::ast::LiteralKind::Meta
                || lit.kind == regex_syntax::ast::LiteralKind::Superfluous
                || matches!(lit.kind, regex_syntax::ast::LiteralKind::Special(_))
            {
                TokenKind::Escape
            } else {
                TokenKind::Literal
            };
            tokens.push(HighlightToken {
                start: span.start.offset,
                end: span.end.offset,
                kind,
                capture_group: enclosing_group,
            });
        }
        Ast::Dot(span) => {
            tokens.push(HighlightToken {
                start: span.start.offset,
                end: span.end.offset,
                kind: TokenKind::Dot,
                capture_group: enclosing_group,
            });
        }
        Ast::Assertion(a) => {
            tokens.push(HighlightToken {
                start: a.span.start.offset,
                end: a.span.end.offset,
                kind: TokenKind::Anchor,
                capture_group: enclosing_group,
            });
        }
        Ast::ClassUnicode(c) => {
            tokens.push(HighlightToken {
                start: c.span.start.offset,
                end: c.span.end.offset,
                kind: TokenKind::CharClass,
                capture_group: enclosing_group,
            });
        }
        Ast::ClassPerl(c) => {
            let kind = match c.kind {
                ClassPerlKind::Word | ClassPerlKind::Space | ClassPerlKind::Digit => {
                    TokenKind::CharClass
                }
            };
            tokens.push(HighlightToken {
                start: c.span.start.offset,
                end: c.span.end.offset,
                kind,
                capture_group: enclosing_group,
            });
        }
        Ast::ClassBracketed(c) => {
            tokens.push(HighlightToken {
                start: c.span.start.offset,
                end: c.span.end.offset,
                kind: TokenKind::CharClass,
                capture_group: enclosing_group,
            });
        }
        Ast::Repetition(rep) => {
            walk_ast(&rep.ast, tokens, enclosing_group);
            let rep_start = match &rep.op.kind {
                RepetitionKind::ZeroOrOne => rep.op.span.start.offset,
                RepetitionKind::ZeroOrMore => rep.op.span.start.offset,
                RepetitionKind::OneOrMore => rep.op.span.start.offset,
                RepetitionKind::Range(_) => rep.op.span.start.offset,
            };
            tokens.push(HighlightToken {
                start: rep_start,
                end: rep.op.span.end.offset,
                kind: TokenKind::Quantifier,
                capture_group: enclosing_group,
            });
            if rep.greedy != default_greedy(&rep.op.kind) {
                tokens.push(HighlightToken {
                    start: rep.op.span.end.offset,
                    end: rep.op.span.end.offset + 1,
                    kind: TokenKind::Quantifier,
                    capture_group: enclosing_group,
                });
            }
        }
        Ast::Group(group) => {
            emit_group_tokens(group, tokens, enclosing_group);
        }
        Ast::Alternation(alt) => {
            for (i, a) in alt.asts.iter().enumerate() {
                walk_ast(a, tokens, enclosing_group);
                if i < alt.asts.len() - 1 {
                    let a_span_end = ast_span(a).end.offset;
                    tokens.push(HighlightToken {
                        start: a_span_end,
                        end: a_span_end + 1,
                        kind: TokenKind::Alternation,
                        capture_group: enclosing_group,
                    });
                }
            }
        }
        Ast::Concat(concat) => {
            for a in &concat.asts {
                walk_ast(a, tokens, enclosing_group);
            }
        }
    }
}

fn emit_group_tokens(
    group: &Group,
    tokens: &mut Vec<HighlightToken>,
    enclosing_group: Option<u32>,
) {
    let span = &group.span;
    let cap_idx = group.capture_index();
    let this_group = cap_idx.or(enclosing_group);

    let open_end = find_group_open_end(group);

    let paren_kind = match cap_idx {
        Some(idx) => TokenKind::GroupParen(idx),
        None => TokenKind::NonCapturingParen,
    };

    tokens.push(HighlightToken {
        start: span.start.offset,
        end: open_end,
        kind: paren_kind,
        capture_group: this_group,
    });

    walk_ast(&group.ast, tokens, this_group);

    tokens.push(HighlightToken {
        start: span.end.offset - 1,
        end: span.end.offset,
        kind: paren_kind,
        capture_group: this_group,
    });
}

fn find_group_open_end(group: &Group) -> usize {
    let base = group.span.start.offset;
    match &group.kind {
        GroupKind::CaptureIndex(_) => base + 1,
        GroupKind::CaptureName { name, starts_with_p, .. } => {
            if *starts_with_p {
                // (?P<name>
                base + 4 + name.name.len() + 1
            } else {
                // (?<name>
                base + 3 + name.name.len() + 1
            }
        }
        GroupKind::NonCapturing(_) => {
            let inner_start = ast_span(&group.ast).start.offset;
            inner_start
        }
    }
}

fn ast_span(ast: &Ast) -> &regex_syntax::ast::Span {
    match ast {
        Ast::Empty(s) => s,
        Ast::Flags(f) => &f.span,
        Ast::Literal(l) => &l.span,
        Ast::Dot(s) => s,
        Ast::Assertion(a) => &a.span,
        Ast::ClassUnicode(c) => &c.span,
        Ast::ClassPerl(c) => &c.span,
        Ast::ClassBracketed(c) => &c.span,
        Ast::Repetition(r) => &r.span,
        Ast::Group(g) => &g.span,
        Ast::Alternation(a) => &a.span,
        Ast::Concat(c) => &c.span,
    }
}

fn default_greedy(kind: &RepetitionKind) -> bool {
    match kind {
        RepetitionKind::ZeroOrOne
        | RepetitionKind::ZeroOrMore
        | RepetitionKind::OneOrMore => true,
        RepetitionKind::Range(_) => true,
    }
}

fn tokenize_fallback(pattern: &str) -> Vec<HighlightToken> {
    let bytes = pattern.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut group_stack: Vec<Option<u32>> = Vec::new();
    let mut next_capture: u32 = 1;

    while i < len {
        let enclosing = group_stack.last().copied().flatten();

        match bytes[i] {
            b'\\' if i + 1 < len => {
                let next = bytes[i + 1];
                let kind = match next {
                    b'b' | b'B' | b'A' | b'z' | b'Z' => TokenKind::Anchor,
                    b'd' | b'D' | b'w' | b'W' | b's' | b'S' => TokenKind::CharClass,
                    b'1'..=b'9' => TokenKind::Escape,
                    _ => TokenKind::Escape,
                };
                tokens.push(HighlightToken {
                    start: i,
                    end: i + 2,
                    kind,
                    capture_group: enclosing,
                });
                i += 2;
            }
            b'(' => {
                let start = i;
                if i + 1 < len && bytes[i + 1] == b'?' {
                    let rest = &pattern[i..];
                    if rest.starts_with("(?=") || rest.starts_with("(?!")
                        || rest.starts_with("(?<=") || rest.starts_with("(?<!")
                        || rest.starts_with("(?>")
                    {
                        let end = if rest.starts_with("(?<=") || rest.starts_with("(?<!") {
                            i + 4
                        } else {
                            i + 3
                        };
                        tokens.push(HighlightToken {
                            start,
                            end,
                            kind: TokenKind::Lookaround,
                            capture_group: enclosing,
                        });
                        group_stack.push(None);
                        i = end;
                    } else {
                        // non-capturing or flags
                        let mut j = i + 2;
                        while j < len && bytes[j] != b')' && bytes[j] != b':' {
                            j += 1;
                        }
                        if j < len && bytes[j] == b':' {
                            j += 1;
                        }
                        tokens.push(HighlightToken {
                            start,
                            end: j,
                            kind: TokenKind::NonCapturingParen,
                            capture_group: enclosing,
                        });
                        group_stack.push(None);
                        i = j;
                    }
                } else {
                    let idx = next_capture;
                    next_capture += 1;
                    tokens.push(HighlightToken {
                        start,
                        end: i + 1,
                        kind: TokenKind::GroupParen(idx),
                        capture_group: Some(idx),
                    });
                    group_stack.push(Some(idx));
                    i += 1;
                }
            }
            b')' => {
                let cap = group_stack.pop().flatten();
                let kind = match cap {
                    Some(idx) => TokenKind::GroupParen(idx),
                    None => TokenKind::NonCapturingParen,
                };
                tokens.push(HighlightToken {
                    start: i,
                    end: i + 1,
                    kind,
                    capture_group: cap,
                });
                i += 1;
            }
            b'[' => {
                let start = i;
                i += 1;
                if i < len && bytes[i] == b'^' { i += 1; }
                if i < len && bytes[i] == b']' { i += 1; }
                while i < len && bytes[i] != b']' {
                    if bytes[i] == b'\\' && i + 1 < len { i += 1; }
                    i += 1;
                }
                if i < len { i += 1; }
                tokens.push(HighlightToken {
                    start,
                    end: i,
                    kind: TokenKind::CharClass,
                    capture_group: enclosing,
                });
            }
            b'^' | b'$' => {
                tokens.push(HighlightToken {
                    start: i,
                    end: i + 1,
                    kind: TokenKind::Anchor,
                    capture_group: enclosing,
                });
                i += 1;
            }
            b'*' | b'+' | b'?' => {
                let start = i;
                i += 1;
                if i < len && bytes[i] == b'?' { i += 1; }
                tokens.push(HighlightToken {
                    start,
                    end: i,
                    kind: TokenKind::Quantifier,
                    capture_group: enclosing,
                });
            }
            b'{' => {
                let start = i;
                let mut j = i + 1;
                while j < len && bytes[j] != b'}' { j += 1; }
                if j < len {
                    j += 1;
                    if j < len && bytes[j] == b'?' { j += 1; }
                    tokens.push(HighlightToken {
                        start,
                        end: j,
                        kind: TokenKind::Quantifier,
                        capture_group: enclosing,
                    });
                    i = j;
                } else {
                    tokens.push(HighlightToken {
                        start,
                        end: start + 1,
                        kind: TokenKind::Literal,
                        capture_group: enclosing,
                    });
                    i += 1;
                }
            }
            b'|' => {
                tokens.push(HighlightToken {
                    start: i,
                    end: i + 1,
                    kind: TokenKind::Alternation,
                    capture_group: enclosing,
                });
                i += 1;
            }
            b'.' => {
                tokens.push(HighlightToken {
                    start: i,
                    end: i + 1,
                    kind: TokenKind::Dot,
                    capture_group: enclosing,
                });
                i += 1;
            }
            _ => {
                tokens.push(HighlightToken {
                    start: i,
                    end: i + 1,
                    kind: TokenKind::Literal,
                    capture_group: enclosing,
                });
                i += 1;
            }
        }
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjacent_groups_different_styles() {
        // (foo)(bar) → group 1 and group 2 get distinct fg + bg
        let tokens = tokenize("(foo)(bar)");
        let group1_lit: Vec<_> = tokens.iter()
            .filter(|t| t.kind == TokenKind::Literal && t.capture_group == Some(1))
            .collect();
        let group2_lit: Vec<_> = tokens.iter()
            .filter(|t| t.kind == TokenKind::Literal && t.capture_group == Some(2))
            .collect();
        assert!(!group1_lit.is_empty());
        assert!(!group2_lit.is_empty());
        let s1 = style_for_token(group1_lit[0]);
        let s2 = style_for_token(group2_lit[0]);
        assert_ne!(s1.fg, s2.fg);
        assert_ne!(s1.bg, s2.bg);
    }

    #[test]
    fn test_capture_group_stable_index() {
        let tokens = tokenize("(a)(b)(c)");
        let indices: Vec<u32> = tokens.iter().filter_map(|t| {
            if let TokenKind::GroupParen(idx) = t.kind { Some(idx) } else { None }
        }).collect();
        assert!(indices.contains(&1));
        assert!(indices.contains(&2));
        assert!(indices.contains(&3));
    }

    #[test]
    fn test_nested_inner_overwrites_outer() {
        // ((a)) → 'a' belongs to inner group 2
        let tokens = tokenize("((a))");
        let a_token = tokens.iter().find(|t| t.kind == TokenKind::Literal).unwrap();
        assert_eq!(a_token.capture_group, Some(2));
        let s = style_for_token(a_token);
        assert_eq!(s.fg, Some(group_color_by_index(2)));
        assert_eq!(s.bg, Some(group_bg_color(2)));
    }

    #[test]
    fn test_nested_outer_parens_still_visible() {
        // ((a)) → outer parens are group 1 color
        let tokens = tokenize("((a))");
        let outer_open = &tokens[0];
        assert!(matches!(outer_open.kind, TokenKind::GroupParen(1)));
        let s = style_for_token(outer_open);
        assert_eq!(s.fg, Some(group_color_by_index(1)));
    }

    #[test]
    fn test_non_capturing_group_no_index() {
        let tokens = tokenize("(?:foo)(bar)");
        let nc: Vec<_> = tokens.iter()
            .filter(|t| matches!(t.kind, TokenKind::NonCapturingParen))
            .collect();
        assert!(!nc.is_empty());
        let cap: Vec<_> = tokens.iter()
            .filter_map(|t| if let TokenKind::GroupParen(idx) = t.kind { Some(idx) } else { None })
            .collect();
        assert!(cap.contains(&1));
    }

    #[test]
    fn test_syntax_token_keeps_fg_inside_group() {
        // (\d+) → \d inside group 1 keeps CharClass fg color, gets group bg
        let tokens = tokenize(r"(\d+)");
        let charclass = tokens.iter().find(|t| t.kind == TokenKind::CharClass).unwrap();
        assert_eq!(charclass.capture_group, Some(1));
        let s = style_for_token(charclass);
        assert_eq!(s.fg, Some(syntax_color(&TokenKind::CharClass)));
        assert_eq!(s.bg, Some(group_bg_color(1)));
    }

    #[test]
    fn test_literal_outside_group_no_bg() {
        let tokens = tokenize("abc");
        let lit = &tokens[0];
        assert_eq!(lit.capture_group, None);
        let s = style_for_token(lit);
        assert_eq!(s.fg, Some(Color::White));
        assert_eq!(s.bg, None);
    }

    #[test]
    fn test_fallback_on_invalid_regex() {
        let tokens = tokenize("(unbalanced");
        assert!(!tokens.is_empty());
    }
}
