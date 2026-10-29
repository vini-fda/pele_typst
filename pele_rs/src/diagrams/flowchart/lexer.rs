// The flowchart lexer, after pele's diagrams/flowchart/lexer.ts. It reads UTF-16 code units, as
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
    subgraph,
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

pub const TOKEN_NAMES: [&str; 77] = [
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
    "subgraph",
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
    "ELLIPSE_END",
    "TEXT",
    "ELLIPSE_START",
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

pub struct Tokens {
    pub types: Vec<T>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
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
    thickEdgeText,
    dottedEdgeText,
    click,
    callbackname,
    callbackargs,
    shapeData,
    shapeDataStr,
}

const NONE: u32 = u32::MAX;
// The second link character of an edge text that has none; never equal to a read character.
const NO_B: u32 = u32::MAX - 1;

fn node_char(c: u32) -> bool {
    c < 128 && (c as u8 as char).is_ascii_alphanumeric()
        || matches!(c, 33 | 34 | 35 | 36 | 37 | 38 | 39 | 42 | 43 | 46 | 96 | 63 | 92 | 95 | 47)
}

#[derive(Clone, Copy)]
enum LinkRe {
    Link,
    Thick,
    Dotted,
}

struct Lexer<'a> {
    src: &'a [u16],
    n: usize,
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
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

    fn emit(&mut self, kind: T, s: usize, e: usize) {
        self.types.push(kind);
        self.texts.push(String::from_utf16_lossy(&self.src[s..e]));
        self.starts.push(s);
        self.p = e;
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

    fn re_link(&self, q: usize, kind: LinkRe) -> Option<usize> {
        let mut j = self.skip_ws(q);
        if matches!(self.cc(j), 120 | 111 | 60) {
            j += 1;
        }
        let end = match kind {
            LinkRe::Link | LinkRe::Thick => {
                let ch = if matches!(kind, LinkRe::Link) { 45 } else { 61 };
                let mut d = 0;
                while self.cc(j + d) == ch {
                    d += 1;
                }
                if d < 2 {
                    return None;
                }
                let next = self.cc(j + d);
                if next == 120 || next == 111 || next == 62 {
                    j + d + 1
                } else if d >= 3 {
                    j + d
                } else {
                    return None;
                }
            }
            LinkRe::Dotted => {
                if self.cc(j) == 45 && self.cc(j + 1) == 46 {
                    j += 1;
                }
                let mut d = 0;
                while self.cc(j + d) == 46 {
                    d += 1;
                }
                if d == 0 || self.cc(j + d) != 45 {
                    return None;
                }
                let mut e = j + d + 1;
                if matches!(self.cc(e), 120 | 111 | 62) {
                    e += 1;
                }
                e
            }
        };
        Some(self.skip_ws(end))
    }

    fn re_start(&self, q: usize, a: u32, b: u32) -> Option<usize> {
        let mut j = self.skip_ws(q);
        if matches!(self.cc(j), 120 | 111 | 60) {
            j += 1;
        }
        if self.cc(j) == a && self.cc(j + 1) == b {
            Some(self.skip_ws(j + 2))
        } else {
            None
        }
    }

    fn re_invis(&self, q: usize) -> Option<usize> {
        let j = self.skip_ws(q);
        let mut d = 0;
        while self.cc(j + d) == 126 {
            d += 1;
        }
        if d >= 3 { Some(self.skip_ws(j + d)) } else { None }
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

    fn re_space(&self, q: usize) -> Option<usize> {
        let mut j = q;
        while j < self.n && is_space(self.src[j] as u32) && self.src[j] != 10 && self.src[j] != 13 {
            j += 1;
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

    // First character after any run of whitespace starting at q, skipping one optional x, o or <.
    fn link_char(&mut self, q: usize) -> u32 {
        let j = self.ws_end(q);
        let d = self.cc(j);
        if d == 120 || d == 111 || d == 60 { self.cc(j + 1) } else { d }
    }

    fn invis_ahead(&mut self, q: usize) -> bool {
        let j = self.ws_end(q);
        self.cc(j) == 126
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

    fn invis_link(&mut self) -> bool {
        if !self.invis_ahead(self.p) {
            return false;
        }
        match self.re_invis(self.p) {
            Some(e) => {
                self.emit(T::LINK, self.p, e);
                true
            }
            None => false,
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

    fn link(&mut self, c: u32) -> bool {
        let d = self.link_char(self.p);
        let p = self.p;
        if d == 45 {
            if let Some(e) = self.re_link(p, LinkRe::Link) {
                self.emit(T::LINK, p, e);
                return true;
            }
            if let Some(e) = self.re_start(p, 45, 45) {
                self.push(S::edgeText);
                self.emit(T::START_LINK, p, e);
                return true;
            }
            if let Some(e) = self.re_link(p, LinkRe::Dotted) {
                self.emit(T::LINK, p, e);
                return true;
            }
            if let Some(e) = self.re_start(p, 45, 46) {
                self.push(S::dottedEdgeText);
                self.emit(T::START_LINK, p, e);
                return true;
            }
        } else if d == 61 {
            if let Some(e) = self.re_link(p, LinkRe::Thick) {
                self.emit(T::LINK, p, e);
                return true;
            }
            if let Some(e) = self.re_start(p, 61, 61) {
                self.push(S::thickEdgeText);
                self.emit(T::START_LINK, p, e);
                return true;
            }
        } else if d == 46
            && let Some(e) = self.re_link(p, LinkRe::Dotted)
        {
            self.emit(T::LINK, p, e);
            return true;
        }
        c != 45 && c != 61 && c != 46 && self.invis_link()
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
            }
            64 => {
                if self.cc(p + 1) == 123 {
                    self.push(S::shapeData);
                    self.types.push(T::SHAPE_DATA);
                    self.texts.push(String::new());
                    self.starts.push(p);
                    self.p += 2;
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
                if self.kw("swimlane-beta") {
                    self.graph(13);
                    return true;
                }
                if self.kw("subgraph") {
                    self.emit(T::subgraph, p, p + 8);
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
                if self.kw("flowchart-elk") {
                    self.graph(13);
                    return true;
                }
                if self.kw("flowchart") {
                    self.graph(9);
                    return true;
                }
            }
            103 => {
                if self.kw("graph") {
                    self.graph(5);
                    return true;
                }
            }
            101 => {
                if self.kw("end") {
                    let e = self.skip_ws(p + 3);
                    self.emit(T::end, p, e);
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
            120 | 111 => return self.link(c) || self.node_string(),
            60 => {
                if self.link(c) {
                    return true;
                }
                self.emit(T::TAGSTART, p, p + 1);
                return true;
            }
            45 => {
                if self.link(c) || self.node_string() {
                    return true;
                }
                self.emit(T::MINUS, p, p + 1);
                return true;
            }
            61 => return self.link(c),
            46 => return self.link(c) || self.node_string(),
            126 => return self.invis_link(),
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
            _ => {}
        }

        if is_space(c) {
            if self.link(c) {
                return true;
            }
            if let Some(e) = self.re_newline(p) {
                self.emit(T::NEWLINE, p, e);
                return true;
            }
            if let Some(e) = self.re_space(p) {
                self.emit(T::SPACE, p, e);
                return true;
            }
            return false;
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

    fn edge_text(&mut self, c: u32, link_re: LinkRe, a: u32, b: u32) -> bool {
        if c == 34 {
            self.quote();
            return true;
        }
        let d = self.link_char(self.p);
        if (d == a || d == b)
            && let Some(e) = self.re_link(self.p, link_re)
        {
            self.pop();
            self.emit(T::LINK, self.p, e);
            return true;
        }
        if c == a && (a != 45 || self.cc(self.p + 1) == 45) {
            return false;
        }
        let mut q = self.p + 1;
        while q < self.n {
            let ch = self.src[q] as u32;
            if ch == 34 {
                break;
            }
            if ch == a {
                if a != 45 || self.cc(q + 1) == 45 {
                    break;
                }
            } else if ch == 120 || ch == 111 || ch == 60 || ch == b || is_space(ch) {
                let dd = self.link_char(q);
                if (dd == a || dd == b) && self.re_link(q, link_re).is_some() {
                    break;
                }
            }
            q += 1;
        }
        self.emit(T::EDGE_TEXT, self.p, q);
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
                        ok = self.invis_link() || self.opener(c);
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
                        _ => {
                            if (c == 126 || is_space(c)) && self.invis_link() {
                            } else {
                                let mut q = p + 1;
                                while q < self.n && !matches!(self.src[q], 91 | 93 | 40 | 41 | 123 | 125 | 124 | 34) {
                                    q += 1;
                                }
                                self.emit(T::TEXT, p, q);
                            }
                        }
                    }
                }
                S::ellipseText => {
                    let d = self.cc(p + 1);
                    if c == 34 {
                        self.quote();
                    } else if (c == 126 || is_space(c)) && self.invis_link() {
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
                            if (ch == 126 || is_space(ch)) && self.invis_ahead(q) && self.re_invis(q).is_some() {
                                break;
                            }
                            q += 1;
                        }
                        self.emit(T::TEXT, p, q);
                    }
                }
                S::trapText => {
                    let d = self.cc(p + 1);
                    if c == 34 {
                        self.quote();
                    } else if (c == 126 || is_space(c)) && self.invis_link() {
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
                            _ => {
                                if (c == 63 || c == 61) && d == 93 {
                                    self.pop();
                                    self.emit(T::TRAPEND, p, p + 2);
                                } else {
                                    let mut q = p + 1;
                                    while q < self.n && !matches!(self.src[q], 92 | 91 | 93 | 40 | 41 | 123 | 125 | 47) {
                                        q += 1;
                                    }
                                    self.emit(T::TEXT, p, q);
                                }
                            }
                        }
                    }
                }
                S::edgeText => ok = self.edge_text(c, LinkRe::Link, 45, NO_B),
                S::thickEdgeText => ok = self.edge_text(c, LinkRe::Thick, 61, NO_B),
                S::dottedEdgeText => ok = self.edge_text(c, LinkRe::Dotted, 46, 45),
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
                        self.types.push(T::SHAPE_DATA);
                        self.texts.push(br_lines(&raw));
                        self.starts.push(p);
                        self.p = e;
                    }
                }
            }
            if !ok {
                self.types.push(T::ERROR);
                self.texts.push(String::new());
                self.starts.push(self.p);
                return Tokens {
                    types: self.types,
                    texts: self.texts,
                    starts: self.starts,
                };
            }
        }
        self.types.push(T::END);
        self.texts.push(String::new());
        self.starts.push(self.n);
        Tokens {
            types: self.types,
            texts: self.texts,
            starts: self.starts,
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
