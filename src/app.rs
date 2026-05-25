use crate::ast::features::detect_features;
use crate::engine::{EngineManager, MatchResult, RegexFlags};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Regex,
    TestString,
}

pub struct App {
    pub regex_input: String,
    pub regex_cursor: usize,
    pub test_input: String,
    pub test_cursor: usize,
    pub focus: Focus,
    pub flags: RegexFlags,
    pub engine_manager: EngineManager,
    pub last_result: Option<MatchResult>,
    pub last_error: Option<String>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            regex_input: String::new(),
            regex_cursor: 0,
            test_input: String::new(),
            test_cursor: 0,
            focus: Focus::Regex,
            flags: RegexFlags::default(),
            engine_manager: EngineManager::new(),
            last_result: None,
            last_error: None,
            should_quit: false,
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Regex => Focus::TestString,
            Focus::TestString => Focus::Regex,
        };
    }

    pub fn cycle_engine(&mut self) {
        self.engine_manager.cycle_engine();
        self.run_match();
    }

    pub fn toggle_flag_case_insensitive(&mut self) {
        self.flags.case_insensitive = !self.flags.case_insensitive;
        self.run_match();
    }

    pub fn toggle_flag_multiline(&mut self) {
        self.flags.multiline = !self.flags.multiline;
        self.run_match();
    }

    pub fn toggle_flag_dotall(&mut self) {
        self.flags.dot_matches_newline = !self.flags.dot_matches_newline;
        self.run_match();
    }

    pub fn toggle_flag_unicode(&mut self) {
        self.flags.unicode = !self.flags.unicode;
        self.run_match();
    }

    pub fn toggle_flag_extended(&mut self) {
        self.flags.extended = !self.flags.extended;
        self.run_match();
    }

    pub fn insert_char(&mut self, c: char) {
        match self.focus {
            Focus::Regex => {
                self.regex_input.insert(self.regex_cursor, c);
                self.regex_cursor += c.len_utf8();
            }
            Focus::TestString => {
                self.test_input.insert(self.test_cursor, c);
                self.test_cursor += c.len_utf8();
            }
        }
        self.on_input_changed();
    }

    pub fn delete_char(&mut self) {
        match self.focus {
            Focus::Regex => {
                if self.regex_cursor > 0 {
                    let prev = prev_char_boundary(&self.regex_input, self.regex_cursor);
                    self.regex_input.drain(prev..self.regex_cursor);
                    self.regex_cursor = prev;
                }
            }
            Focus::TestString => {
                if self.test_cursor > 0 {
                    let prev = prev_char_boundary(&self.test_input, self.test_cursor);
                    self.test_input.drain(prev..self.test_cursor);
                    self.test_cursor = prev;
                }
            }
        }
        self.on_input_changed();
    }

    pub fn delete_forward(&mut self) {
        match self.focus {
            Focus::Regex => {
                if self.regex_cursor < self.regex_input.len() {
                    let next = next_char_boundary(&self.regex_input, self.regex_cursor);
                    self.regex_input.drain(self.regex_cursor..next);
                }
            }
            Focus::TestString => {
                if self.test_cursor < self.test_input.len() {
                    let next = next_char_boundary(&self.test_input, self.test_cursor);
                    self.test_input.drain(self.test_cursor..next);
                }
            }
        }
        self.on_input_changed();
    }

    pub fn move_cursor_left(&mut self) {
        match self.focus {
            Focus::Regex => {
                if self.regex_cursor > 0 {
                    self.regex_cursor = prev_char_boundary(&self.regex_input, self.regex_cursor);
                }
            }
            Focus::TestString => {
                if self.test_cursor > 0 {
                    self.test_cursor = prev_char_boundary(&self.test_input, self.test_cursor);
                }
            }
        }
    }

    pub fn move_cursor_right(&mut self) {
        match self.focus {
            Focus::Regex => {
                if self.regex_cursor < self.regex_input.len() {
                    self.regex_cursor = next_char_boundary(&self.regex_input, self.regex_cursor);
                }
            }
            Focus::TestString => {
                if self.test_cursor < self.test_input.len() {
                    self.test_cursor = next_char_boundary(&self.test_input, self.test_cursor);
                }
            }
        }
    }

    pub fn move_cursor_home(&mut self) {
        match self.focus {
            Focus::Regex => self.regex_cursor = 0,
            Focus::TestString => self.test_cursor = 0,
        }
    }

    pub fn move_cursor_end(&mut self) {
        match self.focus {
            Focus::Regex => self.regex_cursor = self.regex_input.len(),
            Focus::TestString => self.test_cursor = self.test_input.len(),
        }
    }

    fn on_input_changed(&mut self) {
        if !self.engine_manager.is_manual() && !self.regex_input.is_empty() {
            let features = detect_features(&self.regex_input);
            self.engine_manager.set_auto_level(features.required_engine());
        }
        self.run_match();
    }

    pub fn run_match(&mut self) {
        if self.regex_input.is_empty() {
            self.last_result = None;
            self.last_error = None;
            return;
        }

        match self.engine_manager.execute(&self.regex_input, &self.test_input, &self.flags) {
            Ok(result) => {
                self.last_result = Some(result);
                self.last_error = None;
            }
            Err(e) => {
                self.last_result = None;
                self.last_error = Some(e);
            }
        }
    }

    pub fn match_count(&self) -> usize {
        self.last_result.as_ref().map_or(0, |r| r.full_matches.len())
    }
}

fn prev_char_boundary(s: &str, pos: usize) -> usize {
    let mut p = pos.saturating_sub(1);
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

fn next_char_boundary(s: &str, pos: usize) -> usize {
    let mut p = pos + 1;
    while p < s.len() && !s.is_char_boundary(p) {
        p += 1;
    }
    p
}
