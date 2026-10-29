// Draws a pie chart, after pele's diagrams/pie/render.ts.

use std::f64::consts::PI;

use serde_json::Value;

use super::model::PieModel;
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, fit_width, title_room};
use crate::preprocess::{Config, value_string};
use crate::svg::builder::{esc_text, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::series_color;
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{js_num, js_round, num};

const RADIUS: f64 = 120.0;
const SWATCH: f64 = 12.0;
const ROW: f64 = 26.0;
const GAP: f64 = 32.0;
const OUTSIDE: f64 = 28.0;

fn option<'a>(config: &'a Config, key: &str) -> Option<&'a Value> {
    config.get("pie").and_then(|p| p.as_object()).and_then(|p| p.get(key))
}

fn point(angle: f64, radius: f64) -> String {
    format!("{},{}", num(angle.sin() * radius), num(-angle.cos() * radius))
}

fn slice(start: f64, end: f64, outer: f64, inner: f64) -> String {
    if end - start >= PI * 2.0 - 1e-9 {
        let ring = |r: f64, sweep: u8| {
            format!(
                "M0,{}A{},{} 0 1 {} 0,{}A{},{} 0 1 {} 0,{}Z",
                num(-r),
                num(r),
                num(r),
                sweep,
                num(r),
                num(r),
                num(r),
                sweep,
                num(-r)
            )
        };
        return ring(outer, 1) + &if inner > 0.0 { ring(inner, 0) } else { String::new() };
    }
    let large = if end - start > PI { 1 } else { 0 };
    if inner <= 0.0 {
        return format!(
            "M0,0L{}A{},{} 0 {} 1 {}Z",
            point(start, outer),
            num(outer),
            num(outer),
            large,
            point(end, outer)
        );
    }
    format!(
        "M{}A{},{} 0 {} 1 {}L{}A{},{} 0 {} 0 {}Z",
        point(start, outer),
        num(outer),
        num(outer),
        large,
        point(end, outer),
        point(end, inner),
        num(inner),
        num(inner),
        large,
        point(start, inner)
    )
}

// Number.prototype.toFixed(0) of a share in percent, which is never negative.
fn percent(share: f64) -> String {
    js_num(js_round(share * 100.0))
}

pub fn render_pie(model: &PieModel, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    fit_width(options, RADIUS * 2.0, 140.0, |diameter, tight| {
        draw(model, config, options, measurer, diameter / 2.0, tight)
    })
}

struct Placed {
    x: f64,
    y: f64,
    width: f64,
}

