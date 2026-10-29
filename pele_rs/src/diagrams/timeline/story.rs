use crate::errors::{PeleError, syntax_error};
use crate::text::label::{Label, layout_label};
use crate::text::measurer::TextMeasurer;
use crate::util::{is_space16, trim, trim_start};

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: &'static str,
    pub text: String,
    pub at: usize,
}
fn code(s: &[u16], p: usize) -> u16 {
    s.get(p).copied().unwrap_or(u16::MAX)
}
fn space(s: &[u16], p: usize) -> bool {
    s.get(p).is_some_and(|c| is_space16(*c as u32))
}
fn word(s: &[u16], p: usize) -> bool {
    let c = code(s, p);
    c == 95 || (48..=57).contains(&c) || (65..=90).contains(&c) || (97..=122).contains(&c)
}
fn keyword(s: &[u16], p: usize, w: &str) -> bool {
    w.bytes().enumerate().all(|(i, c)| code(s, p + i) | 32 == c as u16)
}
fn line_end(s: &[u16], mut p: usize) -> usize {
    while p < s.len() && s[p] != 10 {
        p += 1
    }
    p
}
fn text_end(s: &[u16], mut p: usize, colon: bool) -> usize {
    while p < s.len() && !matches!(s[p], 35 | 10 | 59) && (!colon || s[p] != 58) {
        p += 1
    }
    p
}
fn emit(out: &mut Vec<Token>, s: &[u16], kind: &'static str, p: usize, e: usize) {
    out.push(Token {
        kind,
        text: String::from_utf16_lossy(&s[p..e]),
        at: p,
    });
}
pub fn tokenize(source: &str, timeline: bool) -> Vec<Token> {
    let s: Vec<u16> = source.encode_utf16().collect();
    let n = s.len();
    let mut out = Vec::new();
    let mut p = 0;
    loop {
        while p < n {
            let c = s[p];
            if (c == 37 && code(&s, p + 1) != 123) || c == 35 {
                p = line_end(&s, p)
            } else if c == 10 {
                break;
            } else if space(&s, p) {
                p += 1;
                while space(&s, p) {
                    p += 1
                }
            } else {
                break;
            }
        }
        if p >= n {
            break;
        }
        let c = s[p];
        let mut e = p + 1;
        if c == 10 {
            while code(&s, e) == 10 {
                e += 1
            }
            emit(&mut out, &s, "NEWLINE", p, e);
            p = e;
            continue;
        }
        let head = if timeline { "timeline" } else { "journey" };
        if keyword(&s, p, head) {
            let end = p + head.len();
            e = end;
            while matches!(code(&s, e), 32 | 9) {
                e += 1
            }
            if timeline && e > end && (keyword(&s, e, "lr") || keyword(&s, e, "td")) && !word(&s, e + 2) {
                emit(
                    &mut out,
                    &s,
                    if keyword(&s, e, "lr") { "timeline_lr" } else { "timeline_td" },
                    p,
                    e + 2,
                );
                p = e + 2;
                continue;
            }
            if !word(&s, end) {
                emit(&mut out, &s, head, p, end);
                p = end;
                continue;
            }
        }
        if keyword(&s, p, "title") {
            e = p + 5;
            while code(&s, e) != 10 && space(&s, e) {
                e += 1
            }
            if e == n || code(&s, e) == 10 {
                p = e;
                continue;
            }
            if code(&s, p + 5) != 13 && space(&s, p + 5) {
                e = if timeline {
                    line_end(&s, p + 6)
                } else {
                    text_end(&s, p + 6, false)
                };
                if timeline || e > p + 6 {
                    emit(&mut out, &s, "title", p, e);
                    p = e;
                    continue;
                }
            }
        }
        let descr = keyword(&s, p, "accdescr");
        if descr || keyword(&s, p, "acctitle") {
            let mut q = p + 8;
            while space(&s, q) {
                q += 1
            }
            if code(&s, q) == 58 {
                q += 1;
                while space(&s, q) {
                    q += 1
                }
                e = line_end(&s, q);
                emit(&mut out, &s, if descr { "acc_descr" } else { "acc_title" }, p, q);
                emit(&mut out, &s, if descr { "acc_descr_value" } else { "acc_title_value" }, q, e);
                p = e;
                continue;
            }
            if descr && code(&s, q) == 123 {
                q += 1;
                while space(&s, q) {
                    q += 1
                }
                if code(&s, q) != 125 {
                    e = q;
                    while e < n && s[e] != 125 {
                        e += 1
                    }
                    emit(&mut out, &s, "acc_descr_multiline_value", q, e);
                    if e == n && q < n {
                        emit(&mut out, &s, "acc_descr_multiline_value", n, n)
                    }
                    q = e
                }
                p = if q < n { q + 1 } else { q };
                continue;
            }
        }
        if keyword(&s, p, "section") && space(&s, p + 7) {
            e = p + 8;
            if timeline {
                while e < n && s[e] != 58 && s[e] != 10 {
                    e += 1
                }
            } else {
                e = text_end(&s, e, true)
            }
            if e > p + 8 {
                emit(&mut out, &s, "section", p, e);
                p = e;
                continue;
            }
        }
        if timeline {
            if c != 58 {
                e = p + 1;
                while e < n && !matches!(s[e], 35 | 58 | 10) {
                    e += 1
                }
                emit(&mut out, &s, "period", p, e);
                p = e;
                continue;
            }
            if space(&s, p + 1) {
                e = p + 2;
                while e < n && s[e] != 10 && !(s[e] == 58 && space(&s, e + 1)) {
                    e += 1
                }
                if e > p + 2 {
                    emit(&mut out, &s, "event", p, e);
                    p = e;
                    continue;
                }
            }
            emit(&mut out, &s, "INVALID", p, p + 1);
            p += 1;
        } else if c == 58 {
            e = text_end(&s, p + 1, false);
            if e > p + 1 {
                emit(&mut out, &s, "taskData", p, e);
                p = e
            } else {
                emit(&mut out, &s, ":", p, p + 1);
                p += 1
            }
        } else if c == 59 {
            emit(&mut out, &s, "INVALID", p, p + 1);
            p += 1
        } else {
            e = text_end(&s, p + 1, true);
            emit(&mut out, &s, "taskName", p, e);
            p = e
        }
    }
    if out.last().is_none_or(|t| t.at < n) {
        emit(&mut out, &s, "EOF", n, n)
    }
    emit(&mut out, &s, "1", n, n);
    out
}
pub fn fail(kind: &str, source: &str, token: &Token, expected: &str) -> PeleError {
    syntax_error(
        kind,
        &source.encode_utf16().collect::<Vec<_>>(),
        token.at,
        &format!("Expecting {expected}, got '{}'", token.kind),
        false,
    )
}
pub fn squeeze(text: &str) -> String {
    let mut out = String::new();
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        out.push(c);
        if c == '\n' {
            while it.peek().is_some_and(|c| crate::util::is_js_space(*c)) {
                it.next();
            }
        }
    }
    out
}
pub fn plain(raw: Option<&str>, m: &dyn TextMeasurer, size: f64, max: f64, base: u32) -> Label {
    static BR: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let br_pattern = BR.get_or_init(|| regex::Regex::new(r"(?i)^<br\s*/?>").unwrap());
    let text = raw.map(|raw| {
        let mut s = String::new();
        let mut rest = raw;
        while let Some(p) = rest.find('<') {
            s.push_str(&rest[..p]);
            let after = &rest[p..];
            let br = br_pattern.is_match(after);
            s.push_str(if br { "<" } else { "&lt;" });
            rest = &after[1..];
        }
        s.push_str(rest);
        s.replace(":fa-", "&colon;fa-")
    });
    layout_label(text.as_deref(), false, m, size, max, base)
}
pub fn fit(raw: Option<&str>, m: &dyn TextMeasurer, size: f64, max: f64, base: u32) -> Label {
    let label = plain(raw, m, size, max, base);
    let Some(raw) = raw else { return label };
    if label.width <= max {
        return label;
    }
    let re = regex::Regex::new(r"(?is)<br\s*/?>|ﬂ°°?[A-Za-z0-9_]+¶ß|&#?[A-Za-z0-9_]+;|.").unwrap();
    let mut out = String::new();
    let mut width = 0.0;
    for unit in re.find_iter(raw).map(|x| x.as_str()) {
        if trim(unit).is_empty() || (unit.encode_utf16().count() > 2 && unit.starts_with('<')) {
            width = 0.0
        } else {
            let w = m.width(if unit.encode_utf16().count() > 2 { "x" } else { unit }, size, base);
            if width > 0.0 && width + w > max {
                out.push('\n');
                width = 0.0
            }
            width += w
        }
        out.push_str(unit)
    }
    plain(Some(&out), m, size, max, base)
}
pub fn acc(text: &str) -> String {
    trim_start(text).to_string()
}

pub fn substring(text: &str, start: usize) -> String {
    String::from_utf16_lossy(&text.encode_utf16().skip(start).collect::<Vec<_>>())
}
