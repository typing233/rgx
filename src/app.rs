use crate::ast::features::detect_features;
use crate::codegen::{self, Language};
use crate::debugger::{self, DebugSession};
use crate::engine::{EngineManager, MatchResult, RegexFlags};
use crate::explain;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Regex,
    TestString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    Debugger,
    CodeGen,
}

#[derive(Debug, Clone)]
struct InputState {
    text: String,
    cursor: usize,
}

#[derive(Debug, Clone)]
struct UndoHistory {
    states: Vec<InputState>,
    current: usize,
}

impl UndoHistory {
    fn new() -> Self {
        Self {
            states: vec![InputState { text: String::new(), cursor: 0 }],
            current: 0,
        }
    }

    fn push(&mut self, text: String, cursor: usize) {
        self.states.truncate(self.current + 1);
        self.states.push(InputState { text, cursor });
        self.current = self.states.len() - 1;
    }

    fn undo(&mut self) -> Option<&InputState> {
        if self.current > 0 {
            self.current -= 1;
            Some(&self.states[self.current])
        } else {
            None
        }
    }

    fn redo(&mut self) -> Option<&InputState> {
        if self.current + 1 < self.states.len() {
            self.current += 1;
            Some(&self.states[self.current])
        } else {
            None
        }
    }
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

    // Overlay state
    pub overlay: Overlay,

    // Debugger
    pub debug_session: Option<DebugSession>,

    // Code generation
    pub codegen_selected: usize,
    pub codegen_output: Option<String>,

    // Explanation
    pub explanation: String,

    // Undo/redo
    regex_history: UndoHistory,
    test_history: UndoHistory,

    // Clipboard
    clipboard: Option<arboard::Clipboard>,
}

impl App {
    pub fn new() -> Self {
        let clipboard = arboard::Clipboard::new().ok();
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
            overlay: Overlay::None,
            debug_session: None,
            codegen_selected: 0,
            codegen_output: None,
            explanation: String::new(),
            regex_history: UndoHistory::new(),
            test_history: UndoHistory::new(),
            clipboard,
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
                self.regex_history.push(self.regex_input.clone(), self.regex_cursor);
            }
            Focus::TestString => {
                self.test_input.insert(self.test_cursor, c);
                self.test_cursor += c.len_utf8();
                self.test_history.push(self.test_input.clone(), self.test_cursor);
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
                    self.regex_history.push(self.regex_input.clone(), self.regex_cursor);
                }
            }
            Focus::TestString => {
                if self.test_cursor > 0 {
                    let prev = prev_char_boundary(&self.test_input, self.test_cursor);
                    self.test_input.drain(prev..self.test_cursor);
                    self.test_cursor = prev;
                    self.test_history.push(self.test_input.clone(), self.test_cursor);
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
                    self.regex_history.push(self.regex_input.clone(), self.regex_cursor);
                }
            }
            Focus::TestString => {
                if self.test_cursor < self.test_input.len() {
                    let next = next_char_boundary(&self.test_input, self.test_cursor);
                    self.test_input.drain(self.test_cursor..next);
                    self.test_history.push(self.test_input.clone(), self.test_cursor);
                }
            }
        }
        self.on_input_changed();
    }

    pub fn undo(&mut self) {
        match self.focus {
            Focus::Regex => {
                if let Some(state) = self.regex_history.undo() {
                    self.regex_input = state.text.clone();
                    self.regex_cursor = state.cursor;
                }
            }
            Focus::TestString => {
                if let Some(state) = self.test_history.undo() {
                    self.test_input = state.text.clone();
                    self.test_cursor = state.cursor;
                }
            }
        }
        self.on_input_changed();
    }

    pub fn redo(&mut self) {
        match self.focus {
            Focus::Regex => {
                if let Some(state) = self.regex_history.redo() {
                    self.regex_input = state.text.clone();
                    self.regex_cursor = state.cursor;
                }
            }
            Focus::TestString => {
                if let Some(state) = self.test_history.redo() {
                    self.test_input = state.text.clone();
                    self.test_cursor = state.cursor;
                }
            }
        }
        self.on_input_changed();
    }

    pub fn copy_matches(&mut self) {
        if let Some(clipboard) = &mut self.clipboard {
            if let Some(result) = &self.last_result {
                let text: String = result.full_matches.iter()
                    .map(|m| m.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                let _ = clipboard.set_text(text);
            }
        }
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

    // Overlay toggles
    pub fn toggle_debugger(&mut self) {
        if self.overlay == Overlay::Debugger {
            self.overlay = Overlay::None;
            self.debug_session = None;
        } else {
            self.overlay = Overlay::Debugger;
            let steps = debugger::collect_debug_steps(&self.regex_input, &self.test_input);
            self.debug_session = Some(DebugSession::new(steps));
        }
    }

    pub fn toggle_codegen(&mut self) {
        if self.overlay == Overlay::CodeGen {
            self.overlay = Overlay::None;
            self.codegen_output = None;
        } else {
            self.overlay = Overlay::CodeGen;
            self.update_codegen();
        }
    }

    pub fn codegen_select_next(&mut self) {
        self.codegen_selected = (self.codegen_selected + 1) % Language::ALL.len();
        self.update_codegen();
    }

    pub fn codegen_select_prev(&mut self) {
        if self.codegen_selected == 0 {
            self.codegen_selected = Language::ALL.len() - 1;
        } else {
            self.codegen_selected -= 1;
        }
        self.update_codegen();
    }

    fn update_codegen(&mut self) {
        let lang = Language::ALL[self.codegen_selected];
        let code = codegen::generate_code(lang, &self.regex_input, &self.flags, self.engine_manager.current_level());
        self.codegen_output = Some(code);
    }

    pub fn copy_codegen(&mut self) {
        if let Some(clipboard) = &mut self.clipboard {
            if let Some(code) = &self.codegen_output {
                let _ = clipboard.set_text(code.clone());
            }
        }
    }

    fn on_input_changed(&mut self) {
        if !self.engine_manager.is_manual() && !self.regex_input.is_empty() {
            let features = detect_features(&self.regex_input);
            self.engine_manager.set_auto_level(features.required_engine());
        }
        self.run_match();
        self.explanation = explain::explain_regex(&self.regex_input);

        if self.overlay == Overlay::Debugger {
            let steps = debugger::collect_debug_steps(&self.regex_input, &self.test_input);
            self.debug_session = Some(DebugSession::new(steps));
        }
        if self.overlay == Overlay::CodeGen {
            self.update_codegen();
        }
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
