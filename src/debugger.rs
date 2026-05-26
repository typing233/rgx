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

pub fn collect_debug_steps(pattern: &str, text: &str) -> Vec<DebugStep> {
    let mut steps = Vec::new();
    let pat_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    if pat_chars.is_empty() {
        steps.push(DebugStep {
            pattern_offset: 0,
            text_offset: 0,
            description: "Empty pattern".to_string(),
            is_backtrack: false,
            matched: true,
        });
        return steps;
    }

    for start in 0..=text_chars.len() {
        let found = simulate_match(&pat_chars, &text_chars, start, &mut steps);
        if found {
            break;
        }
        if start == text_chars.len() && !found {
            break;
        }
        if steps.len() > 5000 {
            steps.push(DebugStep {
                pattern_offset: 0,
                text_offset: start,
                description: "Step limit reached (catastrophic backtracking?)".to_string(),
                is_backtrack: false,
                matched: false,
            });
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
struct State {
    pi: usize,
    ti: usize,
}

fn simulate_match(
    pat: &[char],
    text: &[char],
    start_ti: usize,
    steps: &mut Vec<DebugStep>,
) -> bool {
    let mut stack: Vec<State> = Vec::new();
    let mut pi = 0;
    let mut ti = start_ti;
    let pat_len = pat.len();
    let text_len = text.len();
    let step_base = steps.len();

    if start_ti > 0 || step_base > 0 {
        steps.push(DebugStep {
            pattern_offset: 0,
            text_offset: start_ti,
            description: format!("Try matching from text position {}", start_ti),
            is_backtrack: false,
            matched: false,
        });
    }

    loop {
        if steps.len() - step_base > 2000 {
            steps.push(DebugStep {
                pattern_offset: pi.min(pat_len),
                text_offset: ti.min(text_len),
                description: "Step limit for this start position".to_string(),
                is_backtrack: false,
                matched: false,
            });
            return false;
        }

        if pi >= pat_len {
            steps.push(DebugStep {
                pattern_offset: pi,
                text_offset: ti,
                description: format!("Match found at [{}, {})", start_ti, ti),
                is_backtrack: false,
                matched: true,
            });
            return true;
        }

        // Check for escape sequences
        if pat[pi] == '\\' && pi + 1 < pat_len {
            let esc = pat[pi + 1];
            let (ok, desc) = match esc {
                'd' => (ti < text_len && text[ti].is_ascii_digit(), "\\d digit"),
                'D' => (ti < text_len && !text[ti].is_ascii_digit(), "\\D non-digit"),
                'w' => (ti < text_len && (text[ti].is_alphanumeric() || text[ti] == '_'), "\\w word"),
                'W' => (ti < text_len && !(text[ti].is_alphanumeric() || text[ti] == '_'), "\\W non-word"),
                's' => (ti < text_len && text[ti].is_whitespace(), "\\s space"),
                'S' => (ti < text_len && !text[ti].is_whitespace(), "\\S non-space"),
                'n' => (ti < text_len && text[ti] == '\n', "\\n newline"),
                't' => (ti < text_len && text[ti] == '\t', "\\t tab"),
                _ => (ti < text_len && text[ti] == esc, &*format!("\\{}", esc).leak()),
            };

            let quantifier = get_quantifier(pat, pi + 2, pat_len);
            if let Some((min, max, quant_end, lazy)) = quantifier {
                let result = handle_quantified_atom(pat, text, pi, ti, pi + 2, quant_end, min, max, lazy,
                    &mut stack, steps, |t, pos| {
                        let c = t[pos];
                        match esc {
                            'd' => c.is_ascii_digit(),
                            'D' => !c.is_ascii_digit(),
                            'w' => c.is_alphanumeric() || c == '_',
                            'W' => !(c.is_alphanumeric() || c == '_'),
                            's' => c.is_whitespace(),
                            'S' => !c.is_whitespace(),
                            'n' => c == '\n',
                            't' => c == '\t',
                            _ => c == esc,
                        }
                    }, desc);
                match result {
                    QuantResult::Continue(new_pi, new_ti) => { pi = new_pi; ti = new_ti; continue; }
                    QuantResult::Backtrack => {
                        if let Some(state) = stack.pop() {
                            steps.push(DebugStep {
                                pattern_offset: state.pi,
                                text_offset: state.ti,
                                description: format!("Backtrack to p[{}] t[{}]", state.pi, state.ti),
                                is_backtrack: true,
                                matched: false,
                            });
                            pi = state.pi;
                            ti = state.ti;
                            continue;
                        }
                        return false;
                    }
                }
            }

            steps.push(DebugStep {
                pattern_offset: pi,
                text_offset: ti,
                description: format!("{} {}", desc, if ok { "matched" } else { "failed" }),
                is_backtrack: false,
                matched: ok,
            });
            if ok {
                pi += 2;
                ti += 1;
                continue;
            }
        } else if pat[pi] == '.' {
            let quantifier = get_quantifier(pat, pi + 1, pat_len);
            if let Some((min, max, quant_end, lazy)) = quantifier {
                let result = handle_quantified_atom(pat, text, pi, ti, pi + 1, quant_end, min, max, lazy,
                    &mut stack, steps, |t, pos| t[pos] != '\n', ". (any)");
                match result {
                    QuantResult::Continue(new_pi, new_ti) => { pi = new_pi; ti = new_ti; continue; }
                    QuantResult::Backtrack => {
                        if let Some(state) = stack.pop() {
                            steps.push(DebugStep {
                                pattern_offset: state.pi,
                                text_offset: state.ti,
                                description: format!("Backtrack to p[{}] t[{}]", state.pi, state.ti),
                                is_backtrack: true,
                                matched: false,
                            });
                            pi = state.pi;
                            ti = state.ti;
                            continue;
                        }
                        return false;
                    }
                }
            }

            let ok = ti < text_len && text[ti] != '\n';
            steps.push(DebugStep {
                pattern_offset: pi,
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
                pi += 1;
                ti += 1;
                continue;
            }
        } else if pat[pi] == '^' {
            let ok = ti == 0;
            steps.push(DebugStep {
                pattern_offset: pi,
                text_offset: ti,
                description: format!("^ {}", if ok { "matched" } else { "failed" }),
                is_backtrack: false,
                matched: ok,
            });
            if ok { pi += 1; continue; }
        } else if pat[pi] == '$' {
            let ok = ti == text_len;
            steps.push(DebugStep {
                pattern_offset: pi,
                text_offset: ti,
                description: format!("$ {}", if ok { "matched" } else { "failed" }),
                is_backtrack: false,
                matched: ok,
            });
            if ok { pi += 1; continue; }
        } else if pat[pi] == '[' {
            let (class_end, negated) = parse_char_class(pat, pi);
            let ok = if ti < text_len {
                let in_class = char_in_class(pat, pi, class_end, text[ti]);
                if negated { !in_class } else { in_class }
            } else {
                false
            };

            let quantifier = get_quantifier(pat, class_end, pat_len);
            if let Some((min, max, quant_end, lazy)) = quantifier {
                let class_pi = pi;
                let class_e = class_end;
                let neg = negated;
                let result = handle_quantified_atom(pat, text, pi, ti, class_end, quant_end, min, max, lazy,
                    &mut stack, steps, move |t, pos| {
                        let in_c = char_in_class(pat, class_pi, class_e, t[pos]);
                        if neg { !in_c } else { in_c }
                    }, "[...] class");
                match result {
                    QuantResult::Continue(new_pi, new_ti) => { pi = new_pi; ti = new_ti; continue; }
                    QuantResult::Backtrack => {
                        if let Some(state) = stack.pop() {
                            steps.push(DebugStep {
                                pattern_offset: state.pi,
                                text_offset: state.ti,
                                description: format!("Backtrack to p[{}] t[{}]", state.pi, state.ti),
                                is_backtrack: true,
                                matched: false,
                            });
                            pi = state.pi;
                            ti = state.ti;
                            continue;
                        }
                        return false;
                    }
                }
            }

            steps.push(DebugStep {
                pattern_offset: pi,
                text_offset: ti,
                description: format!("[...] {}", if ok { "matched" } else { "failed" }),
                is_backtrack: false,
                matched: ok,
            });
            if ok {
                pi = class_end;
                ti += 1;
                continue;
            }
        } else {
            // Literal
            let ch = pat[pi];
            let quantifier = get_quantifier(pat, pi + 1, pat_len);
            if let Some((min, max, quant_end, lazy)) = quantifier {
                let result = handle_quantified_atom(pat, text, pi, ti, pi + 1, quant_end, min, max, lazy,
                    &mut stack, steps, move |t, pos| t[pos] == ch, &format!("'{}'", ch));
                match result {
                    QuantResult::Continue(new_pi, new_ti) => { pi = new_pi; ti = new_ti; continue; }
                    QuantResult::Backtrack => {
                        if let Some(state) = stack.pop() {
                            steps.push(DebugStep {
                                pattern_offset: state.pi,
                                text_offset: state.ti,
                                description: format!("Backtrack to p[{}] t[{}]", state.pi, state.ti),
                                is_backtrack: true,
                                matched: false,
                            });
                            pi = state.pi;
                            ti = state.ti;
                            continue;
                        }
                        return false;
                    }
                }
            }

            let ok = ti < text_len && text[ti] == ch;
            steps.push(DebugStep {
                pattern_offset: pi,
                text_offset: ti,
                description: if ok {
                    format!("'{}' matched", ch)
                } else if ti < text_len {
                    format!("'{}' != '{}'", ch, text[ti])
                } else {
                    format!("'{}' at end", ch)
                },
                is_backtrack: false,
                matched: ok,
            });
            if ok {
                pi += 1;
                ti += 1;
                continue;
            }
        }

        // Failed - try backtrack
        if let Some(state) = stack.pop() {
            steps.push(DebugStep {
                pattern_offset: state.pi,
                text_offset: state.ti,
                description: format!("Backtrack to p[{}] t[{}]", state.pi, state.ti),
                is_backtrack: true,
                matched: false,
            });
            pi = state.pi;
            ti = state.ti;
        } else {
            return false;
        }
    }
}

enum QuantResult {
    Continue(usize, usize),
    Backtrack,
}

fn handle_quantified_atom<F>(
    _pat: &[char],
    text: &[char],
    atom_pi: usize,
    ti: usize,
    _atom_end: usize,
    quant_end: usize,
    min: usize,
    max: usize,
    lazy: bool,
    stack: &mut Vec<State>,
    steps: &mut Vec<DebugStep>,
    matcher: F,
    desc: &str,
) -> QuantResult
where
    F: Fn(&[char], usize) -> bool,
{
    let text_len = text.len();
    let mut count = 0;
    let mut pos = ti;

    if !lazy {
        // Greedy
        while count < max && pos < text_len && matcher(text, pos) {
            pos += 1;
            count += 1;
        }
        if count < min {
            steps.push(DebugStep {
                pattern_offset: atom_pi,
                text_offset: ti,
                description: format!("{} quantifier needs {}, got {}", desc, min, count),
                is_backtrack: false,
                matched: false,
            });
            return QuantResult::Backtrack;
        }
        // Push backtrack states from min to count-1 (we try max first)
        for bt_count in min..count {
            stack.push(State { pi: quant_end, ti: ti + bt_count });
        }
        steps.push(DebugStep {
            pattern_offset: atom_pi,
            text_offset: ti,
            description: format!("{} matched {} times (greedy)", desc, count),
            is_backtrack: false,
            matched: true,
        });
        QuantResult::Continue(quant_end, ti + count)
    } else {
        // Lazy
        while count < min {
            if pos >= text_len || !matcher(text, pos) {
                steps.push(DebugStep {
                    pattern_offset: atom_pi,
                    text_offset: ti,
                    description: format!("{} lazy needs min {}", desc, min),
                    is_backtrack: false,
                    matched: false,
                });
                return QuantResult::Backtrack;
            }
            pos += 1;
            count += 1;
        }
        // Push expansion states
        let mut expand_pos = pos;
        let mut expand_count = count;
        while expand_count < max && expand_pos < text_len && matcher(text, expand_pos) {
            expand_pos += 1;
            expand_count += 1;
            stack.push(State { pi: quant_end, ti: expand_pos });
        }
        steps.push(DebugStep {
            pattern_offset: atom_pi,
            text_offset: ti,
            description: format!("{} matched {} times (lazy)", desc, count),
            is_backtrack: false,
            matched: true,
        });
        QuantResult::Continue(quant_end, pos)
    }
}

fn get_quantifier(pat: &[char], pos: usize, pat_len: usize) -> Option<(usize, usize, usize, bool)> {
    if pos >= pat_len {
        return None;
    }
    let (min, max, end) = match pat[pos] {
        '*' => (0, usize::MAX, pos + 1),
        '+' => (1, usize::MAX, pos + 1),
        '?' => (0, 1, pos + 1),
        '{' => {
            if let Some(result) = parse_range_quantifier(pat, pos) {
                result
            } else {
                return None;
            }
        }
        _ => return None,
    };
    let lazy = end < pat_len && pat[end] == '?';
    let final_end = if lazy { end + 1 } else { end };
    Some((min, max, final_end, lazy))
}

fn parse_range_quantifier(pat: &[char], pos: usize) -> Option<(usize, usize, usize)> {
    let mut i = pos + 1;
    let pat_len = pat.len();
    let mut num_str = String::new();

    while i < pat_len && pat[i].is_ascii_digit() {
        num_str.push(pat[i]);
        i += 1;
    }
    if num_str.is_empty() || i >= pat_len {
        return None;
    }

    let min: usize = num_str.parse().ok()?;

    if pat[i] == '}' {
        return Some((min, min, i + 1));
    }
    if pat[i] != ',' {
        return None;
    }
    i += 1;

    let mut max_str = String::new();
    while i < pat_len && pat[i].is_ascii_digit() {
        max_str.push(pat[i]);
        i += 1;
    }
    if i >= pat_len || pat[i] != '}' {
        return None;
    }

    let max = if max_str.is_empty() {
        usize::MAX
    } else {
        max_str.parse().ok()?
    };

    Some((min, max, i + 1))
}

fn parse_char_class(pat: &[char], start: usize) -> (usize, bool) {
    let mut i = start + 1;
    let negated = i < pat.len() && pat[i] == '^';
    if negated { i += 1; }
    if i < pat.len() && pat[i] == ']' { i += 1; }
    while i < pat.len() && pat[i] != ']' {
        if pat[i] == '\\' && i + 1 < pat.len() { i += 1; }
        i += 1;
    }
    if i < pat.len() { i += 1; }
    (i, negated)
}

fn char_in_class(pat: &[char], start: usize, end: usize, ch: char) -> bool {
    let mut i = start + 1;
    if i < end && pat[i] == '^' { i += 1; }

    while i < end - 1 {
        if pat[i] == '\\' && i + 1 < end - 1 {
            let esc = pat[i + 1];
            let matched = match esc {
                'd' => ch.is_ascii_digit(),
                'D' => !ch.is_ascii_digit(),
                'w' => ch.is_alphanumeric() || ch == '_',
                'W' => !(ch.is_alphanumeric() || ch == '_'),
                's' => ch.is_whitespace(),
                'S' => !ch.is_whitespace(),
                _ => ch == esc,
            };
            if matched { return true; }
            i += 2;
        } else if i + 2 < end - 1 && pat[i + 1] == '-' {
            if ch >= pat[i] && ch <= pat[i + 2] {
                return true;
            }
            i += 3;
        } else {
            if ch == pat[i] { return true; }
            i += 1;
        }
    }
    false
}
