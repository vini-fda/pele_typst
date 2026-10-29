//! Radar charts, following Pele's grammar, model, and SVG renderer.
mod render;
use crate::diagrams::tokens::*;
use crate::errors::{PeleError, PeleErrorCode, syntax_error};
use crate::preprocess::Config;
use crate::util::{is_js_space, is_word16, js_number, trim_start};
pub use render::render;

#[derive(Debug, Clone)]
pub struct Axis {
    pub name: String,
    pub label: String,
}
#[derive(Debug, Clone)]
pub struct Curve {
    pub name: String,
    pub label: String,
    pub entries: Vec<f64>,
}
#[derive(Debug, Clone)]
pub struct Options {
    pub show_legend: bool,
    pub ticks: f64,
    pub max: Option<f64>,
    pub min: f64,
    pub graticule: String,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            show_legend: true,
            ticks: 5.0,
            max: None,
            min: 0.0,
            graticule: "circle".into(),
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub axes: Vec<Axis>,
    pub curves: Vec<Curve>,
    pub options: Options,
}

const NAMES: &[&str] = &[
    "radar-beta:",
    "radar-beta",
    "showLegend",
    "graticule",
    "curve",
    "ticks",
    "axis",
    "max",
    "min",
    ":",
    ",",
    "[",
    "]",
    "{",
    "}",
    "GRATICULE",
    "BOOLEAN",
    "ACC_DESCR",
    "ACC_TITLE",
    "TITLE",
    "NUMBER",
    "STRING",
    "ID",
    "NEWLINE",
];
fn at(s: &[u16], p: usize) -> u16 {
    s.get(p).copied().unwrap_or(u16::MAX)
}
fn starts(s: &[u16], p: usize, t: &str) -> bool {
    t.encode_utf16().enumerate().all(|(i, c)| s.get(p + i) == Some(&c))
}
fn id(s: &[u16], p: usize) -> Option<usize> {
    if !is_word16(at(s, p) as u32) {
        return None;
    }
    let mut end = p + 1;
    let mut last = end;
    while is_word16(at(s, end) as u32) || at(s, end) == 45 {
        if at(s, end) != 45 {
            last = end + 1;
        }
        end += 1;
    }
    Some(last)
}
fn number(s: &[u16], p: usize) -> Option<usize> {
    let mut whole = p;
    while (48..=57).contains(&at(s, whole)) {
        whole += 1;
    }
    if whole == p {
        return None;
    }
    let dot = at(s, whole) == 46;
    if dot {
        let mut fraction = whole + 1;
        while (48..=57).contains(&at(s, fraction)) {
            fraction += 1;
        }
        let len = fraction - whole - 1;
        if len > 0 && at(s, fraction) != 46 {
            return Some(fraction);
        }
        if len > 1 {
            return Some(fraction - 1);
        }
    }
    if at(s, p) == 48 {
        return Some(p + 1);
    }
    if !dot {
        return Some(whole);
    }
    (whole - 1 > p).then_some(whole - 1)
}
fn lex(source: &str) -> Result<Tokens, PeleError> {
    let src: Vec<u16> = source.encode_utf16().collect();
    let mut out = Tokens {
        kinds: vec![],
        starts: vec![],
        ends: vec![],
        src,
    };
    let mut p = 0;
    while p < out.src.len() {
        let s = &out.src;
        let mut hit = None;
        for (k, &name) in NAMES.iter().take(15).enumerate() {
            if starts(s, p, name) {
                let end = p + name.len();
                let longer = (1..=8).contains(&k) && id(s, p).is_some_and(|i| i > end);
                let boundary = k != 1 || end == s.len() || crate::util::is_space16(at(s, end) as u32) || starts(s, end, "%%");
                if !longer && boundary {
                    hit = Some((k as i32, end));
                    break;
                }
            }
        }
        let previous = out.kinds.last().copied().unwrap_or(-1);
        if hit.is_none() {
            for (k, words, option) in [(15, &["circle", "polygon"][..], 3), (16, &["true", "false"][..], 2)] {
                for word in words {
                    if starts(s, p, word) {
                        let end = p + word.len();
                        if previous == option || id(s, p).is_none_or(|i| i <= end) {
                            hit = Some((k, end));
                            break;
                        }
                    }
                }
                if hit.is_some() {
                    break;
                }
            }
        }
        if hit.is_none() {
            hit = acc_descr(s, p).map(|e| (17, e));
        }
        if hit.is_none() {
            hit = acc_title(s, p).map(|e| (18, e));
        }
        if hit.is_none() {
            hit = title(s, p).filter(|_| id(s, p).is_none_or(|e| e <= p + 5)).map(|e| (19, e));
        }
        if hit.is_none() {
            hit = number(s, p).map(|e| (20, e));
        }
        if hit.is_none() {
            hit = string(s, p).map(|e| (21, e));
        }
        if hit.is_none() {
            hit = id(s, p).map(|e| (22, e));
        }
        if hit.is_none() {
            hit = newline(s, p).map(|e| (23, e));
        }
        if let Some((k, end)) = hit {
            out.kinds.push(k);
            out.starts.push(p);
            out.ends.push(end);
            p = end;
            continue;
        }
        let hidden = whitespace(s, p)
            .or_else(|| yaml(s, p))
            .or_else(|| directive(s, p))
            .or_else(|| comment(s, p));
        if let Some(end) = hidden {
            p = end;
        } else {
            return Err(syntax_error("radar", s, p, "", true));
        }
    }
    out.kinds.push(-1);
    out.starts.push(p);
    out.ends.push(p);
    Ok(out)
}
fn lines(r: &mut Reader) {
    while r.accept(23) {}
}
fn eol(r: &mut Reader) -> Result<(), PeleError> {
    if r.kind() != -1 && r.kind() != 23 {
        return Err(r.fail("a line break or the end of input"));
    }
    lines(r);
    Ok(())
}
fn label(r: &mut Reader) -> Result<Option<String>, PeleError> {
    if !r.accept(11) {
        return Ok(None);
    }
    let text = string_value(&r.expect(21)?);
    r.expect(12)?;
    Ok(Some(text))
}
fn entry(r: &mut Reader, detailed: bool) -> Result<(Option<String>, f64), PeleError> {
    let axis = if detailed {
        let a = r.expect(22)?;
        r.accept(9);
        Some(a)
    } else {
        None
    };
    Ok((axis, js_number(&r.expect(20)?)))
}
fn invalid(text: String) -> PeleError {
    PeleError::new(text, PeleErrorCode::Semantic).of("radar")
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let types: Vec<TokenType> = NAMES
        .iter()
        .map(|&name| TokenType {
            name,
            pattern: Pattern::Literal(name),
            hidden: false,
            first: None,
        })
        .collect();
    let mut r = Reader {
        i: 0,
        tokens: lex(source)?,
        types: &types,
        diagram: "radar",
    };
    let mut model = Model::default();
    let mut curves = vec![];
    lines(&mut r);
    if !r.accept(0) {
        r.expect(1)?;
        r.accept(9);
    }
    lines(&mut r);
    while r.kind() != -1 {
        match r.kind() {
            23 => r.i += 1,
            17 => {
                model.acc_descr = Some(acc_descr_value(&r.take()));
                eol(&mut r)?;
            }
            18 => {
                model.acc_title = Some(acc_title_value(&r.take()));
                eol(&mut r)?;
            }
            19 => {
                model.title = Some(title_value(&r.take()));
                eol(&mut r)?;
            }
            6 => {
                r.i += 1;
                loop {
                    let name = r.expect(22)?;
                    let text = label(&mut r)?.unwrap_or_else(|| name.clone());
                    model.axes.push(Axis { name, label: text });
                    if !r.accept(10) {
                        break;
                    }
                }
            }
            4 => {
                r.i += 1;
                loop {
                    let name = r.expect(22)?;
                    let text = label(&mut r)?.unwrap_or_else(|| name.clone());
                    r.expect(13)?;
                    lines(&mut r);
                    if r.kind() != 20 && r.kind() != 22 {
                        return Err(r.fail("a number or an axis name"));
                    }
                    let detailed = r.kind() == 22;
                    let mut entries = vec![entry(&mut r, detailed)?];
                    while r.accept(10) {
                        lines(&mut r);
                        entries.push(entry(&mut r, detailed)?);
                    }
                    lines(&mut r);
                    r.expect(14)?;
                    curves.push((name, text, entries));
                    if !r.accept(10) {
                        break;
                    }
                }
            }
            _ => {
                if !matches!(r.kind(), 2 | 3 | 5 | 7 | 8) {
                    return Err(r.fail("an axis, a curve, an option, a title, or a line break"));
                }
                loop {
                    let kind = r.kind();
                    r.i += 1;
                    match kind {
                        2 => model.options.show_legend = r.expect(16)? == "true",
                        3 => model.options.graticule = r.expect(15)?,
                        5 => model.options.ticks = js_number(&r.expect(20)?),
                        7 => model.options.max = Some(js_number(&r.expect(20)?)),
                        8 => model.options.min = js_number(&r.expect(20)?),
                        _ => return Err(r.fail("an option")),
                    }
                    if !r.accept(10) {
                        break;
                    }
                    if !matches!(r.kind(), 2 | 3 | 5 | 7 | 8) {
                        return Err(r.fail("an option"));
                    }
                }
            }
        }
    }
    for (name, label, entries) in curves {
        let values = if entries[0].0.is_none() {
            entries.iter().map(|e| e.1).collect()
        } else {
            if model.axes.is_empty() {
                return Err(invalid("Axes must be populated before curves for reference entries".into()));
            }
            let mut values = vec![];
            for axis in &model.axes {
                let Some((_, value)) = entries.iter().find(|e| e.0.as_deref() == Some(&axis.name)) else {
                    return Err(invalid(format!("Missing entry for axis {}", axis.label)));
                };
                values.push(*value);
            }
            values
        };
        model.curves.push(Curve {
            name,
            label,
            entries: values,
        });
    }
    model.options.ticks = model.options.ticks.min(32.0);
    model.title = model.title.filter(|v| !v.is_empty()).or_else(|| title.map(String::from));
    model.acc_title = model.acc_title.filter(|v| !v.is_empty()).map(|v| trim_start(&v).to_string());
    model.acc_descr = model.acc_descr.filter(|v| !v.is_empty()).map(|v| {
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
