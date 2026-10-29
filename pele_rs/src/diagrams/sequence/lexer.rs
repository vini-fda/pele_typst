use super::tokens as t;
use crate::util::{is_space16, trim};
pub struct Tokens {
    pub types: Vec<usize>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
}
const INITIAL: u8 = 0;
const ID: u8 = 1;
const ALIAS: u8 = 2;
const LINE: u8 = 3;
const CONFIG: u8 = 4;
const ACC_TITLE: u8 = 5;
const ACC_DESCR: u8 = 6;
const ACC_MULTI: u8 = 7;
const ACTOR_START: u8 = 1;
const ACTOR_REST: u8 = 2;
const ID_BIT: u8 = 4;
const ID_WIDE: u8 = 8;
const LINE_BIT: u8 = 16;
const MENU: u8 = 32;
struct Lexer {
    src: Vec<u16>,
    p: usize,
    state: u8,
    stack: Vec<u8>,
    tokens: Tokens,
}
impl Lexer {
    fn c(&self, p: usize) -> u32 {
        self.src.get(p).map_or(u32::MAX, |&c| c as u32)
    }
    fn text(&self, a: usize, b: usize) -> String {
        String::from_utf16_lossy(&self.src[a.min(self.src.len())..b.min(self.src.len())])
    }
    fn emit(&mut self, t: usize, a: usize, b: usize) {
        let text = self.text(a, b);
        self.emit_text(t, a, b, text);
    }
    fn emit_text(&mut self, t: usize, a: usize, b: usize, text: String) {
        self.tokens.types.push(t);
        self.tokens.texts.push(text);
        self.tokens.starts.push(a);
        self.p = b;
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
    fn eol(&self, p: usize) -> usize {
        (p..self.src.len()).find(|&i| self.c(i) == 10).unwrap_or(self.src.len())
    }
    fn has(&self, p: usize, bit: u8) -> bool {
        let c = self.c(p);
        if c >= 128 {
            return if bit == ID_BIT || bit == MENU { !is_space16(c) } else { true };
        }
        let ch = char::from_u32(c).unwrap();
        let ws = c == 32 || (9..=13).contains(&c);
        match bit {
            ACTOR_START => !"/\\+()<->:\n,;".contains(ch),
            ACTOR_REST => !"+<->:\n,;".contains(ch),
            ID_BIT => !"<>:\n,;@".contains(ch) && !ws,
            ID_WIDE => !"<>:\n,;@".contains(ch),
            LINE_BIT => !"#\n;".contains(ch),
            MENU => !"/\\+()<>:\n,;-".contains(ch) && !ws,
            _ => false,
        }
    }
    fn ci(&self, p: usize, word: &str) -> bool {
        word.bytes().enumerate().all(|(k, w)| {
            let c = self.c(p + k);
            if w.is_ascii_lowercase() {
                (c | 32) == w as u32
            } else {
                c == w as u32
            }
        })
    }
    fn word(&self, word: &str) -> bool {
        self.ci(self.p, word) && !is_word(self.c(self.p + word.len()))
    }
    fn ws(&self, mut p: usize) -> usize {
        while p < self.src.len() && is_space16(self.c(p)) {
            p += 1;
        }
        p
    }
    fn skip_line_space(&mut self) -> bool {
        let mut q = self.p;
        while q < self.src.len() && self.c(q) != 10 && is_space16(self.c(q)) {
            q += 1;
        }
        if q == self.p {
            false
        } else {
            self.p = q;
            true
        }
    }
    fn line_run(&self, mut q: usize) -> usize {
        while q < self.src.len() && self.has(q, LINE_BIT) {
            q += 1;
        }
        q
    }
    fn begin(&mut self, t: usize, len: usize, next: u8) {
        self.emit(t, self.p, self.p + len);
        self.push(next);
    }
    fn keyword(&mut self, lit: &str, t: usize, next: Option<u8>) -> bool {
        if !self.word(lit) {
            return false;
        }
        self.emit(t, self.p, self.p + lit.len());
        if let Some(next) = next {
            self.push(next);
        }
        true
    }
    fn menu(&mut self, lit: &str, t: usize) -> bool {
        if !self.ci(self.p, lit) {
            return false;
        }
        let mut q = self.p + lit.len();
        if !matches!(self.c(q), 32 | 9) {
            return false;
        }
        while matches!(self.c(q), 32 | 9) {
            q += 1;
        }
        if q >= self.src.len() || !self.has(q, MENU) {
            return false;
        }
        self.emit(t, self.p, self.p + lit.len());
        true
    }
    fn arrow_ahead(&self, q: usize) -> bool {
        let c = self.c(q);
        let d = self.c(q + 1);
        if c == 45 {
            if matches!(d, 120 | 88 | 41 | 92 | 47 | 45) {
                return true;
            }
            return d == 124 && matches!(self.c(q + 2), 92 | 47);
        }
        if c == 47 || c == 92 {
            return (d == 124 || d == c) && self.c(q + 2) == 45;
        }
        c == 40 && d == 41
    }
    fn actor(&mut self) {
        let mut q = self.p + 1;
        while q < self.src.len() && self.has(q, ACTOR_START) {
            q += 1;
        }
        while q < self.src.len() && !self.arrow_ahead(q) {
            let mut r = q;
            while self.c(r) == 45 {
                r += 1;
            }
            if r >= self.src.len() || !self.has(r, ACTOR_REST) {
                break;
            }
            r += 1;
            while r < self.src.len() && self.has(r, ACTOR_REST) {
                r += 1;
            }
            q = r;
        }
        self.emit_text(t::ACTOR, self.p, q, trim(&self.text(self.p, q)).into());
    }
    fn number(&mut self) -> bool {
        let mut q = self.p;
        while digit(self.c(q)) {
            q += 1;
        }
        let whole = q > self.p;
        if self.c(q) == 46 && digit(self.c(q + 1)) {
            let r = if digit(self.c(q + 2)) { q + 3 } else { q + 2 };
            if matches!(self.c(r), 32 | 10) {
                self.emit(t::NUM, self.p, r);
                return true;
            }
        }
        if whole && matches!(self.c(q), 32 | 10) {
            self.emit(t::NUM, self.p, q);
            return true;
        }
        false
    }
    fn dash(&mut self) {
        let d = self.c(self.p + 1);
        let e = self.c(self.p + 2);
        let (t, n) = match (d, e) {
            (62, 62) => (t::SOLID_ARROW, 3),
            (62, _) => (t::SOLID_OPEN_ARROW, 2),
            (120 | 88, _) => (t::SOLID_CROSS, 2),
            (41, _) => (t::SOLID_POINT, 2),
            (124, 92) => (t::SOLID_ARROW_TOP, 3),
            (124, 47) => (t::SOLID_ARROW_BOTTOM, 3),
            (92, 92) => (t::STICK_ARROW_TOP, 3),
            (47, 47) => (t::STICK_ARROW_BOTTOM, 3),
            (45, _) => {
                let f = self.c(self.p + 3);
                match (e, f) {
                    (62, 62) => (t::DOTTED_ARROW, 4),
                    (62, _) => (t::DOTTED_OPEN_ARROW, 3),
                    (120 | 88, _) => (t::DOTTED_CROSS, 3),
                    (41, _) => (t::DOTTED_POINT, 3),
                    (124, 92) => (t::SOLID_ARROW_TOP_DOTTED, 4),
                    (124, 47) => (t::SOLID_ARROW_BOTTOM_DOTTED, 4),
                    (92, 92) => (t::STICK_ARROW_TOP_DOTTED, 4),
                    (47, 47) => (t::STICK_ARROW_BOTTOM_DOTTED, 4),
                    _ => (t::MINUS, 1),
                }
            }
            _ => (t::MINUS, 1),
        };
        self.emit(t, self.p, self.p + n);
    }
    fn reverse(&mut self, c: u32) -> bool {
        let d = self.c(self.p + 1);
        if (d != 124 && d != c) || self.c(self.p + 2) != 45 {
            return false;
        }
        let dotted = self.c(self.p + 3) == 45;
        let top = c == 47;
        let t = match (d == 124, dotted, top) {
            (true, true, true) => t::SOLID_ARROW_TOP_REVERSE_DOTTED,
            (true, true, false) => t::SOLID_ARROW_BOTTOM_REVERSE_DOTTED,
            (true, false, true) => t::SOLID_ARROW_TOP_REVERSE,
            (true, false, false) => t::SOLID_ARROW_BOTTOM_REVERSE,
            (false, true, true) => t::STICK_ARROW_TOP_REVERSE_DOTTED,
            (false, true, false) => t::STICK_ARROW_BOTTOM_REVERSE_DOTTED,
            (false, false, true) => t::STICK_ARROW_TOP_REVERSE,
            (false, false, false) => t::STICK_ARROW_BOTTOM_REVERSE,
        };
        self.emit(t, self.p, self.p + if dotted { 4 } else { 3 });
        true
    }
    fn title(&mut self) -> bool {
        if !self.ci(self.p, "title") {
            return false;
        }
        let mut q = self.p + 5;
        let legacy = self.c(q) == 58;
        if legacy {
            q += 1;
        }
        let mut e = q;
        while e < self.src.len() && self.c(e) != 10 && is_space16(self.c(e)) {
            e += 1;
        }
        if e == self.src.len() || self.c(e) == 10 {
            self.p = e;
            return true;
        }
        if !is_space16(self.c(q)) || self.c(q) == 13 || !self.has(q + 1, LINE_BIT) {
            return false;
        }
        self.emit(if legacy { t::KW_LEGACY_TITLE } else { t::KW_TITLE }, self.p, self.line_run(q + 1));
        true
    }
    fn acc(&mut self) -> bool {
        let descr = self.ci(self.p, "accdescr");
        if !descr && !self.ci(self.p, "acctitle") {
            return false;
        }
        let q = self.ws(self.p + 8);
        if self.c(q) == 58 {
            self.emit(if descr { t::KW_ACC_DESCR } else { t::KW_ACC_TITLE }, self.p, self.ws(q + 1));
            self.push(if descr { ACC_DESCR } else { ACC_TITLE });
            return true;
        }
        if descr && self.c(q) == 123 {
            self.p = self.ws(q + 1);
            self.push(ACC_MULTI);
            return true;
        }
        false
    }
    fn keywords(&mut self, c: u32) -> bool {
        match c | 32 {
            97 => {
                self.keyword("actor", t::KW_PARTICIPANT_ACTOR, Some(ID))
                    || self.keyword("alt", t::KW_ALT, Some(LINE))
                    || self.keyword("and", t::KW_AND, Some(LINE))
                    || self.keyword("activate", t::KW_ACTIVATE, Some(ID))
                    || self.acc()
                    || self.keyword("autonumber", t::KW_AUTONUMBER, None)
            }
            98 => self.keyword("box", t::KW_BOX, Some(LINE)) || self.keyword("break", t::KW_BREAK, Some(LINE)),
            99 => self.keyword("create", t::KW_CREATE, None) || self.keyword("critical", t::KW_CRITICAL, Some(LINE)),
            100 => {
                self.keyword("destroy", t::KW_DESTROY, Some(ID))
                    || self.menu("details", t::KW_DETAILS)
                    || self.keyword("deactivate", t::KW_DEACTIVATE, Some(ID))
            }
            101 => self.keyword("else", t::KW_ELSE, Some(LINE)) || self.keyword("end", t::KW_END, None),
            108 => {
                self.keyword("loop", t::KW_LOOP, Some(LINE))
                    || self.keyword("left of", t::KW_LEFT_OF, None)
                    || self.menu("links", t::KW_LINKS)
                    || self.menu("link", t::KW_LINK)
            }
            110 => self.keyword("note", t::KW_NOTE, None),
            111 => {
                self.keyword("opt", t::KW_OPT, Some(LINE))
                    || self.keyword("option", t::KW_OPTION, Some(LINE))
                    || self.keyword("over", t::KW_OVER, None)
                    || self.keyword("off", t::KW_OFF, None)
            }
            112 => {
                self.keyword("participant", t::KW_PARTICIPANT, Some(ID))
                    || self.keyword("par", t::KW_PAR, Some(LINE))
                    || self.keyword("par_over", t::KW_PAR_OVER, Some(LINE))
                    || self.menu("properties", t::KW_PROPERTIES)
            }
            114 => self.keyword("rect", t::KW_RECT, Some(LINE)) || self.keyword("right of", t::KW_RIGHT_OF, None),
            115 => self.keyword("sequencediagram", t::SD, None),
            116 => self.title(),
            _ => false,
        }
    }
    fn initial(&mut self) {
        let c = self.c(self.p);
        if c == 10 {
            let mut q = self.p + 1;
            while self.c(q) == 10 {
                q += 1;
            }
            self.emit(t::NEWLINE, self.p, q);
            return;
        }
        if is_space16(c) {
            self.p = self.ws(self.p + 1);
            return;
        }
        if c == 35 || (c == 37 && self.c(self.p + 1) != 123) {
            self.p = self.eol(self.p);
            return;
        }
        if (digit(c) || c == 46) && self.number() {
            return;
        }
        if c < 128 && (97..=122).contains(&(c | 32)) && self.keywords(c) {
            return;
        }
        if c == 44 {
            self.emit(t::COMMA, self.p, self.p + 1);
        } else if c == 59 {
            self.emit(t::NEWLINE, self.p, self.p + 1);
        } else if self.has(self.p, ACTOR_START) {
            self.actor();
        } else if c == 45 {
            self.dash();
        } else if c == 60 && self.ci(self.p, "<<->>") {
            self.emit(t::BIDIRECTIONAL_SOLID_ARROW, self.p, self.p + 5);
        } else if c == 60 && self.ci(self.p, "<<-->>") {
            self.emit(t::BIDIRECTIONAL_DOTTED_ARROW, self.p, self.p + 6);
        } else if matches!(c, 47 | 92) && self.reverse(c) {
        } else if c == 58 {
            self.emit(t::TXT, self.p, self.line_run(self.p + 1));
        } else if c == 43 {
            self.emit(t::PLUS, self.p, self.p + 1);
        } else if c == 40 && self.c(self.p + 1) == 41 {
            self.emit(t::CENTRAL, self.p, self.p + 2);
        } else {
            self.emit(t::INVALID, self.p, self.p + 1);
        }
    }
    fn id(&mut self) -> bool {
        if self.skip_line_space() {
            return true;
        }
        let c = self.c(self.p);
        if c == 35 {
            self.p = self.eol(self.p);
            return true;
        }
        if c == 64 && self.c(self.p + 1) == 123 {
            self.begin(t::CONFIG_START, 2, CONFIG);
            return true;
        }
        let mut a = self.p;
        while a < self.src.len() && self.has(a, ID_BIT) {
            a += 1;
        }
        if a > self.p {
            let w = self.ws(a);
            if self.c(w) == 64 && self.c(w + 1) == 123 {
                self.emit(t::ACTOR, self.p, a);
                return true;
            }
            if w > a && self.ci(w, "as") && is_space16(self.c(w + 2)) {
                self.emit(t::ACTOR, self.p, a);
                self.push(ALIAS);
                return true;
            }
        }
        let mut b = a;
        while b < self.src.len() && self.has(b, ID_WIDE) {
            b += 1;
        }
        if b > self.p {
            let mut cut = b;
            if b < self.src.len() && !matches!(self.c(b), 10 | 59) {
                cut -= 1;
                while cut > self.p && self.c(cut) != 35 {
                    cut -= 1;
                }
            }
            if cut > self.p {
                self.emit_text(t::ACTOR, self.p, cut, trim(&self.text(self.p, cut)).into());
                self.pop();
                return true;
            }
        }
        if self.c(b) == 60 {
            self.emit(t::INVALID, self.p, self.eol(self.p));
            self.pop();
            return true;
        }
        if c != 10 {
            let e = self.eol(self.p);
            self.emit_text(t::INVALID, self.p, e, trim(&self.text(self.p, e)).into());
            self.pop();
            return true;
        }
        false
    }
    fn step(&mut self) -> bool {
        match self.state {
            INITIAL => {
                self.initial();
                true
            }
            ID => self.id(),
            ALIAS => {
                if self.skip_line_space() {
                    return true;
                }
                if self.c(self.p) == 35 {
                    self.p = self.eol(self.p);
                    return true;
                }
                self.pop();
                self.pop();
                if self.word("as") {
                    self.begin(t::AS, 2, LINE);
                } else {
                    self.emit(t::NEWLINE, self.p, self.p);
                }
                true
            }
            LINE => {
                if self.skip_line_space() {
                    return true;
                }
                if self.c(self.p) == 35 {
                    self.p = self.eol(self.p);
                    return true;
                }
                self.emit(t::KW_RESTOFLINE, self.p, self.line_run(self.p));
                self.pop();
                true
            }
            CONFIG => {
                if self.c(self.p) != 125 {
                    let q = (self.p..self.src.len()).find(|&i| self.c(i) == 125).unwrap_or(self.src.len());
                    self.emit(t::CONFIG_CONTENT, self.p, q);
                } else {
                    let w = self.ws(self.p + 1);
                    let alias = w > self.p + 1 && self.ci(w, "as") && is_space16(self.c(w + 2));
                    self.emit(t::CONFIG_END, self.p, self.p + 1);
                    self.pop();
                    if alias {
                        self.push(ALIAS);
                    } else {
                        self.pop();
                    }
                }
                true
            }
            ACC_TITLE | ACC_DESCR => {
                self.emit(
                    if self.state == ACC_TITLE {
                        t::KW_ACC_TITLE_VALUE
                    } else {
                        t::KW_ACC_DESCR_VALUE
                    },
                    self.p,
                    self.eol(self.p),
                );
                self.pop();
                true
            }
            _ => {
                if self.c(self.p) == 125 {
                    self.p += 1;
                    self.pop();
                } else {
                    let q = (self.p..self.src.len()).find(|&i| self.c(i) == 125).unwrap_or(self.src.len());
                    self.emit(t::KW_ACC_DESCR_MULTILINE_VALUE, self.p, q);
                }
                true
            }
        }
    }
}
fn digit(c: u32) -> bool {
    (48..=57).contains(&c)
}
fn is_word(c: u32) -> bool {
    digit(c) || (65..=90).contains(&c) || (97..=122).contains(&c) || c == 95
}
pub fn tokenize(src: &str) -> Tokens {
    let mut l = Lexer {
        src: src.encode_utf16().collect(),
        p: 0,
        state: INITIAL,
        stack: Vec::new(),
        tokens: Tokens {
            types: Vec::new(),
            texts: Vec::new(),
            starts: Vec::new(),
        },
    };
    let n = l.src.len();
    while l.p < n {
        if !l.step() {
            l.emit_text(t::ERROR, l.p, l.p, String::new());
            return l.tokens;
        }
    }
    let token = match l.state {
        INITIAL | ALIAS => Some(t::NEWLINE),
        LINE => Some(t::KW_RESTOFLINE),
        ACC_TITLE => Some(t::KW_ACC_TITLE_VALUE),
        ACC_DESCR => Some(t::KW_ACC_DESCR_VALUE),
        ACC_MULTI => Some(t::KW_ACC_DESCR_MULTILINE_VALUE),
        _ => None,
    };
    if let Some(t) = token {
        l.emit(t, n, n);
    }
    l.emit(t::END, n, n);
    l.tokens
}
