use super::tokens as t;
use crate::util::is_space16;
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
const INITIAL: usize = 0;
const STRING: usize = 1;
const MD_STRING: usize = 2;
const NODE: usize = 3;
const BLOCK_ARROW: usize = 4;
const ARROW_DIR: usize = 5;
const LLABEL: usize = 6;
const CLASSDEF: usize = 7;
const CLASSDEFID: usize = 8;
const CLASS: usize = 9;
const CLASS_STYLE: usize = 10;
const STYLE_STMNT: usize = 11;
const STYLE_DEF: usize = 12;
const ACC_TITLE: usize = 13;
const ACC_DESCR: usize = 14;
const ACC_MULTI: usize = 15;
fn word(c: u16) -> bool {
    c == 95 || c < 128 && (c as u8).is_ascii_alphanumeric()
}
fn space(c: u16) -> bool {
    is_space16(c as u32)
}
impl Lexer {
    fn c(&self, p: usize) -> u16 {
        self.src.get(p).copied().unwrap_or(0)
    }
    fn starts(&self, s: &str, p: usize) -> bool {
        s.bytes().enumerate().all(|(i, c)| self.c(p + i) == c as u16)
    }
    fn keyword(&self, s: &str) -> bool {
        self.starts(s, self.p) && !word(self.c(self.p + s.len()))
    }
    fn emit(&mut self, k: usize, e: usize) -> bool {
        self.put(k, String::from_utf16_lossy(&self.src[self.p..e]), e)
    }
    fn put(&mut self, k: usize, text: String, e: usize) -> bool {
        self.out.push(Token {
            kind: k,
            text,
            start: self.p,
        });
        self.p = e;
        true
    }
    fn push(&mut self, s: usize) {
        self.stack.push(self.state);
        self.state = s;
    }
    fn pop(&mut self) {
        if let Some(s) = self.stack.pop() {
            self.state = s;
        }
    }
    fn ws(&self, mut p: usize) -> usize {
        while p < self.src.len() && space(self.c(p)) {
            p += 1;
        }
        p
    }
    fn digits(&self, mut p: usize) -> usize {
        while p < self.src.len() && (48..=57).contains(&self.c(p)) {
            p += 1;
        }
        p
    }
    fn word(&self, mut p: usize) -> usize {
        while p < self.src.len() && word(self.c(p)) {
            p += 1;
        }
        p
    }
    fn find(&self, c: u16, p: usize) -> usize {
        self.src[p.min(self.src.len())..]
            .iter()
            .position(|&v| v == c)
            .map_or(self.src.len(), |n| p + n)
    }
    fn quote(&mut self) -> bool {
        if self.c(self.p + 1) == 96 {
            self.push(MD_STRING);
            self.p += 2;
        } else {
            self.push(STRING);
            self.p += 1;
        }
        true
    }
    fn dotted(&self, mut q: usize) -> Option<usize> {
        if self.c(q) != 46 {
            return None;
        }
        while self.c(q) == 46 {
            q += 1;
        }
        if self.c(q) != 45 {
            return None;
        }
        Some(if matches!(self.c(q + 1), 120 | 111 | 62) { q + 2 } else { q + 1 })
    }
    fn link_end(&self, mut q: usize) -> Option<usize> {
        if matches!(self.c(q), 120 | 111 | 60) {
            q += 1;
        }
        let line = self.c(q);
        if line == 46 {
            return self.dotted(q);
        }
        if line != 45 && line != 61 {
            return None;
        }
        let mut k = q + 1;
        while self.c(k) == line {
            k += 1;
        }
        if k - q == 1 {
            return if line == 45 { self.dotted(k) } else { None };
        }
        if matches!(self.c(k), 120 | 111 | 62) {
            return Some(k + 1);
        }
        if k - q >= 3 { Some(k) } else { None }
    }
    fn link(&mut self) -> bool {
        if let Some(e) = self.link_end(self.p) {
            return self.emit(t::LINK, self.ws(e));
        }
        let q = self.p + if self.c(self.p) == 60 { 1 } else { 0 };
        let (a, b) = (self.c(q), self.c(q + 1));
        if (a == 45 && (b == 45 || b == 46)) || (a == 61 && b == 61) {
            self.push(LLABEL);
            return self.emit(t::START_LINK, self.ws(q + 2));
        }
        false
    }
    fn line(&mut self, k: usize) {
        let e = self.find(10, self.p);
        self.pop();
        self.emit(k, e);
    }
    fn id_list(&mut self, k: usize, next: usize) -> bool {
        let mut q = self.word(self.p);
        if q == self.p {
            return false;
        }
        while self.c(q) == 44 {
            let from = self.ws(q + 1);
            let to = self.word(from);
            if to == from {
                break;
            }
            q = to;
        }
        self.state = next;
        self.emit(k, q)
    }
    fn initial(&mut self, c: u16) -> bool {
        if space(c) {
            self.p = self.ws(self.p + 1);
            return true;
        }
        match c {
            98 => {
                if self.starts("block", self.p) {
                    if self.keyword("block-beta") {
                        return self.emit(t::KEY, self.p + 10);
                    }
                    if self.c(self.p + 5) == 58 {
                        return self.emit(t::ID_BLOCK, self.p + 6);
                    }
                    if !word(self.c(self.p + 5)) {
                        return self.emit(t::KEY, self.p + 5);
                    }
                }
            }
            99 => {
                if self.starts("columns", self.p) {
                    let q = self.ws(self.p + 7);
                    if q > self.p + 7 {
                        if self.starts("auto", q) && !word(self.c(q + 4)) {
                            return self.put(t::COLUMNS, "-1".into(), q + 4);
                        }
                        let e = self.digits(q);
                        if e > q {
                            return self.put(t::COLUMNS, String::from_utf16_lossy(&self.src[q..e]), e);
                        }
                    }
                } else if self.starts("class", self.p) {
                    let def = self.starts("Def", self.p + 5);
                    let from = self.p + if def { 8 } else { 5 };
                    let q = self.ws(from);
                    if q > from {
                        self.push(if def { CLASSDEF } else { CLASS });
                        return self.emit(if def { t::CLASSDEF } else { t::CLASS }, q);
                    }
                }
            }
            34 => return self.quote(),
            115 => {
                if self.starts("space", self.p) {
                    if self.c(self.p + 5) == 58 {
                        let e = self.digits(self.p + 6);
                        if e > self.p + 6 {
                            return self.put(t::SPACE_BLOCK, String::from_utf16_lossy(&self.src[self.p + 6..e]), e);
                        }
                    }
                    if !word(self.c(self.p + 5)) {
                        return self.put(t::SPACE_BLOCK, "1".into(), self.p + 5);
                    }
                } else if self.starts("style", self.p) {
                    let q = self.ws(self.p + 5);
                    if q > self.p + 5 {
                        self.push(STYLE_STMNT);
                        return self.emit(t::STYLE, q);
                    }
                }
            }
            100 if self.keyword("default") => return self.emit(t::DEFAULT, self.p + 7),
            108 if self.keyword("linkStyle") => return self.emit(t::LINKSTYLE, self.p + 9),
            105 if self.keyword("interpolate") => return self.emit(t::INTERPOLATE, self.p + 11),
            97 => {
                if self.starts("accTitle", self.p) {
                    let q = self.ws(self.p + 8);
                    if self.c(q) == 58 {
                        self.push(ACC_TITLE);
                        return self.emit(t::acc_title, self.ws(q + 1));
                    }
                } else if self.starts("accDescr", self.p) {
                    let q = self.ws(self.p + 8);
                    if self.c(q) == 58 {
                        self.push(ACC_DESCR);
                        return self.emit(t::acc_descr, self.ws(q + 1));
                    }
                    if self.c(q) == 123 {
                        self.push(ACC_MULTI);
                        self.p = self.ws(q + 1);
                        return true;
                    }
                }
            }
            101 if self.keyword("end") => return self.emit(t::end, self.ws(self.p + 3)),
            45 => {
                if self.c(self.p + 1) == 41 {
                    self.push(NODE);
                    return self.emit(t::NODE_DSTART, self.p + 2);
                }
                return self.link();
            }
            61 => return self.link(),
            60 => {
                if self.c(self.p + 1) == 91 {
                    self.push(BLOCK_ARROW);
                    return self.emit(t::BLOCK_ARROW_START, self.p + 2);
                }
                return self.link();
            }
            40 => {
                let d = self.c(self.p + 1);
                let len = if d == 45 || d == 91 {
                    2
                } else if d == 40 {
                    if self.c(self.p + 2) == 40 { 3 } else { 2 }
                } else {
                    1
                };
                self.push(NODE);
                return self.emit(t::NODE_DSTART, self.p + len);
            }
            41 | 123 => {
                self.push(NODE);
                return self.emit(t::NODE_DSTART, self.p + if self.c(self.p + 1) == c { 2 } else { 1 });
            }
            62 => {
                self.push(NODE);
                return self.emit(t::NODE_DSTART, self.p + 1);
            }
            91 => {
                let d = self.c(self.p + 1);
                self.push(NODE);
                return self.emit(t::NODE_DSTART, self.p + if matches!(d, 91 | 124 | 40 | 92 | 47) { 2 } else { 1 });
            }
            58 => {
                let e = self.digits(self.p + 1);
                if e > self.p + 1 {
                    return self.put(t::SIZE, String::from_utf16_lossy(&self.src[self.p + 1..e]), e);
                }
                return false;
            }
            _ => {}
        }
        let mut q = self.p;
        while q < self.src.len() {
            let c = self.c(q);
            if if c < 128 {
                matches!(c, 9 | 10 | 11 | 12 | 13 | 32 | 40 | 41 | 45 | 58 | 60 | 61 | 62 | 91 | 123 | 125)
            } else {
                space(c)
            } {
                break;
            }
            q += 1;
        }
        q > self.p && self.emit(t::NODE_ID, q)
    }
    fn node_end(&mut self, c: u16) -> bool {
        let d = self.c(self.p + 1);
        let len = match c {
            34 => return self.quote(),
            40 => {
                if d == 45 {
                    2
                } else if d == 40 {
                    if self.c(self.p + 2) == 40 { 3 } else { 2 }
                } else {
                    1
                }
            }
            41 => {
                if d == 41 {
                    if self.c(self.p + 2) == 41 { 3 } else { 2 }
                } else if d == 93 {
                    2
                } else {
                    1
                }
            }
            125 => {
                if d == 125 {
                    2
                } else {
                    1
                }
            }
            45 => {
                if d == 41 {
                    2
                } else {
                    0
                }
            }
            93 => {
                if matches!(d, 93 | 41 | 62) {
                    2
                } else {
                    1
                }
            }
            92 | 47 if d == 93 => 2,
            _ => 0,
        };
        if len == 0 {
            return false;
        }
        self.pop();
        self.emit(t::NODE_DEND, self.p + len)
    }
    fn arrow_dir(&mut self, c: u16) -> bool {
        if c == 41 {
            self.pop();
            self.pop();
            return self.put(t::BLOCK_ARROW_END, "]>".into(), self.ws(self.p + 1));
        }
        let from = self.ws(if c == 44 { self.p + 1 } else { self.p });
        for dir in ["right", "left", "x", "y", "up", "down"] {
            if self.starts(dir, from) {
                let e = self.ws(from + dir.len());
                return self.put(
                    t::DIR,
                    String::from_utf16_lossy(&self.src[if c == 44 { from } else { self.p }..e]),
                    e,
                );
            }
        }
        false
    }
    fn scan(mut self) -> Vec<Token> {
        let n = self.src.len();
        loop {
            if self.p >= n {
                let k = match self.state {
                    INITIAL => Some(t::EOF),
                    STRING => Some(t::STR),
                    CLASSDEFID => Some(t::CLASSDEF_STYLEOPTS),
                    CLASS_STYLE => Some(t::STYLECLASS),
                    STYLE_DEF => Some(t::STYLE_DEFINITION_DATA),
                    ACC_TITLE => Some(t::acc_title_value),
                    ACC_DESCR => Some(t::acc_descr_value),
                    ACC_MULTI => Some(t::acc_descr_multiline_value),
                    _ => None,
                };
                self.p = n;
                if let Some(k) = k {
                    self.put(k, String::new(), n);
                }
                break;
            }
            let c = self.c(self.p);
            let mut ok = true;
            match self.state {
                INITIAL => ok = self.initial(c),
                STRING => {
                    if c == 34 {
                        self.pop();
                        self.p += 1;
                    } else {
                        self.emit(t::STR, self.find(34, self.p));
                    }
                }
                MD_STRING => {
                    if c == 96 {
                        ok = self.c(self.p + 1) == 34;
                        if ok {
                            self.pop();
                            self.p += 2;
                        }
                    } else if c == 34 {
                        ok = false;
                    } else {
                        let mut q = self.p + 1;
                        while q < n && !matches!(self.c(q), 96 | 34) {
                            q += 1;
                        }
                        self.emit(t::MD_STR, q);
                    }
                }
                NODE => ok = self.node_end(c),
                BLOCK_ARROW => {
                    if c == 34 {
                        self.quote();
                    } else if c == 93 && self.c(self.p + 1) == 62 {
                        let q = self.ws(self.p + 2);
                        ok = self.c(q) == 40;
                        if ok {
                            self.push(ARROW_DIR);
                            self.p = q + 1;
                        }
                    } else {
                        ok = false
                    }
                }
                ARROW_DIR => ok = self.arrow_dir(c),
                LLABEL => {
                    if c == 34 {
                        if self.c(self.p + 1) == 96 {
                            self.quote();
                        } else {
                            self.emit(t::LINK_LABEL, self.p + 1);
                            self.push(STRING);
                        }
                    } else if let Some(e) = self.link_end(self.ws(self.p)) {
                        self.pop();
                        self.emit(t::LINK, self.ws(e));
                    } else {
                        ok = false
                    }
                }
                CLASSDEF => {
                    let mut kind = t::DEFAULT_CLASSDEF_ID;
                    let mut k = if self.starts("DEFAULT", self.p) { self.p + 7 } else { self.p };
                    let mut q = self.ws(k);
                    if k == self.p || q == k {
                        kind = t::CLASSDEF_ID;
                        k = self.word(self.p);
                        q = self.ws(k);
                    }
                    ok = k > self.p && q > k;
                    if ok {
                        self.state = CLASSDEFID;
                        self.emit(kind, q);
                    }
                }
                CLASSDEFID => self.line(t::CLASSDEF_STYLEOPTS),
                CLASS => ok = self.id_list(t::CLASSENTITY_IDS, CLASS_STYLE),
                CLASS_STYLE => self.line(t::STYLECLASS),
                STYLE_STMNT => ok = self.id_list(t::STYLE_ENTITY_IDS, STYLE_DEF),
                STYLE_DEF => self.line(t::STYLE_DEFINITION_DATA),
                ACC_TITLE => self.line(t::acc_title_value),
                ACC_DESCR => self.line(t::acc_descr_value),
                ACC_MULTI => {
                    if c == 125 {
                        self.pop();
                        self.p += 1;
                    } else {
                        self.emit(t::acc_descr_multiline_value, self.find(125, self.p));
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
        self.put(t::END, String::new(), n);
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
