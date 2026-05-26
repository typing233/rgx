use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DebugStep {
    pub pattern_offset: usize,
    pub text_offset: usize,
    pub description: String,
    pub is_backtrack: bool,
    pub matched: bool,
}

#[derive(Debug, Clone)]
pub struct DebugSession {
    pub steps: Vec<DebugStep>,
    pub current_step: usize,
    pub heatmap: HashMap<usize, usize>,
    pub heatmap_mode: bool,
}

impl DebugSession {
    pub fn new(steps: Vec<DebugStep>) -> Self {
        let mut heatmap = HashMap::new();
        for step in &steps {
            *heatmap.entry(step.pattern_offset).or_insert(0) += 1;
        }
        Self {
            steps,
            current_step: 0,
            heatmap,
            heatmap_mode: false,
        }
    }

    pub fn step_forward(&mut self) {
        if self.current_step < self.steps.len().saturating_sub(1) {
            self.current_step += 1;
        }
    }

    pub fn step_backward(&mut self) {
        if self.current_step > 0 {
            self.current_step -= 1;
        }
    }

    pub fn current(&self) -> Option<&DebugStep> {
        self.steps.get(self.current_step)
    }

    pub fn toggle_heatmap(&mut self) {
        self.heatmap_mode = !self.heatmap_mode;
    }

    pub fn max_heat(&self) -> usize {
        self.heatmap.values().copied().max().unwrap_or(1)
    }

    pub fn total_steps(&self) -> usize {
        self.steps.len()
    }

    pub fn backtrack_count(&self) -> usize {
        self.steps.iter().filter(|s| s.is_backtrack).count()
    }
}

// ===== Regex IR: compile pattern string into a small instruction set =====

#[derive(Debug, Clone)]
enum Inst {
    Literal(char, usize),           // char, pattern_offset
    Dot(usize),                     // pattern_offset
    Anchor(AnchorKind, usize),      // kind, pattern_offset
    Class(Vec<ClassRange>, bool, usize), // ranges, negated, pattern_offset
    Escape(EscKind, usize),         // kind, pattern_offset
    Split(usize, usize, usize),     // target_a, target_b, pattern_offset (for alternation/quantifiers)
    Jump(usize),                    // target
    Match,                          // success
    Save(usize),                    // save point marker (for group tracking)
}

#[derive(Debug, Clone, Copy)]
enum AnchorKind {
    Start,
    End,
}

#[derive(Debug, Clone, Copy)]
enum EscKind {
    Digit,
    NonDigit,
    Word,
    NonWord,
    Space,
    NonSpace,
    Char(char),
}

#[derive(Debug, Clone)]
struct ClassRange {
    start: char,
    end: char,
}

impl ClassRange {
    fn contains(&self, c: char) -> bool {
        c >= self.start && c <= self.end
    }
}

struct Compiler {
    insts: Vec<Inst>,
    pattern: Vec<char>,
    pos: usize,
}

impl Compiler {
    fn new(pattern: &str) -> Self {
        Self {
            insts: Vec::new(),
            pattern: pattern.chars().collect(),
            pos: 0,
        }
    }

    fn compile(mut self) -> Vec<Inst> {
        self.compile_alternation();
        self.insts.push(Inst::Match);
        self.insts
    }

    fn compile_alternation(&mut self) {
        self.compile_alternation_full(0);
    }

    fn compile_concat(&mut self) {
        while self.pos < self.pattern.len()
            && self.pattern[self.pos] != ')'
            && self.pattern[self.pos] != '|'
        {
            self.compile_quantified();
        }
    }

    fn compile_quantified(&mut self) {
        let atom_start = self.insts.len();
        let atom_pat_start = self.pos;
        self.compile_atom();
        let atom_end = self.insts.len();

        if self.pos < self.pattern.len() {
            match self.pattern[self.pos] {
                '*' | '+' | '?' => {
                    let quant = self.pattern[self.pos];
                    self.pos += 1;
                    let lazy = self.pos < self.pattern.len() && self.pattern[self.pos] == '?';
                    if lazy { self.pos += 1; }
                    self.apply_quantifier(atom_start, atom_end, atom_pat_start, quant, lazy);
                }
                '{' => {
                    if let Some((min, max, end_pos)) = self.parse_range_quant() {
                        self.pos = end_pos;
                        let lazy = self.pos < self.pattern.len() && self.pattern[self.pos] == '?';
                        if lazy { self.pos += 1; }
                        self.apply_range_quantifier(atom_start, atom_end, atom_pat_start, min, max, lazy);
                    }
                }
                _ => {}
            }
        }
    }

