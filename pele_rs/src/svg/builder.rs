use crate::text::entities::decode_entities;
use crate::text::label::Label;
use crate::text::measurer::{BOLD, ITALIC, MONO, SUB, SUP, UNDERLINE};
use crate::util::num;

pub fn is_invalid_xml(c: char) -> bool {
    let c = c as u32;
    c <= 0x08 || c == 0x0b || c == 0x0c || (0x0e..=0x1f).contains(&c) || c == 0x7f || c == 0xfffe || c == 0xffff
}

// Escapes text for XML and drops invalid XML characters and DEL.
pub fn esc(text: &str) -> String {
    if !text.chars().any(|c| matches!(c, '&' | '<' | '>' | '"') || is_invalid_xml(c)) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if is_invalid_xml(c) => {}
            c => out.push(c),
        }
    }
    out
}

// Escapes text that is only ever read as text. Entity codes are turned into their characters first.
pub fn esc_text(text: &str) -> String {
    esc(&decode_entities(text))
}

// Writes a measured label as SVG text centered on (cx, cy).
pub fn label_svg(label: &Label, cx: f64, cy: f64, attrs: &str) -> String {
    if label.lines.is_empty() {
        return String::new();
    }
    let shift = label.size * 0.35;
    let mut top = cy - label.height / 2.0;
    if label.lines.len() == 1 && label.lines[0].len() == 1 {
        let only = &label.lines[0][0];
        if only.plain() && only.style == 0 {
            return format!(
                "<text{} x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
                attrs,
                num(cx),
                num(top + label.line_height / 2.0 + shift),
                esc(&only.text)
            );
        }
    }
    let shared = shared_style(label);
    let given = attrs;
    let attrs = text_style(attrs, shared);
    let mut body = String::new();
    let mut extra = String::new();
    for (i, line) in label.lines.iter().enumerate() {
        let h = crate::text::label::line_height(label, i);
        let mid = top + h / 2.0;
        let y = mid + shift;
        if line.iter().any(|s| !s.plain()) {
            let mut x = cx - label.widths[i] / 2.0;
            for span in line {
                if let Some(math) = &span.math {
                    let bbox = format!(
                        "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                        num(x),
                        num(mid - math.height() / 2.0),
                        num(math.width()),
                        num(math.height())
                    );
                    let (look, halo) = math_look(given);
                    if let Some(halo) = halo {
                        extra += &format!("<rect {bbox} rx=\"2\" fill=\"{halo}\"/>");
                    } else {
                        extra += &math_svg(math, &bbox, label.size, &look);
                    }
                } else if let Some(icon) = &span.icon {
                    let side = label.size * 1.1;
                    extra += &format!(
                        "<svg class=\"pele-icon\" data-icon=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{}</svg>",
                        esc_text(icon),
                        num(x),
                        num(mid - side / 2.0),
                        num(side),
                        num(side),
                        span.draw_icon()
                    );
                } else if !span.text.is_empty() {
                    body += &format!(
                        "<tspan x=\"{}\" y=\"{}\" text-anchor=\"start\"{}>{}</tspan>",
                        num(x),
                        num(y),
                        span_style(span.style & !shared),
                        esc(&span.text)
                    );
                }
                x += span.width;
            }
        } else {
            for (j, span) in line.iter().enumerate() {
                let pos = if j == 0 {
                    format!(" x=\"{}\" y=\"{}\"", num(cx), num(y))
                } else {
                    String::new()
                };
                body += &format!("<tspan{}{}>{}</tspan>", pos, span_style(span.style & !shared), esc(&span.text));
            }
            if line.is_empty() {
                body += &format!("<tspan x=\"{}\" y=\"{}\"> </tspan>", num(cx), num(y));
            }
        }
        top += h;
    }
    format!(
        "<text{} text-anchor=\"middle\" xml:space=\"preserve\">{}</text>{}",
        attrs, body, extra
    )
}

