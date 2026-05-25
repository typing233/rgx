use regex_syntax::ast::Ast;
use regex_syntax::ast::parse::Parser;

use crate::engine::EngineLevel;

#[derive(Debug, Default)]
pub struct FeatureSet {
    pub has_lookahead: bool,
    pub has_lookbehind: bool,
    pub has_backreference: bool,
    pub has_atomic_group: bool,
    pub has_conditional: bool,
    pub has_recursion: bool,
}

impl FeatureSet {
    pub fn required_engine(&self) -> EngineLevel {
        if self.has_recursion || self.has_conditional || self.has_atomic_group {
            EngineLevel::Pcre2
        } else if self.has_lookahead || self.has_lookbehind || self.has_backreference {
            EngineLevel::Fancy
        } else {
            EngineLevel::Basic
        }
    }
}

pub fn detect_features(pattern: &str) -> FeatureSet {
    let mut features = FeatureSet::default();

    // Try parsing with regex-syntax AST parser
    if let Ok(ast) = Parser::new().parse(pattern) {
        walk_ast(&ast, &mut features);
    }

    // Scan for patterns regex-syntax can't parse (lookarounds, backrefs, etc.)
    scan_raw_pattern(pattern, &mut features);

    features
}

fn walk_ast(ast: &Ast, features: &mut FeatureSet) {
    match ast {
        Ast::Group(group) => {
            // regex-syntax GroupKind doesn't have lookarounds,
            // but we still walk children for nested features
            walk_ast(&group.ast, features);
        }
        Ast::Concat(concat) => {
            for a in &concat.asts {
                walk_ast(a, features);
            }
        }
        Ast::Alternation(alt) => {
            for a in &alt.asts {
                walk_ast(a, features);
            }
        }
        Ast::Repetition(rep) => {
            walk_ast(&rep.ast, features);
        }
        _ => {}
    }
}

fn scan_raw_pattern(pattern: &str, features: &mut FeatureSet) {
    let bytes = pattern.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'\\' && i + 1 < len {
            let next = bytes[i + 1];
            if next >= b'1' && next <= b'9' {
                features.has_backreference = true;
            }
            i += 2;
            continue;
        }

        if bytes[i] == b'(' && i + 1 < len && bytes[i + 1] == b'?' {
            let rest = &pattern[i..];
            if rest.starts_with("(?=") || rest.starts_with("(?!") {
                features.has_lookahead = true;
            } else if rest.starts_with("(?<=") || rest.starts_with("(?<!") {
                features.has_lookbehind = true;
            } else if rest.starts_with("(?>") {
                features.has_atomic_group = true;
            } else if rest.starts_with("(?(") {
                features.has_conditional = true;
            } else if rest.starts_with("(?R)") || rest.starts_with("(?0)") {
                features.has_recursion = true;
            }
        }

        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_pattern() {
        let f = detect_features(r"\d+");
        assert_eq!(f.required_engine(), EngineLevel::Basic);
    }

    #[test]
    fn test_lookahead() {
        let f = detect_features(r"foo(?=bar)");
        assert!(f.has_lookahead);
        assert_eq!(f.required_engine(), EngineLevel::Fancy);
    }

    #[test]
    fn test_backreference() {
        let f = detect_features(r"(\w+)\s+\1");
        assert!(f.has_backreference);
        assert_eq!(f.required_engine(), EngineLevel::Fancy);
    }
}
