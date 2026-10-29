// The agentflow lexer, after pele's diagrams/agentflow/lexer.ts. It reads UTF-16 code units, as
// the JavaScript does, so that offsets and the treatment of surrogates match.

use crate::unicode::is_unicode_letter;
use crate::util::{is_space16 as is_space, is_word16 as is_word};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
pub enum T {
    END,
    ERROR,
    acc_title,
    acc_title_value,
    acc_descr,
    acc_descr_value,
    acc_descr_multiline_value,
    SHAPE_DATA,
    CALLBACKNAME,
    CALLBACKARGS,
    MD_STR,
    STR,
    STYLE,
    DEFAULT,
    LINKSTYLE,
    INTERPOLATE,
    CLASSDEF,
    CLASS,
    HREF,
    CLICK,
    GRAPH,
    flow,
    connector,
    global,
    end,
    LINK_TARGET,
    NODIR,
    DIR,
    direction_tb,
    direction_bt,
    direction_rl,
    direction_lr,
    direction_td,
    LINK_ID,
    NUM,
    BRKT,
    STYLE_SEPARATOR,
    COLON,
    AMP,
    SEMI,
    COMMA,
    MULT,
    LINK,
    START_LINK,
    EDGE_TEXT,
    ELLIPSE_END,
    TEXT,
    ELLIPSE_START,
    STADIUMEND,
    STADIUMSTART,
    SUBROUTINEEND,
    SUBROUTINESTART,
    VERTEX_WITH_PROPS_START,
    TAGEND,
    CYLINDEREND,
    CYLINDERSTART,
    DOUBLECIRCLEEND,
    DOUBLECIRCLESTART,
    TRAPEND,
    INVTRAPEND,
    TRAPSTART,
    INVTRAPSTART,
    TAGSTART,
    UP,
    SEP,
    DOWN,
    COMMENT,
    NODE_STRING,
    MINUS,
    UNICODE_TEXT,
    PIPE,
    PE,
    PS,
    SQE,
    SQS,
    DIAMOND_STOP,
    DIAMOND_START,
    NEWLINE,
    SPACE,
    EOF,
}
pub const TOKEN_NAMES: &[&str] = &[
    "$end",
    "INVALID",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "SHAPE_DATA",
    "CALLBACKNAME",
    "CALLBACKARGS",
    "MD_STR",
    "STR",
    "STYLE",
    "DEFAULT",
    "LINKSTYLE",
    "INTERPOLATE",
    "CLASSDEF",
    "CLASS",
    "HREF",
    "CLICK",
    "GRAPH",
    "flow",
    "connector",
    "global",
    "end",
    "LINK_TARGET",
    "NODIR",
    "DIR",
    "direction_tb",
    "direction_bt",
    "direction_rl",
    "direction_lr",
    "direction_td",
    "LINK_ID",
    "NUM",
    "BRKT",
    "STYLE_SEPARATOR",
    "COLON",
    "AMP",
    "SEMI",
    "COMMA",
    "MULT",
    "LINK",
    "START_LINK",
    "EDGE_TEXT",
    "-)",
    "TEXT",
    "(-",
    "STADIUMEND",
    "STADIUMSTART",
    "SUBROUTINEEND",
    "SUBROUTINESTART",
    "VERTEX_WITH_PROPS_START",
    "TAGEND",
    "CYLINDEREND",
    "CYLINDERSTART",
    "DOUBLECIRCLEEND",
    "DOUBLECIRCLESTART",
    "TRAPEND",
    "INVTRAPEND",
    "TRAPSTART",
    "INVTRAPSTART",
    "TAGSTART",
    "UP",
    "SEP",
    "DOWN",
    "COMMENT",
    "NODE_STRING",
    "MINUS",
    "UNICODE_TEXT",
    "PIPE",
    "PE",
    "PS",
    "SQE",
    "SQS",
    "DIAMOND_STOP",
    "DIAMOND_START",
    "NEWLINE",
    "SPACE",
    "EOF",
];

