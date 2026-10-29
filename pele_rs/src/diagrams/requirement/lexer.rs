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
    stack: Vec<u8>,
    out: Vec<Token>,
}
fn word(c: u16) -> bool {
    c < 128 && ((c as u8).is_ascii_alphanumeric() || c == 95)
}
fn space(c: u16) -> bool {
    is_space16(c as u32)
}
impl Lexer {
    fn c(&self, p: usize) -> u16 {
        self.src.get(p).copied().unwrap_or(0)
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
    fn push(&mut self, s: u8) {
        self.stack.push(self.state);
        self.state = s;
    }
    fn pop(&mut self) {
        if let Some(s) = self.stack.pop() {
            self.state = s;
        }
    }
    fn re(&self, k: usize) -> Option<usize> {
        static RES: OnceLock<Vec<Regex>> = OnceLock::new();
        let res = RES.get_or_init(|| {
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
        let s = String::from_utf16_lossy(&self.src[self.p..]);
        res[k].find(&s).map(|m| self.p + s[..m.end()].encode_utf16().count())
    }
    fn initial(&mut self) -> bool {
        let c = self.c(self.p);
        let n = self.src.len();
        if c | 32 == 116
            && self
                .src
                .get(self.p + 1..self.p + 5)
                .is_some_and(|s| String::from_utf16_lossy(s).eq_ignore_ascii_case("itle"))
            && space(self.c(self.p + 5))
        {
            let mut q = self.p + 6;
            while q < n && !matches!(self.c(q), 35 | 10 | 59) {
                q += 1;
            }
            if q > self.p + 6 {
                self.emit(t::title, q);
                return true;
            }
        }
        if c | 32 == 97 && self.c(self.p + 1) | 32 == 99 && self.c(self.p + 2) | 32 == 99 {
            for (k, state, token) in [(0, 3, t::acc_title), (1, 4, t::acc_descr), (2, 5, usize::MAX)] {
                if let Some(e) = self.re(k) {
                    self.push(state);
                    if token == usize::MAX {
                        self.p = e;
                    } else {
                        self.emit(token, e);
                    }
                    return true;
                }
            }
        }
        if !matches!(c, 10 | 13 | 0x2028 | 0x2029) {
            for k in 0..4 {
                if let Some(e) = self.re(k + 3) {
                    self.emit(t::direction_tb + k, e);
                    return true;
                }
            }
        }
        if c == 10 || (c == 13 && self.c(self.p + 1) == 10) {
            let mut q = self.p;
            while q < n {
                if self.c(q) == 10 {
                    q += 1;
                } else if self.c(q) == 13 && self.c(q + 1) == 10 {
                    q += 2;
                } else {
                    break;
                }
            }
            self.emit(t::NEWLINE, q);
            return true;
        }
        if space(c) {
            self.p += 1;
            while self.p < n && space(self.c(self.p)) {
                self.p += 1;
            }
            return true;
        }
        if c == 35 || c == 37 {
            self.p = self.find(10, self.p);
            return true;
        }
        if word(c) {
            let mut q = self.p + 1;
            while q < n && word(self.c(q)) {
                q += 1;
            }
            let name = String::from_utf16_lossy(&self.src[self.p..q]).to_ascii_lowercase();
            if let Some(k) = t::keyword(&name) {
                if matches!(k, t::STYLE | t::CLASSDEF | t::CLASS) {
                    self.push(2);
                }
                self.emit(k, q);
                return true;
            }
            while q < n && !matches!(self.c(q), 58 | 44 | 13 | 10 | 123 | 60 | 62 | 45 | 61) {
                q += 1;
            }
            self.emit(t::unqString, q);
            let last = self.out.last_mut().unwrap();
            last.text = last.text.trim().into();
            return true;
        }
        match c {
            123 => self.emit(t::STRUCT_START, self.p + 1),
            125 => self.emit(t::STRUCT_STOP, self.p + 1),
            58 => {
                if self.c(self.p + 1) == 58 && self.c(self.p + 2) == 58 {
                    self.emit(t::STYLE_SEPARATOR, self.p + 3);
                } else {
                    self.emit(t::COLONSEP, self.p + 1);
                }
            }
            60 => {
                if self.c(self.p + 1) != 45 {
                    return false;
                }
                self.emit(t::END_ARROW_L, self.p + 2);
            }
            45 => {
                if self.c(self.p + 1) == 62 {
                    self.emit(t::END_ARROW_R, self.p + 2);
                } else {
                    self.emit(t::LINE, self.p + 1);
                }
            }
            34 => {
                self.push(1);
                self.p += 1;
            }
            44 => self.emit(t::COMMA, self.p + 1),
            _ => return false,
        }
        true
    }
    fn style(&mut self) -> bool {
        let c = self.c(self.p);
        if word(c) {
            let mut q = self.p + 1;
            while q < self.src.len() && word(self.c(q)) {
                q += 1;
            }
            self.emit(t::ALPHA, q);
            return true;
        }
        match c {
            58 | 59 | 37 | 45 | 35 | 44 => self.emit(
                match c {
                    58 => t::COLON,
                    59 => t::SEMICOLON,
                    37 => t::PERCENT,
                    45 => t::MINUS,
                    35 => t::BRKT,
                    _ => t::COMMA,
                },
                self.p + 1,
            ),
            32 => self.p += 1,
            34 => {
                self.push(1);
                self.p += 1;
            }
            10 => {
                self.pop();
                self.p += 1;
            }
            _ => return false,
        }
        true
    }
}
pub fn tokenize(source: &str) -> Vec<Token> {
    let mut l = Lexer {
        src: source.encode_utf16().collect(),
        p: 0,
        state: 0,
        stack: vec![],
        out: vec![],
    };
    let n = l.src.len();
    while l.p < n {
        let ok = match l.state {
            0 => l.initial(),
            2 => l.style(),
            1 => {
                if l.c(l.p) == 34 {
                    l.pop();
                    l.p += 1;
                } else {
                    l.emit(t::qString, l.find(34, l.p));
                }
                true
            }
            3 | 4 => {
                let k = if l.state == 3 { t::acc_title_value } else { t::acc_descr_value };
                l.pop();
                l.emit(k, l.find(10, l.p));
                true
            }
            _ => {
                if l.c(l.p) == 125 {
                    l.pop();
                    l.p += 1;
                } else {
                    l.emit(t::acc_descr_multiline_value, l.find(125, l.p));
                }
                true
            }
        };
        if !ok {
            l.out.push(Token {
                kind: t::ERROR,
                text: String::new(),
                start: l.p,
            });
            return l.out;
        }
    }
    let last = match l.state {
        0 => Some(t::EOF),
        1 => Some(t::qString),
        3 => Some(t::acc_title_value),
        4 => Some(t::acc_descr_value),
        5 => Some(t::acc_descr_multiline_value),
        _ => None,
    };
    if let Some(k) = last {
        l.emit(k, n);
    }
    l.emit(t::END, n);
    l.out
}