    fn compile_atom(&mut self) {
        if self.pos >= self.pattern.len() {
            return;
        }
        let pat_off = self.pos;
        let ch = self.pattern[self.pos];
        match ch {
            '(' => {
                self.pos += 1; // skip '('
                // check for (?:...)
                if self.pos + 1 < self.pattern.len()
                    && self.pattern[self.pos] == '?'
                    && self.pattern[self.pos + 1] == ':'
                {
                    self.pos += 2; // skip '?:'
                }
                self.compile_alternation_full(pat_off);
                if self.pos < self.pattern.len() && self.pattern[self.pos] == ')' {
                    self.pos += 1;
                }
            }
            '[' => self.compile_class(),
            '\\' => {
                self.pos += 1;
                if self.pos < self.pattern.len() {
                    let esc = self.pattern[self.pos];
                    self.pos += 1;
                    let kind = match esc {
                        'd' => EscKind::Digit,
                        'D' => EscKind::NonDigit,
                        'w' => EscKind::Word,
                        'W' => EscKind::NonWord,
                        's' => EscKind::Space,
                        'S' => EscKind::NonSpace,
                        _ => EscKind::Char(esc),
                    };
                    self.insts.push(Inst::Escape(kind, pat_off));
                }
            }
            '.' => {
                self.pos += 1;
                self.insts.push(Inst::Dot(pat_off));
            }
            '^' => {
                self.pos += 1;
                self.insts.push(Inst::Anchor(AnchorKind::Start, pat_off));
            }
            '$' => {
                self.pos += 1;
                self.insts.push(Inst::Anchor(AnchorKind::End, pat_off));
            }
            _ => {
                self.pos += 1;
                self.insts.push(Inst::Literal(ch, pat_off));
            }
        }
    }

    fn compile_alternation_full(&mut self, group_pat_off: usize) {
        // Compile each branch into a separate Vec<Inst>, then assemble with Splits.
        let base = self.insts.len();

        // Collect branches: compile into self.insts temporarily, then extract.
        let mut branches: Vec<Vec<Inst>> = Vec::new();

        let branch_start = self.insts.len();
        self.compile_concat();
        let first: Vec<Inst> = self.insts.drain(branch_start..).collect();
        branches.push(first);

        while self.pos < self.pattern.len() && self.pattern[self.pos] == '|' {
            self.pos += 1; // skip '|'
            let bs = self.insts.len();
            self.compile_concat();
            let branch: Vec<Inst> = self.insts.drain(bs..).collect();
            branches.push(branch);
        }

        let n = branches.len();
        if n == 1 {
            for inst in branches.into_iter().next().unwrap() {
                self.insts.push(inst);
            }
            return;
        }

        // Layout for N branches:
        //   [N-1 Split insts] [Branch0 code] [Jump] [Branch1 code] [Jump] ... [BranchN-1 code]
        //
        // All addresses are absolute (index into self.insts).

        let num_splits = n - 1;

        // Calculate branch code offset (where each branch's code starts)
        let code_area_start = base + num_splits;
        let mut branch_abs_start = Vec::new();
        let mut cursor = code_area_start;
        for (i, branch) in branches.iter().enumerate() {
            branch_abs_start.push(cursor);
            cursor += branch.len();
            if i < n - 1 {
                cursor += 1; // for the Jump instruction after each non-last branch
            }
        }
        let end_pc = cursor;

        // Emit the Split chain
        for i in 0..num_splits {
            let prefer = branch_abs_start[i];
            let alt = if i + 1 < num_splits {
                base + i + 1 // next Split in the chain
            } else {
                branch_abs_start[n - 1] // last branch directly
            };
            self.insts.push(Inst::Split(prefer, alt, group_pat_off));
        }

        // Emit branch code, each (except last) followed by Jump(end_pc)
        for (i, branch) in branches.into_iter().enumerate() {
            // Branch was compiled with self.insts starting at `base`, so any internal
            // Jump/Split targets are absolute indices starting from `base`. We need to
            // shift them to their new absolute position: branch_abs_start[i] - base.
            let reloc = branch_abs_start[i] - base;
            for inst in branch {
                self.insts.push(Self::relocate_inst(inst, reloc));
            }
            if i < n - 1 {
                self.insts.push(Inst::Jump(end_pc));
            }
        }
    }