// `tight` is set when the chart does not fit the width it has with its legend beside it.
fn draw(model: &PieModel, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer, radius: f64, tight: bool) -> Rendered {
    let size = options.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let pad = options.padding.unwrap_or(8.0);

    let text_position = match option(config, "textPosition").and_then(Value::as_f64) {
        Some(p) if (0.0..=1.0).contains(&p) => p,
        _ => 0.75,
    };
    let inner = match option(config, "donutHole").and_then(Value::as_f64) {
        Some(h) if h > 0.0 && h <= 0.9 => h * radius,
        _ => 0.0,
    };
    let placed = match option(config, "legendPosition") {
        None | Some(Value::Null) => "right".to_string(),
        Some(v) => value_string(v),
    };
    let legend_position = if tight && (placed == "left" || placed == "right") {
        "bottom".to_string()
    } else {
        placed
    };

    let entries: Vec<(&String, f64)> = model.sections.iter().map(|(k, &v)| (k, v)).collect();
    let mut sum = 0.0;
    for (_, value) in &entries {
        sum += value;
    }

    let legend: Vec<Label> = entries
        .iter()
        .map(|(label, value)| {
            let text = if model.show_data {
                format!("{} [{}]", label, js_num(*value))
            } else {
                label.to_string()
            };
            layout_label(Some(&text), false, measurer, small, 4000.0, 0)
        })
        .collect();
    let mut legend_width: f64 = 0.0;
    for label in &legend {
        legend_width = legend_width.max(label.width);
    }
    legend_width += if legend.is_empty() { 0.0 } else { SWATCH + 8.0 };
    let legend_height = legend.len() as f64 * ROW;

    let title = layout_label(model.title.as_deref(), false, measurer, size, title_room(options), BOLD);
    let title_height = if title.height > 0.0 { title.height + 16.0 } else { 0.0 };

    // A slice too thin for its percentage gets the label outside the rim, which needs a margin.
    let shares: Vec<f64> = entries
        .iter()
        .map(|(_, value)| {
            let share = if sum > 0.0 { value / sum } else { 0.0 };
            if share * 100.0 >= 1.0 && percent(share) != "0" {
                share
            } else {
                0.0
            }
        })
        .collect();
    let narrow: Vec<bool> = shares
        .iter()
        .map(|&share| {
            if share == 0.0 || share >= 0.5 {
                return false;
            }
            let chord = 2.0 * radius * text_position * (share * PI).sin();
            chord < measurer.width(&format!("{}%", percent(share)), small, 0) + 6.0
        })
        .collect();
    let margin = if narrow.contains(&true) { OUTSIDE } else { 0.0 };

    let lp = legend_position.as_str();
    let side = if matches!(lp, "left" | "right" | "center") { lp } else { "" };
    let stacked = lp == "top" || lp == "bottom";
    let diameter = (radius + margin) * 2.0;
    let mut width = diameter;
    let mut height = diameter;
    if side == "left" || side == "right" {
        width += if legend_width > 0.0 { GAP + legend_width } else { 0.0 };
        height = height.max(legend_height);
    } else if stacked {
        width = width.max(legend_width);
        height += if legend_height > 0.0 { 20.0 + legend_height } else { 0.0 };
    }
    width = width.max(title.width);

    let cx = pad
        + match side {
            "left" => legend_width + GAP + diameter / 2.0,
            "right" => diameter / 2.0,
            _ => width / 2.0,
        };
    let body_top = pad + title_height;
    let cy = body_top
        + if lp == "top" {
            legend_height + 20.0 + diameter / 2.0
        } else if stacked {
            diameter / 2.0
        } else {
            height / 2.0
        };

    let mut slices = String::new();
    let mut labels = String::new();
    let mut outside = String::new();
    let mut angle = 0.0;
    let rim = radius + OUTSIDE / 2.0;
    let mut placed_outside: Vec<Placed> = Vec::new();
    for (index, (label, _)) in entries.iter().enumerate() {
        let share = shares[index];
        if share == 0.0 {
            continue;
        }
        let end = angle + share * PI * 2.0;
        slices += &format!(
            "<path class=\"pele-slice\" data-id=\"{}\" d=\"{}\" fill=\"{}\"/>",
            esc_text(label),
            slice(angle, end, radius, inner),
            series_color(index as i64)
        );
        let mid = (angle + end) / 2.0;
        let text = format!("{}%", percent(share));
        if narrow[index] {
            let width = measurer.width(&text, small, 0);
            let clear = |x: f64, y: f64, placed: &[Placed]| {
                placed
                    .iter()
                    .all(|o| (x - o.x).abs() >= (width + o.width) / 2.0 + 2.0 || (y - o.y).abs() >= small + 2.0)
            };
            let mut at = mid;
            let mut step = 0;
            while step < 360 && !clear(at.sin() * rim, -at.cos() * rim, &placed_outside) {
                at += 2.0 / rim;
                step += 1;
            }
            let x = at.sin() * rim;
            let y = -at.cos() * rim;
            placed_outside.push(Placed { x, y, width });
            outside += &format!("<text x=\"{}\" y=\"{}\">{}</text>", num(x), num(y + small * 0.35), text);
        } else {
            let r = radius * text_position;
            labels += &format!(
                "<text x=\"{}\" y=\"{}\">{}</text>",
                num(mid.sin() * r),
                num(-mid.cos() * r + small * 0.35),
                text
            );
        }
        angle = end;
    }
    if slices.is_empty() {
        slices = format!(
            "<circle class=\"pele-slice\" r=\"{}\" fill=\"none\" stroke=\"var(--_b)\"/>",
            js_num(radius)
        );
    }

    let (lx, ly) = match side {
        "right" => (cx + diameter / 2.0 + GAP, body_top + (height - legend_height) / 2.0),
        "left" => (pad, body_top + (height - legend_height) / 2.0),
        "center" => (cx - legend_width / 2.0, cy - legend_height / 2.0),
        _ => (
            pad + (width - legend_width) / 2.0,
            if lp == "top" { body_top } else { body_top + diameter + 20.0 },
        ),
    };
    let mut legend_out = String::new();
    for (index, label) in legend.iter().enumerate() {
        let y = ly + index as f64 * ROW + ROW / 2.0;
        legend_out += &format!(
            "<g class=\"pele-legend-item\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{}\"/>{}</g>",
            esc_text(entries[index].0),
            num(lx),
            num(y - SWATCH / 2.0),
            js_num(SWATCH),
            js_num(SWATCH),
            series_color(index as i64),
            label_svg(label, lx + SWATCH + 8.0 + label.width / 2.0, y, "")
        );
    }

    let total_width = (width + 2.0 * pad).ceil();
    let total_height = (height + title_height + 2.0 * pad).ceil();
    let translate = format!("translate({},{})", num(cx), num(cy));
    let mut content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    content += &format!(
        "<g class=\"pele-slices\" transform=\"{}\" stroke=\"var(--_bg)\" stroke-width=\"1\" stroke-linejoin=\"round\">{}</g>",
        translate, slices
    );
    if !labels.is_empty() {
        content += &format!(
            "<g class=\"pele-slice-labels\" transform=\"{}\" font-size=\"{}\" text-anchor=\"middle\" fill=\"var(--_bg)\">{}</g>",
            translate,
            js_num(small),
            labels
        );
    }
    if !outside.is_empty() {
        content += &format!(
            "<g class=\"pele-slice-labels\" transform=\"{}\" font-size=\"{}\" text-anchor=\"middle\">{}</g>",
            translate,
            js_num(small),
            outside
        );
    }
    if !legend_out.is_empty() {
        content += &format!("<g class=\"pele-legend\" font-size=\"{}\">{}</g>", js_num(small), legend_out);
    }

    let acc = Accessible {
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    };
    Rendered {
        svg: svg_document("pie", total_width, total_height, size, options, &acc, &content),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}
