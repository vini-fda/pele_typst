//! Sankey CSV parsing and flow diagrams.
mod layout;
mod render;
use crate::errors::{PeleError, syntax_error};
use crate::preprocess::Config;
use crate::text::entities::decode_entities;
use crate::util::{is_space16, is_word16, parse_float, trim};
pub use render::render;

#[derive(Debug, Clone)]
pub struct Link {
    pub source: usize,
    pub target: usize,
    pub value: f64,
}
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub title: Option<String>,
    pub nodes: Vec<String>,
    pub links: Vec<Link>,
}
const NAMES: &[&str] = &[
    "$end",
    "STUCK",
    "SANKEY",
    "NEWLINE",
    "COMMA",
    "DQUOTE",
    "NON_ESCAPED_TEXT",
    "ESCAPED_TEXT",
    "EOF",
];
fn at(s: &[u16], i: usize) -> u32 {
    s.get(i).map_or(u32::MAX, |&c| c as u32)
}
fn blank(c: u32) -> bool {
    !matches!(c, 10 | 13) && is_space16(c)
}
fn prepare(source: &str) -> (Vec<u16>, Vec<usize>) {
    let src: Vec<u16> = source.encode_utf16().collect();
    let mut end = src.len();
    while end > 0 && blank(src[end - 1] as u32) {
        end -= 1;
    }
    let mut keep = vec![true; end];
    let mut quoted = false;
    let mut i = 0;
    while i < end {
        let c = src[i] as u32;
        if quoted {
            if c == 34 {
                if at(&src, i + 1) == 34 {
                    i += 1;
                } else {
                    quoted = false;
                }
            }
            i += 1;
        } else if c == 34 {
            quoted = true;
            i += 1;
        } else if blank(c) {
            let mut j = i + 1;
            while blank(at(&src, j)) {
                j += 1;
            }
            let p = if i == 0 { u32::MAX } else { src[i - 1] as u32 };
            let q = at(&src, j);
            if i == 0 || matches!(q, 10 | 13) || (matches!(p, 10 | 13) && q != 44) {
                for v in keep.iter_mut().take(j.min(end)).skip(i) {
                    *v = false;
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    let mut text = vec![];
    let mut map = vec![];
    let mut line = false;
    for (i, &c) in src[..end].iter().enumerate() {
        if !keep[i] {
            continue;
        }
        if c == 10 || c == 13 {
            if line {
                continue;
            }
            text.push(10);
            map.push(i);
            line = true;
        } else {
            text.push(c);
            map.push(i);
            line = false;
        }
    }
    let from = text.iter().position(|&c| !is_space16(c as u32)).unwrap_or(text.len());
    let to = text.iter().rposition(|&c| !is_space16(c as u32)).map_or(from, |i| i + 1);
    (text[from..to].to_vec(), map[from..to].to_vec())
}
fn text_data(c: u32) -> bool {
    ((32..=126).contains(&c) && c != 34 && c != 44)
        || (0xa0..=0xd7ff).contains(&c)
        || (0xe000..=0xfffd).contains(&c)
        || (0x10000..=0x10ffff).contains(&c)
}
fn codepoint(s: &[u16], p: usize) -> (u32, usize) {
    let c = at(s, p);
    let next = at(s, p + 1);
    if (0xd800..=0xdbff).contains(&c) && (0xdc00..=0xdfff).contains(&next) {
        (0x10000 + (c - 0xd800) * 1024 + next - 0xdc00, 2)
    } else {
        (c, 1)
    }
}
struct Parser {
    source: Vec<u16>,
    text: Vec<u16>,
    map: Vec<usize>,
    types: Vec<usize>,
    texts: Vec<String>,
    starts: Vec<usize>,
    i: usize,
}
impl Parser {
    fn new(source: &str) -> Self {
        let (text, map) = prepare(source);
        let mut out = Self {
            source: source.encode_utf16().collect(),
            text,
            map,
            types: vec![],
            texts: vec![],
            starts: vec![],
            i: 0,
        };
        let mut p = 0;
        let mut state = 0;
        let mut stack = vec![];
        let n = out.text.len();
        while p < n {
            let c = at(&out.text, p);
            let start = p;
            let mut q = p;
            if state == 2 {
                if c == 34 && at(&out.text, p + 1) != 34 {
                    state = stack.pop().unwrap_or(state);
                    out.emit(5, p, p + 1);
                    p += 1;
                    continue;
                }
                while q < n {
                    let (d, w) = codepoint(&out.text, q);
                    if text_data(d) || matches!(d, 44 | 13 | 10) {
                        q += w;
                    } else if d == 34 && at(&out.text, q + 1) == 34 {
                        q += 2;
                    } else {
                        break;
                    }
                }
                out.emit(7, p, q);
            } else {
                if state == 0 && (c | 32) == 115 {
                    let kw = |word: &str| {
                        word.bytes().enumerate().all(|(i, c)| at(&out.text, p + i) | 32 == c as u32)
                            && !is_word16(at(&out.text, p + word.len()))
                    };
                    let len = if kw("sankey-beta") {
                        11
                    } else if kw("sankey") {
                        6
                    } else {
                        0
                    };
                    if len > 0 {
                        stack.push(state);
                        state = 1;
                        out.emit(2, p, p + len);
                        p += len;
                        continue;
                    }
                }
                if c == 10 || (c == 13 && at(&out.text, p + 1) == 10) {
                    let len = if c == 10 { 1 } else { 2 };
                    out.emit(3, p, p + len);
                    p += len;
                    continue;
                }
                if c == 44 {
                    out.emit(4, p, p + 1);
                    p += 1;
                    continue;
                }
                if c == 34 {
                    stack.push(state);
                    state = 2;
                    out.emit(5, p, p + 1);
                    p += 1;
                    continue;
                }
                while q < n {
                    let (d, w) = codepoint(&out.text, q);
                    if !text_data(d) {
                        break;
                    }
                    q += w;
                }
                out.emit(6, p, q);
            }
            p = q;
            if q == start {
                out.emit(1, p, p);
                return out;
            }
        }
        out.emit(if state == 2 { 7 } else { 8 }, n, n);
        out.emit(0, n, n);
        out
    }
    fn emit(&mut self, t: usize, s: usize, e: usize) {
        self.types.push(t);
        self.texts.push(String::from_utf16_lossy(&self.text[s..e]));
        self.starts.push(s);
    }
    fn kind(&self) -> usize {
        self.types.get(self.i).copied().unwrap_or(0)
    }
    fn error(&self, at: usize, detail: &str) -> PeleError {
        let offset = if at > 0 && at >= self.text.len() {
            self.map.last().map_or(0, |v| v + 1)
        } else {
            self.map.get(at).copied().unwrap_or(0)
        };
        syntax_error("sankey", &self.source, offset, detail, false)
    }
    fn fail(&self, expected: &str) -> PeleError {
        let t = self.kind();
        let got = if t == 1 { NAMES[self.types[self.i - 1]] } else { NAMES[t] };
        self.error(
            self.starts.get(self.i).copied().unwrap_or(self.text.len()),
            &format!("Expecting {expected}, got '{got}'"),
        )
    }
    fn expect(&mut self, t: usize, also: &str) -> Result<(), PeleError> {
        if self.kind() != t {
            return Err(self.fail(&format!("'{}'{also}", NAMES[t])));
        }
        self.i += 1;
        Ok(())
    }
    fn field(&mut self) -> Result<String, PeleError> {
        if self.kind() == 6 {
            let s = self.texts[self.i].clone();
            self.i += 1;
            return Ok(s);
        }
        if self.kind() != 5 {
            return Err(self.fail("'DQUOTE', 'NON_ESCAPED_TEXT'"));
        }
        self.i += 1;
        if self.kind() != 7 {
            return Err(self.fail("'ESCAPED_TEXT'"));
        }
        let s = self.texts[self.i].clone();
        self.i += 1;
        self.expect(5, "")?;
        Ok(s)
    }
    fn name(&mut self) -> Result<String, PeleError> {
        let start = self.starts[self.i];
        let text = trim(&self.field()?).replace("\"\"", "\"");
        if decode_entities(&text).chars().any(crate::svg::builder::is_invalid_xml) {
            return Err(self.error(start, "Invalid XML character in node name"));
        }
        Ok(text)
    }
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let mut p = Parser::new(source);
    let mut model = Model {
        title: title.filter(|v| !v.is_empty()).map(String::from),
        ..Default::default()
    };
    p.expect(2, "")?;
    p.expect(3, "")?;
    loop {
        let from = p.name()?;
        p.expect(4, ", 'NEWLINE', 'EOF'")?;
        let to = p.name()?;
        p.expect(4, ", 'NEWLINE', 'EOF'")?;
        let start = p.starts[p.i];
        let value = parse_float(&p.field()?);
        if !value.is_finite() {
            return Err(p.error(start, "Expecting a number"));
        }
        let mut node = |id: String| {
            if let Some(i) = model.nodes.iter().position(|v| v == &id) {
                i
            } else {
                let i = model.nodes.len();
                model.nodes.push(id);
                i
            }
        };
        let source = node(from);
        let target = node(to);
        model.links.push(Link { source, target, value });
        if p.kind() != 3 {
            break;
        }
        p.i += 1;
    }
    if p.kind() == 8 {
        p.i += 1;
    }
    if p.kind() != 0 {
        return Err(p.fail("'NEWLINE', 'EOF'"));
    }
    Ok(model)
}
