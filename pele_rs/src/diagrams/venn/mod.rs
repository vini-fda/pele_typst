mod layout;
mod render;
use crate::PeleError;
use crate::errors::{PeleErrorCode, syntax_error};
use crate::preprocess::Config;
use crate::util::{is_space16, is_word16, trim};
use indexmap::IndexMap;
pub use render::render;
#[derive(Clone, Debug)]
pub struct Subset {
    pub sets: Vec<String>,
    pub size: f64,
    pub label: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Text {
    pub sets: Vec<String>,
    pub id: String,
    pub label: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Style {
    pub targets: Vec<String>,
    pub styles: IndexMap<String, String>,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: Option<String>,
    pub subsets: Vec<Subset>,
    pub text_nodes: Vec<Text>,
    pub styles: Vec<Style>,
}
pub fn normalize(s: &str) -> String {
    let s = trim(s);
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].into()
    } else {
        s.into()
    }
}
fn normalize_ids(ids: &[String]) -> Vec<String> {
    let mut out: Vec<String> = ids.iter().map(|s| normalize(s)).collect();
    out.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    out
}
const NAMES: [&str; 20] = [
    "$end",
    "INVALID",
    "EOF",
    "NEWLINE",
    "VENN",
    "TITLE",
    "SET",
    "UNION",
    "TEXT",
    "INDENT_TEXT",
    "STYLE",
    "BRACKET_LABEL",
    "NUMERIC",
    "HEXCOLOR",
    "RGBCOLOR",
    "RGBACOLOR",
    "IDENTIFIER",
    "STRING",
    "COMMA",
    "COLON",
];
const STATEMENT: &str = "'EOF', 'NEWLINE', 'TITLE', 'SET', 'UNION', 'TEXT', 'INDENT_TEXT', 'STYLE'";
struct Parser {
    src: Vec<u16>,
    at: usize,
    start: usize,
    text: String,
    line_start: bool,
    ended: bool,
    indent: bool,
    kind: i32,
    model: Model,
    current: Option<Vec<String>>,
    known: std::collections::HashSet<String>,
}
impl Parser {
    fn c(&self, i: usize) -> u32 {
        self.src.get(i).map(|c| *c as u32).unwrap_or(u32::MAX)
    }
    fn word(&self, p: usize, w: &str) -> bool {
        w.bytes().enumerate().all(|(i, w)| {
            let c = self.c(p + i);
            c == w as u32 || ((65..=90).contains(&c) && c + 32 == w as u32)
        })
    }
    fn keyword(&self, p: usize, w: &str) -> bool {
        self.word(p, w) && !is_word16(self.c(p + w.len()))
    }
    fn line_end(&self, p: usize) -> usize {
        (p..self.src.len()).find(|&i| self.c(i) == 10).unwrap_or(self.src.len())
    }
    fn slice(&self, a: usize, b: usize) -> String {
        String::from_utf16_lossy(&self.src[a..b])
    }
    fn token(&mut self, k: i32, end: usize) -> i32 {
        self.text = self.slice(self.at, end);
        self.at = end;
        k
    }
    fn color(&self, p: usize, parts: usize) -> Option<usize> {
        let mut i = p + 1;
        for part in 0..parts {
            while is_space16(self.c(i)) {
                i += 1
            }
            let from = i;
            while matches!(self.c(i), 48..=57 | 46) {
                i += 1
            }
            if i == from {
                return None;
            }
            while is_space16(self.c(i)) {
                i += 1
            }
            if self.c(i) != if part == parts - 1 { 41 } else { 44 } {
                return None;
            }
            i += 1
        }
        Some(i)
    }
    fn next(&mut self) -> i32 {
        loop {
            let mut i = self.at;
            self.start = i;
            self.text.clear();
            if i >= self.src.len() {
                if self.ended {
                    return 0;
                }
                self.ended = true;
                return 2;
            }
            let c = self.c(i);
            if c == 37 && self.c(i + 1) == 37 && self.c(i + 2) != 123 {
                self.at = self.line_end(i + 2);
                continue;
            }
            let mut before = i;
            while before > 0 && matches!(self.c(before - 1), 9 | 32) {
                before -= 1
            }
            if self.line_start
                && self.word(i, "title")
                && before > 0
                && self.c(before - 1) == 10
                && trim(&self.slice(i + 5, self.line_end(i + 5))).is_empty()
            {
                let mut j = self.line_end(i + 5);
                while matches!(self.c(j), 10 | 13) {
                    j += 1
                }
                self.at = j;
                continue;
            }
            if self.line_start {
                if matches!(c, 9 | 32) {
                    let mut j = i + 1;
                    while matches!(self.c(j), 9 | 32) {
                        j += 1
                    }
                    self.at = j;
                    if self.keyword(j, "text") && self.indent {
                        self.at = j + 4;
                        self.line_start = false;
                        self.text = self.slice(i, j);
                        return 9;
                    }
                    continue;
                }
                if !matches!(c, 10 | 13) {
                    self.indent = false;
                    self.line_start = false
                }
            }
            if matches!(c, 10 | 13) {
                i += 1;
                while matches!(self.c(i), 10 | 13) {
                    i += 1
                }
                self.line_start = true;
                return self.token(3, i);
            }
            if c == 37 && self.c(i + 1) == 37 {
                self.at = self.line_end(i);
                continue;
            }
            if matches!(c, 9 | 32) {
                i += 1;
                while matches!(self.c(i), 9 | 32) {
                    i += 1
                }
                self.at = i;
                continue;
            }
            if matches!(c | 32, 97..=122) || c == 95 {
                let sep = self.c(i + 5);
                if self.word(i, "title") && is_space16(sep) && !matches!(sep, 10 | 13) {
                    let mut end = i + 6;
                    while end < self.src.len() && !matches!(self.c(end), 35 | 10 | 59) {
                        end += 1
                    }
                    if end > i + 6 {
                        return self.token(5, end);
                    }
                }
                for (w, k) in [("venn-beta", 4), ("set", 6), ("union", 7), ("text", 8), ("style", 10)] {
                    if self.keyword(i, w) && self.c(i + w.len()) != 45 {
                        return self.token(k, i + w.len());
                    }
                }
                if self.word(i, "rgb") {
                    let alpha = self.c(i + 3) | 32 == 97;
                    let open = i + if alpha { 4 } else { 3 };
                    if self.c(open) == 40
                        && let Some(end) = self.color(open, if alpha { 4 } else { 3 })
                    {
                        return self.token(if alpha { 15 } else { 14 }, end);
                    }
                }
                i += 1;
                while is_word16(self.c(i)) || self.c(i) == 45 {
                    i += 1
                }
                return self.token(16, i);
            }
            if c == 91 {
                if self.c(i + 1) == 34 {
                    let Some(q) = (i + 2..self.src.len()).find(|&j| self.c(j) == 34) else {
                        return 1;
                    };
                    if self.c(q + 1) != 93 {
                        return 1;
                    }
                    self.at = q + 2;
                    self.text = self.slice(i + 2, q);
                    return 11;
                }
                let mut end = i + 1;
                while end < self.src.len() && !matches!(self.c(end), 93 | 34) {
                    end += 1
                }
                if end == i + 1 || self.c(end) != 93 {
                    return 1;
                }
                self.at = end + 1;
                self.text = trim(&self.slice(i + 1, end)).into();
                return 11;
            }
            if matches!(c, 48..=57 | 46 | 43 | 45) {
                if matches!(c, 43 | 45) {
                    i += 1
                }
                let from = i;
                while matches!(self.c(i), 48..=57) {
                    i += 1
                }
                if i > from {
                    if self.c(i) == 46 && matches!(self.c(i + 1), 48..=57) {
                        i += 2;
                        while matches!(self.c(i), 48..=57) {
                            i += 1
                        }
                    }
                } else if self.c(i) == 46 && matches!(self.c(i + 1), 48..=57) {
                    i += 2;
                    while matches!(self.c(i), 48..=57) {
                        i += 1
                    }
                } else {
                    return 1;
                }
                return self.token(12, i);
            }
            if c == 35 {
                let mut end = i + 1;
                while end < i + 9 && matches!(self.c(end),48..=57|65..=70|97..=102) {
                    end += 1
                }
                return if end >= i + 4 { self.token(13, end) } else { 1 };
            }
            if c == 34 {
                return if let Some(q) = (i + 1..self.src.len()).find(|&j| self.c(j) == 34) {
                    self.token(17, q + 1)
                } else {
                    1
                };
            }
            if c == 44 {
                return self.token(18, i + 1);
            }
            if c == 58 {
                return self.token(19, i + 1);
            }
            return 1;
        }
    }
    fn peek(&mut self) -> i32 {
        if self.kind < 0 {
            self.kind = self.next()
        }
        self.kind
    }
    fn take(&mut self) -> String {
        self.kind = -1;
        self.text.clone()
    }
    fn fail(&mut self, expected: &str) -> PeleError {
        let k = self.peek();
        syntax_error(
            "venn",
            &self.src,
            self.start,
            if k == 1 {
                String::new()
            } else {
                format!("Expecting {expected}, got '{}'", NAMES[k as usize])
            }
            .as_str(),
            k == 1,
        )
    }
    fn expect(&mut self, k: i32) -> Result<String, PeleError> {
        if self.peek() != k {
            return Err(self.fail(&format!("'{}'", NAMES[k as usize])));
        }
        Ok(self.take())
    }
    fn identifier(&mut self) -> Result<String, PeleError> {
        if !matches!(self.peek(), 16 | 17) {
            return Err(self.fail("'IDENTIFIER', 'STRING'"));
        }
        Ok(self.take())
    }
    fn identifiers(&mut self) -> Result<Vec<String>, PeleError> {
        let mut out = vec![self.identifier()?];
        while self.peek() == 18 {
            self.take();
            out.push(self.identifier()?)
        }
        Ok(out)
    }
    fn subset(&mut self, ids: Vec<String>, union: bool) -> Result<(), PeleError> {
        let label = if self.peek() == 11 { Some(self.take()) } else { None };
        let size = if self.peek() == 19 {
            self.take();
            Some(self.expect(12)?.parse().unwrap_or(f64::INFINITY))
        } else {
            None
        };
        if union {
            if ids.len() < 2 {
                return Err(semantic("union requires multiple identifiers".into()));
            }
            let unknown: Vec<_> = ids.iter().map(|s| normalize(s)).filter(|s| !self.known.contains(s)).collect();
            if !unknown.is_empty() {
                return Err(semantic(format!("unknown set identifier: {}", unknown.join(", "))));
            }
        }
        let sets = normalize_ids(&ids);
        self.current = Some(sets.clone());
        if sets.len() == 1 {
            self.known.insert(sets[0].clone());
        }
        self.model.subsets.push(Subset {
            sets,
            size: size.unwrap_or(10.0 / (ids.len() as f64).powi(2)),
            label: label.filter(|s| !s.is_empty()).map(|s| normalize(&s)),
        });
        self.indent = true;
        Ok(())
    }
    fn text_node(&mut self, sets: Option<Vec<String>>) -> Result<(), PeleError> {
        let k = self.peek();
        if !matches!(k, 12 | 16 | 17) {
            return Err(self.fail("'NUMERIC', 'IDENTIFIER', 'STRING'"));
        }
        let id = self.take();
        let label = if k != 12 && self.peek() == 11 { Some(self.take()) } else { None };
        let sets = sets
            .or_else(|| self.current.clone())
            .ok_or_else(|| semantic("text requires set".into()))?;
        self.model.text_nodes.push(Text {
            sets: normalize_ids(&sets),
            id: normalize(&id),
            label: label.filter(|s| !s.is_empty()).map(|s| normalize(&s)),
        });
        Ok(())
    }
    fn style(&mut self) -> Result<(), PeleError> {
        let ids = self.identifiers()?;
        let mut styles = IndexMap::new();
        loop {
            let key = self.expect(16)?;
            self.expect(19)?;
            let value = if self.peek() == 17 {
                self.take()
            } else {
                let mut parts = Vec::new();
                while matches!(self.peek(), 12..=16) {
                    parts.push(self.take())
                }
                if parts.is_empty() {
                    return Err(self.fail("'NUMERIC', 'IDENTIFIER', 'STRING', 'HEXCOLOR', 'RGBCOLOR', 'RGBACOLOR'"));
                }
                parts.join(" ")
            };
            styles.insert(key, normalize(&value));
            if self.peek() != 18 {
                break;
            }
            self.take();
        }
        self.model.styles.push(Style {
            targets: normalize_ids(&ids),
            styles,
        });
        Ok(())
    }
}
fn semantic(s: String) -> PeleError {
    PeleError::new(s, PeleErrorCode::Semantic).of("venn")
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let mut p = Parser {
        src: source.encode_utf16().collect(),
        at: 0,
        start: 0,
        text: String::new(),
        line_start: false,
        ended: false,
        indent: false,
        kind: -1,
        model: Model {
            title: title.map(str::to_string),
            ..Default::default()
        },
        current: None,
        known: Default::default(),
    };
    while p.peek() == 3 {
        p.take();
    }
    p.expect(4)?;
    loop {
        let k = p.peek();
        if k == 2 {
            return Ok(p.model);
        }
        if !(3..=10).contains(&k) || k == 4 {
            return Err(p.fail(STATEMENT));
        }
        let raw = p.take();
        match k {
            5 => p.model.title = Some(raw.chars().skip(6).collect()),
            6 => {
                let id = p.identifier()?;
                p.subset(vec![id], false)?
            }
            7 => {
                let ids = p.identifiers()?;
                p.subset(ids, true)?
            }
            8 => {
                let ids = p.identifiers()?;
                p.text_node(Some(ids))?
            }
            9 => p.text_node(None)?,
            10 => p.style()?,
            _ => (),
        }
    }
}
