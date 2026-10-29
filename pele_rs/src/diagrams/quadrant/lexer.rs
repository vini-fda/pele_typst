use super::tokens::T;
use crate::util::{is_space16, is_word16};
pub struct Tokens {
    pub types: Vec<T>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
    pub src: Vec<u16>,
}
fn at(s: &[u16], i: usize) -> u32 {
    s.get(i).map_or(u32::MAX, |&c| c as u32)
}
fn digit(c: u32) -> bool {
    (48..=57).contains(&c)
}
fn letter(c: u32) -> bool {
    (65..=90).contains(&c) || (97..=122).contains(&c)
}
fn word(s: &[u16], p: usize, t: &str) -> bool {
    t.bytes().enumerate().all(|(i, c)| at(s, p + i) | 32 == c as u32)
}
fn spaces(s: &[u16], mut p: usize) -> usize {
    while at(s, p) == 32 {
        p += 1;
    }
    p
}
fn run(s: &[u16], mut p: usize) -> usize {
    while is_space16(at(s, p)) {
        p += 1;
    }
    p
}
fn line_end(s: &[u16], p: usize) -> usize {
    (p..s.len()).find(|&i| s[i] == 10).unwrap_or(s.len())
}
fn coordinate(s: &[u16], p: usize) -> Option<usize> {
    let c = at(s, p);
    if c == 49 {
        return Some(p + 1);
    }
    if c != 48 {
        return None;
    }
    let mut e = p + 1;
    if e < s.len() && !matches!(at(s, e), 10 | 13 | 0x2028 | 0x2029) && digit(at(s, e + 1)) {
        e += 2;
        while digit(at(s, e)) {
            e += 1;
        }
    }
    Some(e)
}
fn emit(out: &mut Tokens, t: T, s: usize, e: usize, p: &mut usize) {
    out.types.push(t);
    out.texts.push(String::from_utf16_lossy(&out.src[s..e]));
    out.starts.push(s);
    *p = e;
}
pub fn tokenize(source: &str) -> Tokens {
    let mut out = Tokens {
        types: vec![],
        texts: vec![],
        starts: vec![],
        src: source.encode_utf16().collect(),
    };
    let n = out.src.len();
    let mut stack = vec![];
    let mut state = 0;
    let mut p = 0;
    const END: [T; 6] = [
        T::EOF,
        T::title_value,
        T::acc_title_value,
        T::acc_descr_value,
        T::acc_descr_multiline_value,
        T::STR,
    ];
    while p < n {
        let s = &out.src;
        let c = at(s, p);
        let next = at(s, p + 1);
        match state {
            0 => {}
            1..=3 => {
                let e = line_end(s, p);
                let start = p;
                emit(&mut out, END[state], start, e, &mut p);
                if let Some(old) = stack.pop() {
                    state = old;
                }
                continue;
            }
            4 | 5 => {
                let close = if state == 4 { 125 } else { 34 };
                if c == close {
                    p += 1;
                    if let Some(old) = stack.pop() {
                        state = old;
                    }
                } else {
                    let e = (p..n).find(|&i| s[i] as u32 == close).unwrap_or(n);
                    let start = p;
                    emit(&mut out, END[state], start, e, &mut p);
                }
                continue;
            }
            6 => {
                if c == 96 && next == 34 {
                    p += 2;
                    if let Some(old) = stack.pop() {
                        state = old;
                    }
                    continue;
                }
                if c != 96 && c != 34 {
                    let mut e = p + 1;
                    while e < n && !matches!(at(s, e), 96 | 34) {
                        e += 1;
                    }
                    let start = p;
                    emit(&mut out, T::MD_STR, start, e, &mut p);
                    continue;
                }
                break;
            }
            7 => {
                if is_word16(c) {
                    let mut e = p + 1;
                    while is_word16(at(s, e)) {
                        e += 1;
                    }
                    let start = p;
                    emit(&mut out, T::class_name, start, e, &mut p);
                    if let Some(old) = stack.pop() {
                        state = old;
                    }
                    continue;
                }
                break;
            }
            8 => {
                if let Some(e) = coordinate(s, p) {
                    let start = p;
                    emit(&mut out, T::point_x, start, e, &mut p);
                    stack.push(state);
                    state = 9;
                    continue;
                }
                let k = run(s, p);
                if at(s, k) == 93 {
                    p = spaces(s, k + 1);
                    if let Some(old) = stack.pop() {
                        state = old;
                    }
                    continue;
                }
                break;
            }
            9 => {
                let k = run(s, p);
                if at(s, k) == 44 {
                    p = run(s, k + 1);
                    state = 10;
                    continue;
                }
                break;
            }
            10 => {
                if let Some(e) = coordinate(s, p) {
                    let start = p;
                    emit(&mut out, T::point_y, start, e, &mut p);
                    if let Some(old) = stack.pop() {
                        state = old;
                    }
                    continue;
                }
                break;
            }
            _ => unreachable!(),
        }
        let s = &out.src;
        if c == 37 && next == 37 {
            p = line_end(s, p + 2);
            continue;
        }
        if c == 10 || c == 13 {
            let mut e = p + 1;
            while matches!(at(s, e), 10 | 13) {
                e += 1;
            }
            let start = p;
            emit(&mut out, T::NEWLINE, start, e, &mut p);
            continue;
        }
        let lower = c | 32;
        if lower == 116 && word(s, p, "title") && !is_word16(at(s, p + 5)) {
            let start = p;
            emit(&mut out, T::title, start, start + 5, &mut p);
            stack.push(state);
            state = 1;
            continue;
        }
        if lower == 97 && word(s, p, "acc") {
            let descr = word(s, p + 3, "descr");
            if descr || word(s, p + 3, "title") {
                let k = run(s, p + 8);
                let d = at(s, k);
                if d == 58 {
                    let e = run(s, k + 1);
                    let start = p;
                    emit(&mut out, if descr { T::acc_descr } else { T::acc_title }, start, e, &mut p);
                    stack.push(state);
                    state = if descr { 3 } else { 2 };
                    continue;
                }
                if d == 123 && descr {
                    p = run(s, k + 1);
                    stack.push(state);
                    state = 4;
                    continue;
                }
            }
        }
        let j = if c == 32 { spaces(s, p) } else { p };
        let d = at(s, j);
        let dl = d | 32;
        let mut hit = None;
        if dl == 120 && word(s, j, "x-axis") {
            hit = Some((T::X_AXIS, j + 6));
        } else if dl == 121 && word(s, j, "y-axis") {
            hit = Some((T::Y_AXIS, j + 6));
        } else if d == 45 {
            let mut e = j;
            while at(s, e) == 45 {
                e += 1;
            }
            if e - j >= 2 && at(s, e) == 62 {
                hit = Some((T::DELIMITER, e + 1));
            }
        } else if dl == 113 && word(s, j, "quadrant") {
            let q = at(s, j + 9);
            if at(s, j + 8) == 45 && (49..=52).contains(&q) {
                hit = Some((
                    [T::QUADRANT_1, T::QUADRANT_2, T::QUADRANT_3, T::QUADRANT_4][(q - 49) as usize],
                    j + 10,
                ));
            } else if word(s, j + 8, "chart") {
                hit = Some((T::QUADRANT, j + 13));
            }
        }
        if let Some((t, e)) = hit {
            let e = spaces(s, e);
            let start = p;
            emit(&mut out, t, start, e, &mut p);
            continue;
        }
        if lower == 99 && word(s, p, "classdef") && !is_word16(at(s, p + 8)) {
            let start = p;
            emit(&mut out, T::CLASSDEF, start, start + 8, &mut p);
            continue;
        }
        if c == 34 {
            stack.push(state);
            if next == 96 {
                p += 2;
                state = 6;
            } else {
                p += 1;
                state = 5;
            }
            continue;
        }
        if c == 58 && next == 58 && at(s, p + 2) == 58 {
            p += 3;
            stack.push(state);
            state = 7;
            continue;
        }
        if c == 58 || is_space16(c) {
            let k = if c == 58 { p } else { run(s, p) };
            if at(s, k) == 58 {
                let gap = run(s, k + 1);
                if at(s, gap) == 91 {
                    let e = run(s, gap + 1);
                    let start = p;
                    emit(&mut out, T::point_start, start, e, &mut p);
                    stack.push(state);
                    state = 8;
                    continue;
                }
            }
        }

        let mut e = p + 1;
        let t = if letter(c) {
            while letter(at(s, e)) {
                e += 1;
            }
            T::ALPHA
        } else if c >= 128 {
            while e < n && at(s, e) >= 128 {
                e += 1;
            }
            T::UNICODE_TEXT
        } else if digit(c) {
            while digit(at(s, e)) {
                e += 1;
            }
            T::NUM
        } else {
            match c {
                58 => T::COLON,
                43 => T::PLUS,
                44 => T::COMMA,
                61 => T::EQUALS,
                42 => T::MULT,
                35 => T::BRKT,
                95 => T::UNDERSCORE,
                46 => T::DOT,
                38 => T::AMP,
                45 => T::MINUS,
                32 | 9 | 11 | 12 => T::SPACE,
                59 => T::SEMI,
                33 | 36 | 37 | 39 | 96 | 63 | 92 | 47 => T::PUNCTUATION,
                _ => break,
            }
        };
        let start = p;
        emit(&mut out, t, start, e, &mut p);
    }
    if p < n {
        out.types.push(T::ERROR);
        out.texts.push(String::new());
        out.starts.push(p);
        return out;
    }
    if state < END.len() {
        out.types.push(END[state]);
        out.texts.push(String::new());
        out.starts.push(n);
    }
    out.types.push(T::END);
    out.texts.push(String::new());
    out.starts.push(n);
    out
}