#[derive(Debug, Clone, Copy, Default)]
pub struct Loc {
    pub first_line: usize,
    pub first_column: usize,
    pub last_line: usize,
    pub last_column: usize,
}
pub struct Tokens {
    pub types: Vec<T>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
    pub locs: Vec<Loc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
enum S {
    INITIAL,
    string,
    md_string,
    acc_title,
    acc_descr,
    acc_descr_multiline,
    dir,
    text,
    ellipseText,
    trapText,
    edgeText,
    click,
    callbackname,
    callbackargs,
    shapeData,
    shapeDataStr,
}

const NONE: u32 = u32::MAX;
fn node_char(c: u32) -> bool {
    c < 128 && (c as u8 as char).is_ascii_alphanumeric()
        || matches!(c, 33 | 34 | 35 | 36 | 37 | 38 | 39 | 42 | 43 | 46 | 96 | 63 | 92 | 95 | 47)
}

struct Lexer<'a> {
    src: &'a [u16],
    n: usize,
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
    locs: Vec<Loc>,
    line: usize,
    col: usize,
    scanned: usize,
    stack: Vec<S>,
    state: S,
    p: usize,
    first_graph: bool,
    dir_next: Option<usize>,
    at_next: Option<usize>,
    line_end: isize,
    dir_failed: usize,
    run_end: usize,
    run_at: isize,
    ws_from: isize,
    ws_to: isize,
    no_arrow: Option<usize>,
    no_link: Option<usize>,
}

fn index_of(src: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    if needle.len() > src.len() {
        return None;
    }
    (from..=src.len() - needle.len()).find(|&i| &src[i..i + needle.len()] == needle)
}

