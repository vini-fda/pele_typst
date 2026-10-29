use std::sync::OnceLock;

use regex::Regex;

use crate::util::{parse_float, trim};

// Named colors that #fff contrasts with more than #1f1f1f does, and the rest.
const DARK: &str = "black blue blueviolet brown crimson darkblue darkcyan darkgreen darkmagenta darkolivegreen darkorchid darkred darkslateblue darkslategray darkviolet dimgray firebrick forestgreen green indigo maroon mediumblue mediumslateblue mediumvioletred midnightblue navy olive purple rebeccapurple royalblue saddlebrown seagreen sienna slateblue steelblue teal";
const LIGHT: &str = "aliceblue antiquewhite aqua aquamarine azure beige bisque blanchedalmond burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk cyan darkgoldenrod darkgray darkkhaki darkorange darksalmon darkseagreen darkturquoise deeppink deepskyblue dodgerblue floralwhite fuchsia gainsboro ghostwhite gold goldenrod gray greenyellow honeydew hotpink indianred ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightpink lightsalmon lightseagreen lightskyblue lightslategray lightsteelblue lightyellow lime limegreen linen magenta mediumaquamarine mediumorchid mediumpurple mediumseagreen mediumspringgreen mediumturquoise mintcream mistyrose moccasin navajowhite oldlace olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue red rosybrown salmon sandybrown seashell silver skyblue slategray snow springgreen tan thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen";

pub const ON_LIGHT: &str = "var(--pele-fg-on-light,#1f1f1f)";
pub const ON_DARK: &str = "var(--pele-fg-on-dark,#fff)";

fn re_fill() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"[";]fill:([^;"]*)"#).unwrap())
}

fn re_opacities() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"[";](?:fill-)?opacity:([^;"]*)"#).unwrap())
}

fn re_number() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^[+-]?(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)(?:e[+-]?[0-9]+)?(%|deg|grad|rad|turn)?$").unwrap())
}

fn re_function() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(rgba?|hsla?)\(([^()]*)\)$").unwrap())
}

