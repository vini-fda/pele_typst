//! Pele's bounded JSON-schema YAML reader, including its diagnostics and scalar folding.
use crate::preprocess::{normalize_line_breaks, value_string};
use crate::util::{is_js_space, trim, trim_start};
use regex::Regex;
use serde_json::{Map, Value};
use std::sync::OnceLock;

#[derive(Debug)]
enum Failure {
    Incomplete(&'static str),
    Message(&'static str),
}
type Result<T> = std::result::Result<T, Failure>;
const MAX_DEPTH: usize = 64;
fn message(s: &'static str) -> Failure {
    Failure::Message(s)
}
fn blank(c: Option<char>) -> bool {
    matches!(c, Some(' ' | '\t' | '\n' | '\r'))
}
fn string(s: &[char]) -> String {
    s.iter().collect()
}
fn skip(s: &[char], mut p: usize, commas: bool) -> usize {
    while p < s.len() {
        if blank(Some(s[p])) || (commas && s[p] == ',') {
            p += 1;
        } else if s[p] == '#' && (p == 0 || is_js_space(s[p - 1])) {
            while p < s.len() && s[p] != '\n' {
                p += 1;
            }
        } else {
            break;
        }
    }
    p
}
fn scalar(s: &str) -> Value {
    let s = trim(s);
    match s {
        "" | "~" | "null" | "Null" | "NULL" => return Value::Null,
        "true" | "True" | "TRUE" => return Value::Bool(true),
        "false" | "False" | "FALSE" => return Value::Bool(false),
        _ => {}
    }
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    let res = RES.get_or_init(|| {
        [
            r"^[-+]?(?:0|[1-9][0-9]*)$",
            r"^[-+]?(?:[0-9][0-9]*(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][-+]?[0-9]+)?$",
            r"^[-+]?0x[0-9a-fA-F]+$",
            r"^[-+]?0o[0-7]+$",
            r"^[-+]?0b[01]+$",
            r"^[-+]?\.(?:inf|Inf|INF)$",
            r"^\.(?:nan|NaN|NAN)$",
        ]
        .into_iter()
        .map(|r| Regex::new(r).unwrap())
        .collect()
    });
    if res[0].is_match(s) || res[1].is_match(s) {
        return number(s.parse().unwrap_or(f64::NAN));
    }
    for (i, radix) in [(2, 16u32), (3, 8), (4, 2)] {
        if res[i].is_match(s) {
            let raw = s.trim_start_matches(['-', '+']);
            let n = raw[2..]
                .chars()
                .fold(0.0, |n, c| n * radix as f64 + c.to_digit(radix).unwrap() as f64)
                * (if s.starts_with('-') { -1.0 } else { 1.0 });
            return number(n);
        }
    }
    if res[5].is_match(s) {
        return number(if s.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY });
    }
    if res[6].is_match(s) {
        return number(f64::NAN);
    }
    Value::String(s.into())
}

fn number(n: f64) -> Value {
    // YAML's numeric domain includes IEEE infinities and NaN. Keep these as
    // numbers in the shared value tree instead of silently turning them into
    // null; consumers requiring finite JSON numbers still get None from as_f64.
    Value::Number(serde_json::Number::from_f64(n).unwrap_or_else(|| serde_json::Number::from_string_unchecked(crate::util::js_num(n))))
}
fn fold(text: &str) -> String {
    if !text.contains('\n') {
        return text.into();
    }
    let lines: Vec<_> = text.split('\n').collect();
    let last = lines.len() - 1;
    let trimmed: Vec<_> = lines
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let s = if i > 0 { s.trim_start_matches([' ', '\t']) } else { s };
            if i < last { s.trim_end_matches([' ', '\t']) } else { s }
        })
        .collect();
    let joined = trimmed.join("\n");
    let mut out = String::new();
    let mut chars = joined.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\n' {
            out.push(c);
            continue;
        }
        let mut count = 1;
        while chars.peek() == Some(&'\n') {
            chars.next();
            count += 1;
        }
        if count == 1 {
            out.push(' ');
        } else {
            out.push_str(&"\n".repeat(count - 1));
        }
    }
    out
}
fn double(body: &str) -> Result<String> {
    if !body.contains('\\') {
        return Ok(fold(body));
    }
    let chars: Vec<_> = body.chars().collect();
    let mut plain = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && chars.get(i + 1) == Some(&'\n') {
            i += 2;
            while matches!(chars.get(i), Some(' ' | '\t')) {
                i += 1;
            }
        } else {
            plain.push(chars[i]);
            i += 1;
        }
    }
    let chars: Vec<_> = fold(&plain).chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(&e) = chars.get(i) else {
            return Err(message("YAML: unknown escape sequence"));
        };
        i += 1;
        if matches!(e, 'x' | 'u' | 'U') {
            let n = match e {
                'x' => 2,
                'u' => 4,
                _ => 8,
            };
            let raw = string(&chars[i..(i + n).min(chars.len())]);
            let prefix: String = raw.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            let code = u32::from_str_radix(&prefix, 16)
                .ok()
                .and_then(char::from_u32)
                .ok_or_else(|| message("Invalid code point NaN"))?;
            out.push(code);
            i += n;
        } else {
            out.push(match e {
                '0' => '\0',
                'a' => '\x07',
                'b' => '\x08',
                't' => '\t',
                'n' => '\n',
                'v' => '\x0b',
                'f' => '\x0c',
                'r' => '\r',
                'e' => '\x1b',
                ' ' | '"' | '/' | '\\' => e,
                'N' => '\u{85}',
                '_' => '\u{a0}',
                'L' => '\u{2028}',
                'P' => '\u{2029}',
                _ => return Err(message("YAML: unknown escape sequence")),
            });
        }
    }
    Ok(out)
}
fn flow(s: &[char], p: usize, stops: &str, depth: usize) -> Result<(Value, usize)> {
    if depth > MAX_DEPTH {
        return Err(message("YAML: nesting is too deep"));
    }
    let mut p = skip(s, p, false);
    let ch = s.get(p).copied();
    if matches!(ch, Some('{' | '[')) {
        let object = ch == Some('{');
        let close = if object { '}' } else { ']' };
        let mut map = Map::new();
        let mut list = Vec::new();
        p += 1;
        let mut separated = true;
        loop {
            p = skip(s, p, false);
            if p >= s.len() {
                return Err(Failure::Incomplete("YAML: unexpected end of the stream within a flow collection"));
            }
            if s[p] == close {
                return Ok((if object { Value::Object(map) } else { Value::Array(list) }, p + 1));
            }
            if !separated {
                return Err(message("YAML: missed comma between flow collection entries"));
            }
            if s[p] == ',' {
                return Err(message("YAML: expected the node content, but found ','"));
            }
            if object {
                let (key, end) = flow(s, p, ":,}", depth + 1)?;
                p = skip(s, end, false);
                let mut value = Value::Null;
                if s.get(p) == Some(&':') {
                    (value, p) = flow(s, p + 1, ",}", depth + 1)?;
                }
                map.insert(value_string(&key), value);
            } else {
                let (value, end) = flow(s, p, ",]", depth + 1)?;
                p = end;
                list.push(value);
            }
            p = skip(s, p, false);
            separated = s.get(p) == Some(&',');
            if separated {
                p += 1;
            }
        }
    }
    if ch == Some('"') {
        let mut end = p + 1;
        while end < s.len() && s[end] != '"' {
            end += if s[end] == '\\' { 2 } else { 1 };
        }
        if end >= s.len() {
            return Err(Failure::Incomplete(
                "YAML: unexpected end of the stream within a double quoted scalar",
            ));
        }
        return Ok((Value::String(double(&string(&s[p + 1..end]))?), end + 1));
    }
    if ch == Some('\'') {
        let mut end = p + 1;
        while end < s.len() {
            if s[end] == '\'' {
                if s.get(end + 1) != Some(&'\'') {
                    break;
                }
                end += 1;
            }
            end += 1;
        }
        if end >= s.len() {
            return Err(Failure::Incomplete(
                "YAML: unexpected end of the stream within a single quoted scalar",
            ));
        }
        return Ok((Value::String(fold(&string(&s[p + 1..end])).replace("''", "'")), end + 1));
    }
    let mut end = p;
    while end < s.len() {
        let c = s[end];
        if stops.contains(c) && (c != ':' || end + 1 >= s.len() || is_js_space(s[end + 1]) || ",[]{}".contains(s[end + 1])) {
            break;
        }
        if c == '#' && end > p && is_js_space(s[end - 1]) {
            let next = (end..s.len()).find(|&i| s[i] == '\n').unwrap_or(s.len());
            return Ok((scalar(&fold(trim(&string(&s[p..end])))), next));
        }
        end += 1;
    }
    Ok((scalar(&fold(trim(&string(&s[p..end])))), end))
}
fn split_key(line: &str) -> Option<(String, usize)> {
    let s: Vec<_> = line.chars().collect();
    let first = *s.first()?;
    let end;
    let mut colon;
    if first == '"' || first == '\'' {
        let mut i = 1;
        loop {
            let c = *s.get(i)?;
            if first == '"' && c == '\\' {
                i += 2;
            } else if c == first {
                if first == '"' || s.get(i + 1) != Some(&'\'') {
                    break;
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        end = i + 1;
        colon = end;
        while blank(s.get(colon).copied()) {
            colon += 1;
        }
        if s.get(colon) != Some(&':') {
            return None;
        }
    } else {
        if is_js_space(first) || "#: ,[]{}&*!|>'\"%@`".contains(first) {
            return None;
        }
        if first == '-' && (s.len() < 2 || blank(s.get(1).copied()) || matches!(s.get(1), Some(':' | '#'))) {
            return None;
        }
        colon = 1;
        while colon < s.len() && s[colon] != ':' && s[colon] != '#' {
            colon += 1;
        }
        if s.get(colon) != Some(&':') {
            return None;
        }
        let mut e = colon;
        while e > 0 && blank(s.get(e - 1).copied()) {
            e -= 1;
        }
        end = e;
    }
    let mut length = colon + 1;
    if length < s.len() && !blank(s.get(length).copied()) {
        return None;
    }
    while blank(s.get(length).copied()) {
        length += 1;
    }
    Some((string(&s[..end]), s[..length].iter().map(|c| c.len_utf8()).sum()))
}
fn strip_comment(s: &str) -> String {
    let chars: Vec<_> = s.chars().collect();
    let mut quote = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            if c == '\\' && q == '"' {
                i += 1;
            } else if c == q {
                quote = None;
            }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
        } else if c == '#' && (i == 0 || matches!(chars[i - 1], ' ' | '\t')) {
            return string(&chars[..i]);
        }
        i += 1;
    }
    s.into()
}
struct Reader {
    lines: Vec<String>,
    i: usize,
}
impl Reader {
    fn new(src: &str) -> Self {
        Self {
            lines: normalize_line_breaks(src).split('\n').map(String::from).collect(),
            i: 0,
        }
    }
    fn skip_blank(&mut self) {
        while self.i < self.lines.len() {
            let t = trim(&self.lines[self.i]);
            if !t.is_empty() && !t.starts_with('#') && t != "---" {
                break;
            }
            self.i += 1;
        }
    }
    fn indent(&self) -> Result<usize> {
        let line = &self.lines[self.i];
        let k = line.bytes().take_while(|&c| c == b' ').count();
        if line.as_bytes().get(k) == Some(&b'\t') {
            return Err(message("YAML: tabs are not allowed for indentation"));
        }
        Ok(k)
    }
    fn block_scalar(&mut self, header: &str, parent: isize) -> String {
        let folded = header.starts_with('>');
        let chomp = if header.contains('-') {
            '-'
        } else if header.contains('+') {
            '+'
        } else {
            ' '
        };
        let mut indent = header
            .chars()
            .find(|c| ('1'..='9').contains(c))
            .map(|c| parent + c.to_digit(10).unwrap() as isize)
            .unwrap_or(-1);
        let mut out = Vec::<String>::new();
        while self.i < self.lines.len() {
            let line = &self.lines[self.i];
            if trim(line).is_empty() {
                out.push(String::new());
                self.i += 1;
                continue;
            }
            let k = line.encode_utf16().count() - trim_start(line).encode_utf16().count();
            if indent == -1 {
                if k as isize <= parent {
                    break;
                }
                indent = k as isize;
            }
            if (k as isize) < indent {
                break;
            }
            let byte = line.char_indices().nth(indent as usize).map_or(line.len(), |(i, _)| i);
            out.push(line[byte..].into());
            self.i += 1;
        }
        let mut trailing = 0;
        while out.last().is_some_and(String::is_empty) {
            out.pop();
            trailing += 1;
        }
        let mut text = String::new();
        if folded {
            for (i, cur) in out.iter().enumerate() {
                if i > 0 {
                    let prev = &out[i - 1];
                    if cur.is_empty() {
                        text.push('\n');
                    } else if !prev.is_empty() {
                        text.push(if prev.starts_with([' ', '\t']) || cur.starts_with([' ', '\t']) {
                            '\n'
                        } else {
                            ' '
                        });
                    }
                }
                text += cur;
            }
        } else {
            text = out.join("\n");
        }
        if out.is_empty() {
            return if chomp == '+' { "\n".repeat(trailing) } else { String::new() };
        }
        if chomp == '-' {
            return text;
        }
        text + &"\n".repeat(if chomp == '+' { trailing + 1 } else { 1 })
    }
    fn inline(&mut self, rest: &str, indent: isize) -> Result<Value> {
        let first = rest.chars().next();
        if matches!(first, Some('|' | '>')) {
            static RE: OnceLock<Regex> = OnceLock::new();
            if RE.get_or_init(|| Regex::new(r"^[|>][-+0-9]*\s*(?:#.*)?$").unwrap()).is_match(rest) {
                return Ok(Value::String(self.block_scalar(rest, indent)));
            }
        }
        if matches!(first, Some('{' | '[' | '"' | '\'')) {
            let start = self.i;
            let mut text = rest.to_string();
            let mut taken = 0;
            let mut batch = 1;
            loop {
                let chars: Vec<_> = text.chars().collect();
                match flow(&chars, 0, "", 0) {
                    Ok((value, end)) => {
                        let used = chars[..end].iter().filter(|&&c| c == '\n').count();
                        let line_end = (end..chars.len()).find(|&i| chars[i] == '\n').unwrap_or(chars.len());
                        if !trim(&strip_comment(&string(&chars[end..line_end]))).is_empty() {
                            return Err(message("YAML: unexpected content after a value"));
                        }
                        self.i = start + used;
                        return Ok(value);
                    }
                    Err(Failure::Incomplete(msg)) => {
                        if start + taken >= self.lines.len() {
                            return Err(message(msg));
                        }
                        let more = &self.lines[start + taken..(start + taken + batch).min(self.lines.len())];
                        text.push('\n');
                        text += &more.join("\n");
                        taken += more.len();
                        batch = batch.saturating_mul(2);
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        let mut text = trim(&strip_comment(rest)).to_string();
        while self.i < self.lines.len() {
            let line = &self.lines[self.i];
            if trim(line).is_empty() {
                let mut j = self.i;
                while j < self.lines.len() && trim(&self.lines[j]).is_empty() {
                    j += 1;
                }
                if j >= self.lines.len()
                    || ((self.lines[j].encode_utf16().count() - trim_start(&self.lines[j]).encode_utf16().count()) as isize) <= indent
                {
                    break;
                }
                text += &"\n".repeat(j - self.i);
                self.i = j;
                continue;
            }
            if ((line.encode_utf16().count() - trim_start(line).encode_utf16().count()) as isize) <= indent {
                break;
            }
            let more = strip_comment(line);
            if !text.ends_with('\n') {
                text.push(' ');
            }
            text += trim(&more);
            self.i += 1;
        }
        Ok(scalar(&text))
    }
    fn block(&mut self, min_indent: isize, depth: usize) -> Result<Value> {
        if depth > MAX_DEPTH {
            return Err(message("YAML: nesting is too deep"));
        }
        self.skip_blank();
        if self.i >= self.lines.len() {
            return Ok(Value::Null);
        }
        let indent = self.indent()?;
        if (indent as isize) < min_indent {
            return Ok(Value::Null);
        }
        let line = self.lines[self.i][indent..].to_string();
        if line == "-" || line.starts_with("- ") {
            let mut out = Vec::new();
            while self.i < self.lines.len() {
                self.skip_blank();
                if self.i >= self.lines.len() || self.indent()? != indent {
                    break;
                }
                let cur = self.lines[self.i][indent..].to_string();
                if cur != "-" && !cur.starts_with("- ") {
                    break;
                }
                let rest = &cur[1..];
                let pad = rest.encode_utf16().count() - trim_start(rest).encode_utf16().count() + 1;
                let stripped = strip_comment(rest);
                let body = trim(&stripped);
                if body.is_empty() {
                    self.i += 1;
                    out.push(self.block(indent as isize + 1, depth + 1)?);
                } else if split_key(body).is_some() {
                    self.lines[self.i] = " ".repeat(indent + pad) + trim_start(rest);
                    out.push(self.block((indent + pad) as isize, depth + 1)?);
                } else {
                    self.i += 1;
                    out.push(self.inline(trim(rest), indent as isize)?);
                }
            }
            return Ok(Value::Array(out));
        }
        if split_key(&line).is_none() {
            self.i += 1;
            return self.inline(trim(&line), indent as isize - 1);
        }
        let mut out = Map::new();
        while self.i < self.lines.len() {
            self.skip_blank();
            if self.i >= self.lines.len() {
                break;
            }
            let k = self.indent()?;
            if k < indent {
                break;
            }
            if k > indent {
                return Err(message("YAML: bad indentation of a mapping entry"));
            }
            let cur = self.lines[self.i][indent..].to_string();
            let (key, length) = split_key(&cur).ok_or_else(|| message("YAML: can not read a block mapping entry"))?;
            let key = value_string(&flow(&key.chars().collect::<Vec<_>>(), 0, "", 0)?.0);
            let rest = &cur[length..];
            self.i += 1;
            let value = if trim(&strip_comment(rest)).is_empty() {
                self.skip_blank();
                if self.i < self.lines.len() {
                    let next = self.indent()?;
                    let next_line = &self.lines[self.i][next..];
                    let seq = next_line == "-" || next_line.starts_with("- ");
                    if next > indent || (seq && next == indent) {
                        self.block(if seq { indent } else { indent + 1 } as isize, depth + 1)?
                    } else {
                        Value::Null
                    }
                } else {
                    Value::Null
                }
            } else {
                self.inline(trim(rest), indent as isize)?
            };
            out.insert(key, value);
        }
        Ok(Value::Object(out))
    }
}
pub fn parse(src: &str) -> std::result::Result<Value, String> {
    let run = || -> Result<Value> {
        let trimmed = trim(src);
        if trimmed.starts_with(['{', '[']) {
            let chars: Vec<_> = trimmed.chars().collect();
            let (value, end) = flow(&chars, 0, "", 0)?;
            if skip(&chars, end, false) == chars.len() {
                return Ok(value);
            }
        }
        let mut r = Reader::new(src);
        let value = r.block(0, 0)?;
        r.skip_blank();
        if r.i < r.lines.len() && trim(&r.lines[r.i]) != "..." {
            return Err(message("YAML: end of the stream or a document separator is expected"));
        }
        Ok(value)
    };
    run().map_err(|e| match e {
        Failure::Message(s) | Failure::Incomplete(s) => s.to_string(),
    })
}
