use super::tokens as t;
use crate::util::{is_word16, trim};
use crate::{PeleError, errors::PeleErrorCode};
pub struct Tokens {
    pub src: Vec<u16>,
    pub types: Vec<usize>,
    pub starts: Vec<usize>,
    pub ends: Vec<usize>,
}
fn at(s: &[u16], p: usize) -> u32 {
    s.get(p).map(|c| *c as u32).unwrap_or(u32::MAX)
}
fn starts(s: &[u16], p: usize, w: &str) -> bool {
    w.encode_utf16().enumerate().all(|(i, c)| s.get(p + i) == Some(&c))
}
fn no_case(s: &[u16], p: usize, w: &str) -> bool {
    w.bytes().enumerate().all(|(i, c)| at(s, p + i) | 32 == c as u32)
}
fn blank(c: u32) -> bool {
    matches!(c, 9 | 32)
}
fn digit(c: u32) -> bool {
    matches!(c, 48..=57)
}
fn letter(c: u32) -> bool {
    matches!(c,65..=90|97..=122)
}
fn keyword(s: &[u16], p: usize) -> usize {
    for (k, w) in [
        (t::USECASE, "usecase-beta"),
        (t::ACTOR, "actor"),
        (t::SYSTEM_BOUNDARY, "systemBoundary"),
        (t::END, "end"),
        (t::DIRECTION, "direction"),
        (t::TD, "TD"),
        (t::TB, "TB"),
        (t::BT, "BT"),
        (t::LR, "LR"),
        (t::RL, "RL"),
        (t::NOTE, "note"),
        (t::FOR, "for"),
        (t::JSON, "json"),
        (t::CLASS_DEF, "classDef"),
        (t::CLASS, "class"),
        (t::STYLE, "style"),
        (t::INCLUDE, "include"),
        (t::EXTEND, "extend"),
        (t::TRUE, "true"),
        (t::FALSE, "false"),
    ] {
        if if k == t::INCLUDE || k == t::EXTEND {
            no_case(s, p, w)
        } else {
            starts(s, p, w)
        } {
            return k;
        }
    }
    0
}
const LENGTH: [usize; 21] = [0, 12, 5, 14, 3, 9, 2, 2, 2, 2, 2, 4, 3, 4, 8, 5, 5, 7, 6, 4, 5];
pub fn location(s: &[u16], offset: usize) -> (usize, usize, usize) {
    let (s, offset) = crate::errors::written(s, offset);
    let mut line = 1;
    let mut start = 0;
    let mut i = 0;
    while i < offset && i < s.len() {
        let c = s[i];
        if matches!(c, 10 | 13) {
            if c == 13 && i + 1 < offset && s.get(i + 1) == Some(&10) {
                i += 1
            }
            line += 1;
            start = i + 1
        }
        i += 1
    }
    (line, offset - start + 1, offset)
}
pub fn location_text(s: &[u16], from: usize, to: usize) -> String {
    let (l, c, a) = location(s, from);
    let (_, _, b) = location(s, to);
    format!("line {l}, column {c} [{a},{b})")
}
fn string_end(s: &[u16], p: usize, q: u32) -> Option<usize> {
    for i in p + 1..s.len() {
        let c = s[i] as u32;
        if c == q {
            return Some(i + 1);
        }
        if matches!(c, 10 | 13) {
            return None;
        }
    }
    None
}
fn starts_token(s: &[u16], p: usize) -> bool {
    let c = at(s, p);
    !matches!(c, 34 | 39) || (c == 34 && at(s, p + 1) == 96) || string_end(s, p, c).is_some()
}
fn lex_error(s: &[u16], p: usize, len: usize) -> PeleError {
    let (l, c, a) = location(s, p);
    let (_, _, b) = location(s, p + len);
    let ch = String::from_utf16_lossy(&s[p..p + 1]);
    let mut e=PeleError::new(format!("Error lexing usecase diagram: unexpected character: ->{ch}<- at offset: {a}, skipped {} characters. at line {l}, column {c} [{a},{b})",b-a),PeleErrorCode::Syntax).of("usecase");
    e.line = l;
    e.column = c;
    e
}
fn emit(out: &mut Tokens, k: usize, start: usize, end: usize) {
    out.types.push(k);
    out.starts.push(start);
    out.ends.push(end)
}
pub fn tokenize(source: &str) -> Result<Tokens, PeleError> {
    let s: Vec<u16> = source.encode_utf16().collect();
    let mut out = Tokens {
        src: Vec::new(),
        types: Vec::new(),
        starts: Vec::new(),
        ends: Vec::new(),
    };
    let mut i = 0;
    let n = s.len();
    let mut line_start = true;
    let mut no_brace = false;
    while i < n {
        let c = at(&s, i);
        if blank(c) {
            i += 1;
            continue;
        }
        let mut kind;
        let mut end = i + 1;
        let mut fresh = false;
        if is_word16(c) {
            kind = 0;
            if c == 97 && line_start && starts(&s, i, "acc") {
                let descr = starts(&s, i + 3, "Descr");
                if descr || starts(&s, i + 3, "Title") {
                    let mut j = i + 8;
                    while blank(at(&s, j)) {
                        j += 1
                    }
                    let next = at(&s, j);
                    if next == 58 {
                        kind = if descr { t::ACC_DESCR_LINE } else { t::ACC_TITLE_LINE };
                        end = j + 1;
                        while end < n && !matches!(at(&s, end), 10 | 13) {
                            end += 1
                        }
                    } else if next == 123 && descr && !no_brace {
                        if let Some(close) = (j + 1..n).find(|&k| s[k] == 125) {
                            kind = t::ACC_DESCR_BLOCK;
                            end = close + 1
                        } else {
                            no_brace = true
                        }
                    }
                }
            } else if c == 106 && starts(&s, i, "json") && blank(at(&s, i + 4)) {
                let mut j = i + 5;
                while blank(at(&s, j)) {
                    j += 1
                }
                let id = j;
                while is_word16(at(&s, j)) {
                    j += 1
                }
                if j > id {
                    while blank(at(&s, j)) {
                        j += 1
                    }
                    if at(&s, j) == 64 {
                        j += 1;
                        while blank(at(&s, j)) {
                            j += 1
                        }
                        if at(&s, j) == 123 {
                            emit(&mut out, t::JSON_DECLARATION_START, i, j);
                            let mut depth = 0;
                            let mut quoted = false;
                            let mut escaped = false;
                            let mut k = j;
                            while k < n {
                                let ch = s[k];
                                if quoted {
                                    if escaped {
                                        escaped = false
                                    } else if ch == 92 {
                                        escaped = true
                                    } else if ch == 34 {
                                        quoted = false
                                    }
                                } else if ch == 34 {
                                    quoted = true
                                } else if ch == 123 {
                                    depth += 1
                                } else if ch == 125 {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                k += 1
                            }
                            i = if k < n { k + 1 } else { n };
                            emit(
                                &mut out,
                                if k < n {
                                    t::JSON_OBJECT_LITERAL
                                } else {
                                    t::UNCLOSED_JSON_OBJECT_LITERAL
                                },
                                j,
                                i,
                            );
                            line_start = false;
                            continue;
                        }
                    }
                }
            }
            if kind == 0 {
                while end < n && is_word16(at(&s, end)) {
                    end += 1
                }
                let key = keyword(&s, i);
                if key != 0 {
                    if key == t::USECASE || end - i == LENGTH[key] {
                        kind = key;
                        end = i + LENGTH[key]
                    } else {
                        kind = t::IDENTIFIER
                    }
                } else if matches!(c, 111 | 120) && at(&s, i + 1) == 45 && at(&s, i + 2) == 45 {
                    kind = if c == 111 { t::BACKWARD_CIRCLE } else { t::BACKWARD_CROSS };
                    end = i + 3
                } else if digit(c) {
                    let mut digits = i + 1;
                    while digit(at(&s, digits)) {
                        digits += 1
                    }
                    if at(&s, digits) == 46 && digit(at(&s, digits + 1)) {
                        kind = t::NUMBER;
                        end = digits + 2;
                        while digit(at(&s, end)) {
                            end += 1
                        }
                        while letter(at(&s, end)) {
                            end += 1
                        }
                    } else {
                        kind = t::IDENTIFIER
                    }
                } else {
                    kind = t::IDENTIFIER;
                    while at(&s, end) == 45 && is_word16(at(&s, end + 1)) {
                        kind = t::CSS_IDENTIFIER;
                        end += 2;
                        while is_word16(at(&s, end)) {
                            end += 1
                        }
                    }
                }
            }
        } else if matches!(c, 10 | 13) {
            kind = t::NEWLINE;
            fresh = true;
            if c == 13 && at(&s, end) == 10 {
                end += 1
            }
        } else if matches!(c, 34 | 39) {
            if c == 34 && at(&s, i + 1) == 96 {
                if let Some(close) = (i + 2..n).find(|&k| starts(&s, k, "`\"")) {
                    kind = t::MARKDOWN_STRING;
                    end = close + 2
                } else {
                    kind = t::UNCLOSED_MARKDOWN_STRING;
                    end = n
                }
            } else {
                kind = t::PLAIN_STRING;
                if let Some(e) = string_end(&s, i, c) {
                    end = e
                } else {
                    let mut resume = i + 1;
                    while resume < n && !starts_token(&s, resume) {
                        resume += 1
                    }
                    return Err(lex_error(&s, i, resume - i));
                }
            }
        } else if c == 37 {
            if line_start && at(&s, end) == 37 {
                kind = t::COMMENT;
                while end < n && !matches!(at(&s, end), 10 | 13) {
                    end += 1
                }
            } else {
                kind = t::PERCENT
            }
        } else if c == 45 {
            if at(&s, end) != 45 {
                kind = t::DASH
            } else {
                while at(&s, end) == 45 {
                    end += 1
                }
                let next = at(&s, end);
                let two = end - i == 2;
                if two && next == 124 && at(&s, end + 1) == 62 {
                    kind = t::GENERALIZATION;
                    end += 2
                } else if next == 62 {
                    kind = t::FORWARD_SOLID;
                    end += 1
                } else if two && matches!(next, 111 | 120) {
                    kind = if next == 111 { t::FORWARD_CIRCLE } else { t::FORWARD_CROSS };
                    end += 1
                } else {
                    kind = t::MARKERLESS_SOLID
                }
            }
        } else if c == 46 {
            let next = at(&s, end);
            if next == 46 && at(&s, end + 1) == 62 {
                kind = t::DEPENDENCY_ARROW;
                end += 2
            } else if digit(next) {
                kind = t::NUMBER;
                while digit(at(&s, end)) {
                    end += 1
                }
                while letter(at(&s, end)) {
                    end += 1
                }
            } else {
                kind = t::DOT
            }
        } else if c == 60 {
            if at(&s, end) == 60 {
                emit(&mut out, t::STEREOTYPE_START, i, i + 2);
                i += 2;
                if i >= n {
                    break;
                }
                if at(&s, i) == 62 && at(&s, i + 1) == 62 {
                    kind = t::STEREOTYPE_END;
                    end = i + 2
                } else {
                    let mut close = i;
                    let mut found = false;
                    while close < n {
                        let ch = s[close];
                        if matches!(ch, 10 | 13) {
                            break;
                        }
                        if ch == 62 && at(&s, close + 1) == 62 {
                            found = true;
                            break;
                        }
                        close += 1
                    }
                    if found && !trim(&String::from_utf16_lossy(&s[i..close])).is_empty() {
                        emit(&mut out, t::STEREOTYPE_TEXT, i, close);
                        kind = t::STEREOTYPE_END;
                        i = close;
                        end = close + 2
                    } else {
                        while close < n && !matches!(at(&s, close), 10 | 13) {
                            close += 1
                        }
                        if close == i {
                            while close < n && matches!(at(&s, close), 10 | 13) {
                                close += 1
                            }
                            return Err(lex_error(&s, i, close - i));
                        }
                        kind = t::UNCLOSED_STEREOTYPE_TEXT;
                        end = close
                    }
                }
            } else if at(&s, end) == 45 && at(&s, end + 1) == 45 {
                kind = t::BACKWARD_SOLID;
                end += 2;
                while at(&s, end) == 45 {
                    end += 1
                }
            } else {
                kind = t::LABEL_PUNCTUATION
            }
        } else if c == 58 {
            if at(&s, end) == 58 && at(&s, end + 1) == 58 {
                kind = t::CLASS_SEPARATOR;
                end += 2
            } else {
                kind = t::COLON
            }
        } else if c == 64 {
            if at(&s, end) == 123 {
                kind = t::METADATA_START;
                end += 1
            } else {
                kind = t::AT
            }
        } else {
            kind = match c {
                123 => t::LBRACE,
                125 => t::RBRACE,
                91 => t::LBRACKET,
                93 => t::RBRACKET,
                40 => t::LPAREN,
                41 => t::RPAREN,
                44 => t::COMMA,
                35 => {
                    while matches!(at(&s,end),48..=57|65..=70|97..=102) {
                        end += 1
                    }
                    if end > i + 1 { t::HASH_COLOR } else { t::CSS_PUNCTUATION }
                }
                92 => {
                    if at(&s, end) == 44 {
                        end += 1;
                        t::CSS_ESCAPED_COMMA
                    } else {
                        t::LABEL_PUNCTUATION
                    }
                }
                33..=126 => {
                    if "!#$&*+/=?^|~".contains(char::from_u32(c).unwrap()) {
                        t::CSS_PUNCTUATION
                    } else {
                        t::LABEL_PUNCTUATION
                    }
                }
                _ => {
                    while end < n {
                        let ch = at(&s, end);
                        if (32..127).contains(&ch) || matches!(ch, 9 | 10 | 13) {
                            break;
                        }
                        end += 1
                    }
                    t::LABEL_SYMBOL
                }
            }
        }
        emit(&mut out, kind, i, end);
        i = end;
        line_start = fresh;
    }
    for _ in 0..3 {
        emit(&mut out, t::EOF, n, n)
    }
    out.src = s;
    Ok(out)
}
