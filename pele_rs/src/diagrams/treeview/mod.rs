pub(crate) mod literal;
mod render;
use crate::PeleError;
use crate::diagrams::timeline::story;
use crate::diagrams::tokens::{self, Pattern, Reader, TokenType};
use crate::errors::{PeleErrorCode, locate};
use crate::preprocess::Config;
use crate::util::{is_space16, trim, trim_end};
pub use render::render;
#[derive(Clone, Debug, Default)]
pub struct Node {
    pub id: usize,
    pub level: isize,
    pub name: String,
    pub directory: bool,
    pub icon: Option<String>,
    pub css_class: Option<String>,
    pub description: Option<String>,
    pub children: Vec<usize>,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub nodes: Vec<Node>,
    pub count: usize,
}
fn code(s: &[u16], p: usize) -> u16 {
    s.get(p).copied().unwrap_or(u16::MAX)
}
fn blank(c: u16) -> bool {
    matches!(c, 32 | 9)
}
fn starts(s: &[u16], p: usize, w: &str) -> bool {
    w.encode_utf16().enumerate().all(|(i, c)| code(s, p + i) == c)
}
fn annotation(s: &[u16], p: usize) -> bool {
    if starts(s, p, "##") || starts(s, p, "icon(") {
        return true;
    }
    if !starts(s, p, ":::") {
        return false;
    }
    let mut i = p + 3;
    while blank(code(s, i)) {
        i += 1
    }
    let c = code(s, i);
    c == 95 || (65..=90).contains(&c) || (97..=122).contains(&c)
}
fn bare(s: &[u16], p: usize) -> Option<usize> {
    let c = code(s, p);
    if blank(c) || matches!(c, 10 | 13 | 34 | 39) || starts(s, p, ":::") || starts(s, p, "icon(") || starts(s, p, "##") {
        return None;
    }
    let mut i = p + 1;
    while i < s.len() {
        let c = s[i];
        if c == 10 || c == 13 {
            break;
        }
        if blank(c) {
            let mut j = i + 1;
            while blank(code(s, j)) {
                j += 1
            }
            if annotation(s, j) {
                break;
            }
            i = j
        } else {
            i += 1
        }
    }
    Some(i)
}
fn head(s: &[u16], p: usize) -> Option<usize> {
    let end = p + 13;
    if !starts(s, p, "treeView-beta") || bare(s, p).is_some_and(|v| v > end) {
        return None;
    }
    if end == s.len() || is_space16(code(s, end) as u32) || starts(s, end, "%%") {
        Some(end)
    } else {
        None
    }
}
macro_rules! matcher {
    ($name:ident,$pattern:expr) => {
        fn $name(src: &[u16], p: usize) -> Option<usize> {
            static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
            let text = String::from_utf16_lossy(&src[p..]);
            RE.get_or_init(|| regex::Regex::new($pattern).unwrap())
                .find(&text)
                .map(|m| p + m.as_str().encode_utf16().count())
        }
    };
}
matcher!(class, r"^[ \t]+:::[ \t]*[A-Za-z_][A-Za-z0-9_-]*");
matcher!(icon, r"^[ \t]+icon\([A-Za-z0-9_-]*(?::[A-Za-z0-9_-]+)?\)");
matcher!(descr, r"^[ \t]+##[^\n\r]*");
matcher!(quoted, r#"^(?:"[^"]*"|'[^']*')"#);
matcher!(comment, r"^%%[^\n]*");
fn title(s: &[u16], p: usize) -> Option<usize> {
    let mut q = p;
    while blank(code(s, q)) {
        q += 1
    }
    if !starts(s, q, "title") {
        return None;
    }
    if blank(code(s, q + 5)) {
        return tokens::title(s, p);
    }
    if q + 5 == s.len() || matches!(code(s, q + 5), 10 | 13) || starts(s, q + 5, "%%") {
        Some(q + 5)
    } else {
        None
    }
}
fn token(name: &'static str, m: tokens::Matcher, hidden: bool) -> TokenType {
    TokenType {
        name,
        pattern: Pattern::Match(m),
        hidden,
        first: None,
    }
}
fn parse_inner(source: &str, front_title: Option<&str>) -> Result<Model, PeleError> {
    let types = vec![
        token("treeView-beta", head, false),
        tokens::acc_descr_type(),
        tokens::acc_title_type(),
        token("TITLE", title, false),
        token("CLASS_ANNOTATION", class, false),
        token("ICON_ANNOTATION", icon, false),
        token("DESC_ANNOTATION", descr, false),
        token("INDENTATION", tokens::whitespace, false),
        token("QUOTED_NAME", quoted, false),
        token("WS", tokens::whitespace, true),
        token("ML_COMMENT", comment, true),
        token("NL", tokens::newline, true),
        token("BARE_NAME", bare, false),
    ];
    let src: Vec<u16> = source.encode_utf16().collect();
    let mut r = Reader {
        i: 0,
        tokens: tokens::tokenize(&src, &types, "treeView")?,
        types: &types,
        diagram: "treeView",
    };
    let mut model = Model {
        title: front_title.map(str::to_string),
        nodes: vec![Node {
            id: 0,
            level: -1,
            name: "/".into(),
            directory: true,
            ..Default::default()
        }],
        ..Default::default()
    };
    r.expect(0)?;
    loop {
        match r.kind() {
            1 => model.acc_descr = Some(story::squeeze(&tokens::acc_descr_value(&r.take()))),
            2 => model.acc_title = Some(story::acc(&tokens::acc_title_value(&r.take()))),
            3 => {
                let title = tokens::title_value(&r.take());
                if !title.is_empty() {
                    model.title = Some(title)
                }
            }
            _ => break,
        }
    }
    let mut stack = vec![0];
    while r.kind() != -1 {
        let level = if r.kind() == 7 {
            r.take().encode_utf16().count() as isize
        } else {
            0
        };
        let mut name = match r.kind() {
            8 => {
                let s = r.take();
                s[1..s.len() - 1].to_string()
            }
            12 => r.take().trim_end_matches([' ', '\t']).to_string(),
            _ => return Err(r.fail("a name")),
        };
        let mut class = None;
        let mut icon = None;
        let mut description = None;
        loop {
            match r.kind() {
                4 => {
                    let s = r.take();
                    let v = trim(trim(&s).strip_prefix(":::").unwrap_or(""));
                    class = (!v.is_empty()).then(|| v.to_string())
                }
                5 => {
                    let s = r.take();
                    let s = trim(&s);
                    let name = &s[5..s.len() - 1];
                    icon = Some(if name.is_empty() { "none" } else { name }.into())
                }
                6 => {
                    let s = r.take();
                    let v = trim(trim(&s).strip_prefix("##").unwrap_or(""));
                    description = (!v.is_empty()).then(|| v.to_string())
                }
                _ => break,
            }
        }
        let directory = name.ends_with('/');
        if directory {
            name.pop();
        }
        while level <= model.nodes[*stack.last().unwrap()].level {
            stack.pop();
        }
        let id = model.nodes.len();
        let parent = *stack.last().unwrap();
        model.nodes[parent].children.push(id);
        model.nodes.push(Node {
            id,
            level,
            name,
            directory,
            icon,
            css_class: class,
            description,
            children: Vec::new(),
        });
        stack.push(id)
    }
    model.count = model.nodes.len();
    Ok(model)
}
fn box_char(c: char) -> bool {
    "─━│┃└┗├┣".contains(c)
}
fn branch(c: char) -> bool {
    "└┗├┣".contains(c)
}
type SourceMap = Vec<(usize, Option<Vec<usize>>)>;
fn preprocess(input: &str) -> Result<(String, SourceMap), PeleError> {
    let lines: Vec<&str> = input.split('\n').collect();
    if !input.chars().any(box_char) {
        return Ok((input.into(), Vec::new()));
    }
    let Some(keyword) = lines.iter().position(|line| trim(line) == "treeView-beta") else {
        return Ok((input.into(), Vec::new()));
    };
    let meta = regex::Regex::new(r"^\s*(?:title[\t ]|accTitle[\t ]*:|accDescr[\t ]*[:{])").unwrap();
    let passes = |line: &str| trim(line).is_empty() || trim(line).starts_with("%%") || meta.is_match(line);
    let decoration = |line: &str| !line.is_empty() && line.chars().all(|c| crate::util::is_js_space(c) || c == '│' || c == '┃');
    let content: Vec<String> = lines[keyword + 1..]
        .iter()
        .filter(|l| !passes(l) && !decoration(l))
        .map(|l| l.replace('\t', "    "))
        .collect();
    if !content.iter().any(|s| s.chars().any(box_char)) {
        return Ok((input.into(), Vec::new()));
    }
    let segment = content
        .iter()
        .find_map(|s| s.chars().position(branch).filter(|p| *p > 0))
        .unwrap_or(4);
    let mut out: Vec<String> = Vec::new();
    let mut mapping = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i <= keyword || passes(line) {
            out.push((*line).into());
            mapping.push((i + 1, None));
            continue;
        }
        if decoration(line) {
            continue;
        }
        let normalized = line.replace('\t', "    ");
        let chars: Vec<char> = normalized.chars().collect();
        if let Some(at) = chars.iter().position(|c| branch(*c)) {
            let depth = crate::util::js_round(at as f64 / segment as f64) as usize + 1;
            let mut pos = at + 1;
            while pos < chars.len() && "─━".contains(chars[pos]) {
                pos += 1
            }
            while chars.get(pos) == Some(&' ') {
                pos += 1
            }
            let text: String = chars[pos..].iter().collect();
            let text = trim_end(&text);
            if text.is_empty() {
                let mut error = PeleError::new(
                    format!(
                        "Line {}: Empty node — expected a filename or directory name after the box-drawing prefix",
                        written_line(input, i)
                    ),
                    PeleErrorCode::Syntax,
                )
                .of("treeView");
                error.line = written_line(input, i);
                return Err(error);
            }
            let indent = "    ".repeat(depth);
            let converted = indent + text;
            let mut expanded = Vec::new();
            let mut original = 0;
            for c in line.chars() {
                for _ in 0..if c == '\t' { 4 } else { c.len_utf16() } {
                    expanded.push(original)
                }
                original += c.len_utf16()
            }
            expanded.push(original);
            let mut columns = Vec::new();
            for j in 0..=converted.encode_utf16().count() {
                columns.push(expanded.get(pos + j.saturating_sub(depth * 4)).copied().unwrap_or(original));
            }
            out.push(converted);
            mapping.push((i + 1, Some(columns)));
        } else if chars.iter().all(|c| crate::util::is_js_space(*c) || box_char(*c)) {
            continue;
        } else if chars.iter().any(|c| box_char(*c)) {
            out.push((*line).into());
            mapping.push((i + 1, None));
        } else if chars.first().is_some_and(|c| crate::util::is_js_space(*c)) {
            let mut error=PeleError::new(format!("Line {}: Unexpected indentation without box-drawing characters. In box-drawing format, use ├── or └── prefixes for indented nodes.",written_line(input,i)),PeleErrorCode::Syntax).of("treeView");
            error.line = written_line(input, i);
            return Err(error);
        } else {
            out.push((*line).into());
            mapping.push((i + 1, None));
        }
    }
    Ok((out.join("\n"), mapping))
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let (text, map) = preprocess(source)?;
    match parse_inner(&text, title) {
        Ok(model) => Ok(model),
        Err(mut e) => {
            if text != source
                && let Some((line, columns)) = e.line.checked_sub(1).and_then(|i| map.get(i))
            {
                let at = if e.column > 0 {
                    columns.as_ref().and_then(|v| v.get(e.column - 1)).copied().unwrap_or(e.column - 1)
                } else {
                    0
                };
                let offset = source
                    .split('\n')
                    .take(line - 1)
                    .map(|s| s.encode_utf16().count() + 1)
                    .sum::<usize>()
                    + at;
                let loc = locate(&source.encode_utf16().collect::<Vec<_>>(), offset);
                let head = e.message.find('\n').unwrap_or(e.message.len());
                let first = e.message[..head].replace(&format!("line {}", e.line), &format!("line {}", loc.line));
                let rest = if e.snippet.is_empty() {
                    e.message[head..].to_string()
                } else {
                    e.message[head..].replacen(&e.snippet, &loc.snippet, 1)
                };
                e.message = first + &rest;
                e.line = loc.line;
                if e.column > 0 {
                    e.column = loc.column
                }
                if !e.snippet.is_empty() {
                    e.snippet = loc.snippet
                }
            }
            Err(e)
        }
    }
}

fn written_line(source: &str, index: usize) -> usize {
    let offset = source.split('\n').take(index).map(|s| s.encode_utf16().count() + 1).sum();
    locate(&source.encode_utf16().collect::<Vec<_>>(), offset).line
}
