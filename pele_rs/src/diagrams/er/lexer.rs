use super::tokens as t;
use crate::util::is_space16;
use regex::Regex;
use std::sync::OnceLock;
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: usize,
    pub text: String,
    pub start: usize,
}
struct Lexer {
    src: Vec<u16>,
    p: usize,
    state: u8,
    out: Vec<Token>,
}
fn word(c: u16) -> bool {
    c == 95 || (c as u8).is_ascii_alphanumeric() && c < 128
}
fn digit(c: u16) -> bool {
    (48..=57).contains(&c)
}
fn space(c: u16) -> bool {
    is_space16(c as u32)
}
fn attr(c: u16, start: bool) -> bool {
    if c >= 128 {
        return c >= 0xc0 || c == 0xb5;
    }
    if start {
        c == 42 || c == 95 || (c as u8).is_ascii_alphabetic()
    } else {
        word(c) || b"-*.[ ](),".contains(&(c as u8)) && c != 32
    }
}
impl Lexer {
    fn c(&self, p: usize) -> u16 {
        self.src.get(p).copied().unwrap_or(0)
    }
    fn kw(&self, s: &str) -> bool {
        s.bytes()
            .enumerate()
            .all(|(i, c)| self.c(self.p + i) == c as u16 || (c.is_ascii_lowercase() && (self.c(self.p + i) | 32) == c as u16))
    }
    fn kwb(&self, s: &str) -> bool {
        self.kw(s) && !word(self.c(self.p + s.len()))
    }
    fn emit(&mut self, kind: usize, e: usize) {
        self.out.push(Token {
            kind,
            text: String::from_utf16_lossy(&self.src[self.p..e]),
            start: self.p,
        });
        self.p = e;
    }
    fn find(&self, ch: u16, from: usize) -> usize {
        self.src[from.min(self.src.len())..]
            .iter()
            .position(|&c| c == ch)
            .map_or(self.src.len(), |i| from + i)
    }
    fn re(&self, pattern: &str) -> Option<usize> {
        let text = String::from_utf16_lossy(&self.src[self.p..]);
        static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
        let patterns = PATTERNS.get_or_init(|| {
            [
                r"(?i)^accTitle\s*:\s*",
                r"(?i)^accDescr\s*:\s*",
                r"(?i)^accDescr\s*\{\s*",
                r"(?i)^.*direction\s+TB[^\n]*",
                r"(?i)^.*direction\s+BT[^\n]*",
                r"(?i)^.*direction\s+RL[^\n]*",
                r"(?i)^.*direction\s+LR[^\n]*",
            ]
            .iter()
            .map(|s| Regex::new(s).unwrap())
            .collect()
        });
        patterns
            .iter()
            .find(|r| r.as_str() == pattern)
            .unwrap()
            .find(&text)
            .filter(|m| m.start() == 0)
            .map(|m| self.p + text[..m.end()].encode_utf16().count())
    }
    fn keyword(&mut self) -> bool {
        let c = self.c(self.p) | 32;
        let d = self.c(self.p + 1);
        let words: &[(&str, usize, bool)] = match c {
            101 => &[("erdiagram", t::ER_DIAGRAM, true), ("end", t::END_KW, true)],
            115 => &[("style", t::STYLE, true), ("subgraph", t::SUBGRAPH, true)],
            99 => &[("classdef", t::CLASSDEF, true), ("class", t::CLASS, true)],
            111 => &[
                ("one or zero", t::ZERO_OR_ONE, true),
                ("one or more", t::ONE_OR_MORE, true),
                ("one or many", t::ONE_OR_MORE, true),
                ("one", t::ONLY_ONE, true),
                ("only one", t::ONLY_ONE, true),
                ("optionally to", t::NON_IDENTIFYING, true),
            ],
            122 => &[
                ("zero or one", t::ZERO_OR_ONE, true),
                ("zero or more", t::ZERO_OR_MORE, true),
                ("zero or many", t::ZERO_OR_MORE, true),
            ],
            109 => &[
                ("many(0)", t::ZERO_OR_MORE, false),
                ("many(1)", t::ONE_OR_MORE, false),
                ("many", t::ZERO_OR_MORE, true),
            ],
            116 => &[("to", t::IDENTIFYING, true)],
            _ => &[],
        };
        for &(s, kind, boundary) in words {
            if if boundary { self.kwb(s) } else { self.kw(s) } {
                let mut e = self.p + s.len();
                if kind == t::END_KW {
                    while space(self.c(e)) {
                        e += 1;
                    }
                }
                if kind == t::STYLE || kind == t::CLASSDEF {
                    self.state = 6;
                }
                self.emit(kind, e);
                return true;
            }
        }
        let pair = match (c, d) {
            (111, 124) => Some(t::ZERO_OR_ONE),
            (111, 123) => Some(t::ZERO_OR_MORE),
            (124, 111 | 79) if !word(self.c(self.p + 2)) => Some(t::ZERO_OR_ONE),
            (124, 124) => Some(t::ONLY_ONE),
            (124, 123) => Some(t::ONE_OR_MORE),
            (125, 111 | 79) if !word(self.c(self.p + 2)) => Some(t::ZERO_OR_MORE),
            (125, 124) => Some(t::ONE_OR_MORE),
            (46, 46 | 45) => Some(t::NON_IDENTIFYING),
            (45, 45) => Some(t::IDENTIFYING),
            (45, 46) => Some(t::NON_IDENTIFYING),
            _ => None,
        };
        if let Some(k) = pair {
            self.emit(k, self.p + 2);
            return true;
        }
        if c == 117 && matches!(d, 46 | 45 | 124) {
            self.emit(t::MD_PARENT, self.p + 1);
            return true;
        }
        false
    }
    fn initial(&mut self) {
        let c = self.c(self.p);
        if self.kw("acc") {
            for (pattern, state, token) in [
                (r"(?i)^accTitle\s*:\s*", 3, t::acc_title),
                (r"(?i)^accDescr\s*:\s*", 4, t::acc_descr),
                (r"(?i)^accDescr\s*\{\s*", 5, usize::MAX),
            ] {
                if let Some(e) = self.re(pattern) {
                    self.state = state;
                    if token == usize::MAX {
                        self.p = e;
                    } else {
                        self.emit(token, e);
                    }
                    return;
                }
            }
        }
        for (k, d) in ["TB", "BT", "RL", "LR"].iter().enumerate() {
            if let Some(e) = self.re(&format!(r"(?i)^.*direction\s+{}[^\n]*", d)) {
                self.emit(t::direction_tb + k, e);
                return;
            }
        }
        match c {
            32 | 9 | 13 => {
                self.p += 1;
                while matches!(self.c(self.p), 32 | 9 | 13) {
                    self.p += 1;
                }
                return;
            }
            10 => {
                let mut e = self.p + 1;
                while self.c(e) == 10 {
                    e += 1;
                }
                self.emit(t::NEWLINE, e);
                return;
            }
            34 => {
                let e = self.find(34, self.p + 1);
                if e == self.src.len() {
                    self.emit(t::CHAR, self.p + 1);
                } else {
                    let name = e > self.p + 1 && !self.src[self.p + 1..e].iter().any(|&c| matches!(c, 37 | 92 | 13 | 10 | 11 | 8));
                    self.emit(if name { t::ENTITY_NAME } else { t::WORD }, e + 1);
                }
                return;
            }
            123 => {
                self.state = 1;
                self.emit(t::BLOCK_START, self.p + 1);
                return;
            }
            35 | 44 | 58 | 91 | 93 => {
                let k = match c {
                    35 => t::BRKT,
                    44 => t::COMMA,
                    91 => t::SQS,
                    93 => t::SQE,
                    _ => t::COLON,
                };
                if c == 58 && self.c(self.p + 1) == 58 && self.c(self.p + 2) == 58 {
                    self.emit(t::STYLE_SEPARATOR, self.p + 3);
                } else {
                    self.emit(k, self.p + 1);
                }
                return;
            }
            _ => {}
        }
        if digit(c) {
            let d = self.c(self.p + 1);
            if d == 43 && (c == 49 || c == 48) {
                self.emit(if c == 49 { t::ONE_OR_MORE } else { t::ZERO_OR_MORE }, self.p + 2);
                return;
            }
            let mut q = self.p + 1;
            while digit(self.c(q)) {
                q += 1;
            }
            if self.c(q) == 46 && digit(self.c(q + 1)) {
                q += 2;
                while digit(self.c(q)) {
                    q += 1;
                }
                self.emit(t::DECIMAL_NUM, q);
                return;
            }
            if c == 49 {
                if space(d) {
                    let mut r = self.p + 2;
                    while space(self.c(r)) {
                        r += 1;
                    }
                    let f = self.c(r);
                    if word(f) || f == 34 || f == 39 {
                        self.emit(t::ONLY_ONE, self.p + 1);
                        return;
                    }
                } else if matches!(d, 45 | 46) && matches!(self.c(self.p + 2), 45 | 46) {
                    self.emit(t::ONLY_ONE, self.p + 1);
                    return;
                }
                if !word(d) {
                    self.emit(t::ENTITY_ONE, self.p + 1);
                    return;
                }
            }
            self.emit(t::NUM, q);
            return;
        }
        if self.keyword() {
            return;
        }
        if c >= 128 || word(c) || matches!(c, 45 | 42 | 46) {
            let mut e = self.p + 1;
            while self.c(e) >= 128 || word(self.c(e)) || matches!(self.c(e), 45 | 42 | 46) {
                e += 1;
            }
            self.emit(t::UNICODE_TEXT, e);
        } else {
            self.emit(t::CHAR, self.p + 1);
        }
    }
    fn block(&mut self) {
        let c = self.c(self.p);
        if space(c) {
            self.p += 1;
            while space(self.c(self.p)) {
                self.p += 1;
            }
            return;
        }
        if matches!(c | 32, 112 | 102 | 117) && self.c(self.p + 1) | 32 == 107 && !word(self.c(self.p + 2)) {
            self.emit(t::ATTRIBUTE_KEY, self.p + 2);
            return;
        }
        let first = self.find(126, self.p);
        let mut ws = self.p;
        while ws < self.src.len() && !space(self.c(ws)) {
            ws += 1;
        }
        if first < ws {
            let mut end = self.p;
            while end < self.src.len() && !matches!(self.c(end), 10 | 13 | 0x2028 | 0x2029) {
                end += 1;
            }
            if let Some(last) = self.src[self.p..end].iter().rposition(|&v| v == 126).map(|k| self.p + k)
                && last > first
            {
                let mut e = last + 1;
                while e < self.src.len() && !space(self.c(e)) {
                    e += 1;
                }
                self.emit(t::ATTRIBUTE_WORD, e);
                return;
            }
        }
        if attr(c, true) {
            let mut e = self.p + 1;
            while e < self.src.len() && attr(self.c(e), false) {
                e += 1;
            }
            self.emit(t::ATTRIBUTE_WORD, e);
            return;
        }
        if c == 96 {
            self.state = 2;
            self.p += 1;
            return;
        }
        if c == 34 {
            let e = self.find(34, self.p + 1);
            if e < self.src.len() {
                self.emit(t::COMMENT, e + 1);
                return;
            }
        } else if c == 125 {
            self.state = 0;
            self.emit(t::BLOCK_STOP, self.p + 1);
            return;
        }
        self.emit(t::CHAR, self.p + 1);
    }
}
pub fn tokenize(src: &str) -> Vec<Token> {
    let mut l = Lexer {
        src: src.encode_utf16().collect(),
        p: 0,
        state: 0,
        out: vec![],
    };
    let n = l.src.len();
    while l.p < n {
        let c = l.c(l.p);
        match l.state {
            0 => l.initial(),
            1 => l.block(),
            2 => {
                if c == 96 {
                    l.state = 1;
                    l.p += 1;
                } else {
                    let e = l.find(96, l.p);
                    l.emit(t::ATTRIBUTE_WORD, e);
                }
            }
            3 | 4 => {
                let e = l.find(10, l.p);
                l.emit(if l.state == 3 { t::acc_title_value } else { t::acc_descr_value }, e);
                l.state = 0;
            }
            5 => {
                if c == 125 {
                    l.state = 0;
                    l.p += 1;
                } else {
                    let e = l.find(125, l.p);
                    l.emit(t::acc_descr_multiline_value, e);
                }
            }
            _ => {
                if c == 10 {
                    let mut e = l.p + 1;
                    while l.c(e) == 10 {
                        e += 1;
                    }
                    l.state = 0;
                    l.emit(t::NEWLINE, e);
                } else if space(c) {
                    l.p += 1;
                    while space(l.c(l.p)) {
                        l.p += 1;
                    }
                } else if matches!(c, 58 | 44 | 35 | 59) {
                    l.emit(
                        match c {
                            58 => t::COLON,
                            44 => t::COMMA,
                            35 => t::BRKT,
                            _ => t::SEMI,
                        },
                        l.p + 1,
                    );
                } else if c >= 128 || word(c) || c == 45 || c == 42 {
                    let mut e = l.p + 1;
                    while l.c(e) >= 128 || word(l.c(e)) || l.c(e) == 45 || l.c(e) == 42 {
                        e += 1;
                    }
                    l.emit(t::STYLE_TEXT, e);
                } else {
                    l.out.push(Token {
                        kind: t::ERROR,
                        text: String::new(),
                        start: l.p,
                    });
                    return l.out;
                }
            }
        }
    }
    let last = match l.state {
        0 => Some(t::EOF),
        3 => Some(t::acc_title_value),
        4 => Some(t::acc_descr_value),
        5 => Some(t::acc_descr_multiline_value),
        _ => None,
    };
    if let Some(kind) = last {
        l.out.push(Token {
            kind,
            text: String::new(),
            start: n,
        });
    }
    l.out.push(Token {
        kind: t::END,
        text: String::new(),
        start: n,
    });
    l.out
}
