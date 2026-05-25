use fancy_regex::RegexBuilder;

use super::{EngineLevel, MatchResult, MatchSpan, RegexEngine, RegexFlags};

pub struct FancyEngine;

impl RegexEngine for FancyEngine {
    fn name(&self) -> EngineLevel {
        EngineLevel::Fancy
    }

    fn find_matches(&self, pattern: &str, text: &str, flags: &RegexFlags) -> Result<MatchResult, String> {
        let prefixed = build_pattern(pattern, flags);
        let re = RegexBuilder::new(&prefixed)
            .build()
            .map_err(|e| e.to_string())?;

        let mut full_matches = Vec::new();
        let mut group_matches = Vec::new();

        let mut start = 0;
        while start <= text.len() {
            let caps = re.captures_from_pos(text, start).map_err(|e| e.to_string())?;
            let caps = match caps {
                Some(c) => c,
                None => break,
            };

            let m = caps.get(0).unwrap();
            if m.start() == m.end() {
                start = m.end() + 1;
                if m.start() >= text.len() {
                    break;
                }
                continue;
            }

            full_matches.push(MatchSpan {
                start: m.start(),
                end: m.end(),
                text: m.as_str().to_string(),
            });

            let mut groups = Vec::new();
            let group_count = re.captures_len();
            for i in 1..group_count {
                groups.push(caps.get(i).map(|g| MatchSpan {
                    start: g.start(),
                    end: g.end(),
                    text: g.as_str().to_string(),
                }));
            }
            group_matches.push(groups);

            start = m.end();
        }

        Ok(MatchResult { full_matches, group_matches })
    }
}

fn build_pattern(pattern: &str, flags: &RegexFlags) -> String {
    let mut prefix = String::new();
    let mut has_flags = false;
    prefix.push_str("(?");
    if flags.case_insensitive { prefix.push('i'); has_flags = true; }
    if flags.multiline { prefix.push('m'); has_flags = true; }
    if flags.dot_matches_newline { prefix.push('s'); has_flags = true; }
    if flags.unicode { prefix.push('u'); has_flags = true; }
    if flags.extended { prefix.push('x'); has_flags = true; }
    prefix.push(')');

    if has_flags {
        format!("{}{}", prefix, pattern)
    } else {
        pattern.to_string()
    }
}
