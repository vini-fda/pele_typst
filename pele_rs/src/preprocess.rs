// Front matter, directives and comments, after pele's preprocess.ts.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::{Map, Value};

use crate::errors::{PeleError, PeleErrorCode, Result};
use crate::util::{is_js_space, js_num, trim, trim_start};

pub type Config = Map<String, Value>;

pub struct Preprocessed {
    pub text: String,
    pub title: Option<String>,
    pub config: Config,
    pub with_comments: String,
    pub line_offset: usize,
}

const UNSAFE_KEYS: [&str; 3] = ["__proto__", "constructor", "prototype"];

// Removes `%%` comment lines the way /^\s*%%(?!{)[^\n]+\n?/gm does.
fn remove_comments(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(text.len());
    let mut p = 0;
    let mut kept = 0;
    // Positions where ^ can match: the start, and after each \n.
    while p <= n {
        let mut j = p;
        while j < n && is_js_space(chars[j]) {
            j += 1;
        }
        let matched = j + 2 < n && chars[j] == '%' && chars[j + 1] == '%' && chars[j + 2] != '{' && chars[j + 2] != '\n';
        if matched {
            let mut end = j + 2;
            while end < n && chars[end] != '\n' {
                end += 1;
            }
            if end < n {
                end += 1;
            }
            out.extend(&chars[kept..p]);
            kept = end;
            p = end;
            if p >= n {
                break;
            }
            // The next match may start right here, at a line start.
            if p > 0 && chars[p - 1] == '\n' {
                continue;
            }
        }
        // Move to the next line start.
        let mut q = p;
        while q < n && chars[q] != '\n' {
            q += 1;
        }
        if q >= n {
            break;
        }
        p = q + 1;
    }
    out.extend(&chars[kept.min(n)..]);
    out
}

pub fn cleanup_comments(text: &str) -> String {
    let text = if text.contains("%%") {
        remove_comments(text)
    } else {
        text.to_string()
    };
    trim_start(&text).to_string()
}

pub fn normalize_line_breaks(text: &str) -> String {
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.to_string()
    }
}

struct FrontMatter {
    indent: String,
    body: String,
    length: usize,
}

fn is_line_break(c: Option<&char>) -> bool {
    matches!(c, Some('\n') | Some('\r'))
}

fn is_space_at(chars: &[char], i: usize) -> bool {
    chars.get(i).is_some_and(|&c| is_js_space(c))
}

fn starts_with_at(chars: &[char], at: usize, s: &[char]) -> bool {
    at + s.len() <= chars.len() && &chars[at..at + s.len()] == s
}

fn find_from(chars: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.len() > chars.len() {
        return None;
    }
    (from..=chars.len() - needle.len()).find(|&i| &chars[i..i + needle.len()] == needle)
}

fn rfind_upto(chars: &[char], needle: &[char], upto: usize) -> Option<usize> {
    if needle.len() > chars.len() {
        return None;
    }
    let max = (chars.len() - needle.len()).min(upto);
    (0..=max).rev().find(|&i| &chars[i..i + needle.len()] == needle)
}