fn u16s(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

impl Lexer<'_> {
    fn cc(&self, i: usize) -> u32 {
        if i < self.n { self.src[i] as u32 } else { NONE }
    }

    fn starts_with(&self, word: &str, at: usize) -> bool {
        let w: Vec<u16> = word.encode_utf16().collect();
        at + w.len() <= self.n && self.src[at..at + w.len()] == w[..]
    }

    fn advance(&mut self, e: usize, chars: bool) {
        let mut k = self.scanned;
        let mut brk = None;
        while k < e {
            let c = self.cc(k);
            if c == 10 || c == 13 {
                if !chars && c == 13 && k + 1 < e && self.cc(k + 1) == 10 {
                    k += 1;
                }
                self.line += 1;
                brk = Some(k);
            }
            k += 1;
        }
        if let Some(brk) = brk {
            let mut k = brk + 1;
            while k < e && (chars || !matches!(self.cc(k), 0x2028 | 0x2029)) {
                k += 1;
            }
            self.col = k - brk - 1;
        } else {
            self.col += e - self.scanned;
        }
        self.scanned = e;
    }
    fn emit_mode(&mut self, kind: T, s: usize, e: usize, chars: bool) {
        self.advance(s, false);
        let first_line = self.line;
        let first_column = self.col;
        self.advance(e, chars);
        self.types.push(kind);
        self.texts.push(String::from_utf16_lossy(&self.src[s..e]));
        self.starts.push(s);
        self.locs.push(Loc {
            first_line,
            first_column,
            last_line: self.line,
            last_column: self.col,
        });
        self.p = e;
    }
    fn emit(&mut self, kind: T, s: usize, e: usize) {
        self.emit_mode(kind, s, e, false);
    }
    fn emit_chars(&mut self, kind: T, s: usize, e: usize) {
        self.emit_mode(kind, s, e, true);
    }
    fn emit_text(&mut self, kind: T, s: usize, e: usize, text: String) {
        self.emit(kind, s, e);
        *self.texts.last_mut().unwrap() = text;
    }
    fn blanks(&self, mut p: usize) -> usize {
        while matches!(self.cc(p), 32 | 9) {
            p += 1;
        }
        p
    }
    fn comment_end(&self, p: usize) -> Option<usize> {
        if self.cc(p) == 37 && self.cc(p + 1) == 37 && self.cc(p + 2) != 123 {
            Some(index_of(self.src, &[10], p + 2).unwrap_or(self.n))
        } else {
            None
        }
    }
    fn push(&mut self, s: S) {
        self.stack.push(self.state);
        self.state = s;
    }

    fn pop(&mut self) {
        if let Some(s) = self.stack.pop() {
            self.state = s;
        }
    }

    fn kw(&self, word: &str) -> bool {
        self.starts_with(word, self.p) && !is_word(self.cc(self.p + word.encode_utf16().count()))
    }

    fn skip_ws(&self, mut j: usize) -> usize {
        while j < self.n && is_space(self.src[j] as u32) {
            j += 1;
        }
        j
    }

    // The sticky regexes of the JavaScript lexer, each returning where its match ends.
    fn re_acc(&self, q: usize, word: &str, sep: u32) -> Option<usize> {
        if !self.starts_with(word, q) {
            return None;
        }
        let j = self.skip_ws(q + word.len());
        if self.cc(j) != sep {
            return None;
        }
        Some(self.skip_ws(j + 1))
    }

    fn re_word_ws(&self, q: usize, word: &str) -> Option<usize> {
        if !self.starts_with(word, q) {
            return None;
        }
        let j = self.skip_ws(q + word.len());
        if j == q + word.len() { None } else { Some(j) }
    }

    fn re_nodir(&self, q: usize) -> Option<usize> {
        let end = self.skip_ws(q);
        (q..end).rev().find(|&i| self.src[i] == 10).map(|i| i + 1)
    }

    fn re_dir(&self, q: usize) -> Option<usize> {
        let j = self.skip_ws(q);
        for code in ["LR", "RL", "TB", "BT", "TD", "BR"] {
            if self.starts_with(code, j) && !is_word(self.cc(j + 2)) {
                return Some(j + 2);
            }
        }
        None
    }

    fn re_dir_sym(&self, q: usize) -> Option<usize> {
        let j = self.skip_ws(q);
        if matches!(self.cc(j), 60 | 62 | 94) { Some(j + 1) } else { None }
    }

    fn re_dir_v(&self, q: usize) -> Option<usize> {
        let j = self.skip_ws(q);
        if self.cc(j) == 118 && !is_word(self.cc(j + 1)) {
            Some(j + 1)
        } else {
            None
        }
    }

    // /.*direction\s+XX[^\n]*/y
    fn re_direction(&self, q: usize, code: &str) -> Option<usize> {
        let mut dot_end = q;
        while dot_end < self.n && !matches!(self.src[dot_end], 10 | 13 | 0x2028 | 0x2029) {
            dot_end += 1;
        }
        let mut k = dot_end as isize;
        while k >= q as isize {
            let ku = k as usize;
            if self.starts_with("direction", ku) {
                let j = self.skip_ws(ku + 9);
                if j > ku + 9 && self.starts_with(code, j) {
                    let mut e = j + 2;
                    while e < self.n && self.src[e] != 10 {
                        e += 1;
                    }
                    return Some(e);
                }
            }
            k -= 1;
        }
        None
    }

    fn line_terminator_from(&self, p: usize) -> usize {
        (p..self.n)
            .find(|&i| matches!(self.src[i], 10 | 13 | 0x2028 | 0x2029))
            .unwrap_or(self.n)
    }

    fn re_newline(&self, q: usize) -> Option<usize> {
        let mut j = q;
        loop {
            if self.cc(j) == 10 {
                j += 1;
            } else if self.cc(j) == 13 && self.cc(j + 1) == 10 {
                j += 2;
            } else {
                break;
            }
        }
        if j > q { Some(j) } else { None }
    }

    fn re_empty_call(&self, q: usize) -> Option<usize> {
        if self.cc(q) != 40 {
            return None;
        }
        let j = self.skip_ws(q + 1);
        if self.cc(j) == 41 { Some(j + 1) } else { None }
    }

    fn ws_end(&mut self, q: usize) -> usize {
        let mut j = q;
        if q as isize >= self.ws_from && q as isize <= self.ws_to {
            j = self.ws_to as usize;
        } else if is_space(self.cc(q)) {
            while j < self.n && is_space(self.src[j] as u32) {
                j += 1;
            }
            self.ws_from = q as isize;
            self.ws_to = j as isize;
        }
        j
    }

    fn quote(&mut self) {
        if self.cc(self.p + 1) == 96 {
            self.push(S::md_string);
            self.p += 2;
        } else {
            self.push(S::string);
            self.p += 1;
        }
    }

    fn opener(&mut self, c: u32) -> bool {
        let p = self.p;
        let d = self.cc(p + 1);
        if c == 40 {
            if d == 45 {
                self.push(S::ellipseText);
                self.emit(T::ELLIPSE_START, p, p + 2);
            } else if d == 91 {
                self.push(S::text);
                self.emit(T::STADIUMSTART, p, p + 2);
            } else if d == 40 && self.cc(p + 2) == 40 {
                self.push(S::text);
                self.emit(T::DOUBLECIRCLESTART, p, p + 3);
            } else {
                self.push(S::text);
                self.emit(T::PS, p, p + 1);
            }
            return true;
        }
        if c == 91 {
            if d == 91 {
                self.push(S::text);
                self.emit(T::SUBROUTINESTART, p, p + 2);
            } else if d == 40 {
                self.push(S::text);
                self.emit(T::CYLINDERSTART, p, p + 2);
            } else if d == 47 {
                self.push(S::trapText);
                self.emit(T::TRAPSTART, p, p + 2);
            } else if d == 92 {
                self.push(S::trapText);
                self.emit(T::INVTRAPSTART, p, p + 2);
            } else {
                self.push(S::text);
                self.emit(T::SQS, p, p + 1);
            }
            return true;
        }
        if c == 124 {
            self.push(S::text);
            self.emit(T::PIPE, p, p + 1);
            return true;
        }
        if c == 123 {
            self.push(S::text);
            self.emit(T::DIAMOND_START, p, p + 1);
            return true;
        }
        false
    }

    fn graph(&mut self, len: usize) {
        if self.first_graph {
            self.first_graph = false;
            self.push(S::dir);
        }
        self.emit(T::GRAPH, self.p, self.p + len);
    }

    fn node_string(&mut self) -> bool {
        let mut q = self.p;
        while q < self.n {
            let ch = self.src[q] as u32;
            if ch < 128 && node_char(ch) {
                q += 1;
            } else if ch == 45 && q + 1 < self.n {
                let d = self.src[q + 1];
                if d == 62 || d == 45 || d == 46 {
                    break;
                }
                q += 1;
            } else {
                break;
            }
        }
        if q == self.p {
            return false;
        }
        self.emit(T::NODE_STRING, self.p, q);
        true
    }

    fn arrow_end(&mut self, p: usize) -> Option<usize> {
        let j = self.ws_end(p);
        if self.no_arrow == Some(j) || self.cc(j) != 45 || self.cc(j + 1) != 45 {
            return None;
        }
        let mut k = j + 2;
        while self.cc(k) == 45 {
            k += 1;
        }
        if matches!(self.cc(k), 120 | 62) {
            Some(self.ws_end(k + 1))
        } else {
            self.no_arrow = Some(j);
            None
        }
    }
    fn link(&mut self) -> bool {
        let j = self.ws_end(self.p);
        if self.no_link == Some(j) || self.cc(j) != 45 {
            return false;
        }
        let d = self.cc(j + 1);
        let mut k = j + 2;
        if d == 45 {
            while self.cc(k) == 45 {
                k += 1;
            }
            if matches!(self.cc(k), 120 | 62) {
                let e = self.ws_end(k + 1);
                self.emit(T::LINK, self.p, e);
            } else {
                self.push(S::edgeText);
                let e = self.ws_end(j + 2);
                self.emit(T::START_LINK, self.p, e);
            }
            return true;
        }
        if d == 46 {
            while self.cc(k) == 46 {
                k += 1;
            }
            if self.cc(k) == 45 {
                let e = self.ws_end(k + 1);
                self.emit(T::LINK, self.p, e);
                return true;
            }
        }
        self.no_link = Some(j);
        false
    }
    fn initial(&mut self, c: u32) -> bool {
        let p = self.p;
        match c {
            97 => {
                if self.starts_with("acc", p) {
                    if let Some(e) = self.re_acc(p, "accTitle", 58) {
                        self.push(S::acc_title);
                        self.emit(T::acc_title, p, e);
                        return true;
                    }
                    if let Some(e) = self.re_acc(p, "accDescr", 58) {
                        self.push(S::acc_descr);
                        self.emit(T::acc_descr, p, e);
                        return true;
                    }
                    if let Some(e) = self.re_acc(p, "accDescr", 123) {
                        self.push(S::acc_descr_multiline);
                        self.p = e;
                        return true;
                    }
                }
                if self.kw("agentflow-beta") {
                    self.graph(14);
                    return true;
                }
            }
            64 => {
                if self.cc(p + 1) == 123 {
                    self.push(S::shapeData);
                    self.emit_text(T::SHAPE_DATA, p, p + 2, String::new());
                    return true;
                }
            }
            99 => {
                if let Some(e) = self.re_word_ws(p, "call") {
                    self.push(S::callbackname);
                    self.p = e;
                    return true;
                }
                if self.kw("classDef") {
                    self.emit(T::CLASSDEF, p, p + 8);
                    return true;
                }
                if self.kw("class") {
                    self.emit(T::CLASS, p, p + 5);
                    return true;
                }
                if let Some(e) = self.re_word_ws(p, "click") {
                    self.push(S::click);
                    self.p = e;
                    return true;
                }
                if self.kw("connector") {
                    self.emit(T::connector, p, p + 9);
                    return true;
                }
            }
            34 => {
                self.quote();
                return true;
            }
            115 => {
                if self.kw("style") {
                    self.emit(T::STYLE, p, p + 5);
                    return true;
                }
            }
            100 => {
                if self.kw("default") {
                    self.emit(T::DEFAULT, p, p + 7);
                    return true;
                }
            }
            108 => {
                if self.kw("linkStyle") {
                    self.emit(T::LINKSTYLE, p, p + 9);
                    return true;
                }
            }
            105 => {
                if self.kw("interpolate") {
                    self.emit(T::INTERPOLATE, p, p + 11);
                    return true;
                }
            }
            104 => {
                if self.starts_with("href", p) && is_space(self.cc(p + 4)) {
                    self.emit(T::HREF, p, p + 5);
                    return true;
                }
            }
            102 => {
                if self.kw("flow") {
                    self.emit(T::flow, p, p + 4);
                    return true;
                }
            }
            103 => {
                if self.kw("global") {
                    self.emit(T::global, p, self.blanks(p + 6));
                    return true;
                }
            }
            101 => {
                if self.kw("end") {
                    self.emit(T::end, p, self.blanks(p + 3));
                    return true;
                }
            }
            95 => {
                for (word, len) in [("_self", 5), ("_blank", 6), ("_parent", 7), ("_top", 4)] {
                    if self.kw(word) {
                        self.emit(T::LINK_TARGET, p, p + len);
                        return true;
                    }
                }
            }
            _ => {}
        }

        if let Some(mut dir_next) = self.dir_next
            && p >= self.dir_failed
        {
            if dir_next < p {
                self.dir_next = index_of(self.src, &u16s("direction"), p);
            }
            if let Some(dn) = self.dir_next {
                dir_next = dn;
                if self.line_end < p as isize {
                    self.line_end = self.line_terminator_from(p) as isize;
                }
                if (dir_next as isize) < self.line_end {
                    let kinds = [T::direction_tb, T::direction_bt, T::direction_rl, T::direction_lr, T::direction_td];
                    for (i, code) in ["TB", "BT", "RL", "LR", "TD"].iter().enumerate() {
                        if let Some(e) = self.re_direction(p, code) {
                            self.emit(kinds[i], p, e);
                            return true;
                        }
                    }
                    // No direction statement starts anywhere in the rest of this line either.
                    self.dir_failed = self.line_end as usize;
                }
            }
        }

        // A link id is a run of non-space characters up to its last `@` that is not followed by `{` or `"`.
        if self.at_next.is_some() && c != 34 && !is_space(c) {
            if p >= self.run_end {
                if self.at_next.is_some_and(|a| a < p) {
                    self.at_next = index_of(self.src, &[64], p);
                }
                let mut q = p;
                while q < self.n {
                    let ch = self.src[q] as u32;
                    if ch == 34 || is_space(ch) {
                        break;
                    }
                    q += 1;
                }
                self.run_end = q;
                self.run_at = -1;
                if let Some(at_next) = self.at_next
                    && at_next < q
                {
                    let mut k = q.min(self.n - 1) as isize - 1;
                    while k >= at_next as isize {
                        let ku = k as usize;
                        if self.src[ku] == 64 {
                            let after = self.cc(ku + 1);
                            if after != 123 && after != 34 {
                                self.run_at = k;
                                break;
                            }
                        }
                        k -= 1;
                    }
                }
            }
            if self.run_at > p as isize {
                self.emit(T::LINK_ID, p, self.run_at as usize + 1);
                return true;
            }
        }

        if (48..=57).contains(&c) {
            let mut q = p + 1;
            while q < self.n && (48..=57).contains(&(self.src[q] as u32)) {
                q += 1;
            }
            self.emit(T::NUM, p, q);
            return true;
        }

        match c {
            35 => {
                self.emit(T::BRKT, p, p + 1);
                return true;
            }
            58 => {
                if self.cc(p + 1) == 58 && self.cc(p + 2) == 58 {
                    self.emit(T::STYLE_SEPARATOR, p, p + 3);
                } else {
                    self.emit(T::COLON, p, p + 1);
                }
                return true;
            }
            38 => {
                self.emit(T::AMP, p, p + 1);
                return true;
            }
            59 => {
                self.emit(T::SEMI, p, p + 1);
                return true;
            }
            44 => {
                self.emit(T::COMMA, p, p + 1);
                return true;
            }
            42 => {
                self.emit(T::MULT, p, p + 1);
                return true;
            }
            60 => {
                self.emit(T::TAGSTART, p, p + 1);
                return true;
            }
            45 => {
                if self.link() || self.node_string() {
                    return true;
                }
                self.emit(T::MINUS, p, p + 1);
                return true;
            }
            40 => return self.opener(c),
            91 => {
                if self.cc(p + 1) == 124 {
                    self.emit(T::VERTEX_WITH_PROPS_START, p, p + 2);
                    return true;
                }
                return self.opener(c);
            }
            124 | 123 => return self.opener(c),
            62 => {
                self.push(S::text);
                self.emit(T::TAGEND, p, p + 1);
                return true;
            }
            94 => {
                self.emit(T::UP, p, p + 1);
                return true;
            }
            92 => {
                if self.cc(p + 1) == 124 {
                    self.emit(T::SEP, p, p + 2);
                    return true;
                }
                return self.node_string();
            }
            118 => {
                if !is_word(self.cc(p + 1)) {
                    self.emit(T::DOWN, p, p + 1);
                    return true;
                }
                return self.node_string();
            }
            37 => {
                if let Some(e) = self.comment_end(p) {
                    self.emit(T::COMMENT, p, e);
                    return true;
                }
                return self.node_string();
            }
            _ => {}
        }

        if is_space(c) {
            if self.link() {
                return true;
            }
            if matches!(c, 32 | 9)
                && let Some(e) = self.comment_end(self.blanks(p))
            {
                self.emit(T::COMMENT, p, e);
                return true;
            }
            if let Some(e) = self.re_newline(p) {
                self.emit(T::NEWLINE, p, e);
            } else {
                self.emit(T::SPACE, p, p + 1);
            }
            return true;
        }
        if c < 128 {
            return self.node_string();
        }
        if is_unicode_letter(c) {
            self.emit(T::UNICODE_TEXT, p, p + 1);
            return true;
        }
        false
    }

    fn edge_text(&mut self, c: u32) -> bool {
        if c == 34 {
            self.quote();
            return true;
        }
        if let Some(e) = self.arrow_end(self.p) {
            self.pop();
            self.emit(T::LINK, self.p, e);
            return true;
        }
        if c == 45 && self.cc(self.p + 1) == 45 {
            return false;
        }
        let mut q = self.p + 1;
        while q < self.n {
            let ch = self.cc(q);
            if ch == 34 || (ch == 45 && self.cc(q + 1) == 45) || (is_space(ch) && self.arrow_end(q).is_some()) {
                break;
            }
            q += 1;
        }
        self.emit_chars(T::EDGE_TEXT, self.p, q);
        true
    }
    fn run(mut self) -> Tokens {
        loop {
            if self.p >= self.n {
                let kind = match self.state {
                    S::INITIAL => Some(T::EOF),
                    S::acc_title => Some(T::acc_title_value),
                    S::acc_descr => Some(T::acc_descr_value),
                    S::acc_descr_multiline => Some(T::acc_descr_multiline_value),
                    S::click => Some(T::CLICK),
                    S::callbackname => Some(T::CALLBACKNAME),
                    S::callbackargs => Some(T::CALLBACKARGS),
                    _ => None,
                };
                if let Some(k) = kind {
                    self.emit(k, self.n, self.n);
                }
                break;
            }
            let c = self.src[self.p] as u32;
            let p = self.p;
            let mut ok = true;
            match self.state {
                S::INITIAL => ok = self.initial(c),
                S::string => {
                    if c == 34 {
                        if self.cc(p + 1) == 96 {
                            self.push(S::md_string);
                            self.p += 2;
                        } else {
                            self.pop();
                            self.p += 1;
                        }
                    } else {
                        let e = index_of(self.src, &[34], p).unwrap_or(self.n);
                        self.emit(T::STR, p, e);
                    }
                }
                S::md_string => {
                    if c == 96 {
                        if self.cc(p + 1) == 34 {
                            self.pop();
                            self.p += 2;
                        } else {
                            ok = false;
                        }
                    } else if c == 34 {
                        self.quote();
                    } else {
                        let mut q = p + 1;
                        while q < self.n && self.src[q] != 96 && self.src[q] != 34 {
                            q += 1;
                        }
                        self.emit(T::MD_STR, p, q);
                    }
                }
                S::acc_title | S::acc_descr => {
                    let e = index_of(self.src, &[10], p).unwrap_or(self.n);
                    let kind = if self.state == S::acc_title {
                        T::acc_title_value
                    } else {
                        T::acc_descr_value
                    };
                    self.pop();
                    self.emit(kind, p, e);
                }
                S::acc_descr_multiline => {
                    if c == 125 {
                        self.pop();
                        self.p += 1;
                    } else {
                        let e = index_of(self.src, &[125], p).unwrap_or(self.n);
                        self.emit(T::acc_descr_multiline_value, p, e);
                    }
                }
                S::dir => {
                    if c == 34 {
                        self.quote();
                    } else if let Some(e) = self.re_nodir(p) {
                        self.pop();
                        self.emit(T::NODIR, p, e);
                    } else if let Some(e) = self.re_dir(p).or_else(|| self.re_dir_sym(p)).or_else(|| self.re_dir_v(p)) {
                        self.pop();
                        self.emit(T::DIR, p, e);
                    } else {
                        ok = self.opener(c);
                    }
                }
                S::text => {
                    let d = self.cc(p + 1);
                    match c {
                        34 => self.quote(),
                        40 | 91 | 123 => {
                            self.opener(c);
                        }
                        93 => {
                            self.pop();
                            if d == 41 {
                                self.emit(T::STADIUMEND, p, p + 2);
                            } else if d == 93 {
                                self.emit(T::SUBROUTINEEND, p, p + 2);
                            } else {
                                self.emit(T::SQE, p, p + 1);
                            }
                        }
                        41 => {
                            self.pop();
                            if d == 93 {
                                self.emit(T::CYLINDEREND, p, p + 2);
                            } else if d == 41 && self.cc(p + 2) == 41 {
                                self.emit(T::DOUBLECIRCLEEND, p, p + 3);
                            } else {
                                self.emit(T::PE, p, p + 1);
                            }
                        }
                        124 => {
                            self.pop();
                            self.emit(T::PIPE, p, p + 1);
                        }
                        125 => {
                            self.pop();
                            self.emit(T::DIAMOND_STOP, p, p + 1);
                        }
                        37 => {
                            if let Some(e) = self.comment_end(p) {
                                self.p = e;
                            } else {
                                ok = false;
                            }
                        }
                        _ => {
                            let mut q = p + 1;
                            while q < self.n && !matches!(self.src[q], 91 | 93 | 40 | 41 | 123 | 125 | 124 | 34 | 37) {
                                q += 1;
                            }
                            self.emit(T::TEXT, p, q);
                        }
                    }
                }
                S::ellipseText => {
                    let d = self.cc(p + 1);
                    if c == 34 {
                        self.quote();
                    } else if c == 37 && self.comment_end(p).is_some() {
                        self.p = self.comment_end(p).unwrap();
                    } else if (c == 45 || c == 47 || c == 41) && d == 41 {
                        self.pop();
                        self.emit(T::ELLIPSE_END, p, p + 2);
                    } else if c == 40 || c == 91 || c == 123 {
                        self.opener(c);
                    } else if c == 41 || c == 93 || c == 125 {
                        ok = false;
                    } else {
                        let mut q = p + 1;
                        while q < self.n {
                            let ch = self.src[q] as u32;
                            if matches!(ch, 34 | 40 | 41 | 91 | 93 | 123 | 125) {
                                break;
                            }
                            if (ch == 45 || ch == 47) && self.cc(q + 1) == 41 {
                                break;
                            }
                            if ch == 37 && self.comment_end(q).is_some() {
                                break;
                            }
                            q += 1;
                        }
                        self.emit_chars(T::TEXT, p, q);
                    }
                }
                S::trapText => {
                    let d = self.cc(p + 1);
                    if c == 34 {
                        self.quote();
                    } else {
                        match c {
                            40 => {
                                if d == 45 || d == 91 || (d == 40 && self.cc(p + 2) == 40) {
                                    self.opener(c);
                                } else if d == 93 {
                                    self.pop();
                                    self.emit(T::TRAPEND, p, p + 2);
                                } else {
                                    self.opener(c);
                                }
                            }
                            91 | 123 => {
                                self.opener(c);
                            }
                            92 => {
                                if d == 93 {
                                    self.pop();
                                    self.emit(T::TRAPEND, p, p + 2);
                                } else {
                                    self.emit(T::TEXT, p, p + 1);
                                }
                            }
                            47 => {
                                if d == 93 {
                                    self.pop();
                                    self.emit(T::INVTRAPEND, p, p + 2);
                                } else {
                                    self.emit(T::TEXT, p, p + 1);
                                }
                            }
                            93 | 41 => {
                                if d == 93 {
                                    self.pop();
                                    self.emit(T::TRAPEND, p, p + 2);
                                } else {
                                    ok = false;
                                }
                            }
                            125 => ok = false,
                            37 => {
                                if let Some(e) = self.comment_end(p) {
                                    self.p = e;
                                } else {
                                    ok = false;
                                }
                            }
                            _ => {
                                if (c == 63 || c == 61) && d == 93 {
                                    self.pop();
                                    self.emit(T::TRAPEND, p, p + 2);
                                } else {
                                    let mut q = p + 1;
                                    while q < self.n && !matches!(self.src[q], 92 | 91 | 93 | 40 | 41 | 123 | 125 | 47 | 37) {
                                        q += 1;
                                    }
                                    self.emit(T::TEXT, p, q);
                                }
                            }
                        }
                    }
                }
                S::edgeText => ok = self.edge_text(c),
                S::click => {
                    if c == 34 {
                        self.quote();
                    } else if is_space(c) {
                        self.pop();
                        self.p += 1;
                    } else {
                        let mut q = p + 1;
                        while q < self.n && !is_space(self.src[q] as u32) {
                            q += 1;
                        }
                        self.emit(T::CLICK, p, q);
                    }
                }
                S::callbackname => {
                    if c == 40 {
                        let e = self.re_empty_call(p);
                        self.pop();
                        if let Some(e) = e {
                            self.p = e;
                        } else {
                            self.push(S::callbackargs);
                            self.p += 1;
                        }
                    } else {
                        let e = index_of(self.src, &[40], p).unwrap_or(self.n);
                        self.emit(T::CALLBACKNAME, p, e);
                    }
                }
                S::callbackargs => {
                    if c == 41 {
                        self.pop();
                        self.p += 1;
                    } else {
                        let e = index_of(self.src, &[41], p).unwrap_or(self.n);
                        self.emit(T::CALLBACKARGS, p, e);
                    }
                }
                S::shapeData => {
                    if c == 34 {
                        self.push(S::shapeDataStr);
                        self.emit(T::SHAPE_DATA, p, p + 1);
                    } else if c == 125 {
                        self.pop();
                        self.p += 1;
                    } else if c == 94 {
                        ok = false;
                    } else {
                        let mut q = p + 1;
                        while q < self.n && !matches!(self.src[q], 125 | 94 | 34) {
                            q += 1;
                        }
                        self.emit(T::SHAPE_DATA, p, q);
                    }
                }
                S::shapeDataStr => {
                    if c == 34 {
                        self.pop();
                        self.emit(T::SHAPE_DATA, p, p + 1);
                    } else {
                        let e = index_of(self.src, &[34], p).unwrap_or(self.n);
                        let raw = String::from_utf16_lossy(&self.src[p..e]);
                        self.emit_text(T::SHAPE_DATA, p, e, br_lines(&raw));
                    }
                }
            }
            if !ok {
                self.emit_text(T::ERROR, self.p, self.p, String::new());
                return Tokens {
                    types: self.types,
                    texts: self.texts,
                    starts: self.starts,
                    locs: self.locs,
                };
            }
        }
        self.emit_text(T::END, self.n, self.n, String::new());
        Tokens {
            types: self.types,
            texts: self.texts,
            starts: self.starts,
            locs: self.locs,
        }
    }
}

// /\n\s*/g replaced with <br/>.
fn br_lines(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            while chars.peek().is_some_and(|&n| crate::util::is_js_space(n)) {
                chars.next();
            }
            out.push_str("<br/>");
        } else {
            out.push(c);
        }
    }
    out
}

pub fn tokenize(src: &[u16]) -> Tokens {
    let n = src.len();
    let lexer = Lexer {
        src,
        n,
        types: Vec::new(),
        texts: Vec::new(),
        starts: Vec::new(),
        locs: vec![],
        line: 1,
        col: 0,
        scanned: 0,
        no_arrow: None,
        no_link: None,
        stack: Vec::new(),
        state: S::INITIAL,
        p: 0,
        first_graph: true,
        dir_next: index_of(src, &u16s("direction"), 0),
        at_next: index_of(src, &[64], 0),
        line_end: -1,
        dir_failed: 0,
        run_end: 0,
        run_at: -1,
        ws_from: -1,
        ws_to: -1,
    };
    lexer.run()
}
