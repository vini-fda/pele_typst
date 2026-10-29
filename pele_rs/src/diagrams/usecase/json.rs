use crate::{PeleError, errors::PeleErrorCode};
use indexmap::IndexMap;
#[derive(Clone, Debug)]
pub enum Value {
    Object(IndexMap<String, usize>),
    Array(Vec<usize>),
    Scalar(String),
}
#[derive(Clone, Debug, Default)]
pub struct Data {
    pub values: Vec<Value>,
    pub root: usize,
}
struct Parser<'a> {
    s: Vec<u16>,
    at: usize,
    origin: &'a [u16],
    offset: usize,
    error_offset: Option<usize>,
    data: Data,
}
impl Parser<'_> {
    fn c(&self) -> u16 {
        self.s.get(self.at).copied().unwrap_or(0)
    }
    fn skip(&mut self) {
        while matches!(self.c(), 9 | 10 | 13 | 32) {
            self.at += 1
        }
    }
    fn error(&self, message: String, offset: usize) -> PeleError {
        self.error_units(&message.encode_utf16().collect::<Vec<_>>(), offset)
    }
    fn error_units(&self, message: &[u16], offset: usize) -> PeleError {
        let (line, column, _) = super::lexer::location(self.origin, self.offset + self.error_offset.unwrap_or(offset));
        let units: Vec<u16> = "Invalid JSON: JSON Parse error: "
            .encode_utf16()
            .chain(message.iter().copied())
            .chain(format!(" (line {line}, column {column})").encode_utf16())
            .collect();
        let mut e = PeleError::new(String::from_utf16_lossy(&units), PeleErrorCode::Syntax).of("usecase");
        if String::from_utf16(&units).is_err() {
            e.message_utf16 = Some(Box::new(units));
        }
        e.line = line;
        e.column = column;
        e
    }
    fn fail(&self, message: &str) -> PeleError {
        self.error(message.into(), self.at)
    }
    // JSON.parse lexes the next token before it reports a grammar mismatch.
    fn syntax(&mut self, message: &str) -> PeleError {
        let c = self.c();
        if c == 39 {
            return self.fail("Single quotes (') are not allowed in JSON");
        }
        if self.at < self.s.len() && !matches!(c, 34 | 36 | 40 | 41 | 44 | 45 | 48..=59 | 61 | 65..=91 | 93 | 95 | 97..=123 | 125) {
            let units: Vec<_> = "Unrecognized token '".encode_utf16().chain([c]).chain("'".encode_utf16()).collect();
            return self.error_units(&units, self.at);
        }
        if matches!(c, 34 | 45 | 48..=57) {
            let at = self.at;
            let previous = self.error_offset.replace(at);
            let token = self.scalar(message);
            self.at = at;
            self.error_offset = previous;
            if let Err(error) = token {
                return error;
            }
        }
        self.fail(message)
    }
    fn string(&mut self) -> Result<String, PeleError> {
        self.at += 1;
        let mut out = Vec::new();
        while self.at < self.s.len() {
            let pos = self.at;
            let ch = self.c();
            self.at += 1;
            if ch == 34 {
                return Ok(String::from_utf16_lossy(&out));
            }
            if ch < 32 {
                return Err(self.error("Unterminated string".into(), pos));
            }
            if ch != 92 {
                out.push(ch);
                continue;
            }
            let escape_at = self.at;
            let escape = self.c();
            self.at += 1;
            out.push(match escape {
                34 | 92 | 47 => escape,
                98 => 8,
                102 => 12,
                110 => 10,
                114 => 13,
                116 => 9,
                117 => {
                    let from = self.at;
                    let mut value = 0u16;
                    for _ in 0..4 {
                        let c = self.c();
                        let d = match c {
                            48..=57 => c - 48,
                            65..=70 => c - 55,
                            97..=102 => c - 87,
                            _ => {
                                let units: Vec<_> = "\"\\u"
                                    .encode_utf16()
                                    .chain(self.s[from..(from + 4).min(self.s.len())].iter().copied())
                                    .chain("\" is not a valid unicode escape".encode_utf16())
                                    .collect();
                                return Err(self.error_units(&units, from));
                            }
                        };
                        value = value * 16 + d;
                        self.at += 1
                    }
                    value
                }
                _ => {
                    let units: Vec<_> = "Invalid escape character ".encode_utf16().chain([escape]).collect();
                    return Err(self.error_units(&units, escape_at));
                }
            });
        }
        Err(self.fail("Unterminated string"))
    }
    fn scalar(&mut self, incomplete_exponent: &str) -> Result<String, PeleError> {
        let start = self.at;
        if self.c() == 34 {
            return self.string();
        }
        for (word, text) in [("true", "true"), ("false", "false"), ("null", "null")] {
            if self.s.get(start).copied() == word.encode_utf16().next() {
                let expected: Vec<_> = word.encode_utf16().collect();
                let mut bad = None;
                for (i, c) in expected.iter().enumerate() {
                    if self.s.get(start + i) != Some(c) {
                        bad = Some(start + i);
                        break;
                    }
                }
                if let Some(bad) = bad {
                    let end = (start..self.s.len())
                        .find(|&i| !matches!(self.s[i],65..=90|97..=122|48..=57|95|36))
                        .unwrap_or(self.s.len());
                    let name = String::from_utf16_lossy(&self.s[start..end]);
                    return Err(self.error(format!("Unexpected identifier \"{name}\""), bad));
                }
                self.at += expected.len();
                return Ok(text.into());
            }
        }
        if self.c() == 45 || matches!(self.c(), 48..=57) {
            if self.c() == 45 {
                self.at += 1
            }
            if self.c() == 48 {
                self.at += 1
            } else if matches!(self.c(), 48..=57) {
                while matches!(self.c(), 48..=57) {
                    self.at += 1
                }
            } else {
                return Err(self.fail("Invalid number"));
            }
            if self.c() == 46 {
                self.at += 1;
                if !matches!(self.c(), 48..=57) {
                    return Err(self.fail("Invalid digits after decimal point"));
                }
                while matches!(self.c(), 48..=57) {
                    self.at += 1
                }
            }
            if matches!(self.c(), 69 | 101) {
                self.at += 1;
                if matches!(self.c(), 43 | 45) {
                    self.at += 1
                }
                if !matches!(self.c(), 48..=57) {
                    return Err(self.fail(incomplete_exponent));
                }
                while matches!(self.c(), 48..=57) {
                    self.at += 1
                }
            }
            let raw = String::from_utf16_lossy(&self.s[start..self.at]);
            let n = raw.parse::<f64>().unwrap_or(f64::NAN);
            return Ok(crate::util::js_num(n));
        }
        if self.at == self.s.len() {
            return Err(self.fail("Unexpected EOF"));
        }
        if matches!(self.c(), 36 | 65..=90 | 95 | 97..=122) {
            let end = (start..self.s.len())
                .find(|&i| !matches!(self.s[i], 36 | 48..=57 | 65..=90 | 95 | 97..=122))
                .unwrap_or(self.s.len());
            return Err(self.fail(&format!(
                "Unexpected identifier \"{}\"",
                String::from_utf16_lossy(&self.s[start..end])
            )));
        }
        Err(self.syntax(&format!("Unexpected token '{}'", String::from_utf16_lossy(&[self.c()]))))
    }
}
pub fn parse(raw: &str, origin: &[u16], offset: usize) -> Result<Data, PeleError> {
    struct Frame {
        node: usize,
        key: Option<String>,
        after_comma: bool,
    }
    let mut p = Parser {
        s: raw.encode_utf16().collect(),
        at: 0,
        origin,
        offset,
        error_offset: None,
        data: Default::default(),
    };
    let mut stack: Vec<Frame> = Vec::new();
    let mut state = 0;
    let mut value = 0;
    loop {
        p.skip();
        if state == 0 {
            let c = p.c();
            if matches!(c, 123 | 91) {
                p.at += 1;
                let node = p.data.values.len();
                p.data.values.push(if c == 123 {
                    Value::Object(IndexMap::new())
                } else {
                    Value::Array(Vec::new())
                });
                p.skip();
                if p.c() == if c == 123 { 125 } else { 93 } {
                    p.at += 1;
                    value = node;
                    state = 2
                } else {
                    stack.push(Frame {
                        node,
                        key: None,
                        after_comma: false,
                    });
                    state = if c == 123 { 1 } else { 0 }
                }
            } else {
                let incomplete_exponent = stack.last().map_or("Unable to parse JSON string", |frame| {
                    if matches!(p.data.values[frame.node], Value::Object(_)) {
                        "Expected '}'"
                    } else {
                        "Expected ']'"
                    }
                });
                let text = p.scalar(incomplete_exponent)?;
                value = p.data.values.len();
                p.data.values.push(Value::Scalar(text));
                state = 2
            }
        } else if state == 1 {
            if p.c() != 34 {
                return Err(p.syntax(if stack.last().unwrap().after_comma {
                    "Property name must be a string literal"
                } else {
                    "Expected '}'"
                }));
            }
            let key = p.string()?;
            p.skip();
            if p.c() != 58 {
                return Err(p.syntax("Expected ':' before value in object property definition"));
            }
            p.at += 1;
            stack.last_mut().unwrap().key = Some(key);
            state = 0
        } else {
            let Some(frame) = stack.last_mut() else {
                p.data.root = value;
                break;
            };
            let node = frame.node;
            let object = matches!(p.data.values[node], Value::Object(_));
            match &mut p.data.values[node] {
                Value::Object(map) => {
                    map.insert(frame.key.take().unwrap(), value);
                }
                Value::Array(arr) => arr.push(value),
                _ => unreachable!(),
            }
            p.skip();
            let close = if object { 125 } else { 93 };
            if p.c() == close {
                p.at += 1;
                value = node;
                stack.pop();
            } else if p.c() != 44 {
                return Err(p.syntax(if object { "Expected '}'" } else { "Expected ']'" }));
            } else {
                p.at += 1;
                p.skip();
                if !object && p.c() == 93 {
                    return Err(p.fail("Unexpected comma at the end of array expression"));
                }
                stack.last_mut().unwrap().after_comma = true;
                state = if object { 1 } else { 0 }
            }
        }
    }
    p.skip();
    if p.at != p.s.len() {
        return Err(p.syntax("Unable to parse JSON string"));
    }
    Ok(p.data)
}
