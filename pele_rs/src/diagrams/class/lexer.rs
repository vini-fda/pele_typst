use super::tokens as t;
use crate::{unicode::is_unicode_letter, util::is_space16};
use regex::Regex;
use std::sync::OnceLock;
#[derive(Clone, Debug)]
pub struct Token {
    pub kind: usize,
    pub text: String,
    pub start: usize,
}
struct Lexer {
    src: Vec<u16>,
    p: usize,
    state: usize,
    stack: Vec<usize>,
    out: Vec<Token>,
}
fn word(c: u16) -> bool {
    c == 95 || c < 128 && (c as u8).is_ascii_alphanumeric()
}
fn space(c: u16) -> bool {
    is_space16(c as u32)
}
const INITIAL: usize = 0;
const STRING: usize = 1;
const BQSTRING: usize = 2;
const GENERIC: usize = 3;
const CALLBACK_NAME: usize = 4;
const CALLBACK_ARGS: usize = 5;
const ACC_TITLE: usize = 6;
const ACC_DESCR: usize = 7;
const ACC_MULTI: usize = 8;
const CLS: usize = 9;
const CLASS_BODY: usize = 10;
const NAMESPACE: usize = 11;
const NAMESPACE_BODY: usize = 12;
const AT_END: [usize; 13] = [
    t::EOF,
    t::STR,
    t::EOF,
    t::GENERICTYPE,
    t::CALLBACK_NAME,
    t::CALLBACK_ARGS,
    t::acc_title_value,
    t::acc_descr_value,
    t::acc_descr_multiline_value,
    t::EOF,
    t::EOF_IN_STRUCT,
    t::EOF,
    t::EOF_IN_STRUCT,
];
impl Lexer {
    fn c(&self, p: usize) -> u16 {
        self.src.get(p).copied().unwrap_or(0)
    }
    fn starts(&self, s: &str) -> bool {
        self.src.get(self.p..self.p + s.len()) == Some(s.encode_utf16().collect::<Vec<_>>().as_slice())
    }
    fn kw(&self, s: &str) -> bool {
        self.starts(s) && !word(self.c(self.p + s.len()))
    }
    fn push(&mut self, state: usize) {
        self.stack.push(self.state);
        self.state = state;
    }
    fn pop(&mut self) {
        if let Some(s) = self.stack.pop() {
            self.state = s
        }
    }
    fn emit(&mut self, kind: usize, e: usize) -> bool {
        self.out.push(Token {
            kind,
            text: String::from_utf16_lossy(&self.src[self.p..e]),
            start: self.p,
        });
        self.p = e;
        true
    }
    fn until(&self, c: u16) -> usize {
        self.src[self.p..]
            .iter()
            .position(|&v| v == c)
            .map_or(self.src.len(), |i| self.p + i)
    }
    fn skip_ws(&self, mut q: usize) -> usize {
        while q < self.src.len() && space(self.c(q)) {
            q += 1;
        }
        q
    }
    fn whitespace(&mut self, leaves: bool) {
        let mut q = self.p;
        let mut last = None;
        while q < self.src.len() && space(self.c(q)) {
            if self.c(q) == 10 {
                last = Some(q);
            }
            q += 1;
        }
        if let Some(last) = last {
            if leaves {
                self.pop();
            }
            self.emit(t::NEWLINE, last + 1);
        } else {
            self.p = q;
        }
    }
    fn keyword(&mut self, c: u16) -> bool {
        let words: &[(&str, usize)] = match c {
            99 => &[("cssClass", t::CSSCLASS), ("callback", t::CALLBACK), ("click", t::CLICK)],
            108 => &[("link", t::LINK)],
            110 => &[("note for", t::NOTE_FOR), ("note", t::NOTE)],
            104 => &[("href", t::HREF)],
            60 if self.c(self.p + 1) == 60 => return self.emit(t::ANNOTATION_START, self.p + 2),
            62 if self.c(self.p + 1) == 62 => return self.emit(t::ANNOTATION_END, self.p + 2),
            _ => &[],
        };
        for &(w, k) in words {
            if self.kw(w) {
                return self.emit(k, self.p + w.len());
            }
        }
        false
    }
    fn common(&mut self, c: u16) -> bool {
        if self.keyword(c) {
            return true;
        }
        let d = self.c(self.p + 1);
        match c {
            34 | 126 | 96 => {
                self.push(if c == 34 {
                    STRING
                } else if c == 126 {
                    GENERIC
                } else {
                    BQSTRING
                });
                self.p += 1;
                return true;
            }
            95 => {
                for w in ["_self", "_blank", "_parent", "_top"] {
                    if self.kw(w) {
                        return self.emit(t::LINK_TARGET, self.p + w.len());
                    }
                }
            }
            60 => {
                return self.emit(
                    if d == 124 { t::EXTENSION } else { t::DEPENDENCY },
                    self.p + if d == 124 { 2 } else { 1 },
                );
            }
            124 => return d == 62 && self.emit(t::EXTENSION, self.p + 2),
            62 => return self.emit(t::DEPENDENCY, self.p + 1),
            42 => return self.emit(t::COMPOSITION, self.p + 1),
            111 if !word(d) => return self.emit(t::AGGREGATION, self.p + 1),
            40 => return d == 41 && self.emit(t::LOLLIPOP, self.p + 2),
            45 => return self.emit(if d == 45 { t::LINE } else { t::MINUS }, self.p + if d == 45 { 2 } else { 1 }),
            46 => return self.emit(if d == 46 { t::DOTTED_LINE } else { t::DOT }, self.p + if d == 46 { 2 } else { 1 }),
            58 => {
                let mut q = self.p + 1;
                while q < self.src.len() && !matches!(self.c(q), 58 | 10 | 59) {
                    q += 1;
                }
                if q > self.p + 1 {
                    return self.emit(t::LABEL, q);
                }
                return d == 58 && self.c(self.p + 2) == 58 && self.emit(t::STYLE_SEPARATOR, self.p + 3);
            }
            43 => return self.emit(t::PLUS, self.p + 1),
            37 => return self.emit(t::PCT, self.p + 1),
            61 => return self.emit(t::EQUALS, self.p + 1),
            91 => return self.emit(t::SQS, self.p + 1),
            93 => return self.emit(t::SQE, self.p + 1),
            33 | 35 | 36 | 38 | 39 | 44 | 63 | 92 | 47 => return self.emit(t::PUNCTUATION, self.p + 1),
            _ => {}
        }
        if word(c) {
            let mut q = self.p + 1;
            while word(self.c(q)) {
                q += 1;
            }
            return self.emit(t::ALPHA, q);
        }
        is_unicode_letter(c as u32) && self.emit(t::UNICODE_TEXT, self.p + 1)
    }
    fn direction(&mut self) -> bool {
        static RE: OnceLock<Vec<Regex>> = OnceLock::new();
        let re = RE.get_or_init(|| {
            ["TB", "BT", "RL", "LR"]
                .iter()
                .map(|dir| Regex::new(&format!(r"^[^\n\r\u2028\u2029]*direction\s+{dir}[^\n]*")).unwrap())
                .collect()
        });
        let end = self.src[self.p..]
            .iter()
            .position(|&c| matches!(c, 10 | 13 | 0x2028 | 0x2029))
            .map_or(self.src.len(), |n| self.p + n);
        if !String::from_utf16_lossy(&self.src[self.p..end]).contains("direction") {
            return false;
        }
        let text = String::from_utf16_lossy(&self.src[self.p..]);
        for (i, re) in re.iter().enumerate() {
            if let Some(found) = re.find(&text) {
                return self.emit(t::direction_tb + i, self.p + text[..found.end()].encode_utf16().count());
            }
        }
        false
    }
    fn initial(&mut self, c: u16) -> bool {
        if self.direction() {
            return true;
        }
        if space(c) {
            self.whitespace(false);
            return true;
        }
        match c {
            37 if self.c(self.p + 1) == 37 => {
                let mut e = self.until(10);
                while e < self.src.len() && matches!(self.c(e), 10 | 13) {
                    e += 1;
                }
                self.p = e;
                return true;
            }
            97 => {
                let title = self.starts("accTitle");
                if title || self.starts("accDescr") {
                    let q = self.skip_ws(self.p + 8);
                    let d = self.c(q);
                    if d == 58 {
                        self.emit(if title { t::acc_title } else { t::acc_descr }, self.skip_ws(q + 1));
                        self.push(if title { ACC_TITLE } else { ACC_DESCR });
                        return true;
                    }
                    if d == 123 && !title {
                        self.p = self.skip_ws(q + 1);
                        self.push(ACC_MULTI);
                        return true;
                    }
                }
            }
            99 => {
                if self.kw("classDiagram-v2") {
                    return self.emit(t::CLASS_DIAGRAM, self.p + 15);
                }
                if self.kw("classDiagram") {
                    return self.emit(t::CLASS_DIAGRAM, self.p + 12);
                }
                if self.starts("call") && space(self.c(self.p + 4)) {
                    self.p = self.skip_ws(self.p + 4);
                    self.push(CALLBACK_NAME);
                    return true;
                }
                if self.kw("classDef") {
                    return self.emit(t::CLASSDEF, self.p + 8);
                }
                if self.kw("class") {
                    self.push(CLS);
                    return self.emit(t::CLASS, self.p + 5);
                }
            }
            91 if self.c(self.p + 1) == 42 && self.c(self.p + 2) == 93 => return self.emit(t::EDGE_STATE, self.p + 3),
            115 if self.kw("style") => return self.emit(t::STYLE, self.p + 5),
            110 if self.kw("namespace") => {
                self.push(NAMESPACE);
                return self.emit(t::NAMESPACE, self.p + 9);
            }
            44 => return self.emit(t::COMMA, self.p + 1),
            35 => return self.emit(t::BRKT, self.p + 1),
            _ => {}
        }
        self.common(c) || (c == 58 && self.emit(t::COLON, self.p + 1))
    }
    fn scan(mut self) -> Vec<Token> {
        loop {
            if self.p >= self.src.len() {
                self.emit(AT_END[self.state], self.src.len());
                break;
            }
            let c = self.c(self.p);
            let mut ok = true;
            match self.state {
                INITIAL => ok = self.initial(c),
                CLS => {
                    if space(c) {
                        self.whitespace(true);
                    } else if c == 125 {
                        self.pop();
                        self.pop();
                        self.emit(t::STRUCT_STOP, self.p + 1);
                    } else if c == 123 {
                        self.push(CLASS_BODY);
                        self.emit(t::STRUCT_START, self.p + 1);
                    } else {
                        ok = self.common(c);
                    }
                }
                CLASS_BODY => match c {
                    34 => {
                        self.push(STRING);
                        self.p += 1;
                    }
                    125 => {
                        self.pop();
                        self.emit(t::STRUCT_STOP, self.p + 1);
                    }
                    91 if self.c(self.p + 1) == 42 && self.c(self.p + 2) == 93 => {
                        self.emit(t::EDGE_STATE, self.p + 3);
                    }
                    123 => {
                        self.emit(t::OPEN_IN_STRUCT, self.p + 1);
                    }
                    10 => self.p += 1,
                    _ => {
                        let mut q = self.p + 1;
                        while q < self.src.len() && !matches!(self.c(q), 123 | 125 | 10) {
                            q += 1;
                        }
                        self.emit(t::MEMBER, q);
                    }
                },
                NAMESPACE => {
                    if space(c) {
                        self.whitespace(true);
                    } else if c == 123 {
                        self.push(NAMESPACE_BODY);
                        self.emit(t::STRUCT_START, self.p + 1);
                    } else if c == 125 {
                        self.pop();
                    } else if c == 110 && self.kw("namespace") {
                        self.push(NAMESPACE);
                        self.emit(t::NAMESPACE, self.p + 9);
                    } else {
                        ok = self.common(c);
                    }
                }
                NAMESPACE_BODY => {
                    if space(c) {
                        self.whitespace(false);
                    } else if c == 125 {
                        self.pop();
                        self.emit(t::STRUCT_STOP, self.p + 1);
                    } else if c == 110 && self.kw("namespace") {
                        self.push(NAMESPACE);
                        self.emit(t::NAMESPACE, self.p + 9);
                    } else if c == 91 && self.c(self.p + 1) == 42 && self.c(self.p + 2) == 93 {
                        self.emit(t::EDGE_STATE, self.p + 3);
                    } else if c == 99 && self.kw("class") {
                        self.push(CLS);
                        self.emit(t::CLASS, self.p + 5);
                    } else {
                        ok = self.common(c);
                    }
                }
                STRING => {
                    if c == 34 {
                        self.pop();
                        self.p += 1;
                    } else {
                        self.emit(t::STR, self.until(34));
                    }
                }
                GENERIC => {
                    if c == 34 {
                        self.push(STRING);
                        self.p += 1;
                    } else if self.keyword(c) {
                    } else if c == 126 {
                        self.pop();
                        self.p += 1;
                    } else {
                        self.emit(t::GENERICTYPE, self.until(126));
                    }
                }
                BQSTRING => {
                    if c == 34 {
                        self.push(STRING);
                        self.p += 1;
                    } else if self.keyword(c) {
                    } else if c == 126 {
                        self.push(GENERIC);
                        self.p += 1;
                    } else if c == 96 {
                        self.pop();
                        self.p += 1;
                    } else {
                        self.emit(t::BQUOTE_STR, self.until(96));
                    }
                }
                CALLBACK_NAME => {
                    if c == 40 {
                        let q = self.skip_ws(self.p + 1);
                        self.pop();
                        if self.c(q) == 41 {
                            self.p = q + 1;
                        } else {
                            self.push(CALLBACK_ARGS);
                            self.p += 1;
                        }
                    } else {
                        self.emit(t::CALLBACK_NAME, self.until(40));
                    }
                }
                CALLBACK_ARGS => {
                    if c == 41 {
                        self.pop();
                        self.p += 1;
                    } else {
                        self.emit(t::CALLBACK_ARGS, self.until(41));
                    }
                }
                ACC_TITLE | ACC_DESCR => {
                    self.emit(
                        if self.state == ACC_TITLE {
                            t::acc_title_value
                        } else {
                            t::acc_descr_value
                        },
                        self.until(10),
                    );
                    self.pop();
                }
                ACC_MULTI => {
                    if c == 125 {
                        self.pop();
                        self.p += 1;
                    } else {
                        self.emit(t::acc_descr_multiline_value, self.until(125));
                    }
                }
                _ => unreachable!(),
            }
            if !ok {
                self.out.push(Token {
                    kind: t::ERROR,
                    text: String::new(),
                    start: self.p,
                });
                return self.out;
            }
        }
        self.emit(t::END, self.src.len());
        self.out
    }
}
pub fn tokenize(src: &str) -> Vec<Token> {
    Lexer {
        src: src.encode_utf16().collect(),
        p: 0,
        state: 0,
        stack: vec![],
        out: vec![],
    }
    .scan()
}
