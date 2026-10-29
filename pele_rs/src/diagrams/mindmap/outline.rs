use crate::errors::{PeleError, syntax_error};
use crate::util::is_space16;
const END: usize = 0;
const ERROR: usize = 1;
const SPACELINE: usize = 2;
const NL: usize = 3;
const EOF: usize = 4;
const KEYWORD: usize = 5;
const SPACELIST: usize = 6;
const ICON: usize = 7;
const CLASS: usize = 8;
const START: usize = 9;
const ID: usize = 10;
const DESCR: usize = 11;
const CLOSE: usize = 12;
const DATA: usize = 13;
const NAMES: [&str; 14] = [
    "1",
    "INVALID",
    "SPACELINE",
    "NL",
    "EOF",
    "",
    "SPACELIST",
    "ICON",
    "CLASS",
    "NODE_DSTART",
    "NODE_ID",
    "NODE_DESCR",
    "NODE_DEND",
    "SHAPE_DATA",
];
#[derive(Debug)]
struct Token {
    kind: usize,
    text: String,
    at: usize,
}
#[derive(Debug)]
pub enum Op {
    Node {
        level: usize,
        id: String,
        descr: String,
        kind: usize,
        data: Option<String>,
    },
    Icon(String),
    Class(String),
}
fn code(s: &[u16], p: usize) -> u16 {
    s.get(p).copied().unwrap_or(u16::MAX)
}
fn letters(s: &[u16], p: usize, w: &str) -> bool {
    w.bytes().enumerate().all(|(i, c)| code(s, p + i) | 32 == c as u16)
}
fn word(c: u16) -> bool {
    c == 95 || (48..=57).contains(&c) || (65..=90).contains(&c) || (97..=122).contains(&c)
}
fn line(s: &[u16], mut p: usize) -> usize {
    while p < s.len() && !matches!(s[p], 10 | 13 | 0x2028 | 0x2029) {
        p += 1
    }
    p
}
fn upto(s: &[u16], mut p: usize, c: u16) -> usize {
    while p < s.len() && s[p] != c {
        p += 1
    }
    p
}
fn tokenize(source: &str, kanban: bool) -> Vec<Token> {
    static BREAKS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let breaks = BREAKS.get_or_init(|| regex::Regex::new(r"\n\s*").unwrap());
    let s: Vec<u16> = source.encode_utf16().collect();
    let n = s.len();
    let mut out = Vec::new();
    let head = if kanban { "kanban" } else { "mindmap" };
    let mut state = 0;
    let mut p = 0;
    let mut next_break: Option<usize> = None;
    while p < n {
        let c = s[p];
        let d = code(&s, p + 1);
        let mut e;
        let mut kind = None;
        let mut text = None;
        let mut next = state;
        match state {
            0 => {
                let mut w = p;
                if c <= 32 || c >= 160 {
                    while w < n && is_space16(s[w] as u32) {
                        w += 1
                    }
                }
                if kanban && c == 64 && d == 123 {
                    kind = Some(DATA);
                    text = Some(String::new());
                    e = p + 2;
                    next = 6
                } else if code(&s, w) == 37 && code(&s, w + 1) == 37 {
                    kind = Some(SPACELINE);
                    e = line(&s, w)
                } else if w > p {
                    if next_break.is_none_or(|b| b < p) {
                        next_break = Some(upto(&s, p, 10))
                    }
                    if next_break.unwrap() >= w {
                        kind = Some(SPACELIST);
                        e = w
                    } else {
                        let last = (p..w).rev().find(|&i| s[i] == 10).unwrap_or(p);
                        if last > p {
                            kind = Some(SPACELINE);
                            e = last + 1
                        } else {
                            kind = Some(NL);
                            e = p + 1
                        }
                    }
                } else if letters(&s, p, head) && !word(code(&s, p + head.len())) {
                    kind = Some(KEYWORD);
                    e = p + head.len()
                } else if c == 58 && d == 58 && code(&s, p + 2) == 58 {
                    p += 3;
                    state = 1;
                    continue;
                } else if c == 58 && d == 58 && letters(&s, p + 2, "icon") && code(&s, p + 6) == 40 {
                    p += 7;
                    state = 2;
                    continue;
                } else if (c == 45 && d == 41) || (c == 40 && (d == 45 || d == 40)) || ((c == 41 || c == 123) && d == c) {
                    kind = Some(START);
                    e = p + 2;
                    next = 3
                } else if matches!(c, 41 | 40 | 91) {
                    kind = Some(START);
                    e = p + 1;
                    next = 3
                } else {
                    e = p;
                    while e < n && !matches!(s[e], 40 | 91 | 10 | 41 | 123 | 125) && (!kanban || s[e] != 64) {
                        e += 1
                    }
                    if e > p {
                        kind = Some(ID)
                    }
                }
            }
            3 => {
                if c == 34 {
                    state = if d == 96 { 5 } else { 4 };
                    p += if d == 96 { 2 } else { 1 };
                    continue;
                } else if c == 41 || c == 40 {
                    kind = Some(CLOSE);
                    e = if d == c || (c == 40 && d == 45) { p + 2 } else { p + 1 };
                    next = 0
                } else if c == 93 {
                    kind = Some(CLOSE);
                    e = p + 1;
                    next = 0
                } else if (c == 125 && d == 125) || (c == 45 && d == 41) {
                    kind = Some(CLOSE);
                    e = p + 2;
                    next = 0
                } else {
                    e = p;
                    if c == 125 {
                        e = line(&s, p)
                    } else {
                        while e < n && !matches!(s[e], 41 | 93 | 40 | 125) {
                            e += 1
                        }
                    }
                    if e > p {
                        kind = Some(DESCR)
                    }
                }
            }
            4 => {
                if c == 34 {
                    p += 1;
                    state = 3;
                    continue;
                } else {
                    kind = Some(DESCR);
                    e = upto(&s, p, 34)
                }
            }
            5 => {
                if c == 96 && d == 34 {
                    p += 2;
                    state = 3;
                    continue;
                } else {
                    e = p;
                    while e < n && !matches!(s[e], 96 | 34) {
                        e += 1
                    }
                    if e > p {
                        kind = Some(DESCR)
                    }
                }
            }
            1 => {
                e = line(&s, p);
                if e > p {
                    kind = Some(CLASS);
                    next = 0
                } else if c == 10 {
                    p += 1;
                    state = 0;
                    continue;
                }
            }
            2 => {
                if c == 41 {
                    p += 1;
                    state = 0;
                    continue;
                } else {
                    kind = Some(ICON);
                    e = upto(&s, p, 41)
                }
            }
            6 => {
                if c == 34 {
                    kind = Some(DATA);
                    e = p + 1;
                    next = 7
                } else if c == 125 {
                    p += 1;
                    state = 0;
                    continue;
                } else {
                    e = p;
                    while e < n && !matches!(s[e], 125 | 94 | 34) {
                        e += 1
                    }
                    if e > p {
                        kind = Some(DATA)
                    }
                }
            }
            _ => {
                if c == 34 {
                    kind = Some(DATA);
                    e = p + 1;
                    next = 6
                } else {
                    e = upto(&s, p, 34);
                    kind = Some(DATA);
                    let raw = String::from_utf16_lossy(&s[p..e]);
                    text = Some(breaks.replace_all(&raw, "<br/>").into_owned())
                }
            }
        }
        let Some(kind) = kind else { break };
        out.push(Token {
            kind,
            text: text.unwrap_or_else(|| String::from_utf16_lossy(&s[p..e])),
            at: p,
        });
        p = e;
        state = next;
    }
    if p < n {
        out.push(Token {
            kind: ERROR,
            text: String::new(),
            at: p,
        })
    } else {
        if state == 0 {
            out.push(Token {
                kind: EOF,
                text: String::new(),
                at: n,
            })
        }
        out.push(Token {
            kind: END,
            text: String::new(),
            at: n,
        })
    }
    out
}
pub fn parse(source: &str, kanban: bool, mut apply: impl FnMut(Op) -> Result<(), PeleError>) -> Result<(), PeleError> {
    let tokens = tokenize(source, kanban);
    let src: Vec<u16> = source.encode_utf16().collect();
    let diagram = if kanban { "kanban" } else { "mindmap" };
    let data = if kanban { ", 'SHAPE_DATA'" } else { "" };
    let fallback = Token {
        kind: END,
        text: String::new(),
        at: src.len(),
    };
    let get = |i: usize| tokens.get(i).unwrap_or(&fallback);
    let stop = |k: usize| matches!(k, SPACELINE | NL | EOF);
    let fail = |i: usize, expected: &str| {
        let t = get(i);
        syntax_error(
            diagram,
            &src,
            t.at,
            &format!(
                "Expecting {expected}, got '{}'",
                if t.kind == KEYWORD {
                    if kanban { "KANBAN" } else { "MINDMAP" }
                } else {
                    NAMES[t.kind]
                }
            ),
            t.kind == ERROR,
        )
    };
    let keyword = if kanban { "'KANBAN'" } else { "'MINDMAP'" };
    let mut i = 0;
    if get(i).kind == SPACELINE {
        i += 1;
        while matches!(get(i).kind, SPACELINE | NL) {
            i += 1
        }
        if get(i).kind != KEYWORD {
            return Err(fail(i, &format!("'SPACELINE', 'NL', {keyword}")));
        }
    } else if get(i).kind != KEYWORD {
        return Err(fail(i, &format!("'SPACELINE', {keyword}")));
    }
    i += 1;
    let mut first = "'SPACELINE', 'NL', 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'";
    if get(i).kind == NL {
        i += 1;
        first = "'SPACELINE', 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'"
    }
    loop {
        let mut k = get(i).kind;
        let mut level = 0;
        let indented = k == SPACELIST;
        if indented {
            level = get(i).text.encode_utf16().count();
            i += 1;
            k = get(i).kind
        }
        if k == ID || k == START {
            let mut id = None;
            let descr;
            let mut kind = 0;
            if k == ID {
                id = Some(get(i).text.clone());
                i += 1
            }
            if get(i).kind == START {
                let start = get(i).text.clone();
                i += 1;
                if get(i).kind != DESCR {
                    return Err(fail(i, "'NODE_DESCR'"));
                }
                descr = get(i).text.clone();
                i += 1;
                if get(i).kind != CLOSE {
                    return Err(fail(i, "'NODE_DEND'"));
                }
                let end = get(i).text.clone();
                i += 1;
                if !stop(get(i).kind) && get(i).kind != DATA {
                    return Err(fail(i, &format!("'SPACELINE', 'NL', 'EOF'{data}")));
                }
                kind = match start.as_str() {
                    "[" => 2,
                    "(" => {
                        if end == ")" {
                            1
                        } else {
                            4
                        }
                    }
                    "((" => 3,
                    ")" => 4,
                    "))" => 5,
                    "{{" => 6,
                    _ => 0,
                };
            } else {
                if !stop(get(i).kind) && get(i).kind != DATA {
                    return Err(fail(i, &format!("'SPACELINE', 'NL', 'EOF', 'NODE_DSTART'{data}")));
                }
                descr = id.clone().unwrap_or_default()
            }
            let shape_data = if get(i).kind == DATA {
                let mut s = String::new();
                while get(i).kind == DATA {
                    s += &get(i).text;
                    i += 1
                }
                if !stop(get(i).kind) {
                    return Err(fail(i, &format!("'SPACELINE', 'NL', 'EOF'{data}")));
                }
                Some(s)
            } else {
                None
            };
            apply(Op::Node {
                level,
                id: id.unwrap_or_else(|| descr.clone()),
                descr,
                kind,
                data: shape_data,
            })?;
        } else if k == ICON || k == CLASS {
            let text = get(i).text.clone();
            i += 1;
            if !stop(get(i).kind) {
                return Err(fail(i, "'SPACELINE', 'NL', 'EOF'"));
            }
            apply(if k == ICON { Op::Icon(text) } else { Op::Class(text) })?
        } else if indented {
            if !stop(k) {
                return Err(fail(i, "'SPACELINE', 'NL', 'EOF', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'"));
            }
        } else if k == SPACELINE {
            i += 1;
            if !stop(get(i).kind) {
                return Err(fail(i, "'SPACELINE', 'NL', 'EOF'"));
            }
        } else {
            return Err(fail(i, first));
        }
        i += 1;
        while matches!(get(i).kind, NL | EOF) {
            i += 1
        }
        if get(i).kind == END {
            return Ok(());
        }
        if !matches!(get(i).kind, SPACELINE | SPACELIST | ICON | CLASS | START | ID) {
            return Err(fail(
                i,
                "'SPACELINE', 'NL', 'EOF', 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'",
            ));
        }
    }
}
