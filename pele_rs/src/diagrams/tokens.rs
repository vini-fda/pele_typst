// Tokenizer for the diagram types Mermaid defines with Langium grammars, after pele's
// diagrams/common/tokens.ts. Token types are tried in order and the first match wins. Text is
// read as UTF-16 code units, as the JavaScript does. The sticky regular expressions of the
// TypeScript are written out as matchers that return where their match ends.

use crate::errors::{PeleError, syntax_error};
use crate::util::{is_space16, trim};

pub type Matcher = fn(&[u16], usize) -> Option<usize>;

pub enum Pattern {
    // A literal string.
    Literal(&'static str),
    // A keyword, which Mermaid ends only at whitespace, a comment, or the end of input.
    Keyword(&'static str),
    Match(Matcher),
}

pub struct TokenType {
    pub name: &'static str,
    pub pattern: Pattern,
    pub hidden: bool,
    // Every ASCII character a match can start with, when that is known.
    pub first: Option<&'static str>,
}

pub struct Tokens {
    pub kinds: Vec<i32>,
    pub starts: Vec<usize>,
    pub ends: Vec<usize>,
    pub src: Vec<u16>,
}

fn at(src: &[u16], i: usize) -> u32 {
    src.get(i).map_or(u32::MAX, |&c| c as u32)
}

fn starts_with(src: &[u16], p: usize, word: &str) -> bool {
    (p..).zip(word.encode_utf16()).all(|(i, c)| src.get(i) == Some(&c))
}

fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

fn blanks(src: &[u16], mut i: usize) -> usize {
    while matches!(at(src, i), 9 | 32) {
        i += 1;
    }
    i
}

fn line_end(src: &[u16], mut i: usize) -> usize {
    while i < src.len() && !matches!(src[i], 10 | 13) {
        i += 1;
    }
    i
}

// The end of `(?=%%)` or `(?!\S)` at `i`: a comment, whitespace, or the end of input follows.
fn keyword_end(src: &[u16], i: usize) -> bool {
    starts_with(src, i, "%%") || i >= src.len() || is_space16(src[i] as u32)
}

fn try_match(t: &TokenType, src: &[u16], p: usize) -> Option<usize> {
    match t.pattern {
        Pattern::Literal(s) => starts_with(src, p, s).then(|| p + len16(s)),
        Pattern::Keyword(w) => (starts_with(src, p, w) && keyword_end(src, p + len16(w))).then(|| p + len16(w)),
        Pattern::Match(f) => f(src, p),
    }
}

fn may_start(t: &TokenType, c: u32) -> bool {
    if c >= 128 {
        return true;
    }
    let ch = c as u8 as char;
    match t.pattern {
        Pattern::Literal(s) | Pattern::Keyword(s) => s.starts_with(ch),
        Pattern::Match(_) => t.first.is_none_or(|f| f.contains(ch)),
    }
}

pub fn tokenize(src: &[u16], types: &[TokenType], diagram: &str) -> Result<Tokens, PeleError> {
    let mut kinds = Vec::new();
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    let n = src.len();
    let mut p = 0;
    'scan: while p < n {
        let c = src[p] as u32;
        for (k, t) in types.iter().enumerate() {
            if !may_start(t, c) {
                continue;
            }
            let Some(end) = try_match(t, src, p) else { continue };
            if end <= p {
                continue;
            }
            if !t.hidden {
                kinds.push(k as i32);
                starts.push(p);
                ends.push(end);
            }
            p = end;
            continue 'scan;
        }
        return Err(syntax_error(diagram, src, p, "", true));
    }
    kinds.push(-1);
    starts.push(n);
    ends.push(n);
    Ok(Tokens {
        kinds,
        starts,
        ends,
        src: src.to_vec(),
    })
}

// /[\t ]*accDescr(?:[\t ]*:([^\n\r]*?(?=%%)|[^\n\r]*)|\s*{([^}]*)})/y
pub fn acc_descr(src: &[u16], p: usize) -> Option<usize> {
    let j = blanks(src, p);
    if !starts_with(src, j, "accDescr") {
        return None;
    }
    let k = j + 8;
    let colon = blanks(src, k);
    if at(src, colon) == 58 {
        return Some(up_to_comment(src, colon + 1));
    }
    let mut b = k;
    while b < src.len() && is_space16(src[b] as u32) {
        b += 1;
    }
    if at(src, b) != 123 {
        return None;
    }
    let close = (b + 1..src.len()).find(|&i| src[i] == 125)?;
    Some(close + 1)
}

// `[^\n\r]*?(?=%%)|[^\n\r]*`: to the first comment on the line, or else to its end.
fn up_to_comment(src: &[u16], from: usize) -> usize {
    let end = line_end(src, from);
    (from..end).find(|&i| starts_with(src, i, "%%")).unwrap_or(end)
}

// /[\t ]*accTitle[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)/y
pub fn acc_title(src: &[u16], p: usize) -> Option<usize> {
    let j = blanks(src, p);
    if !starts_with(src, j, "accTitle") {
        return None;
    }
    let colon = blanks(src, j + 8);
    if at(src, colon) != 58 {
        return None;
    }
    Some(up_to_comment(src, colon + 1))
}

// /[\t ]*title(?:[\t ][^\n\r]*?(?=%%)|[\t ][^\n\r]*|)/y
pub fn title(src: &[u16], p: usize) -> Option<usize> {
    let j = blanks(src, p);
    if !starts_with(src, j, "title") {
        return None;
    }
    let k = j + 5;
    if matches!(at(src, k), 9 | 32) {
        Some(up_to_comment(src, k + 1))
    } else {
        Some(k)
    }
}

// /"([^"\\]|\\.)*"|'([^'\\]|\\.)*'/y
pub fn string(src: &[u16], p: usize) -> Option<usize> {
    let quote = at(src, p);
    if quote != 34 && quote != 39 {
        return None;
    }
    let mut i = p + 1;
    while i < src.len() {
        let c = src[i] as u32;
        if c == quote {
            return Some(i + 1);
        }
        if c == 92 {
            // `.` does not match a line terminator.
            if i + 1 >= src.len() || matches!(src[i + 1], 10 | 13 | 0x2028 | 0x2029) {
                return None;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    None
}

// /\r?\n/y
pub fn newline(src: &[u16], p: usize) -> Option<usize> {
    match (at(src, p), at(src, p + 1)) {
        (10, _) => Some(p + 1),
        (13, 10) => Some(p + 2),
        _ => None,
    }
}

// /[\t ]+/y
pub fn whitespace(src: &[u16], p: usize) -> Option<usize> {
    let j = blanks(src, p);
    (j > p).then_some(j)
}

// `(?:\r?\n|(?!\S))` at `i`: the end of the match, or None.
fn line_or_space(src: &[u16], i: usize) -> Option<usize> {
    if let Some(e) = newline(src, i) {
        return Some(e);
    }
    (i >= src.len() || is_space16(src[i] as u32)).then_some(i)
}

// /---[\t ]*\r?\n(?:[\S\s]*?\r?\n)?---(?:\r?\n|(?!\S))/y
pub fn yaml(src: &[u16], p: usize) -> Option<usize> {
    if !starts_with(src, p, "---") {
        return None;
    }
    let body = newline(src, blanks(src, p + 3))?;
    let mut q = body;
    while q + 3 <= src.len() {
        if (q == body || src[q - 1] == 10)
            && starts_with(src, q, "---")
            && let Some(end) = line_or_space(src, q + 3)
        {
            return Some(end);
        }
        q += 1;
    }
    None
}

// /[\t ]*%%{[\S\s]*?}%%(?:\r?\n|(?!\S))/y
pub fn directive(src: &[u16], p: usize) -> Option<usize> {
    let j = blanks(src, p);
    if !starts_with(src, j, "%%{") {
        return None;
    }
    let mut q = j + 3;
    while q + 3 <= src.len() {
        if starts_with(src, q, "}%%")
            && let Some(end) = line_or_space(src, q + 3)
        {
            return Some(end);
        }
        q += 1;
    }
    None
}

// /[\t ]*%%[^\n\r]*/y
pub fn comment(src: &[u16], p: usize) -> Option<usize> {
    let j = blanks(src, p);
    starts_with(src, j, "%%").then(|| line_end(src, j + 2))
}

pub fn acc_descr_type() -> TokenType {
    TokenType {
        name: "ACC_DESCR",
        pattern: Pattern::Match(acc_descr),
        hidden: false,
        first: Some("\t a"),
    }
}
pub fn acc_title_type() -> TokenType {
    TokenType {
        name: "ACC_TITLE",
        pattern: Pattern::Match(acc_title),
        hidden: false,
        first: Some("\t a"),
    }
}
pub fn title_type() -> TokenType {
    TokenType {
        name: "TITLE",
        pattern: Pattern::Match(title),
        hidden: false,
        first: Some("\t t"),
    }
}
pub fn string_type() -> TokenType {
    TokenType {
        name: "STRING",
        pattern: Pattern::Match(string),
        hidden: false,
        first: Some("\"'"),
    }
}
pub fn newline_type() -> TokenType {
    TokenType {
        name: "NEWLINE",
        pattern: Pattern::Match(newline),
        hidden: false,
        first: Some("\r\n"),
    }
}
pub fn whitespace_type() -> TokenType {
    TokenType {
        name: "WHITESPACE",
        pattern: Pattern::Match(whitespace),
        hidden: true,
        first: Some("\t "),
    }
}
pub fn yaml_type() -> TokenType {
    TokenType {
        name: "YAML",
        pattern: Pattern::Match(yaml),
        hidden: true,
        first: Some("-"),
    }
}
pub fn directive_type() -> TokenType {
    TokenType {
        name: "DIRECTIVE",
        pattern: Pattern::Match(directive),
        hidden: true,
        first: Some("\t %"),
    }
}
pub fn comment_type() -> TokenType {
    TokenType {
        name: "SINGLE_LINE_COMMENT",
        pattern: Pattern::Match(comment),
        hidden: true,
        first: Some("\t %"),
    }
}
pub fn keyword(word: &'static str) -> TokenType {
    TokenType {
        name: word,
        pattern: Pattern::Keyword(word),
        hidden: false,
        first: None,
    }
}

// Keeps single tabs and spaces as written, and turns runs of two or more into one space.
fn collapse_blanks(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\t' || chars[i] == ' ' {
            let mut j = i;
            while j < chars.len() && (chars[j] == '\t' || chars[j] == ' ') {
                j += 1;
            }
            if j - i >= 2 {
                out.push(' ');
            } else {
                out.push(chars[i]);
            }
            i = j;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

// `text.trim().replace(/[\t ]{2,}/gm, ' ')`
fn single_line(text: &str) -> String {
    collapse_blanks(trim(text))
}

fn after(text: &str, word: &str) -> String {
    text.find(word).map(|i| text[i + word.len()..].to_string()).unwrap_or_default()
}

// RE_TITLE = /title([\t ][^\n\r]*|)/
pub fn title_value(text: &str) -> String {
    let rest = after(text, "title");
    let value = if rest.starts_with(['\t', ' ']) {
        rest.split(['\n', '\r']).next().unwrap_or("").to_string()
    } else {
        String::new()
    };
    single_line(&value)
}

// RE_ACC_TITLE = /accTitle[\t ]*:([^\n\r]*)/
pub fn acc_title_value(text: &str) -> String {
    let rest = after(text, "accTitle");
    let rest = rest.trim_start_matches(['\t', ' ']);
    let value = rest.strip_prefix(':').unwrap_or("").split(['\n', '\r']).next().unwrap_or("");
    single_line(value)
}

// RE_ACC_DESCR = /accDescr(?:[\t ]*:([^\n\r]*)|\s*{([^}]*)})/
pub fn acc_descr_value(text: &str) -> String {
    let rest = after(text, "accDescr");
    let blank = rest.trim_start_matches(['\t', ' ']);
    if let Some(v) = blank.strip_prefix(':') {
        return single_line(v.split(['\n', '\r']).next().unwrap_or(""));
    }
    let open = crate::util::trim_start(&rest);
    let body = open.strip_prefix('{').unwrap_or(open);
    let body = body.split('}').next().unwrap_or("");
    // .replace(/^\s*/gm, '').replace(/\s+$/gm, ''): each line trimmed, then blank runs collapsed.
    let lines: Vec<String> = body.split('\n').map(|l| trim(l).to_string()).collect();
    let joined = lines.join("\n");
    let collapsed = collapse_blanks(&joined);
    let mut out = String::new();
    let mut prev_break = false;
    for c in collapsed.chars() {
        let is_break = c == '\n' || c == '\r';
        if is_break && prev_break {
            continue;
        }
        prev_break = is_break;
        out.push(c);
    }
    // Blank lines at either end are gone with the whitespace around them.
    trim(&out).to_string()
}

pub fn string_value(text: &str) -> String {
    let inner: Vec<char> = text.chars().collect();
    let inner = &inner[1..inner.len().saturating_sub(1).max(1)];
    if !inner.contains(&'\\') {
        return inner.iter().collect();
    }
    let mut out = String::new();
    let mut i = 0;
    while i < inner.len() {
        let c = inner[i];
        if c == '\\' {
            i += 1;
            let Some(&next) = inner.get(i) else { break };
            out.push(match next {
                'b' => '\u{8}',
                'f' => '\u{c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'v' => '\u{b}',
                '0' => '\0',
                other => other,
            });
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

// Reads tokens for a hand-written parser, with Chevrotain-style messages on failure.
pub struct Reader<'a> {
    pub i: usize,
    pub tokens: Tokens,
    pub types: &'a [TokenType],
    pub diagram: &'static str,
}

impl Reader<'_> {
    pub fn kind(&self) -> i32 {
        self.tokens.kinds[self.i]
    }

    pub fn text(&self, index: usize) -> String {
        String::from_utf16_lossy(&self.tokens.src[self.tokens.starts[index]..self.tokens.ends[index]])
    }

    pub fn take(&mut self) -> String {
        let t = self.text(self.i);
        self.i += 1;
        t
    }

    pub fn accept(&mut self, kind: i32) -> bool {
        if self.kind() != kind {
            return false;
        }
        self.i += 1;
        true
    }

    pub fn expect(&mut self, kind: i32) -> Result<String, PeleError> {
        if self.kind() != kind {
            return Err(self.fail(&format!("token of type '{}'", self.types[kind as usize].name)));
        }
        Ok(self.take())
    }

    pub fn fail(&self, expected: &str) -> PeleError {
        let found = if self.kind() == -1 {
            "end of input".to_string()
        } else {
            format!("`{}`", self.text(self.i))
        };
        let mut at = self.tokens.starts[self.i];
        if self.kind() == -1 {
            // Locate EOF errors after the last token, before trailing line breaks and hidden comments.
            let mut k = self.i;
            while k > 0 && trim(&self.text(k - 1)).is_empty() {
                k -= 1;
            }
            if k > 0 {
                at = self.tokens.ends[k - 1];
            }
        }
        syntax_error(
            self.diagram,
            &self.tokens.src,
            at,
            &format!("Expecting {} but found {}.", expected, found),
            false,
        )
    }
}