    fn relocate_inst(inst: Inst, delta: usize) -> Inst {
        // Shift absolute pc references (Split targets, Jump targets) by `delta`.
        match inst {
            Inst::Split(a, b, off) => Inst::Split(a + delta, b + delta, off),
            Inst::Jump(target) => Inst::Jump(target + delta),
            other => other,
        }
    }

    fn apply_quantifier(&mut self, atom_start: usize, atom_end: usize, pat_off: usize, quant: char, lazy: bool) {
        let body: Vec<Inst> = self.insts[atom_start..atom_end].to_vec();
        self.insts.truncate(atom_start);

        match quant {
            '*' => {
                // Split(body, after); body...; Jump(split)
                let split_pc = self.insts.len();
                let body_pc = split_pc + 1;
                let after_pc = body_pc + body.len() + 1; // +1 for Jump
                if lazy {
                    self.insts.push(Inst::Split(after_pc, body_pc, pat_off));
                } else {
                    self.insts.push(Inst::Split(body_pc, after_pc, pat_off));
                }
                for inst in body {
                    self.insts.push(inst);
                }
                self.insts.push(Inst::Jump(split_pc));
            }
            '+' => {
                // body...; Split(body, after)
                let body_pc = self.insts.len();
                for inst in body {
                    self.insts.push(inst);
                }
                let split_pc = self.insts.len();
                let after_pc = split_pc + 1;
                if lazy {
                    self.insts.push(Inst::Split(after_pc, body_pc, pat_off));
                } else {
                    self.insts.push(Inst::Split(body_pc, after_pc, pat_off));
                }
            }
            '?' => {
                // Split(body, after)
                let split_pc = self.insts.len();
                let body_pc = split_pc + 1;
                let after_pc = body_pc + body.len();
                if lazy {
                    self.insts.push(Inst::Split(after_pc, body_pc, pat_off));
                } else {
                    self.insts.push(Inst::Split(body_pc, after_pc, pat_off));
                }
                for inst in body {
                    self.insts.push(inst);
                }
            }
            _ => {}
        }
    }

    fn apply_range_quantifier(&mut self, atom_start: usize, atom_end: usize, pat_off: usize, min: usize, max: usize, lazy: bool) {
        let body: Vec<Inst> = self.insts[atom_start..atom_end].to_vec();
        self.insts.truncate(atom_start);

        // Emit `min` mandatory copies
        for _ in 0..min {
            for inst in &body {
                self.insts.push(inst.clone());
            }
        }

        // Emit up to (max - min) optional copies
        let extra = if max == usize::MAX {
            // treat as * after min: use loop
            let split_pc = self.insts.len();
            let body_pc = split_pc + 1;
            let after_pc = body_pc + body.len() + 1;
            if lazy {
                self.insts.push(Inst::Split(after_pc, body_pc, pat_off));
            } else {
                self.insts.push(Inst::Split(body_pc, after_pc, pat_off));
            }
            for inst in &body {
                self.insts.push(inst.clone());
            }
            self.insts.push(Inst::Jump(split_pc));
            return;
        } else {
            max - min
        };

        // For bounded: emit `extra` optional (Split + body) sequences
        // We need to know the final end to patch Splits, so pre-calculate
        let opt_size = 1 + body.len(); // Split + body for each optional iteration
        let total_extra_size = extra * opt_size;
        let base = self.insts.len();

        for i in 0..extra {
            let split_pc = base + i * opt_size;
            let body_pc = split_pc + 1;
            let after_pc = base + total_extra_size;
            if lazy {
                self.insts.push(Inst::Split(after_pc, body_pc, pat_off));
            } else {
                self.insts.push(Inst::Split(body_pc, after_pc, pat_off));
            }
            for inst in &body {
                self.insts.push(inst.clone());
            }
        }
    }