fn attr<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let from = attrs.find(&needle)? + needle.len();
    Some(&attrs[from..from + attrs[from..].find('"')?])
}
fn math_look(attrs: &str) -> (String, Option<String>) {
    let style = attr(attrs, "style").unwrap_or("");
    let prop = |name: &str| {
        style.split(';').find_map(|s| {
            let (key, value) = s.trim_start().split_once(':')?;
            (key == name).then_some(value.trim())
        })
    };
    let fill = prop("fill").or_else(|| attr(attrs, "fill")).unwrap_or("").trim();
    if fill == "none" {
        return (String::new(), Some(attr(attrs, "stroke").unwrap_or("none").to_string()));
    }
    let mut out = if fill.is_empty() {
        String::new()
    } else {
        format!(" color=\"{fill}\"")
    };
    if let Some(opacity) = prop("opacity").filter(|s| !s.is_empty()) {
        out += &format!(" opacity=\"{opacity}\"");
    }
    (out, None)
}
fn math_svg(math: &crate::MathMarkup, bbox: &str, size: f64, attrs: &str) -> String {
    match math {
        crate::MathMarkup::Svg { svg, .. } => format!("<svg class=\"pele-math\" {bbox} overflow=\"visible\"{attrs}>{svg}</svg>"),
        crate::MathMarkup::Html { html, .. } => format!(
            "<foreignObject class=\"pele-math\" {bbox} overflow=\"visible\"{attrs}><div xmlns=\"http://www.w3.org/1999/xhtml\" style=\"display:flex;align-items:center;justify-content:center;width:100%;height:100%;font-size:{}px;line-height:normal;white-space:nowrap\">{html}</div></foreignObject>",
            num(size)
        ),
    }
}

const BOLD_ATTR: &str = " font-weight=\"var(--_w)\"";
const ITALIC_ATTR: &str = " font-style=\"italic\"";
const UNDERLINE_ATTR: &str = " text-decoration=\"underline\"";
const MONO_ATTR: &str = " font-family=\"var(--_fm)\"";
const SUB_ATTR: &str = " baseline-shift=\"sub\" font-size=\"smaller\"";
const SUP_ATTR: &str = " baseline-shift=\"super\" font-size=\"smaller\"";

pub fn span_style(style: u32) -> String {
    let mut out = String::new();
    if style & BOLD != 0 {
        out.push_str(BOLD_ATTR);
    }
    if style & ITALIC != 0 {
        out.push_str(ITALIC_ATTR);
    }
    if style & MONO != 0 {
        out.push_str(MONO_ATTR);
    }
    if style & UNDERLINE != 0 {
        out.push_str(UNDERLINE_ATTR);
    }
    if style & SUB != 0 {
        out.push_str(SUB_ATTR);
    } else if style & SUP != 0 {
        out.push_str(SUP_ATTR);
    }
    out
}

// The emphasis every span of a label has. It is written once, on the text element.
pub fn shared_style(label: &Label) -> u32 {
    let mut shared = BOLD | ITALIC | MONO;
    let mut any = false;
    for line in &label.lines {
        for span in line {
            if span.icon.is_some() || span.text.is_empty() {
                continue;
            }
            shared &= span.style;
            any = true;
        }
    }
    if any { shared } else { 0 }
}

// Adds that emphasis to a text element's attributes, unless they give a weight of their own.
pub fn text_style(attrs: &str, shared: u32) -> String {
    let mut out = attrs.to_string();
    if shared & BOLD != 0 && !attrs.contains(" font-weight=\"") {
        out.push_str(BOLD_ATTR);
    }
    if shared & ITALIC != 0 && !attrs.contains(" font-style=\"") {
        out.push_str(ITALIC_ATTR);
    }
    if shared & MONO != 0 && !attrs.contains(" font-family=\"") {
        out.push_str(MONO_ATTR);
    }
    out
}