// Finds a `---` fenced YAML block at the start of the text the way Mermaid's front matter regex does.
fn match_front_matter(text: &str) -> Option<FrontMatter> {
    let chars: Vec<char> = text.chars().collect();
    let mut cursor = 0;
    while cursor < chars.len() && is_space_at(&chars, cursor) && !is_line_break(chars.get(cursor)) {
        cursor += 1;
    }
    let indent: String = chars[..cursor].iter().collect();
    let dashes: Vec<char> = "---".chars().collect();
    if !starts_with_at(&chars, cursor, &dashes) {
        return None;
    }
    let mut opening_ends: Vec<usize> = Vec::new();
    let mut i = cursor + 3;
    while i < chars.len() && is_space_at(&chars, i) {
        if is_line_break(chars.get(i)) {
            opening_ends.push(i);
        }
        i += 1;
    }
    if opening_ends.is_empty() {
        return None;
    }
    let fence: Vec<char> = format!("{}---", indent).chars().collect();
    let closing_end = |i: usize| -> Option<usize> {
        if !is_line_break(chars.get(i)) || !starts_with_at(&chars, i + 1, &fence) {
            return None;
        }
        let mut last = None;
        let mut j = i + 1 + fence.len();
        while j < chars.len() && is_space_at(&chars, j) {
            if is_line_break(chars.get(j)) {
                last = Some(j);
            }
            j += 1;
        }
        last
    };
    let mut needle = vec!['\n'];
    needle.extend(&fence);
    let mut last_fence: Option<usize> = None;
    let mut at = rfind_upto(&chars, &needle, chars.len());
    while let Some(i) = at {
        if i <= opening_ends[0] {
            break;
        }
        if closing_end(i).is_some() {
            last_fence = Some(i);
            break;
        }
        if i == 0 {
            break;
        }
        at = rfind_upto(&chars, &needle, i - 1);
    }
    let last_fence = last_fence?;
    let mut opening_end = opening_ends[0];
    for &c in &opening_ends {
        if c < last_fence {
            opening_end = c;
        }
    }
    let body_start = opening_end + 1;
    let first_at = if body_start > 0 { body_start - 1 } else { 0 };
    let mut at = find_from(&chars, &needle, first_at);
    while let Some(i) = at {
        if i > last_fence {
            break;
        }
        if i >= body_start
            && let Some(end) = closing_end(i)
        {
            return Some(FrontMatter {
                indent,
                body: chars[body_start..i].iter().collect(),
                length: end + 1,
            });
        }
        at = find_from(&chars, &needle, i + 1);
    }
    None
}

