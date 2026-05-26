use regex_syntax::ast::{
    parse::Parser, Ast, ClassPerlKind, ClassSet, ClassSetItem, ClassSetRange,
    GroupKind, Literal, LiteralKind, RepetitionKind, RepetitionRange,
};

pub fn explain_regex(pattern: &str) -> String {
    if pattern.is_empty() {
        return "Empty pattern: matches everything".to_string();
    }

    match Parser::new().parse(pattern) {
        Ok(ast) => {
            let parts = explain_ast(&ast);
            if parts.is_empty() {
                explain_fallback(pattern)
            } else {
                parts
            }
        }
        Err(_) => explain_fallback(pattern),
    }
}

fn explain_ast(ast: &Ast) -> String {
    match ast {
        Ast::Empty(_) => String::new(),
        Ast::Literal(lit) => explain_literal(lit),
        Ast::Dot(_) => "any character (except newline)".to_string(),
        Ast::Assertion(a) => {
            use regex_syntax::ast::AssertionKind;
            match a.kind {
                AssertionKind::StartLine => "start of line".to_string(),
                AssertionKind::EndLine => "end of line".to_string(),
                AssertionKind::StartText => "start of text".to_string(),
                AssertionKind::EndText => "end of text".to_string(),
                AssertionKind::WordBoundary => "word boundary".to_string(),
                AssertionKind::NotWordBoundary => "non-word boundary".to_string(),
                _ => "assertion".to_string(),
            }
        }
        Ast::ClassPerl(c) => {
            let (class, neg) = match c.kind {
                ClassPerlKind::Digit => ("digit", c.negated),
                ClassPerlKind::Space => ("whitespace", c.negated),
                ClassPerlKind::Word => ("word character", c.negated),
            };
            if neg {
                format!("non-{}", class)
            } else {
                class.to_string()
            }
        }
        Ast::ClassUnicode(c) => {
            format!("unicode class {}", if c.negated { "(negated)" } else { "" })
        }
        Ast::ClassBracketed(c) => {
            let neg = if c.negated { "not " } else { "" };
            let items = explain_class_set(&c.kind);
            format!("{}[{}]", neg, items)
        }
        Ast::Repetition(rep) => {
            let inner = explain_ast(&rep.ast);
            let quant = match &rep.op.kind {
                RepetitionKind::ZeroOrOne => "optionally".to_string(),
                RepetitionKind::ZeroOrMore => "zero or more times".to_string(),
                RepetitionKind::OneOrMore => "one or more times".to_string(),
                RepetitionKind::Range(range) => match range {
                    RepetitionRange::Exactly(n) => format!("exactly {} times", n),
                    RepetitionRange::AtLeast(n) => format!("{} or more times", n),
                    RepetitionRange::Bounded(min, max) => format!("{} to {} times", min, max),
                },
            };
            let lazy = if !rep.greedy { " (lazy)" } else { "" };
            if matches!(rep.op.kind, RepetitionKind::ZeroOrOne) {
                format!("{} {}{}", quant, inner, lazy)
            } else {
                format!("{}, {}{}", inner, quant, lazy)
            }
        }
        Ast::Group(group) => {
            let inner = explain_ast(&group.ast);
            match &group.kind {
                GroupKind::CaptureIndex(cap) => {
                    format!("capture group #{}: {}", cap, inner)
                }
                GroupKind::CaptureName { name, .. } => {
                    format!("named capture '{}': {}", name.name, inner)
                }
                GroupKind::NonCapturing(_) => {
                    format!("non-capturing group: {}", inner)
                }
            }
        }
        Ast::Concat(concat) => {
            let parts: Vec<String> = concat.asts.iter()
                .map(|a| explain_ast(a))
                .filter(|s| !s.is_empty())
                .collect();
            if parts.len() == 1 {
                parts[0].clone()
            } else {
                parts.join(", then ")
            }
        }
        Ast::Alternation(alt) => {
            let parts: Vec<String> = alt.asts.iter()
                .map(|a| explain_ast(a))
                .collect();
            parts.join(" OR ")
        }
        Ast::Flags(_) => "flags modifier".to_string(),
    }
}

fn explain_literal(lit: &Literal) -> String {
    match &lit.kind {
        LiteralKind::Verbatim => format!("'{}'", lit.c),
        LiteralKind::Meta => match lit.c {
            '.' => "literal '.'".to_string(),
            _ => format!("'{}'", lit.c),
        },
        LiteralKind::Superfluous => format!("'{}'", lit.c),
        LiteralKind::Octal => format!("octal char '{}'", lit.c),
        LiteralKind::HexFixed(_) | LiteralKind::HexBrace(_) => {
            format!("hex char '{}'", lit.c)
        }
        LiteralKind::Special(sk) => {
            use regex_syntax::ast::SpecialLiteralKind;
            match sk {
                SpecialLiteralKind::Bell => "bell character".to_string(),
                SpecialLiteralKind::FormFeed => "form feed".to_string(),
                SpecialLiteralKind::Tab => "tab".to_string(),
                SpecialLiteralKind::LineFeed => "newline".to_string(),
                SpecialLiteralKind::CarriageReturn => "carriage return".to_string(),
                SpecialLiteralKind::VerticalTab => "vertical tab".to_string(),
                SpecialLiteralKind::Space => "space".to_string(),
            }
        }
    }
}

