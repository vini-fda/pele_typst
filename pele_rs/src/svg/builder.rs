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
    let lines = &label.lines;
    if lines.is_empty() {
        return String::new();
    }
    let lh = label.line_height;
    let shift = label.size * 0.35;
    let mut top = cy - label.height / 2.0;

    if lines.len() == 1 && lines[0].len() == 1 && lines[0][0].style == 0 {
        let only = &lines[0][0];
        return format!(
            "<text{} x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            attrs,
            num(cx),
            num(top + lh / 2.0 + shift),
            esc(&only.text)
        );
    }

    let shared = shared_style(label);
    let attrs = text_style(attrs, shared);
    let mut body = String::new();
    for line in lines {
        let mid = top + lh / 2.0;
        let y = mid + shift;
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
        top += lh;
    }
    format!("<text{} text-anchor=\"middle\" xml:space=\"preserve\">{}</text>", attrs, body)
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
            if span.text.is_empty() {
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
