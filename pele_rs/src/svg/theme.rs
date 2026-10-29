use std::sync::OnceLock;

use indexmap::IndexMap;
use regex::Regex;

use super::builder::{esc, is_invalid_xml};
use crate::util::{JS_WS, js_number, parse_float, trim};

// Private aliases set once on the root element, so each use is short and the public --pele-*
// tokens keep their fallbacks.
pub const ROOT_STYLE: &str = concat!(
    "--_bg:var(--pele-bg,#fff);--_fg:var(--pele-fg,#1f1f1f);--_m:var(--pele-muted,#6e6e6e);",
    "--_l:var(--pele-line,#1f1f1f);--_s:var(--pele-surface,#f3f3f3);--_a:var(--pele-surface-alt,#e6e6e6);",
    "--_b:var(--pele-border,#8a8a8a);--_c:var(--pele-accent,#5b7bd5);color:var(--_fg)"
);

pub const RADIUS: &str = "var(--pele-radius,4px)";

pub const ROUNDED: &str = " style=\"rx:var(--_r)\"";

pub fn sets_fill(shape: &str) -> bool {
    shape.contains("\"fill:") || shape.contains(";fill:")
}

pub fn rounded(attrs: &str) -> String {
    match attrs.find(" style=\"") {
        None => format!("{}{}", attrs, ROUNDED),
        Some(at) => format!("{}rx:var(--_r);{}", &attrs[..at + 8], &attrs[at + 8..]),
    }
}

fn re_families() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let family = r#"(?:"[^"'\\\n<>{}();:@]*"|'[^"'\\\n<>{}();:@]*'|[\p{L}\p{N}_-][\p{L}\p{N}_ .-]*)"#;
        let ws = JS_WS;
        Regex::new(&format!(r"^[{ws}]*{family}(?:[{ws}]*,[{ws}]*{family})*[{ws}]*$")).unwrap()
    })
}

fn families(list: Option<&str>, generic: &str) -> String {
    match list {
        Some(l) if !l.is_empty() && re_families().is_match(l) => l.to_string(),
        _ => generic.to_string(),
    }
}

// The fonts the labels were measured in are the ones they are drawn in, unless the page names others.
pub fn font_style(size: f64, family: Option<&str>, mono: Option<&str>) -> String {
    format!(
        ";--_fm:var(--pele-font-mono,{});font:{}px sans-serif;letter-spacing:normal;word-spacing:normal;text-transform:none;direction:ltr;font-family:var(--pele-font,{})",
        families(mono, "monospace"),
        crate::util::js_num(size),
        families(family, "sans-serif")
    )
}

const SHAPE_PROPS: [&str; 12] = [
    "fill",
    "fill-opacity",
    "stroke",
    "stroke-width",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-opacity",
    "opacity",
    "rx",
    "ry",
];

const TEXT_PROPS: [&str; 9] = [
    "color",
    "font-size",
    "font-family",
    "font-weight",
    "font-style",
    "text-decoration",
    "letter-spacing",
    "word-spacing",
    "opacity",
];

fn re_unsafe() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)url[{ws}]*\(|expression[{ws}]*\(|javascript:|[<>{{}};\\]|@import|image-set|/\*",
            ws = JS_WS
        ))
        .unwrap()
    })
}

fn re_important() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)!important").unwrap())
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedStyle {
    pub shape: String,
    pub line: String,
    pub text: String,
    pub stroke: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub font_size: Option<f64>,
}

// Splits Mermaid style declarations into what applies to a shape and what applies to its text.
pub fn resolve_style(declarations: &[String]) -> ResolvedStyle {
    if declarations.is_empty() {
        return ResolvedStyle::default();
    }
    let mut shape: IndexMap<String, String> = IndexMap::new();
    let mut text: IndexMap<String, String> = IndexMap::new();
    for decl in declarations {
        let Some(at) = decl.find(':') else { continue };
        if at == 0 {
            continue;
        }
        let prop = trim(&decl[..at]).to_lowercase();
        let value = re_important().replace_all(&decl[at + 1..], "");
        let value = trim(&value).to_string();
        if value.is_empty() || re_unsafe().is_match(&value) || value.chars().any(is_invalid_xml) {
            continue;
        }
        if SHAPE_PROPS.contains(&prop.as_str()) {
            shape.insert(prop.clone(), value.clone());
        }
        if TEXT_PROPS.contains(&prop.as_str()) {
            text.insert(if prop == "color" { "fill".to_string() } else { prop.clone() }, value);
        }
    }
    if shape.is_empty() && text.is_empty() {
        return ResolvedStyle::default();
    }
    let mut shape_css = String::new();
    let mut line_css = String::new();
    for (prop, value) in &shape {
        shape_css += &format!("{}:{};", prop, value);
        if prop != "fill" && prop != "fill-opacity" {
            line_css += &format!("{}:{};", prop, value);
        }
    }
    let mut text_css = String::new();
    for (prop, value) in &text {
        text_css += &format!("{}:{};", prop, value);
    }
    let weight = text.get("font-weight").map(|s| s.as_str());
    let size = text.get("font-size");
    let px = size.and_then(|s| {
        let ok = {
            let body = s.strip_suffix("px").unwrap_or(s);
            !body.is_empty() && body.chars().all(|c| c.is_ascii_digit() || c == '.')
        };
        if ok { Some(parse_float(s)) } else { None }
    });
    ResolvedStyle {
        shape: if shape_css.is_empty() {
            String::new()
        } else {
            format!(" style=\"{}\"", esc(&shape_css))
        },
        line: if line_css.is_empty() {
            String::new()
        } else {
            format!(" style=\"{}\"", esc(&line_css))
        },
        text: if text_css.is_empty() {
            String::new()
        } else {
            format!(" style=\"{}\"", esc(&text_css))
        },
        stroke: shape.get("stroke").cloned(),
        bold: weight == Some("bold") || weight == Some("bolder") || weight.map(js_number).is_some_and(|w| w >= 600.0),
        italic: text.get("font-style").map(|s| s.as_str()) == Some("italic"),
        font_size: px.filter(|&p| p > 0.0 && p <= 200.0),
    }
}

pub fn class_names(raw: &str) -> String {
    let mut cleaned = String::new();
    let mut gap = false;
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            if gap {
                cleaned.push(' ');
                gap = false;
            }
            cleaned.push(c);
        } else {
            gap = true;
        }
    }
    // JavaScript replaces each run with a space and trims, which this matches except at the start.
    let cleaned = cleaned.trim_start().to_string();
    if cleaned.is_empty() {
        String::new()
    } else {
        format!(" {}", cleaned)
    }
}

const SERIES: [&str; 8] = [
    "#4c78a8", "#f58518", "#54a24b", "#e45756", "#72b7b2", "#eeca3b", "#b279a2", "#9d755d",
];

pub fn series_color(index: i64) -> String {
    let k = ((index % 8) + 8) % 8;
    format!("var(--pele-series-{},{})", k + 1, SERIES[k as usize])
}

pub const CRITICAL: &str = "var(--pele-critical,#e45756)";