fn explain_class_set(set: &ClassSet) -> String {
    match set {
        ClassSet::Item(item) => explain_class_set_item(item),
        ClassSet::BinaryOp(op) => {
            let lhs = explain_class_set(&op.lhs);
            let rhs = explain_class_set(&op.rhs);
            use regex_syntax::ast::ClassSetBinaryOpKind;
            let op_name = match op.kind {
                ClassSetBinaryOpKind::Intersection => "and",
                ClassSetBinaryOpKind::Difference => "minus",
                ClassSetBinaryOpKind::SymmetricDifference => "xor",
            };
            format!("{} {} {}", lhs, op_name, rhs)
        }
    }
}

fn explain_class_set_item(item: &ClassSetItem) -> String {
    match item {
        ClassSetItem::Empty(_) => String::new(),
        ClassSetItem::Literal(lit) => format!("'{}'", lit.c),
        ClassSetItem::Range(ClassSetRange { start, end, .. }) => {
            format!("'{}'-'{}'", start.c, end.c)
        }
        ClassSetItem::Perl(p) => {
            let (name, neg) = match p.kind {
                ClassPerlKind::Digit => ("digit", p.negated),
                ClassPerlKind::Space => ("whitespace", p.negated),
                ClassPerlKind::Word => ("word", p.negated),
            };
            if neg { format!("non-{}", name) } else { name.to_string() }
        }
        ClassSetItem::Unicode(_) => "unicode class".to_string(),
        ClassSetItem::Bracketed(b) => {
            let inner = explain_class_set(&b.kind);
            if b.negated {
                format!("not({})", inner)
            } else {
                inner
            }
        }
        ClassSetItem::Union(union) => {
            let parts: Vec<String> = union.items.iter()
                .map(|i| explain_class_set_item(i))
                .collect();
            parts.join(", ")
        }
        _ => "character class".to_string(),
    }
}

fn explain_fallback(pattern: &str) -> String {
    let mut parts = Vec::new();
    let bytes = pattern.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'\\' && i + 1 < len {
            let next = bytes[i + 1];
            let desc = match next {
                b'd' => "a digit",
                b'D' => "a non-digit",
                b'w' => "a word character",
                b'W' => "a non-word character",
                b's' => "whitespace",
                b'S' => "non-whitespace",
                b'b' => "word boundary",
                b'B' => "non-word boundary",
                b'n' => "newline",
                b't' => "tab",
                _ => {
                    parts.push(format!("literal '{}'", next as char));
                    i += 2;
                    continue;
                }
            };
            parts.push(desc.to_string());
            i += 2;
        } else if bytes[i] == b'.' {
            parts.push("any character".to_string());
            i += 1;
        } else if bytes[i] == b'^' {
            parts.push("start of line".to_string());
            i += 1;
        } else if bytes[i] == b'$' {
            parts.push("end of line".to_string());
            i += 1;
        } else if bytes[i] == b'*' {
            if let Some(last) = parts.last_mut() {
                *last = format!("{}, zero or more times", last);
            }
            i += 1;
        } else if bytes[i] == b'+' {
            if let Some(last) = parts.last_mut() {
                *last = format!("{}, one or more times", last);
            }
            i += 1;
        } else if bytes[i] == b'?' {
            if let Some(last) = parts.last_mut() {
                *last = format!("{} (optional)", last);
            }
            i += 1;
        } else if bytes[i] == b'|' {
            parts.push("OR".to_string());
            i += 1;
        } else if bytes[i] == b'(' {
            parts.push("group start".to_string());
            i += 1;
        } else if bytes[i] == b')' {
            parts.push("group end".to_string());
            i += 1;
        } else if bytes[i] == b'[' {
            let start = i;
            i += 1;
            while i < len && bytes[i] != b']' {
                if bytes[i] == b'\\' { i += 1; }
                i += 1;
            }
            if i < len { i += 1; }
            let class_str = &pattern[start..i];
            parts.push(format!("character class {}", class_str));
        } else {
            parts.push(format!("'{}'", bytes[i] as char));
            i += 1;
        }
    }

    if parts.is_empty() {
        "matches the input".to_string()
    } else {
        parts.join(", followed by ")
    }
}
