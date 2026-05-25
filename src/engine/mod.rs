#![allow(dead_code)]

pub mod basic;
pub mod fancy;
pub mod pcre;

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EngineLevel {
    Basic,
    Fancy,
    Pcre2,
}

impl EngineLevel {
    pub fn next(self) -> Self {
        match self {
            Self::Basic => Self::Fancy,
            Self::Fancy => Self::Pcre2,
            Self::Pcre2 => Self::Basic,
        }
    }
}

impl fmt::Display for EngineLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Basic => write!(f, "regex"),
            Self::Fancy => write!(f, "fancy-regex"),
            Self::Pcre2 => write!(f, "PCRE2"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegexFlags {
    pub case_insensitive: bool,
    pub multiline: bool,
    pub dot_matches_newline: bool,
    pub unicode: bool,
    pub extended: bool,
}

impl Default for RegexFlags {
    fn default() -> Self {
        Self {
            case_insensitive: false,
            multiline: false,
            dot_matches_newline: false,
            unicode: true,
            extended: false,
        }
    }
}

impl RegexFlags {
    pub fn as_prefix(&self) -> String {
        let mut s = String::new();
        if self.case_insensitive || self.multiline || self.dot_matches_newline || self.unicode || self.extended {
            s.push_str("(?");
            if self.case_insensitive { s.push('i'); }
            if self.multiline { s.push('m'); }
            if self.dot_matches_newline { s.push('s'); }
            if self.unicode { s.push('u'); }
            if self.extended { s.push('x'); }
            s.push(')');
        }
        s
    }
}

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub full_matches: Vec<MatchSpan>,
    pub group_matches: Vec<Vec<Option<MatchSpan>>>,
}

#[derive(Debug, Clone)]
pub struct MatchSpan {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

pub trait RegexEngine: Send {
    fn name(&self) -> EngineLevel;
    fn find_matches(&self, pattern: &str, text: &str, flags: &RegexFlags) -> Result<MatchResult, String>;
}

pub struct EngineManager {
    current: EngineLevel,
    auto_detected: EngineLevel,
    manual_override: bool,
}

impl EngineManager {
    pub fn new() -> Self {
        Self {
            current: EngineLevel::Basic,
            auto_detected: EngineLevel::Basic,
            manual_override: false,
        }
    }

    pub fn current_level(&self) -> EngineLevel {
        self.current
    }

    pub fn set_auto_level(&mut self, level: EngineLevel) {
        self.auto_detected = level;
        if !self.manual_override {
            self.current = level;
        }
    }

    pub fn cycle_engine(&mut self) {
        self.manual_override = true;
        self.current = self.current.next();
    }

    pub fn is_manual(&self) -> bool {
        self.manual_override
    }

    pub fn reset_to_auto(&mut self) {
        self.manual_override = false;
        self.current = self.auto_detected;
    }

    pub fn execute(&self, pattern: &str, text: &str, flags: &RegexFlags) -> Result<MatchResult, String> {
        match self.current {
            EngineLevel::Basic => {
                let engine = basic::BasicEngine;
                engine.find_matches(pattern, text, flags)
            }
            EngineLevel::Fancy => {
                let engine = fancy::FancyEngine;
                engine.find_matches(pattern, text, flags)
            }
            EngineLevel::Pcre2 => {
                let engine = pcre::Pcre2Engine;
                engine.find_matches(pattern, text, flags)
            }
        }
    }
}