fn merge(target: &mut Config, source: &Config) {
    for (key, value) in source {
        if UNSAFE_KEYS.contains(&key.as_str()) {
            continue;
        }
        match (value, target.get_mut(key)) {
            (Value::Object(v), Some(Value::Object(current))) => merge(current, v),
            (Value::Object(v), _) => {
                let mut fresh = Map::new();
                merge(&mut fresh, v);
                target.insert(key.clone(), Value::Object(fresh));
            }
            _ => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

fn yaml_to_json(y: &yaml_rust2::Yaml) -> Value {
    use yaml_rust2::Yaml;
    match y {
        Yaml::Real(s) => s
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Yaml::Integer(i) => Value::Number((*i).into()),
        Yaml::String(s) => Value::String(s.clone()),
        Yaml::Boolean(b) => Value::Bool(*b),
        Yaml::Array(a) => Value::Array(a.iter().map(yaml_to_json).collect()),
        Yaml::Hash(h) => {
            let mut m = Map::new();
            for (k, v) in h {
                let key = match k {
                    Yaml::String(s) => s.clone(),
                    Yaml::Integer(i) => i.to_string(),
                    Yaml::Real(s) => s.clone(),
                    Yaml::Boolean(b) => b.to_string(),
                    Yaml::Null => "null".to_string(),
                    _ => continue,
                };
                m.insert(key, yaml_to_json(v));
            }
            Value::Object(m)
        }
        _ => Value::Null,
    }
}

pub fn parse_yaml(src: &str) -> std::result::Result<Value, String> {
    let docs = yaml_rust2::YamlLoader::load_from_str(src).map_err(|e| e.to_string())?;
    Ok(docs.first().map(yaml_to_json).unwrap_or(Value::Null))
}

// The text JavaScript's String() gives a scalar.
pub fn value_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => js_num(n.as_f64().unwrap_or(f64::NAN)),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(a) => a.iter().map(value_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

struct Directive {
    start: usize,
    end: usize,
    kind: String,
    body: Option<String>,
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

// Matches /%{2}{\s*(?:(\w+)\s*:|(\w+))\s*(?:(\w+)|((?:(?!}%{2}).|\r?\n)*))?\s*(?:}%{2})?/gi by hand.
fn find_directives(text: &str) -> Vec<Directive> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = Vec::new();
    let mut i = 0;
    let byte_at = |ci: usize| -> usize { chars[..ci].iter().map(|c| c.len_utf8()).sum() };
    while i + 2 < n {
        if !(chars[i] == '%' && chars[i + 1] == '%' && chars[i + 2] == '{') {
            i += 1;
            continue;
        }
        let start = i;
        let mut j = i + 3;
        while j < n && is_js_space(chars[j]) {
            j += 1;
        }
        let w0 = j;
        while j < n && is_word(chars[j]) {
            j += 1;
        }
        if j == w0 {
            i += 1;
            continue;
        }
        let kind: String = chars[w0..j].iter().collect();
        let mut k = j;
        while k < n && is_js_space(chars[k]) {
            k += 1;
        }
        if k < n && chars[k] == ':' {
            j = k + 1;
        }
        while j < n && is_js_space(chars[j]) {
            j += 1;
        }
        let mut body = None;
        if j < n && is_word(chars[j]) {
            while j < n && is_word(chars[j]) {
                j += 1;
            }
        } else {
            let b0 = j;
            loop {
                if j >= n {
                    break;
                }
                if chars[j] == '}' && j + 2 < n && chars.get(j + 1) == Some(&'%') && chars.get(j + 2) == Some(&'%') {
                    break;
                }
                let c = chars[j];
                if c == '\n' {
                    j += 1;
                    continue;
                }
                if c == '\r' && chars.get(j + 1) == Some(&'\n') {
                    j += 2;
                    continue;
                }
                if matches!(c, '\r' | '\u{2028}' | '\u{2029}') {
                    break;
                }
                j += 1;
            }
            body = Some(chars[b0..j].iter().collect::<String>());
        }
        // The trailing \s* backtracks into a body that ended in whitespace.
        while j < n && is_js_space(chars[j]) {
            j += 1;
        }
        if j + 2 < n + 1 && chars.get(j) == Some(&'}') && chars.get(j + 1) == Some(&'%') && chars.get(j + 2) == Some(&'%') {
            j += 3;
        }
        out.push(Directive {
            start: byte_at(start),
            end: byte_at(j),
            kind,
            body,
        });
        i = j.max(i + 1);
    }
    out
}

fn directives(text: &str, config: &mut Config) -> String {
    if !text.contains("%%{") {
        return text.to_string();
    }
    let normalized = trim(text).replace('\'', "\"");
    for d in find_directives(&normalized) {
        let kind = d.kind.to_string();
        if kind == "wrap" {
            config.insert("wrap".into(), Value::Bool(true));
        } else if (kind == "init" || kind == "initialize")
            && let Some(body) = d.body.filter(|b| !b.is_empty())
            && let Ok(Value::Object(args)) = serde_json::from_str::<Value>(trim(&body))
        {
            merge(config, &args);
        }
    }
    let mut out = String::new();
    let mut last = 0;
    for d in find_directives(text) {
        out.push_str(&text[last..d.start]);
        last = d.end;
    }
    out.push_str(&text[last..]);
    out
}

fn re_html_tag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"<([0-9A-Za-z_]+)([^>]*)>").unwrap())
}

fn re_attr() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"="([^"]*)""#).unwrap())
}

pub fn preprocess(source: &str) -> Result<Preprocessed> {
    let mut text = normalize_line_breaks(source);
    // A tag needs a closing bracket, so nothing after the last one can match.
    let last_close = if text.contains('<') { text.rfind('>') } else { None };
    if let Some(lc) = last_close.filter(|&lc| lc > 0) {
        let head = re_html_tag()
            .replace_all(&text[..=lc], |c: &regex::Captures| {
                format!("<{}{}>", &c[1], re_attr().replace_all(&c[2], "='$1'"))
            })
            .into_owned();
        text = head + &text[lc + 1..];
    }

    let mut config = Config::new();
    let mut title = None;
    let mut line_offset = 0;

    if text.contains("---")
        && let Some(front) = match_front_matter(&text)
    {
        let body = if !front.indent.is_empty() {
            front
                .body
                .split('\n')
                .map(|line| line.strip_prefix(front.indent.as_str()).unwrap_or(line))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            front.body.clone()
        };
        let parsed =
            parse_yaml(&body).map_err(|e| PeleError::new(format!("Front matter is not valid YAML. {}", e), PeleErrorCode::Syntax))?;
        if let Value::Object(parsed) = &parsed {
            if truthy(parsed.get("title")) {
                title = Some(value_string(&parsed["title"]));
            }
            if let Some(Value::Object(c)) = parsed.get("config") {
                merge(&mut config, c);
            }
            if truthy(parsed.get("displayMode")) {
                let mode = value_string(&parsed["displayMode"]);
                if !matches!(config.get("gantt"), Some(Value::Object(_))) {
                    config.insert("gantt".into(), Value::Object(Map::new()));
                }
                if let Some(Value::Object(g)) = config.get_mut("gantt") {
                    g.insert("displayMode".into(), Value::String(mode));
                }
            }
        }
        let chars: Vec<char> = text.chars().collect();
        line_offset = chars[..front.length.min(chars.len())].iter().filter(|&&c| c == '\n').count();
        text = chars[front.length.min(chars.len())..].iter().collect();
    }

    let text = directives(&text, &mut config);
    Ok(Preprocessed {
        text: cleanup_comments(&text),
        title,
        config,
        with_comments: text,
        line_offset,
    })
}

// Does what Mermaid's `text.replace(/keyword.*:\S*#.*;/g, drop the last character)` does.
fn drop_color_semicolons(text: &str, keyword: &str) -> String {
    let Some(mut from) = text.find(keyword) else {
        return text.to_string();
    };
    let mut out = String::new();
    let mut last = 0;
    loop {
        let rest = &text[from..];
        let line_end = rest
            .find(['\n', '\r', '\u{2028}', '\u{2029}'])
            .map(|i| from + i)
            .unwrap_or(text.len());
        let mut after_colon = false;
        let mut hash: Option<usize> = None;
        for (off, c) in text[from + keyword.len()..line_end].char_indices() {
            let j = from + keyword.len() + off;
            if c == ':' {
                after_colon = true;
            } else if c == '#' {
                if after_colon {
                    hash = Some(j);
                    break;
                }
            } else if is_js_space(c) {
                after_colon = false;
            }
        }
        if let Some(hash) = hash
            && let Some(semi) = text[..line_end].rfind(';')
            && semi > hash
        {
            out.push_str(&text[last..semi]);
            last = semi + 1;
        }
        if line_end >= text.len() {
            break;
        }
        let next_start = line_end + text[line_end..].chars().next().map_or(1, |c| c.len_utf8());
        match text[next_start..].find(keyword) {
            Some(i) => from = next_start + i,
            None => break,
        }
    }
    if last == 0 {
        return text.to_string();
    }
    out.push_str(&text[last..]);
    out
}

// Mermaid hides `#name;` entity codes behind placeholder characters so `#` and `;` survive parsing.
pub fn encode_entities(text: &str) -> String {
    if !text.contains('#') {
        return text.to_string();
    }
    let text = drop_color_semicolons(&drop_color_semicolons(text, "style"), "classDef");
    static R: OnceLock<Regex> = OnceLock::new();
    let re = R.get_or_init(|| Regex::new(r"#([0-9A-Za-z_]+);").unwrap());
    re.replace_all(&text, |c: &regex::Captures| {
        let inner = &c[1];
        let numeric = inner.chars().all(|ch| ch.is_ascii_digit());
        format!("{}{}¶ß", if numeric { "ﬂ°°" } else { "ﬂ°" }, inner)
    })
    .into_owned()
}
