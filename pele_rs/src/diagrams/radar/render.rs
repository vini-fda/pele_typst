use super::Model;
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, fit_width, title_room};
use crate::preprocess::Config;
use crate::svg::builder::{esc_text, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::series_color;
use crate::text::label::layout_label;
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{js_num, js_round, num};
use std::f64::consts::PI;

fn option(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    config
        .get("radar")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_f64())
        .filter(|&v| v >= min)
        .map_or(fallback, |v| v.min(max))
}
fn relative(value: f64, min: f64, max: f64, radius: f64) -> f64 {
    let clipped = value.max(min).min(max);
    let scaled = radius * (clipped - min);
    if scaled.is_finite() {
        scaled / (max - min)
    } else {
        radius * ((clipped - min) / (max - min))
    }
}
fn polygon(points: &[(f64, f64)]) -> String {
    points
        .iter()
        .map(|&(x, y)| format!("{},{}", num(x), num(y)))
        .collect::<Vec<_>>()
        .join(" ")
}
fn closed_curve(points: &[(f64, f64)], tension: f64) -> String {
    if points.is_empty() {
        return String::new();
    }
    let n = points.len();
    let mut d = format!("M{},{}", num(points[0].0), num(points[0].1));
    for i in 0..n {
        let p0 = points[(i + n - 1) % n];
        let p1 = points[i];
        let p2 = points[(i + 1) % n];
        let p3 = points[(i + 2) % n];
        d += &format!(
            " C{},{} {},{} {},{}",
            num(p1.0 + (p2.0 - p0.0) * tension),
            num(p1.1 + (p2.1 - p0.1) * tension),
            num(p2.0 - (p3.0 - p1.0) * tension),
            num(p2.1 - (p3.1 - p1.1) * tension),
            num(p2.0),
            num(p2.1)
        );
    }
    d + " Z"
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    let natural = option(config, "width", 320.0, 1.0, 4000.0);
    fit_width(options, natural, 160.0, |width, tight| {
        draw(
            model,
            config,
            options,
            measurer,
            width,
            option(config, "height", 320.0, 1.0, 4000.0) * width / natural,
            tight,
        )
    })
}
fn draw(
    model: &Model,
    config: &Config,
    options: &RenderOptions,
    measurer: &dyn TextMeasurer,
    width: f64,
    height: f64,
    tight: bool,
) -> Rendered {
    let size = options.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let pad = options.padding.unwrap_or(8.0);
    let axis_scale = option(config, "axisScaleFactor", 1.0, 0.0, 10.0);
    let label_factor = option(config, "axisLabelFactor", 1.05, 0.0, 10.0);
    let tension = option(config, "curveTension", 0.17, 0.0, 1.0);
    let radius = width.min(height) / 2.0;
    let count = model.axes.len();
    let unit: Vec<_> = model
        .axes
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let a = 2.0 * PI * i as f64 / count as f64 - PI / 2.0;
            (a.cos(), a.sin())
        })
        .collect();
    let max = model.options.max.unwrap_or_else(|| {
        model
            .curves
            .iter()
            .flat_map(|c| c.entries.iter())
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    });
    let mut x0 = -width / 2.0 - option(config, "marginLeft", 16.0, 0.0, 2000.0);
    let mut x1 = width / 2.0 + option(config, "marginRight", 16.0, 0.0, 2000.0);
    let mut y0 = -height / 2.0 - option(config, "marginTop", 16.0, 0.0, 2000.0);
    let mut y1 = height / 2.0 + option(config, "marginBottom", 16.0, 0.0, 2000.0);
    let mut grid = String::new();
    let mut i = 0;
    while (i as f64) < model.options.ticks {
        let r = radius.min(radius * (i + 1) as f64 / model.options.ticks);
        if model.options.graticule == "circle" {
            grid += &format!("<circle r=\"{}\"/>", num(r));
        } else {
            grid += &format!(
                "<polygon points=\"{}\"/>",
                polygon(&unit.iter().map(|&(x, y)| (x * r, y * r)).collect::<Vec<_>>())
            );
        }
        i += 1;
    }
    let mut spokes = String::new();
    let mut labels = String::new();
    for (i, &(ux, uy)) in unit.iter().enumerate() {
        let reach = radius * axis_scale;
        spokes += &format!("M0,0L{},{}", num(ux * reach), num(uy * reach));
        let label = layout_label(Some(&model.axes[i].label), false, measurer, small, 4000.0, 0);
        let at = radius * label_factor + 6.0;
        let cx = ux * at
            + if ux > 0.01 {
                label.width / 2.0
            } else if ux < -0.01 {
                -label.width / 2.0
            } else {
                0.0
            };
        let cy = uy * at
            + if uy > 0.01 {
                label.height / 2.0
            } else if uy < -0.01 {
                -label.height / 2.0
            } else {
                0.0
            };
        labels += &format!(
            "<g class=\"pele-axis-label\" data-id=\"{}\">{}</g>",
            esc_text(&model.axes[i].name),
            label_svg(&label, cx, cy, "")
        );
        x0 = x0.min(cx - label.width / 2.0);
        x1 = x1.max(cx + label.width / 2.0);
        y0 = y0.min(cy - label.height / 2.0);
        y1 = y1.max(cy + label.height / 2.0);
    }
    let mut shapes = String::new();
    for (index, curve) in model.curves.iter().enumerate() {
        if curve.entries.len() != count {
            continue;
        }
        let points: Vec<_> = curve
            .entries
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let r = relative(v, model.options.min, max, radius);
                let safe = if r.is_finite() { r } else { 0.0 };
                (unit[i].0 * safe, unit[i].1 * safe)
            })
            .collect();
        let color = series_color(index as i64);
        let attrs = format!(
            " class=\"pele-curve\" data-id=\"{}\" fill=\"{color}\" stroke=\"{color}\"",
            esc_text(&curve.name)
        );
        if model.options.graticule == "circle" {
            shapes += &format!("<path{attrs} d=\"{}\"/>", closed_curve(&points, tension));
        } else {
            shapes += &format!("<polygon{attrs} points=\"{}\"/>", polygon(&points));
        }
    }
    let legend: Vec<_> = if model.options.show_legend {
        model
            .curves
            .iter()
            .map(|c| layout_label(Some(&c.label), false, measurer, small, 4000.0, 0))
            .collect()
    } else {
        vec![]
    };
    let legend_width = legend.iter().map(|l| l.width).fold(0.0, f64::max);
    let legend_height = legend.len() as f64 * 26.0;
    let legend_x = if tight { -(12.0 + 8.0 + legend_width) / 2.0 } else { x1 + 24.0 };
    let legend_top = if tight { y1 + 12.0 } else { -legend_height / 2.0 };
    if !legend.is_empty() && tight {
        x0 = x0.min(legend_x);
        x1 = x1.max(-legend_x);
        y1 = legend_top + legend_height;
    } else if !legend.is_empty() {
        x1 = legend_x + 12.0 + 8.0 + legend_width;
        y0 = y0.min(-legend_height / 2.0);
        y1 = y1.max(legend_height / 2.0);
    }
    let mut legend_out = String::new();
    for (index, label) in legend.iter().enumerate() {
        let y = legend_top + index as f64 * 26.0 + 13.0;
        let color = series_color(index as i64);
        legend_out += &format!(
            "<g class=\"pele-legend-item\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"12\" height=\"12\" rx=\"2\" fill=\"{color}\" fill-opacity=\"0.4\" stroke=\"{color}\"/>{}</g>",
            esc_text(&model.curves[index].name),
            num(legend_x),
            num(y - 6.0),
            label_svg(label, legend_x + 12.0 + 8.0 + label.width / 2.0, y, "")
        );
    }
    let title = layout_label(model.title.as_deref(), false, measurer, size, title_room(options), BOLD);
    let title_height = if title.height > 0.0 { title.height + 8.0 } else { 0.0 };
    let body_width = (x1 - x0).max(title.width);
    let total_width = (body_width + 2.0 * pad).ceil();
    let total_height = (y1 - y0 + title_height + 2.0 * pad).ceil();
    let origin = format!(
        "translate({},{})",
        num(pad + (body_width - (x1 - x0)) / 2.0 - x0),
        num(pad + title_height - y0)
    );
    let mut content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &format!("<g transform=\"{origin}\">");
    if !grid.is_empty() {
        content += &format!("<g class=\"pele-graticule\" fill=\"none\" stroke=\"var(--_a)\">{grid}</g>");
    }
    if !spokes.is_empty() {
        content += &format!("<path class=\"pele-axes\" d=\"{spokes}\" fill=\"none\" stroke=\"var(--_b)\"/>");
    }
    if !shapes.is_empty() {
        content += &format!("<g class=\"pele-curves\" fill-opacity=\"0.2\" stroke-width=\"1.5\" stroke-linejoin=\"round\">{shapes}</g>");
    }
    if !labels.is_empty() {
        content += &format!("<g class=\"pele-axis-labels\" font-size=\"{}\">{labels}</g>", js_num(small));
    }
    if !legend_out.is_empty() {
        content += &format!("<g class=\"pele-legend\" font-size=\"{}\">{legend_out}</g>", js_num(small));
    }
    content += "</g>";
    let acc = Accessible {
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    };
    Rendered {
        svg: svg_document("radar", total_width, total_height, size, options, &acc, &content),
        width: total_width,
        height: total_height,
        links: vec![],
    }
}
