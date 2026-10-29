//! Quadrant chart parsing and placement.
mod lexer;
mod render;
mod tokens;
use crate::errors::{PeleError, PeleErrorCode, syntax_error};
use crate::preprocess::Config;
use crate::util::{is_js_space, js_number, trim};
use indexmap::IndexMap;
pub use render::render;
use tokens::{T, TOKEN_NAMES};

#[derive(Debug, Clone, Default)]
pub struct Text {
    pub text: String,
    pub markdown: bool,
}
#[derive(Debug, Clone, Default)]
pub struct PointStyle {
    pub radius: Option<f64>,
    pub color: Option<String>,
    pub stroke_color: Option<String>,
    pub stroke_width: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub text: Text,
    pub class_name: String,
    pub style: PointStyle,
}
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub quadrants: [Option<Text>; 4],
    pub x_left: Option<Text>,
    pub x_right: Option<Text>,
    pub y_bottom: Option<Text>,
    pub y_top: Option<Text>,
    pub points: Vec<Point>,
    pub classes: IndexMap<String, PointStyle>,
}
fn invalid(key: &str, value: &str, kind: &str) -> PeleError {
    PeleError::new(
        format!("value for {key} {value} is invalid, please use a valid {kind}"),
        PeleErrorCode::Semantic,
    )
    .of("quadrantChart")
}
fn styles(values: Vec<String>) -> Result<PointStyle, PeleError> {
    let mut out = PointStyle::default();
    for style in values {
        let s = trim(&style);
        let mut pair = s.split(':');
        let key = trim(pair.next().unwrap_or(""));
        let value = pair.next().map(trim).unwrap_or("undefined");
        match key {
            "radius" => {
                if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
                    return Err(invalid(key, value, "number"));
                }
                out.radius = Some(js_number(value));
            }
            "color" | "stroke-color" => {
                let hex = value.strip_prefix('#').unwrap_or(value);
                if !matches!(hex.len(), 3 | 6) || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
                    return Err(invalid(key, value, "hex code"));
                }
                if key == "color" {
                    out.color = Some(value.into());
                } else {
                    out.stroke_color = Some(value.into());
                }
            }
            "stroke-width" => {
                if !value
                    .strip_suffix("px")
                    .is_some_and(|v| !v.is_empty() && v.bytes().all(|c| c.is_ascii_digit()))
                {
                    return Err(invalid(key, value, "number of pixels (eg. 10px)"));
                }
                out.stroke_width = Some(value.into());
            }
            _ => return Err(PeleError::new(format!("style named {key} is not supported."), PeleErrorCode::Semantic).of("quadrantChart")),
        }
    }
    Ok(out)
}
fn word(t: T) -> bool {
    matches!(
        t,
        T::PUNCTUATION
            | T::AMP
            | T::NUM
            | T::ALPHA
            | T::COMMA
            | T::PLUS
            | T::EQUALS
            | T::MULT
            | T::DOT
            | T::BRKT
            | T::UNDERSCORE
            | T::UNICODE_TEXT
    )
}
fn ident(t: T) -> bool {
    matches!(
        t,
        T::ALPHA | T::NUM | T::MINUS | T::COMMA | T::COLON | T::AMP | T::BRKT | T::MULT | T::UNICODE_TEXT
    )
}
fn style_token(t: T) -> bool {
    matches!(t, T::ALPHA | T::NUM | T::COLON | T::SPACE | T::BRKT | T::MINUS)
}
fn eol(t: T) -> bool {
    matches!(t, T::NEWLINE | T::SEMI | T::EOF)
}
fn is_text(t: T) -> bool {
    matches!(t, T::STR | T::MD_STR) || word(t)
}
struct Parser {
    tokens: lexer::Tokens,
    i: usize,
}
impl Parser {
    fn kind(&self) -> T {
        self.tokens.types.get(self.i).copied().unwrap_or(T::END)
    }
    fn take(&mut self) -> String {
        let s = self.tokens.texts.get(self.i).cloned().unwrap_or_default();
        self.i += 1;
        s
    }
    fn fail(&self, expected: &str) -> PeleError {
        let t = self.kind();
        syntax_error(
            "quadrantChart",
            &self.tokens.src,
            self.tokens.starts.get(self.i).copied().unwrap_or(self.tokens.src.len()),
            &format!("Expecting {expected}, got '{}'", TOKEN_NAMES[t as usize]),
            t == T::ERROR,
        )
    }
    fn expect(&mut self, t: T) -> Result<String, PeleError> {
        if self.kind() != t {
            return Err(self.fail(&format!("'{}'", TOKEN_NAMES[t as usize])));
        }
        Ok(self.take())
    }
    fn text(&mut self) -> Result<Text, PeleError> {
        let first = self.kind();
        if !is_text(first) {
            return Err(self.fail("'STR', 'MD_STR', 'ALPHA', 'NUM', 'UNICODE_TEXT', 'PUNCTUATION'"));
        }
        let mut s = self.take();
        while word(self.kind()) || matches!(self.kind(), T::SPACE | T::MINUS) {
            s += &self.take();
        }
        Ok(Text {
            text: trim(&s).into(),
            markdown: first == T::MD_STR,
        })
    }
    fn styles(&mut self) -> Result<PointStyle, PeleError> {
        let mut out = vec![];
        loop {
            if !style_token(self.kind()) {
                return Err(self.fail("'ALPHA', 'NUM', 'COLON', 'SPACE', 'BRKT', 'MINUS'"));
            }
            let mut s = self.take();
            while style_token(self.kind()) {
                s += &self.take();
            }
            out.push(trim(&s).into());
            if self.kind() != T::COMMA {
                return styles(out);
            }
            self.i += 1;
        }
    }
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let mut p = Parser {
        tokens: lexer::tokenize(source),
        i: 0,
    };
    let mut model = Model {
        title: title.map(String::from),
        ..Default::default()
    };
    while eol(p.kind()) || p.kind() == T::SPACE {
        p.i += 1;
    }
    p.expect(T::QUADRANT)?;
    while p.kind() != T::END {
        while p.kind() == T::SPACE {
            p.i += 1;
        }
        let t = p.kind();
        match t {
            T::CLASSDEF => {
                p.i += 1;
                p.expect(T::SPACE)?;
                if !ident(p.kind()) {
                    return Err(p.fail("'ALPHA', 'NUM', 'UNICODE_TEXT'"));
                }
                let mut id = p.take();
                while ident(p.kind()) {
                    id += &p.take();
                }
                p.expect(T::SPACE)?;
                model.classes.insert(id, p.styles()?);
            }
            T::X_AXIS | T::Y_AXIS => {
                p.i += 1;
                let mut from = p.text()?;
                let mut to = None;
                if p.kind() == T::DELIMITER {
                    p.i += 1;
                    if is_text(p.kind()) {
                        to = Some(p.text()?);
                    } else {
                        from.text += " ⟶ ";
                    }
                }
                from.text = trim(&from.text).into();
                if t == T::X_AXIS {
                    model.x_left = Some(from);
                    if to.is_some() {
                        model.x_right = to;
                    }
                } else {
                    model.y_bottom = Some(from);
                    if to.is_some() {
                        model.y_top = to;
                    }
                }
            }
            T::QUADRANT_1 | T::QUADRANT_2 | T::QUADRANT_3 | T::QUADRANT_4 => {
                p.i += 1;
                model.quadrants[t as usize - T::QUADRANT_1 as usize] = Some(p.text()?);
            }
            T::title => {
                p.i += 1;
                model.title = Some(trim(&p.expect(T::title_value)?).into());
            }
            T::acc_title => {
                p.i += 1;
                model.acc_title = Some(trim(&p.expect(T::acc_title_value)?).into());
            }
            T::acc_descr => {
                p.i += 1;
                model.acc_descr = Some(trim(&p.expect(T::acc_descr_value)?).into());
            }
            T::acc_descr_multiline_value => model.acc_descr = Some(trim(&p.take()).into()),
            _ => {
                if is_text(t) {
                    let text = p.text()?;
                    let class_name = if p.kind() == T::class_name { p.take() } else { String::new() };
                    p.expect(T::point_start)?;
                    let x = js_number(&p.expect(T::point_x)?);
                    let y = js_number(&p.expect(T::point_y)?);
                    let style = if style_token(p.kind()) {
                        p.styles()?
                    } else {
                        PointStyle::default()
                    };
                    model.points.push(Point {
                        x,
                        y,
                        text,
                        class_name,
                        style,
                    });
                }
            }
        }
        if !eol(p.kind()) {
            return Err(p.fail("'NEWLINE', 'SEMI', 'EOF'"));
        }
        p.i += 1;
    }
    model.acc_descr = model.acc_descr.map(|v| {
        let mut out = String::new();
        let mut chars = v.chars().peekable();
        while let Some(c) = chars.next() {
            out.push(c);
            if c == '\n' {
                while chars.peek().is_some_and(|&c| is_js_space(c)) {
                    chars.next();
                }
            }
        }
        out
    });
    Ok(model)
}
