use super::Model;
use crate::RenderOptions;
use crate::diagrams::common::Rendered;
use crate::errors::{PeleError, PeleErrorCode};
use crate::preprocess::Config;
use crate::svg::builder::{esc, esc_text, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::series_color;
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{js_num, js_round, num};
use serde_json::Value;

struct Mark {
    at: f64,
    label: Label,
}
fn on(c: &Value, key: &str, fallback: bool) -> bool {
    c.get(key).and_then(Value::as_bool).unwrap_or(fallback)
}
fn dimension(v: Option<&Value>, fallback: f64) -> f64 {
    v.and_then(Value::as_f64)
        .filter(|&v| v > 0.0)
        .map_or(fallback, |v| js_round(v.clamp(100.0, 4000.0)))
}
fn finite(v: Option<f64>) -> Option<f64> {
    v.filter(|v| v.is_finite()).map(|v| v.clamp(-1e300, 1e300))
}
fn widest(marks: &[Mark]) -> f64 {
    marks.iter().map(|m| m.label.width).fold(0.0, f64::max)
}
fn aligned(label: &Label, x: f64, y: f64, anchor: &str) -> String {
    if label.lines.len() == 1 && label.lines[0].len() == 1 && label.lines[0][0].style == 0 {
        return format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\">{}</text>",
            num(x),
            num(y + label.size * 0.35),
            esc(&label.lines[0][0].text)
        );
    }
    label_svg(
        label,
        x + match anchor {
            "start" => label.width / 2.0,
            "end" => -label.width / 2.0,
            _ => 0.0,
        },
        y,
        "",
    )
}
fn tick_step(span: f64, count: f64) -> f64 {
    let raw = span / count.max(1.0);
    let power = raw.log10().floor();
    let unit = raw / 10.0_f64.powf(power);
    let factor = if unit >= 7.07 {
        10.0
    } else if unit >= 3.16 {
        5.0
    } else if unit >= 1.41 {
        2.0
    } else {
        1.0
    };
    if power < 0.0 {
        -10.0_f64.powf(-power) / factor
    } else {
        factor * 10.0_f64.powf(power)
    }
}
fn tick_index(step: f64, value: f64) -> f64 {
    if step > 0.0 { value / step } else { value * -step }
}
fn tick_value(step: f64, index: f64) -> f64 {
    if step > 0.0 { index * step } else { index / -step }
}
fn ticks(lo: f64, hi: f64, step: f64) -> Vec<f64> {
    let mut first = js_round(tick_index(step, lo));
    if tick_value(step, first) < lo {
        first += 1.0;
    }
    let mut last = js_round(tick_index(step, hi));
    if tick_value(step, last) > hi {
        last -= 1.0;
    }
    let mut out = vec![];
    let mut i = first;
    while i <= last && out.len() < 100 {
        out.push(tick_value(step, i));
        i += 1.0;
    }
    out
}
fn value_marks(lo: &mut f64, hi: &mut f64, exact: bool, length: f64, spacing: f64, measurer: &dyn TextMeasurer, small: f64) -> Vec<Mark> {
    let step = tick_step((*hi - *lo).abs(), (length / spacing).floor().clamp(2.0, 10.0));
    if !exact {
        *lo = tick_value(step, (tick_index(step, *lo) + 1e-9).floor());
        *hi = tick_value(step, (tick_index(step, *hi) - 1e-9).ceil());
    }
    ticks(lo.min(*hi), lo.max(*hi), step)
        .into_iter()
        .map(|v| Mark {
            at: (v - *lo) / (*hi - *lo),
            label: layout_label(Some(&js_num(v)), false, measurer, small, 4000.0, 0),
        })
        .collect()
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Result<Rendered, PeleError> {
    if model.plots.is_empty() {
        return Err(PeleError::new("No Plot to render, please provide a plot with some data", PeleErrorCode::Semantic).of("xychart"));
    }
    let chart = config.get("xyChart").filter(|v| v.is_object()).unwrap_or(&Value::Null);
    let size = options.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let tiny = js_round(size * 0.75);
    let pad = options.padding.unwrap_or(8.0);
    let natural = dimension(chart.get("width"), 700.0);
    let width = options
        .max_width
        .filter(|&a| a > 0.0)
        .map_or(natural, |a| 240.0_f64.min(natural).max(natural.min(a)));
    let height = js_round(dimension(chart.get("height"), 500.0) * 0.6_f64.max(width / natural));
    let horizontal = model
        .orientation
        .as_deref()
        .or_else(|| chart.get("chartOrientation").and_then(Value::as_str))
        == Some("horizontal");
    let show_values = on(chart, "showDataLabel", false);
    let x = &model.x;
    let plots = &model.plots;
    let text = |raw: Option<&str>, font_size: f64, max: f64| layout_label(raw, false, measurer, font_size, max, 0);
    let line_height = text(Some("0"), small, 4000.0).height;
    let tiny_height = text(Some("0"), tiny, 4000.0).height;
    let title = layout_label(
        if on(chart, "showTitle", true) {
            model.title.as_deref()
        } else {
            None
        },
        false,
        measurer,
        size,
        width - 2.0 * pad,
        BOLD,
    );
    let title_space = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let mut legend = vec![];
    let mut legend_width = 0.0_f64;
    if on(chart, "showLegend", true) {
        for (index, plot) in plots.iter().enumerate() {
            if plot.title.is_empty() {
                continue;
            }
            let label = text(Some(&plot.title), small, width * 0.3);
            legend_width = legend_width.max(label.width + 20.0);
            legend.push((index, label));
        }
    }
    let legend_space = if legend.is_empty() { 0.0 } else { legend_width + 24.0 };
    let bars = plots.iter().filter(|p| p.bar).count();
    let mut lo = finite(Some(model.y.min)).unwrap_or(f64::INFINITY);
    let mut hi = finite(Some(model.y.max)).unwrap_or(f64::NEG_INFINITY);
    let exact = model.has_set_y && lo.is_finite() && hi.is_finite() && lo != hi;
    if !exact {
        lo = f64::INFINITY;
        hi = f64::NEG_INFINITY;
        for plot in plots {
            for (_, raw) in &plot.data {
                if let Some(value) = finite(*raw) {
                    lo = lo.min(value);
                    hi = hi.max(value);
                }
            }
        }
        if lo > hi {
            lo = 0.0;
            hi = 0.0;
        }
        if bars > 0 {
            lo = lo.min(0.0);
            hi = hi.max(0.0);
        }
        if lo == hi {
            if lo == 0.0 {
                hi = 1.0;
            } else {
                lo -= lo.abs() / 2.0;
                hi += hi.abs() / 2.0;
            }
        }
    }
    let count = if let Some(c) = &x.categories {
        c.len().max(1)
    } else {
        plots.iter().map(|p| p.data.len()).max().unwrap_or(1).max(1)
    };
    let slot = 1.0 / count as f64;
    let inset = if x.categories.is_some() || bars > 0 { slot / 2.0 } else { 0.0 };
    let x0 = finite(if x.categories.is_none() { Some(x.min) } else { None });
    let x1 = finite(if x.categories.is_none() { Some(x.max) } else { None });
    let ranged = x0.is_some() && x1.is_some() && x0 != x1;
    let place = |i: usize, n: usize| {
        if x.categories.is_some() {
            (i as f64 + 0.5) * slot
        } else {
            inset
                + if n > 1 {
                    i as f64 / (n - 1) as f64
                } else if ranged {
                    0.0
                } else {
                    0.5
                } * (1.0 - 2.0 * inset)
        }
    };
    let category_conf = chart.get("xAxis").filter(|v| v.is_object()).unwrap_or(&Value::Null);
    let value_conf = chart.get("yAxis").filter(|v| v.is_object()).unwrap_or(&Value::Null);
    let (left_conf, bottom_conf) = if horizontal {
        (category_conf, value_conf)
    } else {
        (value_conf, category_conf)
    };
    let left_title = text(
        if on(left_conf, "showTitle", true) {
            Some(if horizontal { &x.title } else { &model.y.title })
        } else {
            None
        },
        small,
        height * 0.8,
    );
    let bottom_title = text(
        if on(bottom_conf, "showTitle", true) {
            Some(if horizontal { &model.y.title } else { &x.title })
        } else {
            None
        },
        small,
        width * 0.8,
    );
    let left_tick = if on(left_conf, "showTick", true) { 4.0 } else { 0.0 };
    let bottom_tick = if on(bottom_conf, "showTick", true) { 4.0 } else { 0.0 };
    let left_labels = on(left_conf, "showLabel", true);
    let bottom_labels = on(bottom_conf, "showLabel", true);
    let wrap_at = if horizontal {
        width * 0.3
    } else {
        48.0_f64.max((width - 2.0 * pad - 64.0 - legend_space) * slot - 8.0)
    };
    let categories: Vec<_> = if on(category_conf, "showLabel", true) {
        x.categories
            .as_ref()
            .map(|c| c.iter().map(|c| text(Some(c), small, wrap_at)).collect())
            .unwrap_or_default()
    } else {
        vec![]
    };
    let category_width = categories.iter().map(|l| l.width).fold(0.0, f64::max);
    let category_height = categories.iter().map(|l| l.height).fold(0.0, f64::max);
    let category_marks = |length: f64| {
        let mut marks = vec![];
        if let Some(cats) = &x.categories {
            let need = if categories.is_empty() {
                8.0
            } else if horizontal {
                category_height
            } else {
                category_width + 12.0
            };
            let every = 1.0_f64.max((need / (length * slot)).ceil()) as usize;
            for i in (0..cats.len()).step_by(every.max(1)) {
                marks.push(Mark {
                    at: (i as f64 + 0.5) * slot,
                    label: categories.get(i).cloned().unwrap_or_else(|| text(None, small, 0.0)),
                });
            }
        } else if ranged {
            let x0 = x0.unwrap();
            let x1 = x1.unwrap();
            let step = tick_step(
                (x1 - x0).abs(),
                (length / if horizontal { 32.0 } else { 80.0 }).floor().clamp(2.0, 10.0),
            );
            for value in ticks(x0.min(x1), x0.max(x1), step) {
                marks.push(Mark {
                    at: inset + ((value - x0) / (x1 - x0)) * (1.0 - 2.0 * inset),
                    label: text(Some(&js_num(value)), small, 4000.0),
                });
            }
        } else if let Some(x0) = x0 {
            marks.push(Mark {
                at: 0.5,
                label: text(Some(&js_num(x0)), small, 4000.0),
            });
        }
        marks
    };
    let mut point_label_width = 0.0_f64;
    let mut value_width = 0.0_f64;
    for plot in plots {
        if let Some(labels) = &plot.point_labels {
            for label in labels {
                point_label_width = point_label_width.max(measurer.width(label, tiny, 0));
            }
        }
        if show_values && horizontal && plot.bar {
            for (_, value) in &plot.data {
                value_width = value_width.max(measurer.width(&finite(*value).map(js_num).unwrap_or_default(), tiny, 0));
            }
        }
    }
    let bottom_label_height = if !bottom_labels {
        0.0
    } else if horizontal || x.categories.is_none() {
        line_height
    } else {
        category_height
    };
    let bottom_space = bottom_tick
        + if bottom_label_height > 0.0 {
            bottom_label_height + 4.0
        } else {
            0.0
        }
        + if bottom_title.height > 0.0 {
            bottom_title.height + 4.0
        } else {
            0.0
        };
    let headroom = if horizontal {
        0.0
    } else if point_label_width > 0.0 || show_values {
        tiny_height + 4.0
    } else if left_labels {
        line_height / 2.0
    } else {
        0.0
    };
    let top = pad + title_space + headroom;
    let plot_height = 1.0_f64.max(height - pad - bottom_space - top);
    let bottom = top + plot_height;
    let left_marks = if horizontal {
        category_marks(plot_height)
    } else {
        value_marks(&mut lo, &mut hi, exact, plot_height, 56.0, measurer, small)
    };
    let left_label_width = if left_labels { widest(&left_marks) } else { 0.0 };
    let left = pad
        + if left_title.height > 0.0 { left_title.height + 6.0 } else { 0.0 }
        + if left_label_width > 0.0 { left_label_width + 6.0 } else { 0.0 }
        + left_tick;
    let beside =
        (if point_label_width > 0.0 { point_label_width + 12.0 } else { 0.0 }).max(if value_width > 0.0 { value_width + 8.0 } else { 0.0 });
    let reach = if horizontal { (width * 0.25).min(beside) } else { 0.0 };
    let mut plot_width = 1.0_f64.max(width - pad - left - legend_space - reach);
    let bottom_marks = if horizontal {
        value_marks(&mut lo, &mut hi, exact, plot_width, 80.0, measurer, small)
    } else {
        category_marks(plot_width)
    };
    if bottom_labels && legend_space + reach == 0.0 {
        let mut over = 0.0_f64;
        for mark in &bottom_marks {
            over = over.max(mark.at * plot_width + mark.label.width / 2.0 - plot_width);
        }
        plot_width = 1.0_f64.max(plot_width - over);
    }
    let right = left + plot_width;
    let px = |c: f64, v: f64| if horizontal { left + v * plot_width } else { left + c * plot_width };
    let py = |c: f64, v: f64| {
        if horizontal {
            top + c * plot_height
        } else {
            bottom - v * plot_height
        }
    };
    let scale = |value: f64| (value - lo) / (hi - lo);
    let clamp = |v: f64| v.clamp(0.0, 1.0);
    let rule = |v: f64| {
        if horizontal {
            format!("M{},{}V{}", num(px(0.0, v)), num(top), num(bottom))
        } else {
            format!("M{},{}H{}", num(left), num(py(0.0, v)), num(right))
        }
    };
    let mut grid = String::new();
    for mark in if horizontal { &bottom_marks } else { &left_marks } {
        grid += &rule(mark.at);
    }
    let mut axis = String::new();
    let mut labels = String::new();
    if on(left_conf, "showAxisLine", true) {
        axis += &format!("M{},{}V{}", num(left), num(top), num(bottom));
    }
    if on(bottom_conf, "showAxisLine", true) {
        axis += &format!("M{},{}H{}", num(left), num(bottom), num(right));
    }
    for mark in &left_marks {
        let y = if horizontal {
            top + mark.at * plot_height
        } else {
            bottom - mark.at * plot_height
        };
        if left_tick != 0.0 {
            axis += &format!("M{},{}h{}", num(left), num(y), js_num(-left_tick));
        }
        if left_labels {
            labels += &aligned(&mark.label, left - left_tick - 6.0, y, "end");
        }
    }
    for mark in &bottom_marks {
        let at = left + mark.at * plot_width;
        if bottom_tick != 0.0 {
            axis += &format!("M{},{}v{}", num(at), num(bottom), js_num(bottom_tick));
        }
        if bottom_labels {
            labels += &aligned(&mark.label, at, bottom + bottom_tick + 4.0 + mark.label.height / 2.0, "middle");
        }
    }
    let span = if horizontal { plot_height } else { plot_width } * slot;
    let group = span * 0.7;
    let thickness = group / bars.max(1) as f64;
    let gap = if bars > 1 { 2.0_f64.min(thickness * 0.2) } else { 0.0 };
    let base = clamp(scale(0.0));
    if bars > 0 && base > 0.0 && base < 1.0 {
        axis += &rule(base);
    }
    let mut series = String::new();
    let mut lines = String::new();
    let mut outside = String::new();
    let mut point_labels = String::new();
    let mut bar = 0;
    for (index, plot) in plots.iter().enumerate() {
        let color = series_color(index as i64);
        let id = if plot.title.is_empty() {
            String::new()
        } else {
            format!(" data-id=\"{}\"", esc_text(&plot.title))
        };
        let n = if x.categories.is_some() {
            plot.data.len().min(count)
        } else {
            plot.data.len()
        };
        if plot.bar {
            let offset = (bar as f64 - (bars as f64 - 1.0) / 2.0) * thickness - (thickness - gap) / 2.0;
            bar += 1;
            let mut rects = String::new();
            for i in 0..n {
                let Some(value) = finite(plot.data[i].1) else {
                    continue;
                };
                let c = place(i, n);
                let v = clamp(scale(value));
                let from = base.min(v);
                let to = base.max(v);
                let bx = if horizontal { px(c, from) } else { px(c, 0.0) + offset };
                let by = if horizontal { py(c, 0.0) + offset } else { py(c, to) };
                let bw = if horizontal { (to - from) * plot_width } else { thickness - gap };
                let bh = if horizontal { thickness - gap } else { (to - from) * plot_height };
                rects += &format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                    num(bx),
                    num(by),
                    num(bw),
                    num(bh)
                );
                if !show_values {
                    continue;
                }
                let label = js_num(value);
                let w = measurer.width(&label, tiny, 0);
                let out = if v >= base { 1.0 } else { -1.0 };
                let mut tx = bx + bw / 2.0;
                let mut ty = by + bh / 2.0;
                let mut anchor = "middle";
                if horizontal {
                    if bh < tiny {
                        continue;
                    }
                    tx = if out > 0.0 { bx + bw } else { bx } + out * 6.0;
                    anchor = if out > 0.0 { "start" } else { "end" };
                } else {
                    if w + 2.0 > span / bars as f64 {
                        continue;
                    }
                    ty = if out > 0.0 { by } else { by + bh } - out * (tiny_height / 2.0 + 2.0);
                }
                outside += &format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\">{label}</text>",
                    num(tx),
                    num(ty + tiny * 0.35)
                );
            }
            series += &format!("<g class=\"pele-series pele-bars\"{id} fill=\"{color}\">{rects}</g>");
            continue;
        }
        let mut d = String::new();
        let mut dots = String::new();
        let mut pen = false;
        let mut pc = 0.0;
        let mut pv = f64::NAN;
        for i in 0..n {
            let Some(value) = finite(plot.data[i].1) else {
                pv = f64::NAN;
                continue;
            };
            let c = place(i, n);
            let v = scale(value);
            let visible = (0.0..=1.0).contains(&v);
            if pv.is_nan() {
                if visible {
                    d += &format!("M{},{}", num(px(c, v)), num(py(c, v)));
                }
                pen = visible;
            } else {
                let dv = v - pv;
                let mut t0 = 0.0_f64;
                let mut t1 = 1.0_f64;
                if dv != 0.0 {
                    let a = -pv / dv;
                    let b = (1.0 - pv) / dv;
                    t0 = t0.max(a.min(b));
                    t1 = t1.min(a.max(b));
                } else if !visible {
                    t0 = 2.0;
                }
                if t0 <= t1 {
                    let c0 = pc + (c - pc) * t0;
                    let v0 = pv + dv * t0;
                    let c1 = pc + (c - pc) * t1;
                    let v1 = pv + dv * t1;
                    if !pen || t0 > 0.0 {
                        d += &format!("M{},{}", num(px(c0, v0)), num(py(c0, v0)));
                    }
                    d += &format!("L{},{}", num(px(c1, v1)), num(py(c1, v1)));
                }
                pen = visible;
            }
            pc = c;
            pv = v;
            if !visible {
                continue;
            }
            let cx = px(c, v);
            let cy = py(c, v);
            if span >= 12.0 {
                dots += &format!("<circle cx=\"{}\" cy=\"{}\" r=\"3\"/>", num(cx), num(cy));
            }
            let label = text(plot.point_labels.as_ref().and_then(|l| l.get(i)).map(String::as_str), tiny, 4000.0);
            if label.height > 0.0 {
                let half = label.width / 2.0;
                point_labels += &if horizontal {
                    aligned(&label, cx + 10.0, cy, "start")
                } else {
                    aligned(
                        &label,
                        cx.min(right - half).max(left + half),
                        cy - 6.0 - label.height / 2.0,
                        "middle",
                    )
                };
            }
        }
        lines += &format!(
            "<g class=\"pele-series pele-line\"{id} fill=\"{color}\">{}{dots}</g>",
            if d.is_empty() {
                String::new()
            } else {
                format!(
                    "<path d=\"{d}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"
                )
            }
        );
    }
    let mut titles = String::new();
    if left_title.height > 0.0 {
        titles += &format!(
            "<g transform=\"translate({},{}) rotate(-90)\">{}</g>",
            num(pad + left_title.height / 2.0),
            num(top + plot_height / 2.0),
            label_svg(&left_title, 0.0, 0.0, "")
        );
    }
    titles += &label_svg(&bottom_title, left + plot_width / 2.0, height - pad - bottom_title.height / 2.0, "");
    let legend_height = legend.iter().map(|(_, l)| 26.0_f64.max(l.height + 6.0)).sum::<f64>();
    let mut ly = top + 0.0_f64.max((plot_height - legend_height) / 2.0);
    let lx = right + reach + 24.0;
    let mut legend_out = String::new();
    for (index, label) in legend {
        let row = 26.0_f64.max(label.height + 6.0);
        if ly + row > height {
            break;
        }
        let y = ly + row / 2.0;
        let color = series_color(index as i64);
        legend_out += &format!(
            "<g class=\"pele-legend-item\" data-id=\"{}\">{}{}</g>",
            esc_text(&plots[index].title),
            if plots[index].bar {
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"12\" height=\"12\" rx=\"2\" fill=\"{color}\"/>",
                    num(lx),
                    num(y - 6.0)
                )
            } else {
                format!(
                    "<path d=\"M{},{}h12\" stroke=\"{color}\" stroke-width=\"2\" stroke-linecap=\"round\"/>",
                    num(lx),
                    num(y)
                )
            },
            label_svg(&label, lx + 20.0 + label.width / 2.0, y, "")
        );
        ly += row;
    }
    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !grid.is_empty() {
        content += &format!("<path class=\"pele-grid\" d=\"{grid}\" stroke=\"var(--_a)\"/>");
    }
    content += &format!("<g class=\"pele-plot\">{series}{lines}</g>");
    if !axis.is_empty() {
        content += &format!("<path class=\"pele-axis\" d=\"{axis}\" fill=\"none\" stroke=\"var(--_b)\"/>");
    }
    if !labels.is_empty() {
        content += &format!(
            "<g class=\"pele-axis-labels\" font-size=\"{}\" fill=\"var(--_m)\">{labels}</g>",
            js_num(small)
        );
    }
    for (class, size, body) in [
        ("pele-axis-titles", small, titles),
        ("pele-data-labels", tiny, outside),
        ("pele-point-labels", tiny, point_labels),
        ("pele-legend", small, legend_out),
    ] {
        if !body.is_empty() {
            content += &format!("<g class=\"{class}\" font-size=\"{}\">{body}</g>", js_num(size));
        }
    }
    let acc = Accessible {
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    };
    Ok(Rendered {
        svg: svg_document("xychart", width, height, size, options, &acc, &content),
        width,
        height,
        links: vec![],
    })
}
