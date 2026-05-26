use crate::engine::{EngineLevel, RegexFlags};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    Go,
    Java,
    Php,
}

impl Language {
    pub const ALL: [Language; 6] = [
        Language::Rust,
        Language::Python,
        Language::JavaScript,
        Language::Go,
        Language::Java,
        Language::Php,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::JavaScript => "JavaScript",
            Language::Go => "Go",
            Language::Java => "Java",
            Language::Php => "PHP",
        }
    }
}

pub fn generate_code(
    lang: Language,
    pattern: &str,
    flags: &RegexFlags,
    engine: EngineLevel,
) -> String {
    match lang {
        Language::Rust => generate_rust(pattern, flags, engine),
        Language::Python => generate_python(pattern, flags),
        Language::JavaScript => generate_javascript(pattern, flags),
        Language::Go => generate_go(pattern, flags),
        Language::Java => generate_java(pattern, flags),
        Language::Php => generate_php(pattern, flags),
    }
}

fn escape_rust_string(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_python_string(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_js_string(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '/' => out.push_str("\\/"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_go_string(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_java_string(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_php_string(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

fn flags_suffix_js(flags: &RegexFlags) -> String {
    let mut s = String::from("g");
    if flags.case_insensitive { s.push('i'); }
    if flags.multiline { s.push('m'); }
    if flags.dot_matches_newline { s.push('s'); }
    if flags.unicode { s.push('u'); }
    s
}

fn flags_python(flags: &RegexFlags) -> Vec<&'static str> {
    let mut f = Vec::new();
    if flags.case_insensitive { f.push("re.IGNORECASE"); }
    if flags.multiline { f.push("re.MULTILINE"); }
    if flags.dot_matches_newline { f.push("re.DOTALL"); }
    if flags.unicode { f.push("re.UNICODE"); }
    if flags.extended { f.push("re.VERBOSE"); }
    f
}

fn generate_rust(pattern: &str, flags: &RegexFlags, engine: EngineLevel) -> String {
    let escaped = escape_rust_string(pattern);
    match engine {
        EngineLevel::Basic => {
            let mut code = String::new();
            code.push_str("use regex::RegexBuilder;\n\n");
            code.push_str(&format!("let re = RegexBuilder::new(\"{}\")\n", escaped));
            if flags.case_insensitive { code.push_str("    .case_insensitive(true)\n"); }
            if flags.multiline { code.push_str("    .multi_line(true)\n"); }
            if flags.dot_matches_newline { code.push_str("    .dot_matches_new_line(true)\n"); }
            if !flags.unicode { code.push_str("    .unicode(false)\n"); }
            code.push_str("    .build()\n    .unwrap();\n\n");
            code.push_str("for m in re.find_iter(text) {\n");
            code.push_str("    println!(\"Match: {} [{}-{}]\", m.as_str(), m.start(), m.end());\n");
            code.push_str("}\n");
            code
        }
        EngineLevel::Fancy => {
            let mut code = String::new();
            code.push_str("use fancy_regex::Regex;\n\n");
            let prefix = flags.as_prefix();
            let full = if prefix.is_empty() {
                escaped.clone()
            } else {
                format!("{}{}", escape_rust_string(&prefix), escaped)
            };
            code.push_str(&format!("let re = Regex::new(\"{}\").unwrap();\n\n", full));
            code.push_str("let mut start = 0;\n");
            code.push_str("while let Ok(Some(m)) = re.find_from_pos(text, start) {\n");
            code.push_str("    println!(\"Match: {} [{}-{}]\", m.as_str(), m.start(), m.end());\n");
            code.push_str("    start = if m.start() == m.end() { m.end() + 1 } else { m.end() };\n");
            code.push_str("}\n");
            code
        }
        EngineLevel::Pcre2 => {
            let mut code = String::new();
            code.push_str("use pcre2::bytes::RegexBuilder;\n\n");
            code.push_str(&format!("let re = RegexBuilder::new()\n"));
            if flags.case_insensitive { code.push_str("    .caseless(true)\n"); }
            if flags.multiline { code.push_str("    .multi_line(true)\n"); }
            if flags.dot_matches_newline { code.push_str("    .dotall(true)\n"); }
            if flags.extended { code.push_str("    .extended(true)\n"); }
            if flags.unicode { code.push_str("    .utf(true)\n    .ucp(true)\n"); }
            code.push_str(&format!("    .build(\"{}\")\n    .unwrap();\n\n", escaped));
            code.push_str("for m in re.find_iter(text.as_bytes()) {\n");
            code.push_str("    let m = m.unwrap();\n");
            code.push_str("    let matched = std::str::from_utf8(m.as_bytes()).unwrap();\n");
            code.push_str("    println!(\"Match: {} [{}-{}]\", matched, m.start(), m.end());\n");
            code.push_str("}\n");
            code
        }
    }
}

fn generate_python(pattern: &str, flags: &RegexFlags) -> String {
    let escaped = escape_python_string(pattern);
    let py_flags = flags_python(flags);
    let mut code = String::new();
    code.push_str("import re\n\n");
    if py_flags.is_empty() {
        code.push_str(&format!("pattern = re.compile(r'{}')\n\n", escaped));
    } else {
        code.push_str(&format!("pattern = re.compile(r'{}', {})\n\n", escaped, py_flags.join(" | ")));
    }
    code.push_str("for match in pattern.finditer(text):\n");
    code.push_str("    print(f'Match: {match.group()} [{match.start()}-{match.end()}]')\n");
    code
}

fn generate_javascript(pattern: &str, flags: &RegexFlags) -> String {
    let escaped = escape_js_string(pattern);
    let js_flags = flags_suffix_js(flags);
    let mut code = String::new();
    code.push_str(&format!("const re = new RegExp('{}', '{}');\n\n", escaped, js_flags));
    code.push_str("let match;\n");
    code.push_str("while ((match = re.exec(text)) !== null) {\n");
    code.push_str("  console.log(`Match: ${match[0]} [${match.index}-${match.index + match[0].length}]`);\n");
    code.push_str("}\n");
    code
}

fn generate_go(pattern: &str, flags: &RegexFlags) -> String {
    let escaped = escape_go_string(pattern);
    let mut prefix = String::new();
    if flags.case_insensitive || flags.multiline || flags.dot_matches_newline {
        prefix.push_str("(?");
        if flags.case_insensitive { prefix.push('i'); }
        if flags.multiline { prefix.push('m'); }
        if flags.dot_matches_newline { prefix.push('s'); }
        prefix.push(')');
    }
    let full_pattern = format!("{}{}", prefix, escaped);

    let mut code = String::new();
    code.push_str("import (\n    \"fmt\"\n    \"regexp\"\n)\n\n");
    code.push_str(&format!("re := regexp.MustCompile(\"{}\")\n\n", full_pattern));
    code.push_str("matches := re.FindAllStringIndex(text, -1)\n");
    code.push_str("for _, loc := range matches {\n");
    code.push_str("    fmt.Printf(\"Match: %s [%d-%d]\\n\", text[loc[0]:loc[1]], loc[0], loc[1])\n");
    code.push_str("}\n");
    code
}

fn generate_java(pattern: &str, flags: &RegexFlags) -> String {
    let escaped = escape_java_string(pattern);
    let mut flag_parts = Vec::new();
    if flags.case_insensitive { flag_parts.push("Pattern.CASE_INSENSITIVE"); }
    if flags.multiline { flag_parts.push("Pattern.MULTILINE"); }
    if flags.dot_matches_newline { flag_parts.push("Pattern.DOTALL"); }
    if flags.unicode { flag_parts.push("Pattern.UNICODE_CHARACTER_CLASS"); }
    if flags.extended { flag_parts.push("Pattern.COMMENTS"); }

    let mut code = String::new();
    code.push_str("import java.util.regex.*;\n\n");
    if flag_parts.is_empty() {
        code.push_str(&format!("Pattern pattern = Pattern.compile(\"{}\");\n", escaped));
    } else {
        code.push_str(&format!("Pattern pattern = Pattern.compile(\"{}\", {});\n", escaped, flag_parts.join(" | ")));
    }
    code.push_str("Matcher matcher = pattern.matcher(text);\n\n");
    code.push_str("while (matcher.find()) {\n");
    code.push_str("    System.out.printf(\"Match: %s [%d-%d]%n\", matcher.group(), matcher.start(), matcher.end());\n");
    code.push_str("}\n");
    code
}

fn generate_php(pattern: &str, flags: &RegexFlags) -> String {
    let escaped = escape_php_string(pattern);
    let mut modifiers = String::new();
    if flags.case_insensitive { modifiers.push('i'); }
    if flags.multiline { modifiers.push('m'); }
    if flags.dot_matches_newline { modifiers.push('s'); }
    if flags.unicode { modifiers.push('u'); }
    if flags.extended { modifiers.push('x'); }

    let mut code = String::new();
    code.push_str(&format!("$pattern = '/{}/';\n", escaped));
    if !modifiers.is_empty() {
        // Replace the trailing /' with modifiers
        code.clear();
        code.push_str(&format!("$pattern = '/{}/{}';\n", escaped, modifiers));
    }
    code.push_str("\n");
    code.push_str("preg_match_all($pattern, $text, $matches, PREG_OFFSET_CAPTURE);\n\n");
    code.push_str("foreach ($matches[0] as $match) {\n");
    code.push_str("    $value = $match[0];\n");
    code.push_str("    $offset = $match[1];\n");
    code.push_str("    $end = $offset + strlen($value);\n");
    code.push_str("    echo \"Match: $value [$offset-$end]\\n\";\n");
    code.push_str("}\n");
    code
}
