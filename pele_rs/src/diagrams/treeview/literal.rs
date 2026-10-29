use crate::svg::builder::{esc, label_svg, shared_style, span_style, text_style};
use crate::text::label::{Label, Span};
use crate::text::measurer::TextMeasurer;
use crate::util::{is_js_space, js_round, num, trim_end};
pub(crate) fn literal_label(text: &str, m: &dyn TextMeasurer, size: f64, room: f64, style: u32) -> Label {
    let mut raw_lines = Vec::new();
    for raw in text.split('\n') {
        let chars: Vec<char> = raw.chars().collect();
        if chars.is_empty() {
            raw_lines.push(String::new())
        }
        let mut from = 0;
        while from < chars.len() {
            let fits = |length: usize| m.width(&chars[from..from + length].iter().collect::<String>(), size, style) <= room;
            let mut hi = 1;
            while hi < chars.len() - from && fits(hi) {
                hi = (chars.len() - from).min(hi * 2)
            }
            let mut lo = 1;
            while lo < hi {
                let mid = (lo + hi).div_ceil(2);
                if fits(mid) { lo = mid } else { hi = mid - 1 }
            }
            let mut take = lo;
            let wraps = from + take < chars.len();
            if wraps && !is_js_space(chars[from + take]) {
                for i in (1..take).rev() {
                    if is_js_space(chars[from + i]) {
                        take = i + 1;
                        break;
                    }
                }
            }
            let line: String = chars[from..from + take].iter().collect();
            let trimmed = trim_end(&line);
            raw_lines.push(if wraps && !trimmed.is_empty() { trimmed.to_string() } else { line });
            from += take;
            while wraps && from < chars.len() && is_js_space(chars[from]) {
                from += 1
            }
        }
    }
    if text.is_empty() {
        raw_lines.clear()
    }
    let widths: Vec<f64> = raw_lines.iter().map(|line| m.width(line, size, style)).collect();
    let width = widths.iter().copied().fold(0.0_f64, f64::max);
    let lh = js_round(size * 1.5);
    Label {
        lines: raw_lines
            .into_iter()
            .enumerate()
            .map(|(i, text)| {
                vec![Span {
                    text,
                    style,
                    width: widths[i],
                    ..Default::default()
                }]
            })
            .collect(),
        height: widths.len() as f64 * lh,
        widths,
        width,
        size,
        line_height: lh,
    }
}
pub(crate) fn left_label(label: &Label, x: f64, top: f64, attrs: &str) -> String {
    let shared = shared_style(label);
    let open = "<text".to_string() + &text_style(attrs, shared);
    let mut body = String::new();
    for (i, spans) in label.lines.iter().enumerate() {
        let cy = top + (i as f64 + 0.5) * label.line_height;
        if spans.iter().any(|s| s.icon.is_some()) {
            let mut line = label.clone();
            line.lines = vec![spans.clone()];
            line.widths = vec![label.widths[i]];
            line.width = label.widths[i];
            line.height = label.line_height;
            body += &label_svg(&line, x + line.width / 2.0, cy, attrs);
            continue;
        }
        let mut text = String::new();
        for span in spans {
            let own = span_style(span.style & !shared);
            text += &if own.is_empty() {
                esc(&span.text)
            } else {
                format!("<tspan{own}>{}</tspan>", esc(&span.text))
            }
        }
        if !text.is_empty() {
            body += &format!(
                "{open} x=\"{}\" y=\"{}\" text-anchor=\"start\" xml:space=\"preserve\">{text}</text>",
                num(x),
                num(cy + label.size * 0.35)
            )
        }
    }
    body
}
pub(crate) fn break_label(mut label: Label, m: &dyn TextMeasurer, room: f64) -> Label {
    if label.width <= room {
        return label;
    }
    let mut lines = Vec::new();
    let mut widths = Vec::new();
    for (i, spans) in label.lines.iter().enumerate() {
        if label.widths[i] <= room {
            lines.push(spans.clone());
            widths.push(label.widths[i]);
            continue;
        }
        let mut current: Vec<Span> = Vec::new();
        let mut width = 0.0;
        for span in spans {
            if !span.plain() {
                if !current.is_empty() && width + span.width > room {
                    lines.push(std::mem::take(&mut current));
                    widths.push(width);
                    width = 0.0
                }
                current.push(span.clone());
                width += span.width;
                continue;
            }
            for ch in span.text.chars() {
                let last = current.last();
                let joined = last.is_some_and(|s| s.plain() && s.style == span.style);
                let text = if joined {
                    last.unwrap().text.clone() + &ch.to_string()
                } else {
                    ch.to_string()
                };
                let next = m.width(&text, label.size, span.style);
                let mut added = next - if joined { last.unwrap().width } else { 0.0 };
                if !current.is_empty() && width + added > room {
                    lines.push(std::mem::take(&mut current));
                    widths.push(width);
                    width = 0.0;
                    added = m.width(&ch.to_string(), label.size, span.style)
                }
                if let Some(last) = current.last_mut().filter(|s| s.plain() && s.style == span.style) {
                    last.text.push(ch);
                    last.width += added
                } else {
                    let mut piece = span.clone();
                    piece.text = ch.to_string();
                    piece.width = added;
                    current.push(piece)
                }
                width += added
            }
        }
        if !current.is_empty() || spans.is_empty() {
            lines.push(current);
            widths.push(width)
        }
    }
    label.width = widths.iter().copied().fold(0.0_f64, f64::max);
    label.height = lines.len() as f64 * label.line_height;
    label.lines = lines;
    label.widths = widths;
    crate::text::label::with_heights(label)
}