    fn parse_range_quant(&self) -> Option<(usize, usize, usize)> {
        let mut i = self.pos + 1;
        let mut num_str = String::new();
        while i < self.pattern.len() && self.pattern[i].is_ascii_digit() {
            num_str.push(self.pattern[i]);
            i += 1;
        }
        if num_str.is_empty() || i >= self.pattern.len() { return None; }
        let min: usize = num_str.parse().ok()?;
        if self.pattern[i] == '}' {
            return Some((min, min, i + 1));
        }
        if self.pattern[i] != ',' { return None; }
        i += 1;
        let mut max_str = String::new();
        while i < self.pattern.len() && self.pattern[i].is_ascii_digit() {
            max_str.push(self.pattern[i]);
            i += 1;
        }
        if i >= self.pattern.len() || self.pattern[i] != '}' { return None; }
        let max = if max_str.is_empty() { usize::MAX } else { max_str.parse().ok()? };
        Some((min, max, i + 1))
    }

    fn compile_class(&mut self) {
        let pat_off = self.pos;
        self.pos += 1; // skip '['
        let negated = self.pos < self.pattern.len() && self.pattern[self.pos] == '^';
        if negated { self.pos += 1; }

        let mut ranges = Vec::new();
        // handle ']' as first char
        if self.pos < self.pattern.len() && self.pattern[self.pos] == ']' {
            ranges.push(ClassRange { start: ']', end: ']' });
            self.pos += 1;
        }

        while self.pos < self.pattern.len() && self.pattern[self.pos] != ']' {
            let c = self.pattern[self.pos];
            if c == '\\' && self.pos + 1 < self.pattern.len() {
                self.pos += 1;
                let esc = self.pattern[self.pos];
                self.pos += 1;
                // Could be \d, \w, etc inside class - simplify to char
                let ch = match esc {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    _ => esc,
                };
                if self.pos + 1 < self.pattern.len() && self.pattern[self.pos] == '-' && self.pattern[self.pos + 1] != ']' {
                    self.pos += 1;
                    let end_c = self.pattern[self.pos];
                    self.pos += 1;
                    ranges.push(ClassRange { start: ch, end: end_c });
                } else {
                    ranges.push(ClassRange { start: ch, end: ch });
                }
            } else {
                self.pos += 1;
                if self.pos + 1 < self.pattern.len() && self.pattern[self.pos] == '-' && self.pattern[self.pos + 1] != ']' {
                    self.pos += 1;
                    let end_c = self.pattern[self.pos];
                    self.pos += 1;
                    ranges.push(ClassRange { start: c, end: end_c });
                } else {
                    ranges.push(ClassRange { start: c, end: c });
                }
            }
        }
        if self.pos < self.pattern.len() { self.pos += 1; } // skip ']'
        self.insts.push(Inst::Class(ranges, negated, pat_off));
    }
}

// ===== VM interpreter with backtracking and step recording =====

const MAX_STEPS: usize = 10000;

pub fn collect_debug_steps(pattern: &str, text: &str) -> Vec<DebugStep> {
    if pattern.is_empty() {
        return vec![DebugStep {
            pattern_offset: 0,
            text_offset: 0,
            description: "Empty pattern".to_string(),
            is_backtrack: false,
            matched: true,
        }];
    }

    let compiler = Compiler::new(pattern);
    let program = compiler.compile();
    let text_chars: Vec<char> = text.chars().collect();

    let mut steps = Vec::new();

    for start in 0..=text_chars.len() {
        if steps.len() >= MAX_STEPS {
            steps.push(DebugStep {
                pattern_offset: 0,
                text_offset: start,
                description: "Step limit reached (catastrophic backtracking detected)".to_string(),
                is_backtrack: true,
                matched: false,
            });
            break;
        }

        let found = vm_execute(&program, &text_chars, start, pattern, &mut steps);
        if found {
            break;
        }
    }

    if steps.is_empty() {
        steps.push(DebugStep {
            pattern_offset: 0,
            text_offset: 0,
            description: "No match attempt".to_string(),
            is_backtrack: false,
            matched: false,
        });
    }

    steps
}

