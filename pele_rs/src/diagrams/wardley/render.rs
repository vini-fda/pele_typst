use super::{Model, Node};
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, fit_width};
use crate::preprocess::Config;
use crate::svg::builder::{esc, esc_text, label_svg, shared_style, span_style, text_style};
use crate::svg::edges::{marker, marker_trim};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::ROUNDED;
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{js_num, js_round, num};
const R: f64 = 6.0;
const LINE: &str = "var(--_l)";
const ACCENT: &str = "var(--_c)";
fn setting(c: &Config, key: &str, f: f64, min: f64, max: f64) -> f64 {
    c.get("wardley-beta")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_f64())
        .filter(|v| *v >= min && *v <= max)
        .unwrap_or(f)
}
fn dimension(c: &Config, g: Option<f64>, key: &str, f: f64) -> f64 {
    g.filter(|v| v.is_finite())
        .unwrap_or_else(|| setting(c, key, f, 1.0, 1e6))
        .clamp(200.0, 10000.0)
        - 96.0
}
fn start_label(l: &Label, x: f64, y: f64, attrs: &str) -> String {
    let shared = shared_style(l);
    let mut body = String::new();
    let mut baseline = y;
    for line in &l.lines {
        let mut first = true;
        for s in line {
            if s.icon.is_some() {
                continue;
            }
            body += &format!(
                "<tspan{}{}>{}</tspan>",
                if first {
                    format!(" x=\"{}\" y=\"{}\"", num(x), num(baseline))
                } else {
                    String::new()
                },
                span_style(s.style & !shared),
                esc(&s.text)
            );
            first = false
        }
        baseline += l.line_height
    }
    if body.is_empty() {
        body
    } else {
        format!("<text{} xml:space=\"preserve\">{body}</text>", text_style(attrs, shared))
    }
}
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}
impl Bounds {
    fn grow(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        self.min_x = self.min_x.min(x0);
        self.min_y = self.min_y.min(y0);
        self.max_x = self.max_x.max(x1);
        self.max_y = self.max_y.max(y1)
    }
    fn center(&mut self, l: &Label, x: f64, y: f64, a: &str) -> String {
        self.grow(x - l.width / 2.0, y - l.height / 2.0, x + l.width / 2.0, y + l.height / 2.0);
        label_svg(l, x, y, a)
    }
    fn start(&mut self, l: &Label, x: f64, y: f64, a: &str) -> String {
        self.grow(x, y - l.size, x + l.width, y - l.size + l.height + 4.0);
        start_label(l, x, y, a)
    }
}
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}
fn reach(
    i: usize,
    nodes: &[Node],
    ux: f64,
    uy: f64,
    labels: &mut [Option<Label>],
    m: &dyn TextMeasurer,
    small: f64,
    w: f64,
    tight: bool,
) -> f64 {
    let n = &nodes[i];
    if n.class_name == "anchor" {
        let l = labels[i].get_or_insert_with(|| {
            layout_label(
                Some(&n.label),
                false,
                m,
                small,
                if tight { 64.0_f64.max(w / 2.0) } else { 4000.0 },
                BOLD,
            )
        });
        return 1.0 / (ux / (l.width / 2.0 + 5.0)).hypot(uy / (small * 0.8));
    }
    if n.pipeline_parent {
        return 10.0 / std::f64::consts::SQRT_2;
    }
    if n.strategy.is_some() { 2.0 * R } else { R }
}
fn occupy(taken: &mut [u8], i: usize, ux: f64, uy: f64) {
    if ux.abs() < 0.25 || uy.abs() < 0.25 {
        return;
    }
    taken[i] |= if ux > 0.0 {
        if uy < 0.0 { 1 } else { 2 }
    } else if uy < 0.0 {
        4
    } else {
        8
    };
}
pub fn render(model: &Model, c: &Config, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    let natural = dimension(c, model.size.map(|s| s.0), "width", 900.0);
    fit_width(o, natural, 160.0, |w, tight| draw(model, c, o, m, w, natural, tight))
}
fn draw(model: &Model, c: &Config, o: &RenderOptions, m: &dyn TextMeasurer, w: f64, natural: f64, tight: bool) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.8125);
    let tiny = js_round(size * 0.6875);
    let pad = o.padding.unwrap_or(8.0);
    let h = dimension(c, model.size.map(|s| s.1), "height", 600.0) * 0.75_f64.max(w / natural);
    let grid = c.get("wardley-beta").and_then(|v| v.get("showGrid")).and_then(|v| v.as_bool()) == Some(true);
    let mut b = Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: w,
        max_y: h,
    };
    let room = if tight { 64.0_f64.max(120.0_f64.min(w / 2.0)) } else { 4000.0 };
    let text = |s: &str, size: f64, max: f64, style: u32| layout_label(Some(s), false, m, size, max, style);
    let px = |v: f64| v / 100.0 * w;
    let py = |v: f64| h - v / 100.0 * h;
    let stages: Vec<&str> = if model.stages.is_empty() {
        vec!["Genesis", "Custom Built", "Product", "Commodity"]
    } else {
        model.stages.iter().map(String::as_str).collect()
    };
    let custom = model.boundaries.len() == stages.len();
    let mut frame = String::new();
    let mut stage_labels = String::new();
    let mut stage_bottom = h;
    let mut previous = 0.0;
    for (i, stage) in stages.iter().enumerate() {
        let start = if custom { previous } else { i as f64 / stages.len() as f64 };
        let end = if custom {
            model.boundaries[i].clamp(0.0, 1.0)
        } else {
            (i + 1) as f64 / stages.len() as f64
        };
        previous = end;
        if i > 0 {
            frame += &format!(
                "<path class=\"pele-wardley-stage\" d=\"M{},0V{}\" stroke=\"var(--_b)\" stroke-dasharray=\"4 4\"/>",
                num(start * w),
                num(h)
            )
        }
        let l = text(stage, small, 48.0_f64.max((end - start).abs() * w - 12.0), 0);
        stage_labels += &b.center(
            &l,
            (start + end) / 2.0 * w,
            h + 8.0 + l.height / 2.0,
            &format!(
                " class=\"pele-wardley-stage-label\" font-size=\"{}\" fill=\"var(--_m)\"",
                js_num(small)
            ),
        );
        stage_bottom = stage_bottom.max(h + 8.0 + l.height)
    }
    if grid {
        for i in 1..4 {
            frame += &format!(
                "<path class=\"pele-wardley-grid\" d=\"M{},0V{}M0,{}H{}\" stroke=\"var(--_b)\" stroke-dasharray=\"2 6\"/>",
                num(w * i as f64 / 4.0),
                num(h),
                num(h * i as f64 / 4.0),
                num(w)
            )
        }
    }
    frame += &format!(
        "<path class=\"pele-wardley-axis\" d=\"M0,0V{}H{}\" stroke=\"{LINE}\"/>",
        num(h),
        num(w)
    );
    let xt = text("Evolution", small, room, 0);
    let yt = text("Visibility", small, room, 0);
    let attrs = format!(
        " class=\"pele-wardley-axis-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
        js_num(small)
    );
    let axis_labels = b.center(&xt, w / 2.0, stage_bottom + 6.0 + xt.height / 2.0, &attrs)
        + &label_svg(
            &yt,
            -16.0,
            h / 2.0,
            &format!("{attrs} transform=\"rotate(-90 -16 {})\"", num(h / 2.0)),
        );
    b.grow(-16.0 - yt.height / 2.0, 0.0, 0.0, 0.0);
    let legend_top = stage_bottom + 6.0 + xt.height + 10.0;
    let mut positions: Vec<Point> = model.nodes.iter().map(|n| Point { x: px(n.x), y: py(n.y) }).collect();
    let index: std::collections::HashMap<&str, usize> = model.nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let mut pipelines = String::new();
    let mut members: std::collections::HashMap<&str, &[String]> = std::collections::HashMap::new();
    for pipe in &model.pipelines {
        members.insert(&pipe.node_id, &pipe.components);
        let mut parts: Vec<Point> = pipe
            .components
            .iter()
            .filter_map(|id| index.get(id.as_str()).map(|&i| positions[i]))
            .collect();
        if parts.is_empty() {
            continue;
        }
        parts.sort_by(|a, b| a.x.total_cmp(&b.x));
        let left = parts[0].x;
        let right = parts.last().unwrap().x;
        let y = parts[0].y;
        let top = y - 2.0 * R;
        if let Some(&i) = index.get(pipe.node_id.as_str()) {
            positions[i] = Point {
                x: (left + right) / 2.0,
                y: top - 10.0 / 6.0,
            }
        }
        b.grow(left - 15.0, top, right + 15.0, top + 4.0 * R);
        pipelines += &format!(
            "<g class=\"pele-wardley-pipeline\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"24\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{ROUNDED}/>{}</g>",
            esc_text(&pipe.node_id),
            num(left - 15.0),
            num(top),
            num(right - left + 30.0),
            if parts.len() > 1 {
                format!(
                    "<path d=\"M{},{}H{}\" stroke=\"{LINE}\" stroke-dasharray=\"4 4\"/>",
                    num(left),
                    num(y),
                    num(right)
                )
            } else {
                String::new()
            }
        );
    }
    let mut anchor_labels: Vec<Option<Label>> = vec![None; model.nodes.len()];
    let mut taken = vec![0; model.nodes.len()];
    let mut links = String::new();
    for link in &model.links {
        let (Some(&s), Some(&t)) = (index.get(link.source.as_str()), index.get(link.target.as_str())) else {
            continue;
        };
        if s == t || members.get(link.target.as_str()).is_some_and(|list| list.contains(&link.source)) {
            continue;
        }
        let from = positions[s];
        let to = positions[t];
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let distance = dx.hypot(dy);
        if distance < 0.01 {
            continue;
        }
        let ux = dx / distance;
        let uy = dy / distance;
        let start = reach(s, &model.nodes, ux, uy, &mut anchor_labels, m, small, w, tight);
        let end = reach(t, &model.nodes, ux, uy, &mut anchor_labels, m, small, w, tight);
        if start + end >= distance {
            continue;
        }
        occupy(&mut taken, s, ux, uy);
        occupy(&mut taken, t, -ux, -uy);
        let mut x1 = from.x + ux * start;
        let mut y1 = from.y + uy * start;
        let mut x2 = to.x - ux * end;
        let mut y2 = to.y - uy * end;
        let mut heads = String::new();
        if matches!(link.flow.as_deref(), Some("forward" | "bidirectional")) {
            heads += &marker("arrow_point", x2, y2, ux, uy, LINE, "none");
            x2 -= ux * marker_trim("arrow_point");
            y2 -= uy * marker_trim("arrow_point")
        }
        if matches!(link.flow.as_deref(), Some("backward" | "bidirectional")) {
            heads += &marker("arrow_point", x1, y1, -ux, -uy, LINE, "none");
            x1 += ux * marker_trim("arrow_point");
            y1 += uy * marker_trim("arrow_point")
        }
        let caption = if let Some(s) = link.label.as_ref().filter(|s| !s.is_empty()) {
            let l = text(s, tiny + 1.0, 200.0, 0);
            let side = if ux >= 0.0 { 1.0 } else { -1.0 };
            let nx = uy * side;
            let ny = -ux * side;
            let away = 5.0 + nx.abs() * l.width / 2.0 + ny.abs() * l.height / 2.0;
            b.center(
                &l,
                (from.x + to.x) / 2.0 + nx * away,
                (from.y + to.y) / 2.0 + ny * away,
                &format!(" class=\"pele-edge-label\" font-size=\"{}\" fill=\"var(--_m)\"", js_num(tiny + 1.0)),
            )
        } else {
            String::new()
        };
        links += &format!(
            "<g class=\"pele-edge{}\" data-id=\"{}-{}\"><path d=\"M{},{}L{},{}\" stroke=\"{LINE}\"/>{heads}{caption}</g>",
            if link.dashed { " pele-wardley-dashed" } else { "" },
            esc_text(&link.source),
            esc_text(&link.target),
            num(x1),
            num(y1),
            num(x2),
            num(y2)
        );
        if link.dashed {
            let at = links.rfind(&format!("stroke=\"{LINE}\"/>")).unwrap();
            links.insert_str(at + format!("stroke=\"{LINE}\"").len(), " stroke-dasharray=\"6 6\"");
        }
    }
    let mut trends = String::new();
    for trend in &model.trends {
        let Some(&i) = index.get(trend.node_id.as_str()) else { continue };
        let origin = positions[i];
        let tx = px(trend.x);
        let ty = py(trend.y);
        let dx = tx - origin.x;
        let dy = ty - origin.y;
        let distance = dx.hypot(dy);
        let start = if distance > 0.0 {
            reach(
                i,
                &model.nodes,
                dx / distance,
                dy / distance,
                &mut anchor_labels,
                m,
                small,
                w,
                tight,
            )
        } else {
            0.0
        };
        let mut arrow = String::new();
        if distance > start + R + 4.0 {
            let ux = dx / distance;
            let uy = dy / distance;
            let ex = tx - ux * (R + 2.0);
            let ey = ty - uy * (R + 2.0);
            arrow = format!(
                "<path d=\"M{},{}L{},{}\" stroke=\"{ACCENT}\" stroke-dasharray=\"4 4\"/>",
                num(origin.x + ux * start),
                num(origin.y + uy * start),
                num(ex - ux * marker_trim("arrow_point")),
                num(ey - uy * marker_trim("arrow_point"))
            ) + &marker("arrow_point", ex, ey, ux, uy, ACCENT, "none")
        }
        b.grow(tx - R, ty - R, tx + R, ty + R);
        trends += &format!(
            "<g class=\"pele-wardley-trend\" data-id=\"{}\">{arrow}<circle cx=\"{}\" cy=\"{}\" r=\"6\" fill=\"var(--_bg)\" stroke=\"{ACCENT}\"/></g>",
            esc_text(&trend.node_id),
            num(tx),
            num(ty)
        );
    }
    let mut nodes = String::new();
    let tri = 1.2 * R;
    let dot = 0.7 * R;
    for (i, node) in model.nodes.iter().enumerate() {
        let Point { x, y } = positions[i];
        let anchor = node.class_name == "anchor";
        let strategy = node.strategy.as_deref();
        let mut shape = String::new();
        if let Some(s) = strategy {
            let fill = match s {
                "outsource" => "var(--_m)",
                "buy" => "var(--_a)",
                "build" => "var(--_s)",
                _ => "var(--_bg)",
            };
            shape += &format!(
                "<circle class=\"pele-wardley-{s}\" cx=\"{}\" cy=\"{}\" r=\"12\" fill=\"{fill}\" stroke=\"{LINE}\"/>",
                num(x),
                num(y)
            );
        }
        if strategy == Some("market") {
            let bx = tri * (std::f64::consts::PI / 6.0).cos();
            let by = y + tri * (std::f64::consts::PI / 6.0).sin();
            shape += &format!(
                "<path d=\"M{},{}L{},{}H{}Z\" fill=\"none\" stroke=\"{LINE}\"/>",
                num(x),
                num(y - tri),
                num(x - bx),
                num(by),
                num(x + bx)
            );
            for (cx, cy) in [(x, y - tri), (x - bx, by), (x + bx, by)] {
                shape += &format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\" stroke=\"{LINE}\"/>",
                    num(cx),
                    num(cy),
                    num(dot)
                )
            }
        } else if node.pipeline_parent {
            shape += &format!(
                "<rect x=\"{}\" y=\"{}\" width=\"10\" height=\"10\" fill=\"var(--_bg)\" stroke=\"{LINE}\"/>",
                num(x - 5.0),
                num(y - 5.0)
            )
        } else if !anchor {
            shape += &format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"6\" fill=\"var(--_bg)\" stroke=\"{LINE}\"/>",
                num(x),
                num(y)
            )
        }
        if node.inertia {
            let offset = if node.pipeline_parent { 5.0 } else { R } + 10.0 + if strategy.is_some() { R + 4.0 } else { 0.0 };
            shape += &format!(
                "<rect class=\"pele-wardley-inertia\" x=\"{}\" y=\"{}\" width=\"4\" height=\"12\" fill=\"{LINE}\"/>",
                num(x + offset),
                num(y - R)
            )
        }
        let ox = node.ox.map(|v| v.clamp(-5000.0, 5000.0));
        let oy = node.oy.map(|v| v.clamp(-5000.0, 5000.0));
        let caption = if anchor {
            let l =
                anchor_labels[i].get_or_insert_with(|| text(&node.label, small, if tight { 64.0_f64.max(w / 2.0) } else { 4000.0 }, BOLD));
            b.center(
                l,
                x + ox.unwrap_or(0.0),
                y + oy.unwrap_or(-3.0),
                &format!(" class=\"pele-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"", js_num(small)),
            )
        } else {
            let l = text(&node.label, small, room, 0);
            let attrs = format!(" class=\"pele-label\" font-size=\"{}\"", js_num(small));
            let lift = 8.0 + if strategy.is_some() { 10.0 } else { 0.0 };
            if ox.is_some() || oy.is_some() {
                b.start(&l, x + ox.unwrap_or(lift), y + oy.unwrap_or(-lift), &attrs)
            } else if node.in_pipeline {
                b.center(&l, x, y + 2.0 * R + 4.0 + l.height / 2.0, &attrs)
            } else {
                let busy = taken[i];
                let corner = [1, 2, 4, 8].into_iter().find(|c| busy & c == 0).unwrap_or(1);
                let right = corner == 1 || corner == 2;
                let up = corner == 1 || corner == 4;
                b.start(
                    &l,
                    if right { x + lift } else { x - lift - l.width },
                    if up { y - lift } else { y + lift + small * 0.75 },
                    &attrs,
                )
            }
        };
        b.grow(x - 2.0 * R, y - 2.0 * R, x + 2.0 * R + 20.0, y + 2.0 * R);
        nodes += &format!(
            "<g class=\"pele-node pele-wardley-{}\" data-id=\"{}\">{shape}{caption}</g>",
            if anchor {
                "anchor"
            } else if node.pipeline_parent {
                "pipeline-parent"
            } else if node.in_pipeline {
                "pipeline-component"
            } else {
                "component"
            },
            esc_text(&node.id)
        );
    }
    let mut annotations = String::new();
    for a in &model.annotations {
        let n = js_num(a.number);
        let l = text(&n, tiny, 4000.0, BOLD);
        let x = px(a.x);
        let y = py(a.y);
        let r = 10.0_f64.max(l.width / 2.0 + 4.0);
        b.grow(x - r, y - r, x + r, y + r);
        let marks = format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\" stroke=\"{LINE}\"/>",
            num(x),
            num(y),
            num(r)
        ) + &label_svg(&l, x, y, &format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", js_num(tiny)));
        annotations += &format!("<g class=\"pele-wardley-annotation\" data-id=\"{}\">{marks}</g>", esc_text(&n));
    }
    let mut listed: Vec<_> = model.annotations.iter().filter(|a| !a.text.is_empty()).collect();
    listed.sort_by(|a, b| a.number.total_cmp(&b.number));
    if !listed.is_empty() {
        let lines: Vec<_> = listed
            .iter()
            .map(|a| {
                text(
                    &format!("{}. {}", js_num(a.number), a.text),
                    small,
                    if tight { 80.0_f64.max(w - 20.0) } else { 320.0 },
                    0,
                )
            })
            .collect();
        let width = lines.iter().map(|l| l.width).fold(0.0_f64, f64::max) + 20.0;
        let height = lines.iter().map(|l| l.height).sum::<f64>() + 14.0;
        let (bx, by) = model
            .annotations_box
            .map(|(x, y)| (px(x).clamp(0.0, 0.0_f64.max(w - width)), py(y).clamp(0.0, 0.0_f64.max(h - height))))
            .unwrap_or((0.0, legend_top));
        b.grow(bx, by, bx + width, by + height);
        let mut rows = String::new();
        let mut y = by + 7.0;
        for line in lines {
            rows += &start_label(
                &line,
                bx + 10.0,
                y + line.line_height / 2.0 + small * 0.35,
                &format!(" font-size=\"{}\"", js_num(small)),
            );
            y += line.height
        }
        annotations += &format!(
            "<g class=\"pele-wardley-annotations\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>{rows}</g>",
            num(bx),
            num(by),
            num(width),
            num(height)
        );
    }
    let mut notes = String::new();
    for n in &model.notes {
        notes += &b.start(
            &text(&n.text, small, 360.0_f64.min(w * 0.6), BOLD),
            px(n.x),
            py(n.y),
            &format!(
                " class=\"pele-wardley-note\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
                js_num(small)
            ),
        )
    }
    let mut forces = String::new();
    for (forward, list) in [(true, &model.accelerators), (false, &model.deaccelerators)] {
        for f in list {
            let x = px(f.x);
            let y = py(f.y);
            let tip = if forward { x + 60.0 } else { x };
            let tail = if forward { x } else { x + 60.0 };
            let neck = if forward { x + 40.0 } else { x + 20.0 };
            let d = format!(
                "M{},{}H{}V{}L{},{}L{},{}V{}H{}Z",
                num(tail),
                num(y - 15.0),
                num(neck),
                num(y - 23.0),
                num(tip),
                num(y),
                num(neck),
                num(y + 23.0),
                num(y + 15.0),
                num(tail)
            );
            b.grow(x, y - 23.0, x + 60.0, y + 23.0);
            let caption = b.center(
                &text(&f.name, tiny + 1.0, 160.0, BOLD),
                x + 30.0,
                y + 35.0,
                &format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", js_num(tiny + 1.0)),
            );
            forces += &format!(
                "<g class=\"pele-wardley-{}\" data-id=\"{}\"><path d=\"{d}\" fill=\"var(--_s)\" stroke=\"{LINE}\"/>{caption}</g>",
                if forward { "accelerator" } else { "deaccelerator" },
                esc_text(&f.name)
            );
        }
    }
    let title = layout_label(
        model.title.as_deref(),
        false,
        m,
        size,
        if tight { 80.0_f64.max(w) } else { 4000.0 },
        BOLD,
    );
    let title_h = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let left = b.min_x.min(w / 2.0 - title.width / 2.0);
    let right = b.max_x.max(w / 2.0 + title.width / 2.0);
    let tx = pad - left;
    let ty = pad + title_h - b.min_y;
    let width = (right - left + 2.0 * pad).ceil();
    let height = (b.max_y - b.min_y + title_h + 2.0 * pad).ceil();
    let content = label_svg(
        &title,
        tx + w / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &format!(
        "<g transform=\"translate({},{})\"><g class=\"pele-wardley-frame\" fill=\"none\">{frame}</g>{stage_labels}{axis_labels}{pipelines}{}{}{nodes}{annotations}{notes}{forces}</g>",
        num(tx),
        num(ty),
        if links.is_empty() {
            links
        } else {
            format!("<g class=\"pele-edges\" fill=\"none\">{links}</g>")
        },
        if trends.is_empty() {
            trends
        } else {
            format!("<g class=\"pele-wardley-trends\" fill=\"none\">{trends}</g>")
        }
    );
    Rendered {
        svg: svg_document(
            "wardley",
            width,
            height,
            size,
            o,
            &Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            &content,
        ),
        width,
        height,
        links: Vec::new(),
    }
}