fn linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn luminance(r: f64, g: f64, b: f64) -> f64 {
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

fn clamp01(n: f64) -> f64 {
    n.max(0.0).min(1.0)
}

// Luminance where #1f1f1f and #fff have equal contrast.
fn even() -> f64 {
    (1.05 * (luminance(31.0 / 255.0, 31.0 / 255.0, 31.0 / 255.0) + 0.05)).sqrt() - 0.05
}

fn amount(raw: &str, full: f64) -> f64 {
    let Some(m) = re_number().captures(raw) else { return f64::NAN };
    let unit = m.get(1).map(|u| u.as_str());
    if unit.is_some_and(|u| u != "%") {
        return f64::NAN;
    }
    let v = parse_float(raw) / if unit.is_some() { 100.0 } else { full };
    if v.is_nan() { v } else { clamp01(v) }
}

fn hue(raw: &str) -> f64 {
    let Some(m) = re_number().captures(raw) else { return f64::NAN };
    let unit = m.get(1).map(|u| u.as_str().to_lowercase());
    if unit.as_deref() == Some("%") {
        return f64::NAN;
    }
    let n = parse_float(raw);
    match unit.as_deref() {
        Some("turn") => n * 360.0,
        Some("rad") => (n * 180.0) / std::f64::consts::PI,
        Some("grad") => n * 0.9,
        _ => n,
    }
}

// The comma syntax takes no `none`, rgb() channels all or none in %, and hsl() saturation and lightness in %.
fn args(body: &str, hsl: bool) -> Option<Vec<String>> {
    let legacy = body.contains(',');
    let (main, alpha) = if legacy {
        (body.to_string(), None)
    } else {
        let parts: Vec<&str> = body.split('/').collect();
        if parts.len() > 2 {
            return None;
        }
        (parts[0].to_string(), parts.get(1).map(|s| s.to_string()))
    };
    let main = trim(&main);
    let mut list: Vec<String> = if legacy {
        Regex::new(r"\s*,\s*").unwrap().split(main).map(String::from).collect()
    } else {
        Regex::new(r"\s+").unwrap().split(main).map(String::from).collect()
    };
    if legacy && (list.len() < 3 || list.len() > 4) || !legacy && list.len() != 3 {
        return None;
    }
    if let Some(a) = alpha {
        list.push(trim(&a).to_string());
    }
    if !legacy {
        return Some(list.into_iter().map(|c| if c == "none" { "0".to_string() } else { c }).collect());
    }
    let from = if hsl { 1 } else { 0 };
    let percent = list[from..3].iter().filter(|c| c.ends_with('%')).count();
    if percent == if hsl { 2 } else { 3 } || (!hsl && percent == 0) {
        Some(list)
    } else {
        None
    }
}

// Luminance and alpha; named colors use 0 or 1 to preserve the contrast choice.
fn color(raw: &str) -> Option<(f64, f64)> {
    let value = trim(raw).to_lowercase();
    let name = crate::util::replace_first(&value, "grey", "gray");
    if DARK.split(' ').any(|c| c == name) {
        return Some((0.0, 1.0));
    }
    if LIGHT.split(' ').any(|c| c == name) {
        return Some((1.0, 1.0));
    }
    let hex_ok = value.starts_with('#') && matches!(value.len() - 1, 3 | 4 | 6 | 8) && value[1..].chars().all(|c| c.is_ascii_hexdigit());
    if hex_ok {
        let digits: String = if value.len() < 6 {
            value[1..].chars().flat_map(|c| [c, c]).collect()
        } else {
            value[1..].to_string()
        };
        let n = |i: usize| u8::from_str_radix(&digits[2 * i..2 * i + 2], 16).unwrap() as f64 / 255.0;
        return Some((luminance(n(0), n(1), n(2)), if digits.len() == 8 { n(3) } else { 1.0 }));
    }
    let f = re_function().captures(&value)?;
    let hsl = f.get(1).unwrap().as_str().starts_with("hsl");
    let list = args(f.get(2).unwrap().as_str(), hsl)?;
    let alpha = if list.len() == 4 { amount(&list[3], 1.0) } else { 1.0 };
    let rgb: [f64; 3] = if !hsl {
        [amount(&list[0], 255.0), amount(&list[1], 255.0), amount(&list[2], 255.0)]
    } else {
        let h = (((hue(&list[0]) % 360.0) + 360.0) % 360.0) / 30.0;
        let s = amount(&list[1], 100.0);
        let l = amount(&list[2], 100.0);
        let a = s * l.min(1.0 - l);
        let ch = |k: f64| {
            let t = (k + h) % 12.0;
            l - a * (-1.0f64).max((t - 3.0).min(9.0 - t).min(1.0))
        };
        [ch(0.0), ch(8.0), ch(4.0)]
    };
    let lum = luminance(rgb[0], rgb[1], rgb[2]);
    if (lum + alpha).is_nan() { None } else { Some((lum, alpha)) }
}

// How opaque a style's fill-opacity and opacity leave its fill, or NaN when either isn't a plain number.
fn opacity(shape: &str) -> f64 {
    let mut product = 1.0;
    for c in re_opacities().captures_iter(shape) {
        product *= amount(trim(c.get(1).unwrap().as_str()), 1.0);
    }
    product
}

// Styles come from resolve_style(). Fills below half opacity depend on an unknown background.
pub fn ink(shape: &str, text: &str) -> Option<&'static str> {
    let fill = re_fill().captures(shape)?;
    if re_fill().is_match(text) {
        return None;
    }
    let found = color(fill.get(1).unwrap().as_str())?;
    // An opacity that isn't a plain number, such as calc() or var(), may still be low.
    if !(found.1 * opacity(shape) >= 0.5) {
        return None;
    }
    Some(if found.0 > even() { ON_LIGHT } else { ON_DARK })
}

// Unknown fill colors and opacities are treated as opaque.
pub fn paints(shape: &str) -> bool {
    let Some(fill) = re_fill().captures(shape) else { return false };
    let fill = trim(fill.get(1).unwrap().as_str()).to_lowercase();
    if fill.is_empty() || fill == "none" || fill == "transparent" {
        return false;
    }
    let scale = opacity(shape);
    color(&fill).map(|c| c.1).unwrap_or(1.0) * if scale.is_nan() { 1.0 } else { scale } > 0.0
}

pub fn ink_fill(shape: &str, text: &str, fallback: Option<&str>) -> String {
    match ink(shape, text).or(fallback) {
        Some(c) => format!(" fill=\"{}\"", c),
        None => String::new(),
    }
}

pub fn fill_of(shape: &str) -> Option<String> {
    re_fill().captures(shape).map(|c| c.get(1).unwrap().as_str().to_string())
}
