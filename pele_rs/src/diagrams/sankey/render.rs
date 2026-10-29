use super::{Model, layout};
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, title_room};
use crate::errors::PeleError;
use crate::preprocess::Config;
use crate::svg::builder::{esc, esc_text, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{resolve_style, series_color};
use crate::text::entities::decode_entities;
use crate::text::label::layout_label;
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{is_word_char, js_num, js_round, num};
use indexmap::IndexMap;
use serde_json::Value;
use std::collections::HashMap;
fn option(conf: &Value, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    conf.get(key)
        .and_then(Value::as_f64)
        .filter(|&v| v >= min)
        .map_or(fallback, |v| v.min(max))
}
fn text_option(conf: &Value, key: &str) -> String {
    match conf.get(key) {
        Some(Value::String(v)) => v.clone(),
        Some(Value::Number(v)) => v.as_f64().filter(|v| v.is_finite()).map(js_num).unwrap_or_default(),
        _ => String::new(),
    }
}
fn rounded(value: f64) -> f64 {
    if value.abs() < 1e15 {
        js_round(value * 100.0) / 100.0
    } else {
        value
    }
}
fn fill_style(color: Option<&Value>) -> String {
    color
        .and_then(Value::as_str)
        .map(|v| resolve_style(&[format!("fill:{v}")]).shape)
        .unwrap_or_default()
}
fn content_id(text: &str) -> String {
    fn radix(mut n: u32) -> String {
        if n == 0 {
            return "0".into();
        }
        let mut chars = vec![];
        while n > 0 {
            let c = n % 36;
            chars.push(if c < 10 { b'0' + c as u8 } else { b'a' + (c - 10) as u8 } as char);
            n /= 36;
        }
        chars.into_iter().rev().collect()
    }
    let (mut a, mut b) = (0x811c9dc5_u32, 5381_u32);
    for c in text.encode_utf16() {
        a = (a ^ c as u32).wrapping_mul(0x01000193);
        b = b.wrapping_mul(33).wrapping_add(c as u32);
    }
    radix(a) + &radix(b)
}
struct NodeLabel {
    value: String,
    width: f64,
    after: bool,
    left: f64,
    y: f64,
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Result<Rendered, PeleError> {
    let conf = config.get("sankey").filter(|v| v.is_object()).unwrap_or(&Value::Null);
    let natural = option(conf, "width", 600.0, 1.0, 10000.0);
    let mut result = draw(model, conf, options, measurer, natural, natural)?;
    let Some(available) = options.max_width.filter(|&a| a > 0.0) else {
        return Ok(result);
    };
    if result.width <= available {
        return Ok(result);
    }
    let floor = 160.0_f64.min(natural);
    let mut plot = natural;
    result = draw(model, conf, options, measurer, plot, natural)?;
    let mut round = 0;
    while round < 3 && result.width > available && plot > floor {
        plot = floor.max(plot - (result.width - available));
        result = draw(model, conf, options, measurer, plot, natural)?;
        round += 1;
    }
    Ok(result)
}
fn draw(
    model: &Model,
    conf: &Value,
    options: &RenderOptions,
    measurer: &dyn TextMeasurer,
    chart_width: f64,
    natural: f64,
) -> Result<Rendered, PeleError> {
    let size = options.font_size.unwrap_or(16.0);
    let least = js_round(size * 0.625);
    let mut small = least.max(js_round(size * 0.875 * 1.0_f64.min(chart_width / natural)));
    let pad = options.padding.unwrap_or(8.0);
    let chart_height = option(conf, "height", 400.0, 1.0, 10000.0);
    let node_width = option(conf, "nodeWidth", 10.0, 1.0, 200.0).min(chart_width);
    let show_values = conf.get("showValues").and_then(Value::as_bool) != Some(false);
    let prefix = decode_entities(&text_option(conf, "prefix"));
    let suffix = decode_entities(&text_option(conf, "suffix"));
    let outlined = conf.get("labelStyle").and_then(Value::as_str) == Some("outlined");
    let alignment = conf.get("nodeAlignment").and_then(Value::as_str).unwrap_or("justify");
    let link_color = conf.get("linkColor").and_then(Value::as_str).unwrap_or("gradient");
    let lay = |label_size| {
        layout::layout(
            model,
            &layout::Options {
                width: chart_width,
                height: chart_height,
                node_width,
                node_padding: option(conf, "nodePadding", 12.0, 0.0, 200.0) + if show_values { 15.0 } else { 0.0 },
                label_height: label_size * 1.15,
                alignment,
            },
        )
    };
    let mut layout = lay(small)?;
    let mut rows = HashMap::new();
    let mut fullest = 1;
    for node in &layout.nodes {
        let count = rows.entry(node.layer).or_insert(0);
        *count += 1;
        fullest = fullest.max(*count);
    }
    let crowded = least.max(js_round(chart_height / fullest as f64 / 1.8));
    if crowded < small {
        small = crowded;
        layout = lay(small)?;
    }
    let nodes = &layout.nodes;
    let links = &layout.links;
    let line_height = small * 1.5;
    let mut custom = HashMap::new();
    if let Some(colors) = conf.get("nodeColors").and_then(Value::as_object) {
        for (id, color) in colors {
            let style = fill_style(Some(color));
            if !style.is_empty() {
                custom.insert(id.as_str(), style);
            }
        }
    }
    let node_style = |i: usize| custom.get(model.nodes[i].as_str()).map(String::as_str).unwrap_or("");
    let node_color = |i: usize| {
        let s = node_style(i);
        s.strip_prefix(" style=\"fill:")
            .and_then(|v| v.strip_suffix(";\""))
            .map(String::from)
            .unwrap_or_else(|| series_color(i as i64))
    };
    let (mut central, mut most) = (0, 0.0);
    for node in nodes {
        if node.value > most {
            most = node.value;
            central = node.layer;
        }
    }
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (0.0_f64, chart_width, 0.0_f64, chart_height);
    let mut labels: Vec<_> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let name = decode_entities(&model.nodes[i]);
            let value = if show_values {
                format!("{prefix}{}{suffix}", js_num(rounded(node.value)))
            } else {
                String::new()
            };
            let width = measurer.width(&if value.is_empty() { name } else { format!("{name} {value}") }, small, 0);
            let after = if outlined {
                node.layer >= central
            } else {
                node.x < chart_width / 2.0
            };
            let x = if after { node.x + node_width + 6.0 } else { node.x - 6.0 };
            let y = node.y + node.height / 2.0;
            min_x = min_x.min(if after { x } else { x - width });
            max_x = max_x.max(if after { x + width } else { x });
            min_y = min_y.min(y - small * 0.75);
            max_y = max_y.max(y + small * 0.75);
            NodeLabel {
                value,
                width,
                after,
                left: if after { x } else { x - width },
                y,
            }
        })
        .collect();
    let mut bands: HashMap<i64, Vec<usize>> = HashMap::new();
    if nodes.len() <= 300 {
        for (i, label) in labels.iter().enumerate() {
            let band = (label.y / line_height).floor() as i64;
            for key in [band - 2, band - 1, band, band + 1, band + 2] {
                bands.entry(key).or_default().push(i);
            }
        }
    }
    let mut labels_out = String::new();
    let mut nodes_out = String::new();
    for (i, node) in nodes.iter().enumerate() {
        let id = &model.nodes[i];
        let name = decode_entities(id);
        let height = node.height.max(1.0);
        nodes_out += &format!(
            "<rect class=\"pele-sankey-node\" data-id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\"{}/>",
            esc_text(id),
            num(node.x),
            num(node.y),
            num(node_width),
            num(height),
            num(2.0_f64.min(height / 2.0)),
            series_color(i as i64),
            node_style(i)
        );
        let overlaps = |left: f64, y: f64| {
            bands.get(&((y / line_height).floor() as i64)).is_some_and(|near| {
                near.iter().any(|&j| {
                    let other = &labels[j];
                    j != i
                        && (other.y - y).abs() < line_height
                        && left < other.left + other.width + 6.0
                        && left + labels[i].width > other.left - 6.0
                })
            })
        };
        let mut chosen = None;
        if nodes.len() <= 300 && overlaps(labels[i].left, labels[i].y) {
            'place: for dy in [0.0, -line_height, line_height] {
                for after in [labels[i].after, !labels[i].after] {
                    let left = if after {
                        node.x + node_width + 6.0
                    } else {
                        node.x - 6.0 - labels[i].width
                    };
                    let y = labels[i].y + dy;
                    if overlaps(left, y)
                        || left < min_x
                        || left + labels[i].width > max_x
                        || y - small * 0.75 < min_y
                        || y + small * 0.75 > max_y
                    {
                        continue;
                    }
                    if nodes.iter().enumerate().any(|(j, bar)| {
                        j != i
                            && left < bar.x + node_width + 6.0
                            && left + labels[i].width > bar.x - 6.0
                            && y - small * 0.75 < bar.y + bar.height
                            && y + small * 0.75 > bar.y
                    }) {
                        continue;
                    }
                    let own = 0.0_f64.max(node.y - y).max(y - node.y - node.height);
                    if nodes
                        .iter()
                        .enumerate()
                        .any(|(j, bar)| j != i && bar.x == node.x && 0.0_f64.max(bar.y - y).max(y - bar.y - bar.height) <= own)
                    {
                        continue;
                    }
                    chosen = Some((left, y, after));
                    break 'place;
                }
            }
        }
        if let Some((left, y, after)) = chosen {
            labels[i].left = left;
            labels[i].y = y;
            labels[i].after = after;
        }
        let label = &labels[i];
        let x = if label.after { node.x + node_width + 6.0 } else { node.x - 6.0 };
        let position = format!(
            " x=\"{}\" y=\"{}\" text-anchor=\"{}\"",
            num(x),
            num(label.y + small * 0.35),
            if label.after { "start" } else { "end" }
        );
        let text = esc(&name)
            + &if label.value.is_empty() {
                String::new()
            } else {
                format!(
                    " <tspan class=\"pele-sankey-value\" fill=\"var(--_m)\">{}</tspan>",
                    esc(&label.value)
                )
            };
        labels_out += &format!(
            "<g class=\"pele-label\" data-id=\"{}\">{}<text{position}>{text}</text></g>",
            esc_text(id),
            if outlined {
                format!(
                    "<text{position} fill=\"none\" stroke=\"var(--_bg)\" stroke-width=\"3\" stroke-linejoin=\"round\">{}{}</text>",
                    esc(&name),
                    if label.value.is_empty() {
                        String::new()
                    } else {
                        format!(" {}", esc(&label.value))
                    }
                )
            } else {
                String::new()
            }
        );
    }
    let fixed = if matches!(link_color, "gradient" | "source" | "target") {
        String::new()
    } else {
        fill_style(Some(&Value::String(link_color.into())))
    };
    let id_prefix = options.id_prefix.as_deref().unwrap_or("");
    let gradient = link_color == "gradient" && !id_prefix.is_empty() && id_prefix.chars().all(|c| is_word_char(c) || c == '-');
    let mut gradients = IndexMap::new();
    let mut links_out = String::new();
    for link in links {
        let end = if link_color == "target" { link.target } else { link.source };
        let half = link.thickness.max(1.0) / 2.0;
        let x0 = nodes[link.source].x + node_width;
        let x1 = nodes[link.target].x;
        let mut paint = format!(
            "fill=\"{}\"{}",
            series_color(end as i64),
            if fixed.is_empty() { node_style(end) } else { &fixed }
        );
        if gradient {
            let stops = format!(
                "x1=\"{}\" x2=\"{}\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/>",
                num(x0),
                num(x1),
                node_color(link.source),
                node_color(link.target)
            );
            let id = format!("{id_prefix}-sankey-{}", content_id(&stops));
            gradients.insert(
                id.clone(),
                format!("<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" {stops}</linearGradient>"),
            );
            paint = format!("fill=\"url(#{id}) {}\"", node_color(link.source));
        }
        let mid = num((x0 + x1) / 2.0);
        let from = esc_text(&model.nodes[link.source]);
        let to = esc_text(&model.nodes[link.target]);
        links_out += &format!(
            "<path class=\"pele-sankey-link\" data-id=\"{from}-&gt;{to}\" d=\"M{},{}C{mid},{} {mid},{} {},{}L{},{}C{mid},{} {mid},{} {},{}Z\" {paint}><title>{from} → {to}: {}{}{}</title></path>",
            num(x0),
            num(link.source_y - half),
            num(link.source_y - half),
            num(link.target_y - half),
            num(x1),
            num(link.target_y - half),
            num(x1),
            num(link.target_y + half),
            num(link.target_y + half),
            num(link.source_y + half),
            num(x0),
            num(link.source_y + half),
            esc(&prefix),
            js_num(rounded(link.value)),
            esc(&suffix)
        );
    }
    let title = layout_label(model.title.as_deref(), false, measurer, size, title_room(options), BOLD);
    let title_height = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let inner = (max_x - min_x).max(title.width);
    let ox = pad - min_x + (inner - (max_x - min_x)) / 2.0;
    let oy = pad + title_height - min_y;
    let width = (inner + 2.0 * pad).ceil();
    let height = (max_y - min_y + title_height + 2.0 * pad).ceil();
    let content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &format!(
        "<g transform=\"translate({},{})\">{}<g class=\"pele-sankey-links\" fill-opacity=\"0.4\">{links_out}</g><g class=\"pele-sankey-nodes\">{nodes_out}</g><g class=\"pele-labels\" font-size=\"{}\">{labels_out}</g></g>",
        num(ox),
        num(oy),
        if gradients.is_empty() {
            String::new()
        } else {
            format!("<defs>{}</defs>", gradients.values().cloned().collect::<String>())
        },
        js_num(small)
    );
    let acc = Accessible {
        acc_title: None,
        acc_descr: None,
    };
    Ok(Rendered {
        svg: svg_document("sankey", width, height, size, options, &acc, &content),
        width,
        height,
        links: vec![],
    })
}
