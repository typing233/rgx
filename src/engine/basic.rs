use regex::RegexBuilder;

use super::{EngineLevel, MatchResult, MatchSpan, RegexEngine, RegexFlags};

pub struct BasicEngine;

impl RegexEngine for BasicEngine {
    fn name(&self) -> EngineLevel {
        EngineLevel::Basic
    }

    fn find_matches(&self, pattern: &str, text: &str, flags: &RegexFlags) -> Result<MatchResult, String> {
        let re = RegexBuilder::new(pattern)
            .case_insensitive(flags.case_insensitive)
            .multi_line(flags.multiline)
            .dot_matches_new_line(flags.dot_matches_newline)
            .unicode(flags.unicode)
            .ignore_whitespace(flags.extended)
            .build()
            .map_err(|e| e.to_string())?;

        let mut full_matches = Vec::new();
        let mut group_matches = Vec::new();

        for caps in re.captures_iter(text) {
            let m = caps.get(0).unwrap();
            full_matches.push(MatchSpan {
                start: m.start(),
                end: m.end(),
                text: m.as_str().to_string(),
            });

            let mut groups = Vec::new();
            for i in 1..caps.len() {
                groups.push(caps.get(i).map(|g| MatchSpan {
                    start: g.start(),
                    end: g.end(),
                    text: g.as_str().to_string(),
                }));
            }
            group_matches.push(groups);
        }

        Ok(MatchResult { full_matches, group_matches })
    }
}
