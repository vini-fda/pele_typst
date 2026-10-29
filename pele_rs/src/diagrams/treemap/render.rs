use super::{Model, layout};
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, title_room};
use crate::preprocess::Config;
use crate::svg::builder::{esc, esc_text, label_svg};
use crate::svg::ink::ink_fill;
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{class_names, resolve_style, rounded, series_color};
use crate::text::entities::decode_entities;
use crate::text::label::layout_label;
use crate::text::measurer::{BOLD, ITALIC, TextMeasurer};
use crate::util::{js_num, js_round, num, trim, trim_end};
fn fixed(x: f64, p: usize) -> String {
    if !x.is_finite() || x >= 1e21 {
        return js_num(x);
    }
    let bits = x.to_bits();
    let mantissa = (bits & ((1_u64 << 52) - 1)) | if bits >> 52 & 2047 != 0 { 1 << 52 } else { 0 };
    let exp = ((bits >> 52 & 2047) as i32).max(1) - 1023 - 52 + p as i32;
    let n = mantissa as u128 * 5_u128.pow(p as u32);
    let scaled = if exp >= 0 {
        n.checked_shl(exp as u32).filter(|_| (128 - n.leading_zeros()) as i32 + exp <= 128)
    } else if -exp >= 128 {
        Some(0)
    } else {
        let shift = (-exp) as u32;
        Some((n >> shift) + u128::from((n & ((1_u128 << shift) - 1)) >= (1_u128 << (shift - 1))))
    };
    let Some(scaled) = scaled else { return format!("{x:.p$}") };
    let mut digits = scaled.to_string();
    if p == 0 {
        return digits;
    }
    if digits.len() <= p {
        digits = "0".repeat(p + 1 - digits.len()) + &digits
    }
    digits.insert(digits.len() - p, '.');
    digits
}
fn value_format(spec: &str, value: f64) -> String {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"^(?:.?[<>=^])?([-+( ])?(\$)?0?\d*(,)?(?:\.(\d+))?(~)?([a-zA-Z%])?$").unwrap());
    let spec = if spec == "$0,0" { "$," } else { spec };
    let c = re.captures(spec).unwrap_or_else(|| re.captures(",").unwrap());
    let plus = c.get(1).is_some_and(|m| m.as_str() == "+");
    let comma = c.get(3).is_some();
    let precision = c.get(4).map(|m| m.as_str().parse::<usize>().unwrap_or(20).min(20));
    let kind = c.get(6).map(|m| m.as_str()).unwrap_or("");
    let abs = value.abs();
    let mut suffix = "";
    let mut body = match kind {
        "f" => fixed(abs, precision.unwrap_or(6)),
        "%" => {
            suffix = "%";
            fixed(abs * 100.0, precision.unwrap_or(6))
        }
        "d" => fixed(js_round(abs), 0),
        "e" => {
            let p = precision.unwrap_or(6);
            let s = format!("{abs:.p$e}");
            let (a, b) = s.split_once('e').unwrap();
            let exponent = b.parse::<i32>().unwrap_or(0);
            format!("{a}e{}{exponent}", if exponent >= 0 { "+" } else { "" })
        }
        _ => {
            let p = precision.unwrap_or(12).max(1) - 1;
            let s = format!("{abs:.p$e}");
            js_num(s.parse().unwrap_or(abs))
        }
    };
    if c.get(5).is_some() && body.contains('.') && !body.contains('e') {
        while body.ends_with('0') {
            body.pop();
        }
        if body.ends_with('.') {
            body.pop();
        }
    }
    if comma && !body.contains('e') {
        let (digits, fraction) = body.split_once('.').map(|(a, b)| (a, Some(b))).unwrap_or((&body, None));
        let mut grouped = String::new();
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                grouped.push(',')
            }
            grouped.push(ch)
        }
        if let Some(f) = fraction {
            grouped.push('.');
            grouped.push_str(f)
        }
        body = grouped
    }
    format!(
        "{}{}{body}{suffix}",
        if value < 0.0 {
            "-"
        } else if plus {
            "+"
        } else {
            ""
        },
        c.get(2).map(|m| m.as_str()).unwrap_or("")
    )
}
struct Fitted {
    text: String,
    size: f64,
}
fn fit_text(text: &str, max: f64, size: f64, min: f64, m: &dyn TextMeasurer, style: u32) -> Option<Fitted> {
    if text.is_empty() || max <= 0.0 {
        return None;
    }
    let mut width = m.width(text, size, style);
    if width <= max {
        return Some(Fitted { text: text.into(), size });
    }
    let mut fitted = min.max((size * max / width).floor());
    while fitted >= min {
        width = m.width(text, fitted, style);
        if width <= max {
            return Some(Fitted {
                text: text.into(),
                size: fitted,
            });
        }
        fitted -= 1.0
    }
    let chars: Vec<char> = text.chars().collect();
    let mut low = 0;
    let mut high = chars.len() - 1;
    while low < high {
        let mid = (low + high).div_ceil(2);
        let s: String = chars[..mid].iter().collect();
        if m.width(&(trim_end(&s).to_string() + "…"), min, style) <= max {
            low = mid
        } else {
            high = mid - 1
        }
    }
    if low == 0 {
        return None;
    }
    let s: String = chars[..low].iter().collect();
    let cut = trim_end(&s).to_string() + "…";
    if m.width(&cut, min, style) <= max {
        Some(Fitted { text: cut, size: min })
    } else {
        None
    }
}
fn option(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    config
        .get("treemap")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_f64())
        .filter(|v| *v >= min)
        .map(|v| v.min(max))
        .unwrap_or(fallback)
}
fn corners(attrs: &str, b: &layout::Box) -> String {
    let cap = b.w.min(b.h) / 4.0;
    if cap < 4.0 {
        rounded(attrs).replace("rx:var(--_r)", &format!("rx:min(var(--_r),{}px)", num(cap)))
    } else {
        rounded(attrs)
    }
}
fn title_part(raw: &str) -> String {
    trim(&esc(trim(&decode_entities(raw)))).into()
}
pub fn render(model: &Model, config: &Config, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let tiny = js_round(size * 0.75);
    let least = js_round(size * 0.5625);
    let conf = config.get("treemap");
    let pad = o.padding.unwrap_or_else(|| option(config, "diagramPadding", 8.0, 0.0, 2000.0));
    let natural = option(config, "nodeWidth", 64.0, 1.0, 400.0) * 10.0;
    let width = js_round(
        o.max_width
            .filter(|x| *x > 0.0)
            .map(|x| natural.min(200.0_f64.max(x - 2.0 * pad)))
            .unwrap_or(natural),
    );
    let height = js_round(option(config, "nodeHeight", 40.0, 1.0, 400.0) * 10.0);
    let show_values = conf.and_then(|v| v.get("showValues")).and_then(|v| v.as_bool()) != Some(false);
    let spec = conf.and_then(|v| v.get("valueFormat")).and_then(|v| v.as_str()).unwrap_or(",");
    let leaf_count = model
        .nodes
        .iter()
        .filter(|n| n.children.is_none() && n.value.is_some_and(|v| v.is_finite() && v > 0.0))
        .count();
    let crowd = (width * height / leaf_count.max(1) as f64 / 800.0).log10().max(0.0).min(1.0);
    let (boxes, placed) = layout::layout(
        model,
        width,
        height,
        option(config, "padding", js_round(2.0 + 2.0 * crowd), 0.0, 200.0),
        js_round(3.0 + 3.0 * crowd),
    );
    let title = layout_label(model.title.as_deref(), false, m, size, title_room(o), BOLD);
    let title_h = if title.height > 0.0 { title.height + 8.0 } else { 0.0 };
    let left = pad + 0.0_f64.max((title.width - width) / 2.0);
    let top = pad + title_h;
    let mut sections = String::new();
    let mut leaves = String::new();
    for index in placed {
        let b = &boxes[index];
        let node = &model.nodes[b.node];
        let style = resolve_style(&node.css_compiled_styles);
        let color = series_color(b.series as i64);
        let x = left + b.x;
        let y = top + b.y;
        let name = trim(&decode_entities(&node.name)).to_string();
        let value = if show_values { value_format(spec, b.value) } else { String::new() };
        let group = format!("data-id=\"{}\">", esc_text(&node.name));
        let own = title_part(&node.name);
        let mut path = if own.is_empty() { Vec::new() } else { vec![own.clone()] };
        let mut parent = b.parent;
        while let Some(p) = parent {
            if boxes[p].headed {
                break;
            }
            let section = title_part(&model.nodes[boxes[p].node].name);
            if !section.is_empty() {
                path.insert(0, section)
            }
            parent = boxes[p].parent
        }
        let full = path.join(" › ");
        let tooltip = |drawn: Option<&str>| {
            if full.is_empty() || (drawn == Some(name.as_str()) && full == own) {
                String::new()
            } else {
                format!("<title>{full}</title>")
            }
        };
        let rect = format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{color}\"",
            num(x),
            num(y),
            num(b.w),
            num(b.h)
        );
        let fixed = style.font_size;
        let text_style = if style.bold { BOLD } else { 0 } | if style.italic { ITALIC } else { 0 };
        let ink = ink_fill(&style.shape, &style.text, None);
        if node.children.is_some() {
            let mut text = String::new();
            let mut drawn = None;
            if b.headed {
                let short = b.header < 28.0;
                let label_size = fixed.unwrap_or(if short { tiny } else { small });
                let value_size = fixed.unwrap_or(tiny);
                let room = b.w - 16.0;
                let value_width = if !value.is_empty() {
                    m.width(&value, value_size, text_style)
                } else {
                    0.0
                };
                let with_value = !short && value_width > 0.0 && room - value_width - 12.0 >= 3.0 * label_size;
                let label = if label_size * 1.25 < b.header {
                    fit_text(
                        &name,
                        if with_value { room - value_width - 12.0 } else { room },
                        label_size,
                        label_size,
                        m,
                        text_style | BOLD,
                    )
                } else {
                    None
                };
                let baseline = y + b.header / 2.0 + 1.0;
                if let Some(l) = label {
                    drawn = Some(l.text.clone());
                    text += &format!(
                        "<text x=\"{}\" y=\"{}\" font-weight=\"var(--_hw)\"{}{ink}{}>{}</text>",
                        num(x + 8.0),
                        num(baseline + label_size * 0.35),
                        if short {
                            format!(" font-size=\"{}\"", js_num(label_size))
                        } else {
                            String::new()
                        },
                        style.text,
                        esc(&l.text)
                    )
                }
                if with_value && value_size * 1.25 <= 28.0 {
                    text += &format!(
                        "<text class=\"pele-value\" x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"end\"{}{}>{}</text>",
                        num(x + b.w - 8.0),
                        num(baseline + value_size * 0.35),
                        js_num(tiny),
                        if ink.is_empty() { " fill=\"var(--_m)\"" } else { &ink },
                        style.text,
                        esc(&value)
                    )
                }
            }
            let tint = if style.shape.contains("\"fill:") || style.shape.contains(";fill:") {
                ""
            } else {
                " fill-opacity=\"0.12\""
            };
            let shape = if b.headed {
                format!("{rect}{}/>", corners(&format!("{tint} stroke=\"{color}\"{}", style.shape), b))
            } else {
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\"/>",
                    num(x),
                    num(y),
                    num(b.w),
                    num(b.h)
                )
            };
            sections += &format!(
                "<g class=\"pele-node pele-section{}\" {group}{}{shape}{text}</g>",
                class_names(node.class_selector.as_deref().unwrap_or("")),
                tooltip(drawn.as_deref())
            );
            continue;
        }
        let mut text = String::new();
        let room = b.w - 8.0;
        let smallest = fixed.unwrap_or(least);
        let tallest = if let Some(f) = fixed {
            if f * 1.25 <= b.h - 4.0 { f } else { 0.0 }
        } else {
            size.min(((b.h - 4.0) / 1.25).floor())
        };
        let label = if tallest >= smallest {
            fit_text(&name, room, tallest, smallest, m, text_style)
        } else {
            None
        };
        if let Some(l) = &label {
            let cx = num(x + b.w / 2.0);
            let cy = y + b.h / 2.0;
            let value_size = fixed.unwrap_or(least.max(js_round(l.size * 0.8)));
            let lines = (l.size + value_size) * 1.25;
            if !value.is_empty() && lines <= b.h - 4.0 && m.width(&value, value_size, text_style) <= room {
                let first = cy - lines / 2.0 + l.size * 1.25 / 2.0;
                let second = cy + lines / 2.0 - value_size * 1.25 / 2.0;
                text = format!(
                    "<text x=\"{cx}\" y=\"{}\" font-size=\"{}\"{ink}{}>{}</text><text class=\"pele-value\" x=\"{cx}\" y=\"{}\" font-size=\"{}\"{ink}{}>{}</text>",
                    num(first + l.size * 0.35),
                    num(l.size),
                    style.text,
                    esc(&l.text),
                    num(second + value_size * 0.35),
                    num(value_size),
                    style.text,
                    esc(&value)
                )
            } else {
                text = format!(
                    "<text x=\"{cx}\" y=\"{}\" font-size=\"{}\"{ink}{}>{}</text>",
                    num(cy + l.size * 0.35),
                    num(l.size),
                    style.text,
                    esc(&l.text)
                )
            }
        }
        leaves += &format!(
            "<g class=\"pele-node pele-leaf{}\" {group}{}{rect}{}/>{text}</g>",
            class_names(node.class_selector.as_deref().unwrap_or("")),
            tooltip(label.as_ref().map(|l| l.text.as_str())),
            corners(&style.shape, b)
        );
    }
    let tw = (width.max(title.width) + 2.0 * pad).ceil();
    let th = (height + title_h + 2.0 * pad).ceil();
    let mut content = label_svg(
        &title,
        tw / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !sections.is_empty() {
        content += &format!("<g class=\"pele-sections\" font-size=\"{}\">{sections}</g>", js_num(small))
    }
    if !leaves.is_empty() {
        content += &format!("<g class=\"pele-leaves\" text-anchor=\"middle\" fill=\"var(--_bg)\">{leaves}</g>")
    }
    Rendered {
        svg: svg_document(
            "treemap",
            tw,
            th,
            size,
            o,
            &Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            &content,
        ),
        width: tw,
        height: th,
        links: Vec::new(),
    }
}