#[derive(Clone)]
struct Thread {
    pc: usize,
    ti: usize,
}

fn vm_execute(
    program: &[Inst],
    text: &[char],
    start_ti: usize,
    pattern: &str,
    steps: &mut Vec<DebugStep>,
) -> bool {
    let mut stack: Vec<Thread> = Vec::new();
    let mut pc = 0;
    let mut ti = start_ti;
    let text_len = text.len();
    let step_base = steps.len();

    steps.push(DebugStep {
        pattern_offset: 0,
        text_offset: start_ti,
        description: format!("Try from text[{}]", start_ti),
        is_backtrack: false,
        matched: false,
    });

    loop {
        if steps.len() >= MAX_STEPS {
            return false;
        }
        if steps.len() - step_base > 5000 {
            steps.push(DebugStep {
                pattern_offset: inst_pat_off(program, pc),
                text_offset: ti,
                description: "Per-position step limit".to_string(),
                is_backtrack: false,
                matched: false,
            });
            return false;
        }

        if pc >= program.len() {
            // Shouldn't happen, but just in case
            if let Some(t) = stack.pop() {
                steps.push(DebugStep {
                    pattern_offset: inst_pat_off(program, t.pc),
                    text_offset: t.ti,
                    description: format!("Backtrack to p[{}] t[{}]", inst_pat_off(program, t.pc), t.ti),
                    is_backtrack: true,
                    matched: false,
                });
                pc = t.pc;
                ti = t.ti;
                continue;
            }
            return false;
        }

        match &program[pc] {
            Inst::Match => {
                steps.push(DebugStep {
                    pattern_offset: pattern.len(),
                    text_offset: ti,
                    description: format!("Match found [{},{})", start_ti, ti),
                    is_backtrack: false,
                    matched: true,
                });
                return true;
            }
            Inst::Literal(ch, pat_off) => {
                let ok = ti < text_len && text[ti] == *ch;
                steps.push(DebugStep {
                    pattern_offset: *pat_off,
                    text_offset: ti,
                    description: if ok {
                        format!("'{}' matched", ch)
                    } else if ti < text_len {
                        format!("'{}' != '{}'", ch, text[ti])
                    } else {
                        format!("'{}' at end of text", ch)
                    },
                    is_backtrack: false,
                    matched: ok,
                });
                if ok {
                    pc += 1;
                    ti += 1;
                    continue;
                }
            }
            Inst::Dot(pat_off) => {
                let ok = ti < text_len && text[ti] != '\n';
                steps.push(DebugStep {
                    pattern_offset: *pat_off,
                    text_offset: ti,
                    description: if ok {
                        format!(". matches '{}'", text[ti])
                    } else {
                        ". failed".to_string()
                    },
                    is_backtrack: false,
                    matched: ok,
                });
                if ok {
                    pc += 1;
                    ti += 1;
                    continue;
                }
            }
            Inst::Anchor(kind, pat_off) => {
                let ok = match kind {
                    AnchorKind::Start => ti == 0,
                    AnchorKind::End => ti == text_len,
                };
                steps.push(DebugStep {
                    pattern_offset: *pat_off,
                    text_offset: ti,
                    description: format!("{} {}",
                        match kind { AnchorKind::Start => "^", AnchorKind::End => "$" },
                        if ok { "matched" } else { "failed" }),
                    is_backtrack: false,
                    matched: ok,
                });
                if ok {
                    pc += 1;
                    continue;
                }
            }
            Inst::Escape(kind, pat_off) => {
                let ok = ti < text_len && match_escape(*kind, text[ti]);
                let desc_str = match kind {
                    EscKind::Digit => "\\d",
                    EscKind::NonDigit => "\\D",
                    EscKind::Word => "\\w",
                    EscKind::NonWord => "\\W",
                    EscKind::Space => "\\s",
                    EscKind::NonSpace => "\\S",
                    EscKind::Char(c) => {
                        // leak a description string for display
                        // (only during debug stepping, acceptable)
                        let _ = c;
                        "\\esc"
                    }
                };
                steps.push(DebugStep {
                    pattern_offset: *pat_off,
                    text_offset: ti,
                    description: format!("{} {}", desc_str, if ok { "matched" } else { "failed" }),
                    is_backtrack: false,
                    matched: ok,
                });
                if ok {
                    pc += 1;
                    ti += 1;
                    continue;
                }
            }
            Inst::Class(ranges, negated, pat_off) => {
                let ok = if ti < text_len {
                    let in_class = ranges.iter().any(|r| r.contains(text[ti]));
                    if *negated { !in_class } else { in_class }
                } else {
                    false
                };
                steps.push(DebugStep {
                    pattern_offset: *pat_off,
                    text_offset: ti,
                    description: format!("[...] {}", if ok { "matched" } else { "failed" }),
                    is_backtrack: false,
                    matched: ok,
                });
                if ok {
                    pc += 1;
                    ti += 1;
                    continue;
                }
            }
            Inst::Split(a, b, pat_off) => {
                let a = *a;
                let b = *b;
                let pat_off = *pat_off;
                // Preferred branch is `a`, save `b` as backtrack
                stack.push(Thread { pc: b, ti });
                steps.push(DebugStep {
                    pattern_offset: pat_off,
                    text_offset: ti,
                    description: format!("Split: try preferred, save alternative"),
                    is_backtrack: false,
                    matched: true,
                });
                pc = a;
                continue;
            }
            Inst::Jump(target) => {
                pc = *target;
                continue;
            }
            Inst::Save(_) => {
                pc += 1;
                continue;
            }
        }

        // If we reach here, the current instruction failed. Backtrack.
        if let Some(t) = stack.pop() {
            steps.push(DebugStep {
                pattern_offset: inst_pat_off(program, t.pc),
                text_offset: t.ti,
                description: format!("Backtrack to p[{}] t[{}]", inst_pat_off(program, t.pc), t.ti),
                is_backtrack: true,
                matched: false,
            });
            pc = t.pc;
            ti = t.ti;
        } else {
            return false;
        }
    }
}

