use super::tokens as t;
use crate::util::is_space16;
#[derive(Clone, Debug)]
pub struct Token {
    pub kind: usize,
    pub text: String,
    pub start: usize,
}
pub struct Tokens {
    pub out: Vec<Token>,
    pub error: Option<String>,
}
struct Lexer {
    src: Vec<u16>,
    p: usize,
    state: usize,
    stack: Vec<usize>,
    out: Vec<Token>,
    error: Option<String>,
    direction_failed_until: usize,
}
const INITIAL: usize = 0;
const STRUCT: usize = 1;
const STATE: usize = 2;
const STATE_STRING: usize = 3;
const STATE_ID: usize = 4;
const SCALE: usize = 5;
const ACC_TITLE: usize = 6;
const ACC_DESCR: usize = 7;
const ACC_MULTI: usize = 8;
const CLASSDEF: usize = 9;
const CLASSDEF_ID: usize = 10;
const CLASS: usize = 11;
const CLASS_STYLE: usize = 12;
const STYLE: usize = 13;
const STYLEDEF_STYLES: usize = 14;
const NOTE: usize = 15;
const NOTE_ID: usize = 16;
const NOTE_TEXT: usize = 17;
const FLOATING_NOTE: usize = 18;
const FLOATING_NOTE_ID: usize = 19;
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
    fn word(&self, w: &str, q: usize) -> bool {
        w.bytes()
            .enumerate()
            .all(|(i, a)| self.c(q + i) == a as u16 || a.is_ascii_lowercase() && self.c(q + i) == (a - 32) as u16)
    }
    fn ws_end(&self, mut q: usize) -> usize {
        while q < self.src.len() && space(self.c(q)) {
            q += 1;
        }
        q
    }
    fn word_end(&self, mut q: usize) -> usize {
        while q < self.src.len() && word(self.c(q)) {
            q += 1;
        }
        q
    }
    fn find(&self, c: u16, q: usize) -> usize {
        self.src[q.min(self.src.len())..]
            .iter()
            .position(|&v| v == c)
            .map_or(self.src.len(), |n| q + n)
    }
    fn eol(&self, q: usize) -> usize {
        self.find(10, q)
    }
    fn line_end(&self, q: usize) -> usize {
        self.src[q.min(self.src.len())..]
            .iter()
            .position(|&c| matches!(c, 10 | 13 | 0x2028 | 0x2029))
            .map_or(self.src.len(), |n| q + n)
    }
    fn emit(&mut self, kind: usize, e: usize) {
        self.emit_text(kind, String::from_utf16_lossy(&self.src[self.p..e]), e);
    }
    fn emit_text(&mut self, kind: usize, text: String, e: usize) {
        self.out.push(Token { kind, text, start: self.p });
        self.p = e;
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
    fn direction(&mut self, q: usize) -> Option<(usize, usize)> {
        if q < self.direction_failed_until {
            return None;
        }
        let end = self.line_end(q);
        let mut dirs = [None; 4];
        for p in q..end {
            if self.word("direction", p) {
                let w = self.ws_end(p + 9);
                if w == p + 9 {
                    continue;
                }
                for (i, dir) in ["tb", "bt", "rl", "lr"].iter().enumerate() {
                    if self.word(dir, w) {
                        dirs[i] = Some(w + 2);
                        break;
                    }
                }
            }
        }
        let result = dirs.iter().enumerate().find_map(|(i, &end)| end.map(|end| (i, end)));
        if result.is_none() {
            self.direction_failed_until = end;
        }
        result
    }
    fn stereotype(&self, q: usize) -> Option<(usize, usize)> {
        let end = self.line_end(q);
        for (i, stereo) in ["<<fork>>", "<<join>>", "<<choice>>", "[[fork]]", "[[join]]", "[[choice]]"]
            .iter()
            .enumerate()
        {
            if let Some(at) = (q..end).rev().find(|&at| self.word(stereo, at)) {
                return Some((i, at));
            }
        }
        None
    }
    fn emit_id(&mut self, e: usize) -> bool {
        let at = (self.p..e.saturating_sub(1)).find(|&i| self.c(i) == 37 && self.c(i + 1) == 37);
        if let Some(at) = at {
            if at == self.p {
                self.p = e;
                false
            } else {
                self.emit(t::ID, at);
                true
            }
        } else {
            self.emit(t::ID, e);
            true
        }
    }
    fn finish(mut self, kind: usize) -> Tokens {
        self.out.push(Token {
            kind,
            text: String::new(),
            start: self.p,
        });
        Tokens {
            out: self.out,
            error: self.error,
        }
    }
    fn scan(mut self) -> Tokens {
        let n = self.src.len();
        loop {
            if self.p >= n {
                let kind = match self.state {
                    INITIAL => Some(t::NL),
                    ACC_TITLE => Some(t::acc_title_value),
                    ACC_DESCR => Some(t::acc_descr_value),
                    ACC_MULTI => Some(t::acc_descr_multiline_value),
                    CLASSDEF_ID => Some(t::CLASSDEF_STYLEOPTS),
                    CLASS_STYLE => Some(t::STYLECLASS),
                    STYLEDEF_STYLES => Some(t::STYLEDEF_STYLEOPTS),
                    STATE_STRING => Some(t::STATE_DESCR),
                    FLOATING_NOTE => Some(t::NOTE_TEXT),
                    STATE_ID | FLOATING_NOTE_ID => Some(t::ID),
                    _ => None,
                };
                if let Some(k) = kind {
                    self.emit(k, n);
                }
                return self.finish(t::END);
            }
            let c = self.c(self.p);
            match self.state {
                INITIAL | STRUCT => {
                    let root = self.state == INITIAL;
                    let lc = c | 32;
                    if root {
                        if lc == 99 && self.word("click", self.p) && !word(self.c(self.p + 5)) {
                            self.emit(t::CLICK, self.p + 5);
                            continue;
                        }
                        if lc == 104 && self.word("href", self.p) && !word(self.c(self.p + 4)) {
                            self.emit(t::HREF, self.p + 4);
                            continue;
                        }
                        if c == 34 {
                            let close = self.find(34, self.p + 1);
                            if close < n {
                                self.emit(t::STRING, close + 1);
                                continue;
                            }
                        }
                        if lc == 100 && self.word("default", self.p) && !word(self.c(self.p + 7)) {
                            self.emit(t::DEFAULT, self.p + 7);
                            continue;
                        }
                        if let Some((kind, end)) = self.direction(self.p) {
                            self.emit(t::direction_tb + kind, self.eol(end));
                            continue;
                        }
                        if c == 10 {
                            let mut j = self.p + 1;
                            while self.c(j) == 10 {
                                j += 1;
                            }
                            self.emit(t::NL, j);
                            continue;
                        }
                        if space(c) {
                            self.p = self.ws_end(self.p);
                            continue;
                        }
                    } else if c != 10 && space(c) {
                        let mut j = self.p + 1;
                        while j < n && self.c(j) != 10 && space(self.c(j)) {
                            j += 1;
                        }
                        self.p = j;
                        continue;
                    }
                    if c == 35 || (c == 37 && self.c(self.p + 1) == 37 && self.c(self.p + 2) != 123) {
                        self.p = self.eol(self.p);
                        continue;
                    }
                    if lc == 115 {
                        if root && self.word("scale", self.p) {
                            let e = self.ws_end(self.p + 5);
                            if e > self.p + 5 {
                                self.emit(t::scale, e);
                                self.push(SCALE);
                                continue;
                            }
                        }
                        if self.word("style", self.p) {
                            let e = self.ws_end(self.p + 5);
                            if e > self.p + 5 {
                                self.emit(t::style, e);
                                self.push(STYLE);
                                continue;
                            }
                        }
                        if self.word("state", self.p) {
                            let mut e = self.ws_end(self.p + 5);
                            if e > self.p + 5 {
                                self.p = e;
                                self.push(STATE);
                                continue;
                            }
                            if root && self.word("diagram", self.p + 5) {
                                let from = if self.word("-v2", self.p + 12) { self.p + 15 } else { self.p + 12 };
                                e = self.ws_end(from);
                                if e > from {
                                    self.emit(t::SD, e);
                                    continue;
                                }
                            }
                        }
                    } else if lc == 99 {
                        if self.word("class", self.p) {
                            let def = self.word("def", self.p + 5);
                            let from = if def { self.p + 8 } else { self.p + 5 };
                            let e = self.ws_end(from);
                            if e > from {
                                self.emit(if def { t::classDef } else { t::class }, e);
                                self.push(if def { CLASSDEF } else { CLASS });
                                continue;
                            }
                        }
                    } else if lc == 97 && root && (self.word("acctitle", self.p) || self.word("accdescr", self.p)) {
                        let title = (self.c(self.p + 3) | 32) == 116;
                        let q = self.ws_end(self.p + 8);
                        let d = self.c(q);
                        if d == 58 {
                            self.emit(if title { t::acc_title } else { t::acc_descr }, self.ws_end(q + 1));
                            self.push(if title { ACC_TITLE } else { ACC_DESCR });
                            continue;
                        }
                        if d == 123 && !title {
                            self.p = self.ws_end(q + 1);
                            self.push(ACC_MULTI);
                            continue;
                        }
                    }
                    if root {
                        if c == 123 {
                            self.emit(t::STRUCT_START, self.p + 1);
                            self.push(STRUCT);
                            continue;
                        }
                    } else {
                        if let Some((kind, end)) = self.direction(self.p) {
                            self.emit(t::direction_tb + kind, self.eol(end));
                            continue;
                        }
                        if c == 125 {
                            self.emit(t::STRUCT_STOP, self.p + 1);
                            self.pop();
                            continue;
                        }
                        if c == 10 {
                            self.p += 1;
                            continue;
                        }
                    }
                    if lc == 110 && self.word("note", self.p) {
                        let e = self.ws_end(self.p + 4);
                        if e > self.p + 4 {
                            self.emit(t::note, e);
                            self.push(NOTE);
                            continue;
                        }
                    }
                    if root && lc == 104 && self.word("hide empty description", self.p) && !word(self.c(self.p + 22)) {
                        self.emit(t::HIDE_EMPTY, self.p + 22);
                        continue;
                    }
                    if c == 91 && self.c(self.p + 1) == 42 && self.c(self.p + 2) == 93 {
                        self.emit(t::EDGE_STATE, self.p + 3);
                        continue;
                    }
                    let mut j = self.p;
                    while j < n && !matches!(self.c(j), 58 | 45 | 123) && !space(self.c(j)) {
                        j += 1;
                    }
                    if j > self.p {
                        self.emit_id(j);
                        continue;
                    }
                    if c == 58 {
                        j = self.p + 1;
                        while j < n {
                            let d = self.c(j);
                            if d == 10 || d == 59 {
                                break;
                            }
                            if d == 58 {
                                let after = self.c(j + 1);
                                if j + 1 >= n || matches!(after, 58 | 10 | 59) {
                                    break;
                                }
                                j += 1;
                            }
                            j += 1;
                        }
                        if j > self.p + 1 {
                            self.emit_text(t::DESCR, String::from_utf16_lossy(&self.src[self.p..j]).trim().into(), j);
                            continue;
                        }
                        if self.c(self.p + 1) == 58 && self.c(self.p + 2) == 58 {
                            self.emit(t::STYLE_SEPARATOR, self.p + 3);
                            continue;
                        }
                    } else if c == 45 && self.c(self.p + 1) == 45 {
                        if self.c(self.p + 2) == 62 {
                            self.emit(t::ARROW, self.p + 3);
                            continue;
                        }
                        if !root {
                            self.emit(t::CONCURRENT, self.p + 2);
                            continue;
                        }
                    }
                    if !root {
                        return self.finish(t::ERROR);
                    }
                    self.emit(t::INVALID, self.p + 1);
                }
                STATE => {
                    if c != 10 && space(c) {
                        let mut j = self.p + 1;
                        while j < n && self.c(j) != 10 && space(self.c(j)) {
                            j += 1;
                        }
                        self.p = j;
                        continue;
                    }
                    if c == 35 || (c == 37 && self.c(self.p + 1) == 37 && self.c(self.p + 2) != 123) {
                        self.p = self.eol(self.p);
                        continue;
                    }
                    if let Some((kind, at)) = self.stereotype(self.p) {
                        let choice = kind == 2 || kind == 5;
                        self.emit_text(
                            if choice {
                                t::CHOICE
                            } else if kind % 3 == 0 {
                                t::FORK
                            } else {
                                t::JOIN
                            },
                            String::from_utf16_lossy(&self.src[self.p..at]).trim().into(),
                            at + if choice { 10 } else { 8 },
                        );
                        self.pop();
                        continue;
                    }
                    if c == 34 {
                        self.p += 1;
                        self.push(STATE_STRING);
                        continue;
                    }
                    let q = if c == 10 { self.ws_end(self.p) } else { self.p };
                    if self.word("as", q) {
                        let e = self.ws_end(q + 2);
                        if e > q + 2 {
                            self.emit(t::AS, e);
                            self.push(STATE_ID);
                            continue;
                        }
                    }
                    if c == 10 {
                        self.p += 1;
                        self.pop();
                        continue;
                    }
                    if c == 123 {
                        self.emit(t::STRUCT_START, self.p + 1);
                        self.pop();
                        self.push(STRUCT);
                        continue;
                    }
                    if word(c) {
                        let first = self.word_end(self.p);
                        let second = self.ws_end(first);
                        if second > first && word(self.c(second)) {
                            let brace = self.find(123, second + 1);
                            if brace < n && brace < self.line_end(second) {
                                self.error = Some(format!(
                                    "Error: State name must be a single word. Found: \"{}\"",
                                    String::from_utf16_lossy(&self.src[self.p..brace + 1]).trim()
                                ));
                                return self.finish(t::ERROR);
                            }
                        }
                    }
                    let mut j = self.p + 1;
                    while j < n && self.c(j) != 123 && !space(self.c(j)) {
                        j += 1;
                    }
                    self.emit(t::COMPOSIT_STATE, j);
                }
                STATE_STRING => {
                    if c == 34 {
                        self.p += 1;
                        self.pop();
                    } else {
                        self.emit(t::STATE_DESCR, self.find(34, self.p));
                    }
                }
                STATE_ID => {
                    let mut j = self.p;
                    while j < n && !matches!(self.c(j), 10 | 123) {
                        j += 1;
                    }
                    if self.emit_id(j) {
                        self.pop();
                    }
                }
                SCALE => {
                    if (48..=57).contains(&c) {
                        let mut j = self.p + 1;
                        while j < n && (48..=57).contains(&self.c(j)) {
                            j += 1;
                        }
                        self.emit(t::WIDTH, j);
                        continue;
                    }
                    let q = self.ws_end(self.p);
                    if q > self.p && self.word("width", q) && !word(self.c(q + 5)) {
                        self.p = q + 5;
                        self.pop();
                        continue;
                    }
                    return self.finish(t::ERROR);
                }
                ACC_TITLE | ACC_DESCR => {
                    self.emit(
                        if self.state == ACC_TITLE {
                            t::acc_title_value
                        } else {
                            t::acc_descr_value
                        },
                        self.eol(self.p),
                    );
                    self.pop();
                }
                ACC_MULTI => {
                    if c == 125 {
                        self.p += 1;
                        self.pop();
                    } else {
                        self.emit(t::acc_descr_multiline_value, self.find(125, self.p));
                    }
                }
                CLASSDEF => {
                    let is_default = self.word("default", self.p) && self.ws_end(self.p + 7) > self.p + 7;
                    let j = if is_default { self.p + 7 } else { self.word_end(self.p) };
                    let e = self.ws_end(j);
                    if j == self.p || e == j {
                        return self.finish(t::ERROR);
                    }
                    self.emit(if is_default { t::DEFAULT_CLASSDEF_ID } else { t::CLASSDEF_ID }, e);
                    self.pop();
                    self.push(CLASSDEF_ID);
                }
                CLASSDEF_ID | CLASS_STYLE | STYLEDEF_STYLES => {
                    self.emit(
                        if self.state == CLASSDEF_ID {
                            t::CLASSDEF_STYLEOPTS
                        } else if self.state == CLASS_STYLE {
                            t::STYLECLASS
                        } else {
                            t::STYLEDEF_STYLEOPTS
                        },
                        self.eol(self.p),
                    );
                    self.pop();
                }
                CLASS => {
                    let mut j = self.word_end(self.p);
                    if j == self.p {
                        return self.finish(t::ERROR);
                    }
                    while self.c(j) == 44 {
                        let k = self.ws_end(j + 1);
                        let e = self.word_end(k);
                        if e == k {
                            break;
                        }
                        j = e;
                    }
                    self.emit(t::CLASSENTITY_IDS, j);
                    self.pop();
                    self.push(CLASS_STYLE);
                }
                STYLE => {
                    let mut j = self.p;
                    while j < n && (word(self.c(j)) || self.c(j) == 44) {
                        j += 1;
                    }
                    let e = self.ws_end(j);
                    if j == self.p || e == j {
                        return self.finish(t::ERROR);
                    }
                    self.emit(t::STYLE_IDS, e);
                    self.pop();
                    self.push(STYLEDEF_STYLES);
                }
                NOTE => {
                    if self.word("left of", self.p) && !word(self.c(self.p + 7)) {
                        self.emit(t::left_of, self.p + 7);
                        self.pop();
                        self.push(NOTE_ID);
                        continue;
                    }
                    if self.word("right of", self.p) && !word(self.c(self.p + 8)) {
                        self.emit(t::right_of, self.p + 8);
                        self.pop();
                        self.push(NOTE_ID);
                        continue;
                    }
                    if c == 34 {
                        self.p += 1;
                        self.pop();
                        self.push(FLOATING_NOTE);
                        continue;
                    }
                    return self.finish(t::ERROR);
                }
                NOTE_ID => {
                    let q = self.ws_end(self.p);
                    let mut j = q;
                    while j < n && !matches!(self.c(j), 58 | 45) && !space(self.c(j)) {
                        j += 1;
                    }
                    if j == q {
                        return self.finish(t::ERROR);
                    }
                    if self.emit_id(j) {
                        self.pop();
                        self.push(NOTE_TEXT);
                    }
                }
                NOTE_TEXT => {
                    let q = self.ws_end(self.p);
                    if self.c(q) == 58 {
                        let mut j = q + 1;
                        while j < n && !matches!(self.c(j), 58 | 10 | 59) {
                            j += 1;
                        }
                        if j > q + 1 {
                            self.emit_text(
                                t::NOTE_TEXT,
                                String::from_utf16_lossy(&self.src[(self.p + 2).min(j)..j]).trim().into(),
                                j,
                            );
                            self.pop();
                            continue;
                        }
                    }
                    let mut k = self.find(10, self.p);
                    let mut end = None;
                    while k < n {
                        let e = self.ws_end(k);
                        if self.word("end note", e) && !word(self.c(e + 8)) {
                            end = Some(e);
                            break;
                        }
                        k = self.find(10, e);
                    }
                    let Some(e) = end else { return self.finish(t::ERROR) };
                    self.emit_text(t::NOTE_TEXT, String::from_utf16_lossy(&self.src[self.p..e]).trim().into(), e + 8);
                    self.pop();
                }
                FLOATING_NOTE => {
                    let q = self.ws_end(self.p);
                    if self.word("as", q) {
                        self.emit(t::AS, self.ws_end(q + 2));
                        self.pop();
                        self.push(FLOATING_NOTE_ID);
                        continue;
                    }
                    if c == 34 {
                        self.p += 1;
                        continue;
                    }
                    self.emit(t::NOTE_TEXT, self.find(34, self.p));
                }
                FLOATING_NOTE_ID => {
                    if self.emit_id(self.eol(self.p)) {
                        self.pop();
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}
pub fn tokenize(src: &str) -> Tokens {
    Lexer {
        src: src.encode_utf16().collect(),
        p: 0,
        state: 0,
        stack: vec![],
        out: vec![],
        error: None,
        direction_failed_until: 0,
    }
    .scan()
}
