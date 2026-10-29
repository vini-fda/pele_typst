use super::builder::{esc, esc_text};
use super::theme::{RADIUS, ROOT_STYLE, font_style};
use crate::RenderOptions;
use crate::util::js_num;

pub struct Accessible<'a> {
    pub acc_title: Option<&'a str>,
    pub acc_descr: Option<&'a str>,
}

// The weights of bold text, each set only in a diagram that has such text.
const WEIGHTS: [(&str, &str); 3] = [("tw", "title"), ("hw", "heading"), ("w", "bold")];

pub fn svg_document(
    kind: &str,
    width: f64,
    height: f64,
    font_size: f64,
    options: &RenderOptions,
    acc: &Accessible,
    content: &str,
) -> String {
    let prefix = esc(options.id_prefix.as_deref().unwrap_or("pele"));
    let mut head = String::new();
    let mut aria = String::new();
    if let Some(t) = acc.acc_title.filter(|t| !t.is_empty()) {
        head += &format!("<title id=\"{}-title\">{}</title>", prefix, esc_text(t));
        aria += &format!(" aria-labelledby=\"{}-title\"", prefix);
    }
    if let Some(d) = acc.acc_descr.filter(|d| !d.is_empty()) {
        head += &format!("<desc id=\"{}-desc\">{}</desc>", prefix, esc_text(d));
        aria += &format!(" aria-describedby=\"{}-desc\"", prefix);
    }
    let fit = if options.responsive == Some(false) {
        ""
    } else {
        "max-width:100%;height:auto;"
    };
    let mut used: Vec<&str> = Vec::new();
    let mut rest = content;
    while let Some(at) = rest.find("var(--_") {
        let after = &rest[at + 7..];
        for name in ["r", "tw", "hw", "w"] {
            if after.starts_with(name) && after[name.len()..].starts_with(')') {
                if !used.contains(&name) {
                    used.push(name);
                }
                break;
            }
        }
        if used.len() == 4 {
            break;
        }
        rest = after;
    }
    let mut weights = if used.contains(&"r") {
        format!(";--_r:{}", RADIUS)
    } else {
        String::new()
    };
    for (alias, name) in WEIGHTS {
        if used.contains(&alias) {
            weights += &format!(";--_{}:var(--pele-{}-weight,bold)", alias, name);
        }
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"pele pele-{kind}\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" style=\"{fit}{ROOT_STYLE}{weights}{font}\" fill=\"var(--_fg)\" role=\"graphics-document document\" aria-roledescription=\"{kind}\"{aria}>{head}{content}</svg>",
        w = js_num(width),
        h = js_num(height),
        font = esc(&font_style(
            font_size,
            options.font_family.as_deref(),
            options.font_family_mono.as_deref()
        )),
    )
}
