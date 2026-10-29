use super::tokens::T;
use crate::util::{is_space16, is_word16};
pub struct Tokens {
    pub types: Vec<T>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
    pub src: Vec<u16>,
}
fn at(s: &[u16], p: usize) -> u32 {
    s.get(p).map_or(u32::MAX, |&c| c as u32)
}
fn word(s: &[u16], p: usize, t: &str) -> bool {
    t.bytes().enumerate().all(|(i, c)| at(s, p + i).to_ascii_lowercase() == c as u32)
}
trait Lower {
    fn to_ascii_lowercase(self) -> Self;
}
impl Lower for u32 {
    fn to_ascii_lowercase(self) -> Self {
        if (65..=90).contains(&self) { self + 32 } else { self }
    }
}
fn run(s: &[u16], mut p: usize) -> usize {
    while is_space16(at(s, p)) {
        p += 1;
    }
    p
}
fn until(s: &[u16], p: usize, c: u16) -> usize {
    (p..s.len()).find(|&i| s[i] == c).unwrap_or(s.len())
}
fn emit(out: &mut Tokens, t: T, start: usize, end: usize, p: &mut usize) {
    out.types.push(t);
    out.texts.push(String::from_utf16_lossy(&out.src[start..end]));
    out.starts.push(start);
    *p = end;
}
pub fn tokenize(source: &str) -> Tokens {
    let mut out = Tokens {
        types: vec![],
        texts: vec![],
        starts: vec![],
        src: source.encode_utf16().collect(),
    };
    let n = out.src.len();
    let mut p = 0;
    let mut state = 0;
    const EOF: [T; 8] = [
        T::EOF,
        T::acc_title_value,
        T::acc_descr_value,
        T::acc_descr_multiline_value,
        T::href,
        T::callbackname,
        T::callbackargs,
        T::click,
    ];
    while p < n {
        let s = &out.src;
        let c = at(s, p);
        let start = p;
        match state {
            1 | 2 => {
                let end = until(s, p, 10);
                emit(&mut out, EOF[state], start, end, &mut p);
                state = 0;
                continue;
            }
            3 | 4 | 6 => {
                let close = if state == 3 {
                    125
                } else if state == 4 {
                    34
                } else {
                    41
                };
                if c == close {
                    state = 0;
                    p += 1;
                } else {
                    let end = until(s, p, close as u16);
                    emit(&mut out, EOF[state], start, end, &mut p);
                }
                continue;
            }
            5 => {
                if c == 40 {
                    let end = run(s, p + 1);
                    if at(s, end) == 41 {
                        state = 0;
                        p = end + 1;
                    } else {
                        state = 6;
                        p += 1;
                    }
                } else {
                    let end = until(s, p, 40);
                    emit(&mut out, T::callbackname, start, end, &mut p);
                }
                continue;
            }
            7 => {
                if is_space16(c) {
                    state = 0;
                    p += 1;
                } else {
                    let mut end = p + 1;
                    while end < n && !is_space16(at(s, end)) {
                        end += 1;
                    }
                    emit(&mut out, T::click, start, end, &mut p);
                }
                continue;
            }
            _ => {}
        }
        if c == 37 {
            if at(s, p + 1) != 37 {
                p = until(s, p + 1, 10);
                while at(s, p) == 10 {
                    p += 1;
                }
            } else if at(s, p + 2) == 123 {
                emit(&mut out, T::open_directive, start, start + 3, &mut p);
                let end = p;
                emit(&mut out, T::ERROR, end, end, &mut p);
                return out;
            } else {
                p = until(s, p + 2, 10);
            }
            continue;
        }
        if c == 10 {
            let mut end = p + 1;
            while at(s, end) == 10 {
                end += 1;
            }
            emit(&mut out, T::NL, start, end, &mut p);
            continue;
        }
        if is_space16(c) {
            p = run(s, p + 1);
            continue;
        }
        let mut matched = false;
        for (key, delimiter, new_state, token) in [
            ("acctitle", 58, 1, Some(T::acc_title)),
            ("accdescr", 58, 2, Some(T::acc_descr)),
            ("accdescr", 123, 3, None),
        ] {
            if word(s, p, key) {
                let end = run(s, p + key.len());
                if at(s, end) == delimiter {
                    let end = run(s, end + 1);
                    state = new_state;
                    if let Some(t) = token {
                        emit(&mut out, t, start, end, &mut p);
                    } else {
                        p = end;
                    }
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }
        let s = &out.src;
        for (key, new_state, quote) in [("href", 4, true), ("call", 5, false), ("click", 7, false)] {
            if word(s, p, key) && is_space16(at(s, p + key.len())) {
                let end = run(s, p + key.len());
                if !quote || at(s, end) == 34 {
                    p = end + usize::from(quote);
                    state = new_state;
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }
        if word(s, p, "title") {
            let mut end = p + 5;
            while is_space16(at(s, end)) && !matches!(at(s, end), 10 | 13) {
                end += 1;
            }
            if end == n || matches!(at(s, end), 10 | 13) {
                p = end;
                continue;
            }
        }
        let mut weekday_match = None;
        'weekday: for (key, days) in [
            (
                "weekday",
                &["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"][..],
            ),
            ("weekend", &["friday", "saturday"][..]),
        ] {
            if word(s, p, key) && is_space16(at(s, p + key.len())) {
                let from = run(s, p + key.len());
                for (i, day) in days.iter().enumerate() {
                    let end = from + day.len();
                    if word(s, from, day) && !is_word16(at(s, end)) {
                        let t = if key == "weekday" {
                            [
                                T::weekday_monday,
                                T::weekday_tuesday,
                                T::weekday_wednesday,
                                T::weekday_thursday,
                                T::weekday_friday,
                                T::weekday_saturday,
                                T::weekday_sunday,
                            ][i]
                        } else {
                            [T::weekend_friday, T::weekend_saturday][i]
                        };
                        weekday_match = Some((t, end));
                        break 'weekday;
                    }
                }
            }
        }
        if let Some((t, end)) = weekday_match {
            emit(&mut out, t, start, end, &mut p);
            continue;
        }
        let s = &out.src;
        for (key, t, mode) in [
            ("axisformat", T::axisFormat, 0),
            ("accdescription", T::accDescription, 0),
            ("gantt", T::gantt, 1),
            ("dateformat", T::dateFormat, 0),
            ("inclusiveenddates", T::inclusiveEndDates, 1),
            ("includes", T::includes, 0),
            ("topaxis", T::topAxis, 1),
            ("tickinterval", T::tickInterval, 0),
            ("todaymarker", T::todayMarker, 2),
            ("title", T::title, 3),
            ("excludes", T::excludes, 0),
            ("section", T::section, 4),
        ] {
            if !word(s, p, key) {
                continue;
            }
            let from = p + key.len();
            let end = if mode == 1 {
                if is_word16(at(s, from)) {
                    continue;
                }
                from
            } else {
                if !is_space16(at(s, from)) || (mode == 3 && matches!(at(s, from), 10 | 13)) {
                    continue;
                }
                let mut end = from + 1;
                while end < n && at(s, end) != 10 && !(mode == 0 && at(s, end) == 35) && !((mode == 0 || mode == 2) && at(s, end) == 59) {
                    end += 1;
                }
                if end == from + 1 {
                    continue;
                }
                end
            };
            emit(&mut out, t, start, end, &mut p);
            matched = true;
            break;
        }
        if matched {
            continue;
        }
        let s = &out.src;
        if (0..10).all(|i| {
            if i == 4 || i == 7 {
                at(s, p + i) == 45
            } else {
                (48..=57).contains(&at(s, p + i))
            }
        }) && !is_word16(at(s, p + 10))
        {
            emit(&mut out, T::date, start, start + 10, &mut p);
            continue;
        }
        if c != 58 {
            let mut end = p + 1;
            while end < n && !matches!(at(s, end), 58 | 10) {
                end += 1;
            }
            emit(&mut out, T::taskTxt, start, end, &mut p);
        } else {
            let mut end = p + 1;
            while end < n && !matches!(at(s, end), 35 | 10 | 59) {
                end += 1;
            }
            emit(&mut out, if end > p + 1 { T::taskData } else { T::COLON }, start, end, &mut p);
        }
    }
    emit(&mut out, EOF[state], n, n, &mut p);
    emit(&mut out, T::END, n, n, &mut p);
    out
}
