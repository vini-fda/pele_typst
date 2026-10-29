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
    t.bytes().enumerate().all(|(i, c)| at(s, p + i) | 32 == (c as u32 | 32))
}
fn run(s: &[u16], mut p: usize) -> usize {
    while is_space16(at(s, p)) {
        p += 1;
    }
    p
}
fn digits(s: &[u16], mut p: usize) -> usize {
    while digit(at(s, p)) {
        p += 1;
    }
    p
}
fn line_end(s: &[u16], p: usize) -> usize {
    (p..s.len()).find(|&i| s[i] == 10).unwrap_or(s.len())
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
    const END: [T; 9] = [
        T::EOF,
        T::EOF,
        T::EOF,
        T::EOF,
        T::EOF,
        T::STR,
        T::acc_title_value,
        T::acc_descr_value,
        T::acc_descr_multiline_value,
    ];
    let broken = format!(
        "`){}{{ this.pushstate(md_string); }}\n<md_string>(?:(?!`\").)+{}{{ return md_str; }}\n<md_string>(?:`",
        " ".repeat(36),
        " ".repeat(18)
    );
    while p < n {
        let s = &out.src;
        let c = at(s, p);
        if state == 5 || state == 8 {
            let close = if state == 5 { 34 } else { 125 };
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
        if state == 6 || state == 7 {
            let e = line_end(s, p);
            let start = p;
            emit(&mut out, END[state], start, e, &mut p);
            if let Some(old) = stack.pop() {
                state = old;
            }
            continue;
        }
        let next = at(s, p + 1);
        if c == 37 && next == 37 {
            p = line_end(s, p + 2);
            continue;
        }
        if c == 10 || c == 13 {
            if (state == 1 || state == 3) && (c == 10 || next == 10) {
                let e = if c == 10 { p + 1 } else { p + 2 };
                let start = p;
                emit(&mut out, T::NEWLINE, start, e, &mut p);
                if let Some(old) = stack.pop() {
                    state = old;
                }
                continue;
            }
            let mut e = p + 1;
            while matches!(at(s, e), 10 | 13) {
                e += 1;
            }
            let start = p;
            emit(&mut out, T::NEWLINE, start, e, &mut p);
            continue;
        }
        let keyword = |t: &str| word(s, p, t) && !is_word16(at(s, p + t.len()));
        let mut hit = None;
        match c | 32 {
            116 => {
                if keyword("title") {
                    hit = Some((T::title, p + 5, None));
                }
            }
            97 => {
                if word(s, p, "acc") {
                    let descr = word(s, p + 3, "descr");
                    if descr || word(s, p + 3, "title") {
                        let k = run(s, p + 8);
                        let d = at(s, k);
                        if d == 58 {
                            hit = Some((
                                if descr { T::acc_descr } else { T::acc_title },
                                run(s, k + 1),
                                Some(if descr { 7 } else { 6 }),
                            ));
                        } else if d == 123 && descr {
                            p = run(s, k + 1);
                            stack.push(state);
                            state = 8;
                            continue;
                        }
                    }
                }
            }
            120 => {
                if keyword("xychart-beta") {
                    hit = Some((T::XYCHART, p + 12, None));
                } else if keyword("xychart") {
                    hit = Some((T::XYCHART, p + 7, None));
                } else if keyword("x-axis") {
                    hit = Some((T::X_AXIS, p + 6, Some(1)));
                }
            }
            121 => {
                if keyword("y-axis") {
                    hit = Some((T::Y_AXIS, p + 6, Some(1)));
                }
            }
            118 => {
                if word(s, p, "vertical") {
                    hit = Some((T::CHART_ORIENTATION, p + 8, None));
                }
            }
            104 => {
                if word(s, p, "horizontal") {
                    hit = Some((T::CHART_ORIENTATION, p + 10, None));
                }
            }
            108 => {
                if keyword("line") {
                    hit = Some((T::LINE, p + 4, Some(3)));
                }
            }
            98 if keyword("bar") => {
                hit = Some((T::BAR, p + 3, Some(3)));
            }
            _ => {}
        }
        if let Some((t, e, next)) = hit {
            let start = p;
            emit(&mut out, t, start, e, &mut p);
            if let Some(next) = next {
                stack.push(state);
                state = next;
            }
            continue;
        }
        if c == 91 {
            if state == 1 {
                stack.push(state);
                state = 2;
            } else if state == 3 {
                stack.push(state);
                state = 4;
            }
            let start = p;
            emit(&mut out, T::SQUARE_BRACES_START, start, start + 1, &mut p);
            continue;
        }
        if c == 93 {
            if (state == 4 || state == 2)
                && let Some(old) = stack.pop()
            {
                state = old;
            }
            let start = p;
            emit(&mut out, T::SQUARE_BRACES_END, start, start + 1, &mut p);
            continue;
        }
        if state == 1 && c == 45 && next == 45 && at(s, p + 2) == 62 {
            let start = p;
            emit(&mut out, T::ARROW_DELIMITER, start, start + 3, &mut p);
            continue;
        }
        if state == 1 || state == 4 {
            let q = if c == 43 || c == 45 { p + 1 } else { p };
            let d = at(s, q);
            let e = if digit(d) {
                let mut e = digits(s, q + 1);
                if at(s, e) == 46 && digit(at(s, e + 1)) {
                    e = digits(s, e + 2);
                }
                Some(e)
            } else if d == 46 && digit(at(s, q + 1)) {
                Some(digits(s, q + 2))
            } else {
                None
            };
            if let Some(e) = e {
                let start = p;
                emit(&mut out, T::NUMBER_WITH_DECIMAL, start, e, &mut p);
                continue;
            }
        }
        if c == 96 && word(s, p, &broken) {
            p += broken.len();
            if let Some(old) = stack.pop() {
                state = old;
            }
            continue;
        }
        if c == 34 {
            p += 1;
            stack.push(state);
            state = 5;
            continue;
        }
        if is_space16(c) {
            p = run(s, p + 1);
            continue;
        }
        let t;
        let mut e = p + 1;
        if letter(c) {
            while letter(at(s, e)) {
                e += 1;
            }
            t = T::ALPHA;
        } else if digit(c) {
            e = digits(s, e);
            t = T::NUM;
        } else {
            t = match c {
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
                59 => T::SEMI,
                _ => break,
            };
        }
        let start = p;
        emit(&mut out, t, start, e, &mut p);
    }
    if p < n {
        out.types.push(T::ERROR);
        out.texts.push(String::new());
        out.starts.push(p);
        return out;
    }
    out.types.push(END[state]);
    out.texts.push(String::new());
    out.starts.push(n);
    out.types.push(T::END);
    out.texts.push(String::new());
    out.starts.push(n);
    out
}
