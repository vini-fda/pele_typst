//! Cartesian bar and line charts.
mod lexer;
mod render;
mod tokens;
use crate::errors::{PeleError, syntax_error};
use crate::preprocess::Config;
use crate::util::{is_js_space, js_num, js_number, trim};
pub use render::render;
use tokens::{T, TOKEN_NAMES};

#[derive(Debug, Clone)]
pub struct Axis {
    pub title: String,
    pub categories: Option<Vec<String>>,
    pub min: f64,
    pub max: f64,
}
#[derive(Debug, Clone)]
pub struct Plot {
    pub bar: bool,
    pub title: String,
    pub data: Vec<(String, Option<f64>)>,
    pub point_labels: Option<Vec<String>>,
}
#[derive(Debug, Clone)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub orientation: Option<String>,
    pub x: Axis,
    pub y: Axis,
    pub has_set_x: bool,
    pub has_set_y: bool,
    pub plots: Vec<Plot>,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            title: None,
            acc_title: None,
            acc_descr: None,
            orientation: None,
            x: Axis {
                title: String::new(),
                categories: Some(vec![]),
                min: 0.0,
                max: 0.0,
            },
            y: Axis {
                title: String::new(),
                categories: None,
                min: f64::INFINITY,
                max: f64::NEG_INFINITY,
            },
            has_set_x: false,
            has_set_y: false,
            plots: vec![],
        }
    }
}
impl Model {
    fn add_plot(&mut self, bar: bool, title: String, data: Vec<(f64, String)>) {
        let mut values: Vec<_> = data.iter().map(|d| d.0).collect();
        if !self.has_set_x {
            self.x.categories = None;
            self.x.min = 1.0;
            self.x.max = values.len() as f64;
            self.has_set_x = true;
        }
        if let Some(categories) = &self.x.categories {
            values.truncate(categories.len());
        }
        if !self.has_set_y {
            for &v in &values {
                if v < self.y.min {
                    self.y.min = v;
                }
                if v > self.y.max {
                    self.y.max = v;
                }
            }
        }
        let points = if let Some(categories) = &self.x.categories {
            categories
                .iter()
                .enumerate()
                .map(|(i, c)| (c.clone(), values.get(i).copied()))
                .collect()
        } else if values.len() == 1 {
            vec![(js_num(self.x.min), Some(values[0]))]
        } else {
            let step = (self.x.max - self.x.min) / (values.len() - 1) as f64;
            values
                .iter()
                .enumerate()
                .map(|(i, &v)| (js_num(self.x.min + i as f64 * step), Some(v)))
                .collect()
        };
        let point_labels = if !bar && data.iter().any(|d| !trim(&d.1).is_empty()) {
            Some(data.iter().map(|d| trim(&d.1).into()).collect())
        } else {
            None
        };
        self.plots.push(Plot {
            bar,
            title: trim(&title).into(),
            data: points,
            point_labels,
        });
    }
}
fn word(t: T) -> bool {
    matches!(
        t,
        T::AMP | T::NUM | T::ALPHA | T::PLUS | T::EQUALS | T::MULT | T::DOT | T::BRKT | T::MINUS | T::UNDERSCORE
    )
}
fn eol(t: T) -> bool {
    matches!(t, T::NEWLINE | T::SEMI | T::EOF)
}
fn is_text(t: T) -> bool {
    t == T::STR || word(t)
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
            "xychart",
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
    fn text(&mut self) -> Result<String, PeleError> {
        let first = self.kind();
        if !is_text(first) {
            return Err(self.fail("'STR', 'ALPHA', 'NUM'"));
        }
        let mut s = self.take();
        if first != T::STR {
            while word(self.kind()) {
                s += &self.take();
            }
        }
        Ok(s)
    }
    fn range(&mut self) -> Result<(f64, f64), PeleError> {
        let min = js_number(&self.expect(T::NUMBER_WITH_DECIMAL)?);
        self.expect(T::ARROW_DELIMITER)?;
        Ok((min, js_number(&self.expect(T::NUMBER_WITH_DECIMAL)?)))
    }
    fn points(&mut self) -> Result<Vec<(f64, String)>, PeleError> {
        self.expect(T::SQUARE_BRACES_START)?;
        let mut out = vec![];
        loop {
            let value = js_number(&self.expect(T::NUMBER_WITH_DECIMAL)?);
            let label = if self.kind() == T::STR { self.take() } else { String::new() };
            out.push((value, label));
            if self.kind() != T::COMMA {
                break;
            }
            self.i += 1;
        }
        self.expect(T::SQUARE_BRACES_END)?;
        Ok(out)
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
    loop {
        if eol(p.kind()) {
            p.i += 1;
        } else if p.kind() != T::XYCHART {
            break;
        } else {
            p.i += 1;
            if p.kind() == T::CHART_ORIENTATION {
                model.orientation = Some(if p.take() == "horizontal" { "horizontal" } else { "vertical" }.into());
            }
        }
    }
    while p.kind() != T::END {
        let t = p.kind();
        p.i += 1;
        match t {
            T::title => model.title = Some(trim(&p.text()?).into()),
            T::X_AXIS | T::Y_AXIS => {
                let title = if is_text(p.kind()) { Some(p.text()?) } else { None };
                if t == T::X_AXIS && p.kind() == T::SQUARE_BRACES_START {
                    let mut cats = vec![];
                    loop {
                        p.i += 1;
                        cats.push(trim(&p.text()?).into());
                        if p.kind() != T::COMMA {
                            break;
                        }
                    }
                    p.expect(T::SQUARE_BRACES_END)?;
                    model.x.categories = Some(cats);
                    model.has_set_x = true;
                } else if p.kind() == T::NUMBER_WITH_DECIMAL || title.is_none() {
                    let (min, max) = p.range()?;
                    let axis = if t == T::X_AXIS {
                        model.has_set_x = true;
                        &mut model.x
                    } else {
                        model.has_set_y = true;
                        &mut model.y
                    };
                    axis.categories = None;
                    axis.min = min;
                    axis.max = max;
                }
                let axis = if t == T::X_AXIS { &mut model.x } else { &mut model.y };
                axis.title = trim(title.as_deref().unwrap_or("")).into();
            }
            T::LINE | T::BAR => {
                let title = if p.kind() == T::SQUARE_BRACES_START {
                    String::new()
                } else {
                    p.text()?
                };
                let data = p.points()?;
                model.add_plot(t == T::BAR, title, data);
            }
            T::acc_title => model.acc_title = Some(trim(&p.expect(T::acc_title_value)?).into()),
            T::acc_descr => model.acc_descr = Some(trim(&p.expect(T::acc_descr_value)?).into()),
            T::acc_descr_multiline_value => model.acc_descr = Some(trim(&p.tokens.texts[p.i - 1]).into()),
            _ => {
                p.i -= 1;
                return Err(p.fail("'title', 'X_AXIS', 'Y_AXIS', 'LINE', 'BAR', 'acc_title', 'acc_descr', 'acc_descr_multiline_value'"));
            }
        }
        while eol(p.kind()) {
            p.i += 1;
        }
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
