use super::{Model, Text};
use crate::RenderOptions;
use crate::diagrams::common::Rendered;
use crate::preprocess::Config;
use crate::svg::builder::{esc_text, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{ROUNDED, class_names, resolve_style, series_color};
use crate::text::label::layout_label;
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{js_num, js_round, num};
fn length(config: &Config, key: &str, fallback: f64, max: f64) -> f64 {
    config
        .get("quadrantChart")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_f64())
        .filter(|&v| v >= 0.0)
        .map_or(fallback, |v| v.min(max))
}
fn hex(color: &str) -> String {
    if color.starts_with('#') {
        color.into()
    } else {
        format!("#{color}")
    }
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    let size = options.font_size.unwrap_or(16.0);
    let axis_size = js_round(size * 0.875);
    let point_size = js_round(size * 0.75);
    let natural = js_round(100.0_f64.max(length(config, "chartWidth", 500.0, 4000.0)));
    let width = js_round(
        options
            .max_width
            .filter(|&a| a > 0.0)
            .map_or(natural, |a| 280.0_f64.min(natural).max(natural.min(a))),
    );
    let height = js_round(100.0_f64.max(length(config, "chartHeight", 500.0, 4000.0)) * width / natural);
    let outer = length(config, "quadrantPadding", options.padding.unwrap_or(8.0), 200.0);
    let x_pad = length(config, "xAxisLabelPadding", 5.0, 200.0);
    let y_pad = length(config, "yAxisLabelPadding", 5.0, 200.0);
    let has_points = !model.points.is_empty();
    let label = |text: Option<&Text>, font_size: f64, max: f64| {
        layout_label(
            text.map(|t| t.text.as_str()),
            text.is_some_and(|t| t.markdown),
            measurer,
            font_size,
            max.max(40.0),
            0,
        )
    };
    let title = layout_label(model.title.as_deref(), false, measurer, size, (width - 2.0 * outer).max(40.0), BOLD);
    let title_pad = length(config, "titlePadding", 10.0, 200.0);
    let title_space = if title.height > 0.0 { title.height + title_pad } else { 0.0 };
    let has_x = model.x_left.as_ref().is_some_and(|v| !v.text.is_empty()) || model.x_right.as_ref().is_some_and(|v| !v.text.is_empty());
    let has_y = model.y_bottom.as_ref().is_some_and(|v| !v.text.is_empty()) || model.y_top.as_ref().is_some_and(|v| !v.text.is_empty());
    let section = config.get("quadrantChart");
    let x_below = has_points || section.and_then(|v| v.get("xAxisPosition")).and_then(|v| v.as_str()) == Some("bottom");
    let y_right = section.and_then(|v| v.get("yAxisPosition")).and_then(|v| v.as_str()) == Some("right");
    let side = width - 2.0 * outer;
    let x_left = label(
        model.x_left.as_ref(),
        axis_size,
        if model.x_right.is_some() { side / 2.0 - 16.0 } else { side },
    );
    let x_right = label(model.x_right.as_ref(), axis_size, side / 2.0 - 16.0);
    let x_height = x_left.height.max(x_right.height);
    let x_space = if has_x { x_height + x_pad } else { 0.0 };
    let tall = height - 2.0 * outer - title_space - x_space;
    let y_bottom = label(
        model.y_bottom.as_ref(),
        axis_size,
        if model.y_top.is_some() { tall / 2.0 - 16.0 } else { tall },
    );
    let y_top = label(model.y_top.as_ref(), axis_size, tall / 2.0 - 16.0);
    let y_height = y_bottom.height.max(y_top.height);
    let y_space = if has_y { y_height + y_pad } else { 0.0 };
    let left = outer + if y_right { 0.0 } else { y_space };
    let top = outer + title_space + if x_below { 0.0 } else { x_space };
    let w = 0.0_f64.max(side - y_space);
    let h = 0.0_f64.max(tall);
    let tile_w = 0.0_f64.max((w - 4.0) / 2.0);
    let tile_h = 0.0_f64.max((h - 4.0) / 2.0);
    let mut taken = vec![
        left + tile_w,
        top,
        left + w - tile_w,
        top + h,
        left,
        top + tile_h,
        left + w,
        top + h - tile_h,
    ];
    let text_top = length(config, "quadrantTextTopPadding", 5.0, 200.0);
    let mut quadrants = String::new();
    for k in 0..4 {
        let x = if k == 0 || k == 3 { left + w - tile_w } else { left };
        let y = if k < 2 { top } else { top + h - tile_h };
        let text = label(model.quadrants[k].as_ref(), size, tile_w - 16.0);
        let cx = x + tile_w / 2.0;
        let cy = if has_points {
            y + text_top + text.height / 2.0
        } else {
            y + tile_h / 2.0
        };
        taken.extend([
            cx - text.width / 2.0,
            cy - text.height / 2.0,
            cx + text.width / 2.0,
            cy + text.height / 2.0,
        ]);
        quadrants += &format!(
            "<g class=\"pele-quadrant\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{ROUNDED}/>{}</g>",
            k + 1,
            num(x),
            num(y),
            num(tile_w),
            num(tile_h),
            label_svg(&text, cx, cy, " class=\"pele-label\" fill=\"var(--_fg)\" stroke=\"none\"")
        );
    }
    let mut axes = String::new();
    if has_x {
        let y = if x_below {
            top + h + x_pad + x_height / 2.0
        } else {
            top - x_pad - x_height / 2.0
        };
        axes += &label_svg(
            &x_left,
            if x_right.height > 0.0 {
                left + w / 4.0
            } else {
                left + x_left.width / 2.0
            },
            y,
            "",
        );
        axes += &label_svg(&x_right, left + w * 3.0 / 4.0, y, "");
    }
    if has_y {
        let x = if y_right {
            left + w + y_pad + y_height / 2.0
        } else {
            left - y_pad - y_height / 2.0
        };
        for (text, y) in [
            (
                &y_bottom,
                if y_top.height > 0.0 {
                    top + h * 3.0 / 4.0
                } else {
                    top + h - y_bottom.width / 2.0
                },
            ),
            (&y_top, top + h / 4.0),
        ] {
            if text.height > 0.0 {
                axes += &format!(
                    "<g transform=\"translate({},{}) rotate(-90)\">{}</g>",
                    num(x),
                    num(y),
                    label_svg(text, 0.0, 0.0, "")
                );
            }
        }
    }
    let radius = length(config, "pointRadius", 5.0, 200.0);
    let text_pad = length(config, "pointTextPadding", 5.0, 200.0);
    let fill = series_color(0);
    let mut dots = vec![];
    let mut circles = vec![];
    for point in &model.points {
        if !(point.x >= 0.0 && point.x <= 1.0 && point.y >= 0.0 && point.y <= 1.0) {
            continue;
        }
        let shared = model.classes.get(&point.class_name);
        let r = point
            .style
            .radius
            .or_else(|| shared.and_then(|s| s.radius))
            .unwrap_or(radius)
            .min(2000.0);
        let color = point.style.color.as_deref().or_else(|| shared.and_then(|s| s.color.as_deref()));
        let stroke_color = point
            .style
            .stroke_color
            .as_deref()
            .or_else(|| shared.and_then(|s| s.stroke_color.as_deref()));
        let stroke_width = point
            .style
            .stroke_width
            .as_deref()
            .or_else(|| shared.and_then(|s| s.stroke_width.as_deref()));
        let mut styles = vec![];
        if let Some(c) = color {
            styles.push(format!("fill:{}", hex(c)));
        }
        if let Some(sw) = stroke_width {
            if let Some(c) = stroke_color {
                styles.push(format!("stroke:{}", hex(c)));
            }
            styles.push(format!("stroke-width:{sw}"));
        }
        let cx = left + point.x * w;
        let cy = top + (1.0 - point.y) * h;
        let reach = r + stroke_width.map_or(0.0, |v| crate::util::parse_float(v).min(400.0) / 2.0);
        dots.push((cx, cy, reach, label(Some(&point.text), point_size, 200.0)));
        circles.push(format!(
            "<circle class=\"pele-point{}\" data-id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\"{}{}/>",
            class_names(&point.class_name),
            esc_text(&point.text.text),
            num(cx),
            num(cy),
            num(r),
            if stroke_width.is_some() && stroke_color.is_none() {
                format!(" stroke=\"{fill}\"")
            } else {
                String::new()
            },
            resolve_style(&styles).shape
        ));
    }
    let careful = dots.len() <= 300;
    if careful {
        for (cx, cy, reach, _) in &dots {
            taken.extend([cx - reach, cy - reach, cx + reach, cy + reach]);
        }
    }
    let mut labels = String::new();
    for (cx, cy, reach, text) in &dots {
        let half_w = text.width / 2.0;
        let half_h = text.size * 0.6 + (text.height - text.line_height) / 2.0;
        let away = reach + text_pad;
        let inset = away.min(0.0_f64.max((tile_w - text.width) / 2.0));
        let heading = 8 + 4 * if *cx < left + w / 2.0 {
            if *cy < top + h / 2.0 { 1 } else { 2 }
        } else if *cy < top + h / 2.0 {
            0
        } else {
            3
        };
        let below = (cy + away).max(taken[heading + 3] + text_pad) + half_h;
        let spots = [
            *cx,
            cy + away + half_h,
            *cx,
            cy - away - half_h,
            cx + away + half_w,
            *cy,
            cx - away - half_w,
            *cy,
            *cx,
            below,
        ];
        let mut tx = *cx;
        let mut ty = spots[1];
        let mut found = false;
        for k in (0..20).step_by(2) {
            let spot = k % 10;
            let margin = if spot == 8 { inset } else { 0.0 };
            let x = if (spot < 4 || spot == 8) && text.width < w {
                spots[spot].max(left + half_w + margin).min(left + w - half_w - margin)
            } else {
                spots[spot]
            };
            let y = spots[spot + 1];
            if x - half_w < left || x + half_w > left + w || y - half_h < top || y + half_h > top + h {
                continue;
            }
            let free = !careful
                || (if k < 10 { 0 } else { 8 }..taken.len()).step_by(4).all(|i| {
                    taken[i] >= taken[i + 2]
                        || taken[i + 1] >= taken[i + 3]
                        || !(x - half_w < taken[i + 2] && x + half_w > taken[i] && y - half_h < taken[i + 3] && y + half_h > taken[i + 1])
                });
            if !found || free {
                tx = x;
                ty = y;
                found = true;
            }
            if free {
                break;
            }
        }
        if careful {
            taken.extend([tx - half_w, ty - half_h, tx + half_w, ty + half_h]);
        }
        labels += &label_svg(text, tx, ty, " class=\"pele-label\"");
    }
    let mut content = label_svg(
        &title,
        width / 2.0,
        title_pad / 2.0 + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &format!("<g class=\"pele-quadrants\" fill=\"var(--_s)\" stroke=\"var(--_b)\">{quadrants}</g>");
    if !axes.is_empty() {
        content += &format!(
            "<g class=\"pele-axis-labels\" font-size=\"{}\" fill=\"var(--_m)\">{axes}</g>",
            js_num(axis_size)
        );
    }
    if !dots.is_empty() {
        circles.reverse();
        content += &format!(
            "<g class=\"pele-points\" fill=\"{fill}\">{}</g><g class=\"pele-point-labels\" font-size=\"{}\">{labels}</g>",
            circles.join(""),
            js_num(point_size)
        );
    }
    let acc = Accessible {
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    };
    Rendered {
        svg: svg_document("quadrantChart", width, height, size, options, &acc, &content),
        width,
        height,
        links: vec![],
    }
}