fn inst_pat_off(program: &[Inst], pc: usize) -> usize {
    if pc >= program.len() {
        return 0;
    }
    match &program[pc] {
        Inst::Literal(_, off) => *off,
        Inst::Dot(off) => *off,
        Inst::Anchor(_, off) => *off,
        Inst::Escape(_, off) => *off,
        Inst::Class(_, _, off) => *off,
        Inst::Split(_, _, off) => *off,
        Inst::Jump(_) => 0,
        Inst::Match => 0,
        Inst::Save(_) => 0,
    }
}

fn match_escape(kind: EscKind, c: char) -> bool {
    match kind {
        EscKind::Digit => c.is_ascii_digit(),
        EscKind::NonDigit => !c.is_ascii_digit(),
        EscKind::Word => c.is_alphanumeric() || c == '_',
        EscKind::NonWord => !(c.is_alphanumeric() || c == '_'),
        EscKind::Space => c.is_whitespace(),
        EscKind::NonSpace => !c.is_whitespace(),
        EscKind::Char(expected) => c == expected,
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_literal_match() {
        let steps = collect_debug_steps("abc", "abc");
        assert!(steps.last().unwrap().matched);
        assert!(steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_simple_literal_no_match() {
        let steps = collect_debug_steps("xyz", "abc");
        assert!(!steps.last().unwrap().matched);
    }

    #[test]
    fn test_dot_star() {
        let steps = collect_debug_steps("a.*b", "aXXXb");
        assert!(steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_alternation_first_branch() {
        let steps = collect_debug_steps("(cat|dog)", "cat");
        assert!(steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_alternation_second_branch() {
        let steps = collect_debug_steps("(cat|dog)", "dog");
        let found = steps.iter().any(|s| s.description.contains("Match found"));
        assert!(found, "Should match 'dog' via second branch");
        // Should have some backtracking since first branch fails
        let backtracks = steps.iter().filter(|s| s.is_backtrack).count();
        assert!(backtracks > 0, "Should backtrack from first branch to second");
    }

    #[test]
    fn test_group_with_quantifier() {
        let steps = collect_debug_steps("(ab)+", "ababab");
        assert!(steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_nested_group_quantifier() {
        // (a+)+ is a classic catastrophic backtracking pattern when it fails
        let steps = collect_debug_steps("(a+)+b", "aaab");
        assert!(steps.iter().any(|s| s.description.contains("Match found")),
            "Should match 'aaab'");
    }

    #[test]
    fn test_catastrophic_backtracking_detected() {
        // (a+)+b against "aaaaaaaaaaaaa!" triggers exponential backtracking
        // With our step limit, we should see many backtrack steps
        let steps = collect_debug_steps("(a+)+b", "aaaaaaaaaa!");
        let backtrack_count = steps.iter().filter(|s| s.is_backtrack).count();
        // Should have significant backtracking
        assert!(backtrack_count > 20,
            "Expected significant backtracking, got {} backtracks in {} steps",
            backtrack_count, steps.len());
        // Should hit step limit or fail without match
        assert!(!steps.iter().any(|s| s.description.contains("Match found")),
            "Should not match since input doesn't end with 'b'");
    }

    #[test]
    fn test_alternation_catastrophic() {
        // (a|aa)+b against "aaaaaaa!" - another catastrophic pattern
        let steps = collect_debug_steps("(a|aa)+b", "aaaaaaa!");
        let backtrack_count = steps.iter().filter(|s| s.is_backtrack).count();
        assert!(backtrack_count > 10,
            "Expected backtracking from alternation, got {} backtracks in {} steps",
            backtrack_count, steps.len());
    }

    #[test]
    fn test_heatmap_shows_hot_spots() {
        // (a+)+b on failing input should have hot spots at the group/quantifier
        let steps = collect_debug_steps("(a+)+b", "aaaa!");
        let session = DebugSession::new(steps);
        let max = session.max_heat();
        assert!(max > 3, "Heatmap max should be high due to backtracking, got {}", max);
        // Pattern offset 0 (the group start) should be frequently visited
        let heat_at_0 = session.heatmap.get(&0).copied().unwrap_or(0);
        assert!(heat_at_0 > 2, "Group start should be visited multiple times, got {}", heat_at_0);
    }

    #[test]
    fn test_non_capturing_group() {
        let steps = collect_debug_steps("(?:ab)+c", "ababc");
        assert!(steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_nested_alternation() {
        let steps = collect_debug_steps("(a|b)(c|d)", "bd");
        assert!(steps.iter().any(|s| s.description.contains("Match found")),
            "Should match 'bd': b from first group, d from second");
    }

    #[test]
    fn test_quantifier_on_group_with_alternation() {
        // (ab|cd)+ should match "abcdab"
        let steps = collect_debug_steps("(ab|cd)+", "abcdab");
        assert!(steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_step_navigation() {
        let steps = collect_debug_steps("a+b", "aab");
        let mut session = DebugSession::new(steps.clone());
        assert_eq!(session.current_step, 0);
        session.step_forward();
        assert_eq!(session.current_step, 1);
        session.step_backward();
        assert_eq!(session.current_step, 0);
        session.step_backward(); // shouldn't go below 0
        assert_eq!(session.current_step, 0);
    }

    #[test]
    fn test_heatmap_toggle() {
        let steps = collect_debug_steps("a", "a");
        let mut session = DebugSession::new(steps);
        assert!(!session.heatmap_mode);
        session.toggle_heatmap();
        assert!(session.heatmap_mode);
        session.toggle_heatmap();
        assert!(!session.heatmap_mode);
    }

    #[test]
    fn test_escape_in_pattern() {
        let steps = collect_debug_steps(r"\d+", "123");
        assert!(steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_char_class() {
        let steps = collect_debug_steps("[a-z]+", "hello");
        assert!(steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_dot_star_catastrophic_variant() {
        // (.*a){3} on "aaa...no-final" is known catastrophic
        // Simplified version: (.*a) repeated
        let steps = collect_debug_steps("(.*a)(.*a)(.*a)b", "aaaa");
        let backtrack_count = steps.iter().filter(|s| s.is_backtrack).count();
        assert!(backtrack_count > 5,
            "Expected backtracking, got {} backtracks in {} steps",
            backtrack_count, steps.len());
    }

    #[test]
    fn test_empty_pattern() {
        let steps = collect_debug_steps("", "hello");
        assert_eq!(steps.len(), 1);
        assert!(steps[0].matched);
    }

    #[test]
    fn test_anchored_pattern() {
        let steps = collect_debug_steps("^abc$", "abc");
        assert!(steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_anchored_pattern_fail() {
        let steps = collect_debug_steps("^abc$", "xabc");
        assert!(!steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_three_branch_alternation() {
        // (a|b|c) should match each of a, b, c
        let steps_a = collect_debug_steps("(a|b|c)", "a");
        assert!(steps_a.iter().any(|s| s.description.contains("Match found")));

        let steps_b = collect_debug_steps("(a|b|c)", "b");
        assert!(steps_b.iter().any(|s| s.description.contains("Match found")));

        let steps_c = collect_debug_steps("(a|b|c)", "c");
        assert!(steps_c.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_alternation_with_quantifier_on_group() {
        // (ab|cd)+ against "abab" should match
        let steps = collect_debug_steps("(ab|cd)+", "abab");
        assert!(steps.iter().any(|s| s.description.contains("Match found")));

        // (ab|cd)+ against "cdab" should match
        let steps2 = collect_debug_steps("(ab|cd)+", "cdab");
        assert!(steps2.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_catastrophic_nested_quantifiers() {
        // (a*)*b is a classic evil regex - causes excessive steps
        let steps = collect_debug_steps("(a*)*b", "aaaa!");
        // Should hit the step limit or produce many steps due to the nested loop
        assert!(steps.len() > 100,
            "Nested quantifiers should cause excessive steps, got {} steps",
            steps.len());
        assert!(!steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_catastrophic_shows_in_heatmap() {
        // (a+)+b against "aaaaaa!" should show high heat at group positions
        let steps = collect_debug_steps("(a+)+b", "aaaaaa!");
        let session = DebugSession::new(steps);
        // In catastrophic backtracking, pattern positions are visited many times
        assert!(session.max_heat() > 5,
            "Heatmap should show high repetition, max_heat = {}", session.max_heat());
        // Total steps should be significantly more than the text length
        assert!(session.total_steps() > 30,
            "Should have many steps due to backtracking, got {}", session.total_steps());
    }

    #[test]
    fn test_alternation_within_quantified_group_catastrophic() {
        // (a|a)+b - exponential because both branches consume same char
        let steps = collect_debug_steps("(a|a)+b", "aaaaaaa!");
        let backtrack_count = steps.iter().filter(|s| s.is_backtrack).count();
        assert!(backtrack_count > 20,
            "Expected heavy backtracking from (a|a)+, got {} backtracks in {} steps",
            backtrack_count, steps.len());
    }

    #[test]
    fn test_group_match_succeeds() {
        // (foo)(bar) against "foobar" should match
        let steps = collect_debug_steps("(foo)(bar)", "foobar");
        assert!(steps.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_optional_group() {
        // (a)?b should match both "ab" and "b"
        let steps_ab = collect_debug_steps("(a)?b", "ab");
        assert!(steps_ab.iter().any(|s| s.description.contains("Match found")));

        let steps_b = collect_debug_steps("(a)?b", "b");
        assert!(steps_b.iter().any(|s| s.description.contains("Match found")));
    }

    #[test]
    fn test_deeply_nested_groups() {
        // ((a+)b)+ against "abaab" should match
        let steps = collect_debug_steps("((a+)b)+", "abaab");
        assert!(steps.iter().any(|s| s.description.contains("Match found")));
    }
}
