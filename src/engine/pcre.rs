use pcre2::bytes::RegexBuilder;

use super::{EngineLevel, MatchResult, MatchSpan, RegexEngine, RegexFlags};

pub struct Pcre2Engine;

impl RegexEngine for Pcre2Engine {
    fn name(&self) -> EngineLevel {
        EngineLevel::Pcre2
    }

    fn find_matches(&self, pattern: &str, text: &str, flags: &RegexFlags) -> Result<MatchResult, String> {
        let re = RegexBuilder::new()
            .caseless(flags.case_insensitive)
            .multi_line(flags.multiline)
            .dotall(flags.dot_matches_newline)
            .ucp(flags.unicode)
            .utf(true)
            .extended(flags.extended)
            .build(pattern)
            .map_err(|e| e.to_string())?;

        let text_bytes = text.as_bytes();
        let mut full_matches = Vec::new();
        let mut group_matches = Vec::new();

        let mut start = 0;
        loop {
            if start > text_bytes.len() {
                break;
            }

            let subject = &text_bytes[start..];
            let caps = re.captures(subject).map_err(|e| e.to_string())?;
            let caps = match caps {
                Some(c) => c,
                None => break,
            };

            let m = match caps.get(0) {
                Some(m) => m,
                None => break,
            };

            let abs_start = start + m.start();
            let abs_end = start + m.end();

            if m.start() == m.end() {
                start += m.end() + 1;
                continue;
            }

            let match_text = String::from_utf8_lossy(&text_bytes[abs_start..abs_end]).to_string();
            full_matches.push(MatchSpan {
                start: abs_start,
                end: abs_end,
                text: match_text,
            });

            let mut groups = Vec::new();
            for i in 1..caps.len() {
                groups.push(caps.get(i).map(|g| {
                    let g_abs_start = start + g.start();
                    let g_abs_end = start + g.end();
                    let g_text = String::from_utf8_lossy(&text_bytes[g_abs_start..g_abs_end]).to_string();
                    MatchSpan {
                        start: g_abs_start,
                        end: g_abs_end,
                        text: g_text,
                    }
                }));
            }
            group_matches.push(groups);

            start = abs_end;
        }

        Ok(MatchResult { full_matches, group_matches })
    }
}
