use super::{layout::place, model::Model};
use crate::{
    Config, RenderOptions, TextMeasurer,
    diagrams::common::{Rendered, tighten, title_room},
    svg::{
        builder::{esc_text, label_svg},
        edges::{edge_label_svg, marker, marker_trim},
        root::{Accessible, svg_document},
        theme::rounded,
        title::title_label,
    },
    text::label::{Label, Span, layout_label},
    util::{js_num, js_round, num},
};
use std::collections::BTreeMap;
const STUB: f64 = 12.;
const DOT: f64 = 3.;
const DX: [f64; 4] = [-1., 1., 0., 0.];
const DY: [f64; 4] = [0., 0., -1., 1.];
fn setting(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    config
        .get("architecture")
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_f64())
        .filter(|n| n.is_finite())
        .map_or(fallback, |n| n.clamp(min, max))
}
fn rank(mut values: Vec<i32>) -> BTreeMap<i32, usize> {
    values.sort();
    values.dedup();
    values.into_iter().enumerate().map(|(i, v)| (v, i)).collect()
}
fn icon_slot(name: &str, x: f64, y: f64, side: f64, options: &RenderOptions) -> String {
    let inner = options.icons.as_ref().and_then(|r| r.resolve(name)).unwrap_or_else(|| {
        let d = match name {
            "database" => "M4 6a8 3 0 1 0 16 0a8 3 0 1 0-16 0M4 6v12a8 3 0 0 0 16 0V6M4 12a8 3 0 0 0 16 0",
            "server" => "M3 4h18v7H3zM3 13h18v7H3zM7 7.5h.01M7 16.5h.01M11 7.5h6M11 16.5h6",
            "disk" => "M5 3h14v18H5zM12 6.5a4 4 0 1 0 0 8a4 4 0 1 0 0-8M12 10.5h.01M8 18h3",
            "internet" => "M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0M3 12h18M12 3a13 13 0 0 0 0 18M12 3a13 13 0 0 1 0 18",
            "cloud" => "M7 18a4 4 0 0 1-.5-7.97A6 6 0 0 1 18 9.5a4.25 4.25 0 0 1-.5 8.5z",
            _ => return String::new(),
        };
        format!("<path d=\"{d}\" stroke=\"var(--_fg)\" stroke-width=\"1.5\"/>")
    });
    format!(
        "<svg class=\"pele-icon\" data-icon=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{inner}</svg>",
        esc_text(name),
        num(x),
        num(y),
        num(side),
        num(side)
    )
}
fn fit(mut label: Label, max_width: f64, max_lines: usize, m: &dyn TextMeasurer) -> Label {
    let cut = label.lines.len() > max_lines;
    if !cut && label.width <= max_width {
        return label;
    }
    let dots = m.width("…", label.size, 0);
    let mut lines = Vec::new();
    let mut widths = Vec::new();
    let mut widest: f64 = 0.;
    for i in 0..max_lines.min(label.lines.len()) {
        let mut line = label.lines[i].clone();
        let mut width = label.widths[i];
        if width > max_width || (cut && i == max_lines - 1) {
            loop {
                let Some(span) = line.last_mut() else {
                    line.push(Span {
                        text: "…".into(),
                        style: 0,
                        width: dots,
                        ..Default::default()
                    });
                    width = dots;
                    break;
                };
                let rest = width - span.width;
                if span.icon.is_none() && rest + dots < max_width {
                    let text: Vec<u16> = span.text.encode_utf16().collect();
                    let (mut lo, mut hi) = (0, text.len());
                    while lo < hi {
                        let mid = (lo + hi).div_ceil(2);
                        if rest + m.width(&String::from_utf16_lossy(&text[..mid]), label.size, span.style) + dots <= max_width {
                            lo = mid
                        } else {
                            hi = mid - 1
                        }
                    }
                    if lo > 0 && (0xd800..=0xdbff).contains(&text[lo - 1]) {
                        lo -= 1;
                    }
                    span.text = format!("{}…", String::from_utf16_lossy(&text[..lo]).trim_end());
                    span.width = m.width(&span.text, label.size, span.style);
                    width = rest + span.width;
                    break;
                }
                line.pop();
                width = rest;
            }
        }
        lines.push(line);
        widths.push(width);
        widest = widest.max(width);
    }
    label.height = lines.len() as f64 * label.line_height;
    label.lines = lines;
    label.widths = widths;
    label.width = widest;
    label
}
#[derive(Clone, Copy)]
struct Port {
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    gap: f64,
    wire: bool,
}
fn route(s: Port, e: Port) -> Vec<f64> {
    let (sx, sy, sdx, sdy, ex, ey, edx, edy) = (s.x, s.y, s.dx, s.dy, e.x, e.y, e.dx, e.dy);
    let (s1x, s1y, e1x, e1y) = (sx + sdx * STUB, sy + sdy * STUB, ex + edx * STUB, ey + edy * STUB);
    if sdx * edx + sdy * edy == 0. {
        let (cx, cy) = if sdx != 0. { (ex, sy) } else { (sx, ey) };
        if (cx - sx) * sdx + (cy - sy) * sdy > 0. && (cx - ex) * edx + (cy - ey) * edy > 0. {
            return vec![sx, sy, cx, cy, ex, ey];
        }
        return if sdx != 0. {
            vec![sx, sy, s1x, s1y, s1x, e1y, e1x, e1y, ex, ey]
        } else {
            vec![sx, sy, s1x, s1y, e1x, s1y, e1x, e1y, ex, ey]
        };
    }
    let sideways = sdx != 0.;
    let offset = if sideways { ey - sy } else { ex - sx };
    if sdx == -edx && sdy == -edy {
        let ahead = (ex - sx) * sdx + (ey - sy) * sdy;
        if ahead > 0. {
            if offset.abs() < 0.5 {
                return vec![sx, sy, ex, ey];
            }
            if sideways {
                let (lo, hi) = if sdx > 0. { (s.x1, e.x0) } else { (e.x1, s.x0) };
                let mx = if lo < hi { (lo + hi) / 2. } else { (sx + ex) / 2. };
                return vec![sx, sy, mx, sy, mx, ey, ex, ey];
            }
            let (lo, hi) = if sdy > 0. { (s.y1, e.y0) } else { (e.y1, s.y0) };
            let my = if lo < hi { (lo + hi) / 2. } else { (sy + ey) / 2. };
            return vec![sx, sy, sx, my, ex, my, ex, ey];
        }
    } else if offset.abs() >= 1. {
        if sideways {
            let x = if sdx > 0. { sx.max(ex) + STUB } else { sx.min(ex) - STUB };
            return vec![sx, sy, x, sy, x, ey, ex, ey];
        }
        let y = if sdy > 0. { sy.max(ey) + STUB } else { sy.min(ey) - STUB };
        return vec![sx, sy, sx, y, ex, y, ex, ey];
    }
    if sideways {
        let lane = if s.y1 + STUB <= e.y0 {
            (s.y1 + e.y0) / 2.
        } else if e.y1 + STUB <= s.y0 {
            (e.y1 + s.y0) / 2.
        } else {
            s.y1.max(e.y1) + STUB
        };
        vec![sx, sy, s1x, sy, s1x, lane, e1x, lane, e1x, ey, ex, ey]
    } else {
        let lane = if s.x1 + STUB <= e.x0 {
            (s.x1 + e.x0) / 2.
        } else if e.x1 + STUB <= s.x0 {
            (e.x1 + s.x0) / 2.
        } else {
            s.x1.max(e.x1) + STUB
        };
        vec![sx, sy, sx, s1y, lane, s1y, lane, e1y, ex, e1y, ex, ey]
    }
}
fn simplify(p: &[f64]) -> Vec<f64> {
    let mut out = vec![p[0], p[1]];
    for point in p[2..].as_chunks::<2>().0 {
        let (x, y, n) = (point[0], point[1], out.len());
        let same_x = (x - out[n - 2]).abs() < 0.01;
        let same_y = (y - out[n - 1]).abs() < 0.01;
        if same_x && same_y {
            continue;
        }
        if n >= 4 && ((same_x && (out[n - 4] - out[n - 2]).abs() < 0.01) || (same_y && (out[n - 3] - out[n - 1]).abs() < 0.01)) {
            out[n - 2] = x;
            out[n - 1] = y;
        } else {
            out.extend_from_slice(&[x, y]);
        }
    }
    out
}
fn path_data(p: &[f64], start_trim: f64, end_trim: f64) -> String {
    let n = p.len() / 2;
    let (mut xs, mut ys): (Vec<_>, Vec<_>) = p.as_chunks::<2>().0.iter().map(|p| (p[0], p[1])).unzip();
    for (at, toward, amount) in [(0, 1, start_trim), (n - 1, n - 2, end_trim)] {
        let len = (xs[toward] - xs[at]).hypot(ys[toward] - ys[at]);
        let t = amount.min((len - 1.).max(0.));
        if len > 0. && t > 0. {
            xs[at] += (xs[toward] - xs[at]) / len * t;
            ys[at] += (ys[toward] - ys[at]) / len * t;
        }
    }
    let mut d = format!("M{},{}", num(xs[0]), num(ys[0]));
    for i in 1..n - 1 {
        let before = (xs[i] - xs[i - 1]).hypot(ys[i] - ys[i - 1]);
        let after = (xs[i + 1] - xs[i]).hypot(ys[i + 1] - ys[i]);
        let r = 6f64.min(before / 2.).min(after / 2.);
        if r < 0.5 {
            d += &format!("L{},{}", num(xs[i]), num(ys[i]));
            continue;
        }
        let ix = xs[i] - (xs[i] - xs[i - 1]) / before * r;
        let iy = ys[i] - (ys[i] - ys[i - 1]) / before * r;
        let ox = xs[i] + (xs[i + 1] - xs[i]) / after * r;
        let oy = ys[i] + (ys[i + 1] - ys[i]) / after * r;
        d += &format!("L{},{}Q{},{} {},{}", num(ix), num(iy), num(xs[i]), num(ys[i]), num(ox), num(oy));
    }
    d + &format!("L{},{}", num(xs[n - 1]), num(ys[n - 1]))
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    tighten(options, |tight| draw(model, config, options, m, tight))
}
fn draw(model: &Model, config: &Config, options: &RenderOptions, m: &dyn TextMeasurer, tight: bool) -> Rendered {
    let icon_size = setting(config, "iconSize", 48., 8., 512.);
    let size = setting(config, "fontSize", options.font_size.unwrap_or(16.), 4., 128.);
    let group_pad = setting(config, "padding", 20., 0., 256.).min(if tight { 12. } else { f64::INFINITY });
    let small = js_round(size * 0.875);
    let pad = options.padding.unwrap_or(8.);
    let half = icon_size / 2.;
    let nodes = model.nodes.values().collect::<Vec<_>>();
    let groups = model.groups.values().collect::<Vec<_>>();
    let (n, g) = (nodes.len(), groups.len());
    let grid = place(&nodes, &groups, &model.edges, &model.hints);
    let index: BTreeMap<_, _> = nodes.iter().enumerate().map(|(i, node)| (node.id.as_str(), i)).collect();
    let (mut labels, mut box_w, mut box_h) = (Vec::new(), vec![0f64; n], vec![0f64; n]);
    let wrap = if tight { icon_size.max(64.) } else { (icon_size * 2.25).max(96.) };
    for i in 0..n {
        let label = layout_label(
            if nodes[i].junction { None } else { nodes[i].title.as_deref() },
            true,
            m,
            size,
            wrap,
            0,
        );
        box_w[i] = icon_size.max(2. * (label.width / 2.).ceil());
        box_h[i] = icon_size + if label.height > 0. { 4. + label.height } else { 0. };
        labels.push(label);
    }
    let (mut col_values, mut row_values) = (grid.col.clone(), grid.row.clone());
    for i in 0..g {
        if grid.group_empty[i] {
            col_values.push(grid.group_col[i]);
            row_values.push(grid.group_row[i]);
        }
    }
    let col_of = rank(col_values);
    let row_of = rank(row_values);
    let (c, r) = (col_of.len(), row_of.len());
    let (mut col_w, mut row_h) = (vec![0f64; c], vec![0f64; r]);
    let gap = if tight { 24. } else { 40f64.max(js_round(icon_size * 0.75)) };
    let (mut gap_x, mut gap_y) = (vec![gap; c], vec![gap; r]);
    let (mut node_col, mut node_row) = (vec![0; n], vec![0; n]);
    for i in 0..n {
        let c = col_of[&grid.col[i]];
        let r = row_of[&grid.row[i]];
        node_col[i] = c;
        node_row[i] = r;
        col_w[c] = col_w[c].max(box_w[i]);
        row_h[r] = row_h[r].max(box_h[i]);
    }
    for i in 0..g {
        if grid.group_empty[i] {
            let c = col_of[&grid.group_col[i]];
            let r = row_of[&grid.group_row[i]];
            col_w[c] = col_w[c].max(icon_size);
            row_h[r] = row_h[r].max(half);
        }
    }
    let e = model.edges.len();
    let mut edge_labels = Vec::new();
    let mut ends = Vec::new();
    for edge in &model.edges {
        let a = index.get(edge.lhs_id.as_str()).copied();
        let b = index.get(edge.rhs_id.as_str()).copied();
        ends.push((a, b));
        let label = layout_label(edge.title.as_deref(), true, m, small, 160., 0);
        if label.width > 0.
            && let (Some(a), Some(b)) = (a, b)
        {
            if node_col[a] != node_col[b] {
                let c = node_col[a].min(node_col[b]);
                gap_x[c] = gap_x[c].max(label.width.ceil() + 28.);
            } else {
                col_w[node_col[a]] = col_w[node_col[a]].max(2. * (label.width / 2.).ceil() + 8.);
                if node_row[a] != node_row[b] {
                    let r = node_row[a].min(node_row[b]);
                    gap_y[r] = gap_y[r].max(label.height.ceil() + 24.);
                }
            }
        }
        edge_labels.push(label);
    }
    let (mut group_labels, mut c0, mut c1, mut r0, mut r1) = (Vec::new(), vec![0; g], vec![0; g], vec![0; g], vec![0; g]);
    let (mut ins_l, mut ins_r, mut ins_t, mut ins_b, mut head_w, mut head_h) = (
        vec![0f64; g],
        vec![0f64; g],
        vec![0f64; g],
        vec![0f64; g],
        vec![0f64; g],
        vec![0f64; g],
    );
    let (mut col_l, mut col_r, mut row_t, mut row_b) = (vec![0f64; c], vec![0f64; c], vec![0f64; r], vec![0f64; r]);
    let badge = js_round(size * 1.25);
    for i in 0..g {
        c0[i] = col_of[&grid.group_col[i]];
        c1[i] = col_of[&(grid.group_col[i] + grid.group_cols[i] - 1)];
        r0[i] = row_of[&grid.group_row[i]];
        r1[i] = row_of[&(grid.group_row[i] + grid.group_rows[i] - 1)];
        let label = layout_label(
            groups[i].title.as_deref(),
            true,
            m,
            size,
            if tight { 80f64.max(title_room(options) / 2.) } else { 320. },
            0,
        );
        let icon = if groups[i].icon.as_ref().is_some_and(|s| !s.is_empty()) {
            badge
        } else {
            0.
        };
        head_w[i] = label.width + icon + if label.width > 0. && icon > 0. { 6. } else { 0. };
        head_h[i] = label.height.max(icon);
        group_labels.push(label);
    }
    for i in (0..g).rev() {
        let left = group_pad + ins_l[i];
        let mut right = group_pad + ins_r[i];
        let top = if head_h[i] > 0. {
            group_pad.max(head_h[i] + 12.)
        } else {
            group_pad
        } + ins_t[i];
        let bottom = group_pad + ins_b[i];
        if head_w[i] > 0. {
            let mut inner = left + right;
            for c in c0[i]..=c1[i] {
                inner += col_w[c] + if c < c1[i] { col_r[c] + gap_x[c] + col_l[c + 1] } else { 0. };
            }
            if head_w[i] + 20. > inner {
                right += (head_w[i] + 20. - inner).ceil();
            }
        }
        ins_l[i] = left;
        ins_r[i] = right;
        ins_t[i] = top;
        ins_b[i] = bottom;
        col_l[c0[i]] = col_l[c0[i]].max(left);
        col_r[c1[i]] = col_r[c1[i]].max(right);
        row_t[r0[i]] = row_t[r0[i]].max(top);
        row_b[r1[i]] = row_b[r1[i]].max(bottom);
        let p = grid.group_parent[i];
        if p >= 0 {
            let p = p as usize;
            if c0[p] == c0[i] {
                ins_l[p] = ins_l[p].max(left);
            }
            if c1[p] == c1[i] {
                ins_r[p] = ins_r[p].max(right);
            }
            if r0[p] == r0[i] {
                ins_t[p] = ins_t[p].max(top);
            }
            if r1[p] == r1[i] {
                ins_b[p] = ins_b[p].max(bottom);
            }
        }
    }
    let (mut x, mut y) = (vec![0f64; c], vec![0f64; r]);
    let (mut inner_w, mut inner_h) = (0., 0.);
    for i in 0..c {
        x[i] = inner_w + col_l[i];
        inner_w = x[i] + col_w[i] + col_r[i] + if i + 1 < c { gap_x[i] } else { 0. };
    }
    for i in 0..r {
        y[i] = inner_h + row_t[i];
        inner_h = y[i] + row_h[i] + row_b[i] + if i + 1 < r { gap_y[i] } else { 0. };
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (0f64, 0f64, inner_w, inner_h);
    let left = |i: usize| x[c0[i]] - ins_l[i];
    let right = |i: usize| x[c1[i]] + col_w[c1[i]] + ins_r[i];
    let top = |i: usize| y[r0[i]] - ins_t[i];
    let bottom = |i: usize| y[r1[i]] + row_h[r1[i]] + ins_b[i];
    let center_x = |i: usize| x[node_col[i]] + col_w[node_col[i]] / 2.;
    let port = |i: usize, side: usize, via_group: bool| {
        let (dx, dy) = (DX[side], DY[side]);
        let cx = center_x(i);
        let y = y[node_row[i]];
        let mut p = Port {
            x: cx,
            y: y + half,
            dx,
            dy,
            x0: cx - box_w[i] / 2.,
            y0: y,
            x1: cx + box_w[i] / 2.,
            y1: y + box_h[i],
            gap: 0.,
            wire: false,
        };
        if nodes[i].junction {
            p.x0 = cx;
            p.x1 = cx;
            p.y0 = p.y;
            p.y1 = p.y;
            p.gap = DOT;
        } else if dx != 0. {
            p.x = cx + dx * half;
        } else {
            p.y = if dy < 0. {
                y
            } else {
                y + box_h[i] + if labels[i].height > 0. { 2. } else { 0. }
            };
        }
        let g = if via_group { grid.node_parent[i] } else { -1 };
        if g >= 0 {
            let g = g as usize;
            p.x0 = left(g);
            p.x1 = right(g);
            p.y0 = top(g);
            p.y1 = bottom(g);
            p.gap = 0.;
            p.wire = nodes[i].junction;
            if dx != 0. {
                p.x = if dx < 0. { p.x0 } else { p.x1 }
            } else {
                p.y = if dy < 0. { p.y0 } else { p.y1 }
            }
        }
        p
    };
    let (mut edges_out, mut labels_out) = (String::new(), String::new());
    let mut runs = Vec::new();
    let trim_length = marker_trim("arrow_point");
    for k in 0..e {
        let edge = &model.edges[k];
        let (Some(a), Some(b)) = ends[k] else { continue };
        let (mut s, mut end) = (port(a, edge.lhs_dir, edge.lhs_group), port(b, edge.rhs_dir, edge.rhs_group));
        if s.x == end.x && s.y == end.y && s.dx == end.dx {
            s.x -= 8. * s.dy.abs();
            s.y -= 8. * s.dx.abs();
            end.x += 8. * s.dy.abs();
            end.y += 8. * s.dx.abs();
        }
        let mut routed = route(s, end);
        if s.wire {
            routed.splice(..0, [center_x(a), y[node_row[a]] + half]);
        }
        if end.wire {
            routed.extend_from_slice(&[center_x(b), y[node_row[b]] + half]);
        }
        let points = simplify(&routed);
        if points.len() < 4 {
            continue;
        }
        let id = format!("{}-{}", edge.lhs_id, edge.rhs_id);
        edges_out += &format!(
            "<g class=\"pele-edge\" data-id=\"{}\"><path d=\"{}\"/>{}{}</g>",
            esc_text(&id),
            path_data(
                &points,
                if edge.lhs_into && !s.wire { trim_length + s.gap } else { 0. },
                if edge.rhs_into && !end.wire { trim_length + end.gap } else { 0. }
            ),
            if edge.lhs_into {
                marker(
                    "arrow_point",
                    s.x + s.dx * s.gap,
                    s.y + s.dy * s.gap,
                    -s.dx,
                    -s.dy,
                    "var(--_l)",
                    "none",
                )
            } else {
                String::new()
            },
            if edge.rhs_into {
                marker(
                    "arrow_point",
                    end.x + end.dx * end.gap,
                    end.y + end.dy * end.gap,
                    -end.dx,
                    -end.dy,
                    "var(--_l)",
                    "none",
                )
            } else {
                String::new()
            }
        );
        let (mut best, mut best_length, mut flat, mut flat_length) = (0, -1., 0, -1.);
        let label = &edge_labels[k];
        for p in points.as_chunks::<2>().0 {
            min_x = min_x.min(p[0] - 4.);
            max_x = max_x.max(p[0] + 4.);
            min_y = min_y.min(p[1] - 4.);
            max_y = max_y.max(p[1] + 4.);
        }
        for i in (0..points.len() - 2).step_by(2) {
            let horizontal = (points[i + 1] - points[i + 3]).abs() < 0.01;
            let length = (points[i] - points[i + 2]).abs() + (points[i + 1] - points[i + 3]).abs();
            if !horizontal {
                runs.extend_from_slice(&[points[i], points[i + 1].min(points[i + 3]), points[i + 1].max(points[i + 3])]);
            }
            if length > best_length {
                best = i;
                best_length = length;
            }
            if horizontal && length > flat_length {
                flat = i;
                flat_length = length;
            }
        }
        if label.width > 0. {
            let at = if flat_length >= label.width + 8. { flat } else { best };
            let mut lx = (points[at] + points[at + 2]) / 2.;
            let mut ly = (points[at + 1] + points[at + 3]) / 2.;
            let w = label.width + 8.;
            let upright = (points[at] - points[at + 2]).abs() < 0.01;
            for p in [s, end] {
                if lx + w / 2. <= p.x0 || lx - w / 2. >= p.x1 || ly + label.height / 2. <= p.y0 || ly - label.height / 2. >= p.y1 {
                    continue;
                }
                if upright {
                    lx = if lx >= (p.x0 + p.x1) / 2. {
                        p.x1 + w / 2. + 4.
                    } else {
                        p.x0 - w / 2. - 4.
                    };
                } else {
                    ly = if ly >= (p.y0 + p.y1) / 2. {
                        p.y1 + label.height / 2. + 4.
                    } else {
                        p.y0 - label.height / 2. - 4.
                    };
                }
            }
            min_x = min_x.min(lx - w / 2.);
            max_x = max_x.max(lx + w / 2.);
            min_y = min_y.min(ly - label.height / 2.);
            max_y = max_y.max(ly + label.height / 2.);
            labels_out += &edge_label_svg(&id, label, lx, ly, "");
        }
    }
    let mut clusters = String::new();
    for i in 0..g {
        let group = groups[i];
        let x = left(i);
        let y = top(i);
        let w = right(i) - x;
        let label = &group_labels[i];
        let mut head = String::new();
        if head_w[i] > 0. {
            let band = y + head_h[i] + 12.;
            let mut crossings: Vec<f64> = runs
                .as_chunks::<3>()
                .0
                .iter()
                .filter(|p| p[1] < band && p[2] > y && p[0] > x && p[0] < x + w)
                .map(|p| p[0])
                .collect();
            crossings.sort_by(f64::total_cmp);
            crossings.push(x + w - 4.);
            let mut start = x + 10.;
            for crossing in crossings {
                if crossing - 6. - start >= head_w[i] {
                    break;
                }
                start = start.max(crossing + 6.);
            }
            if start + head_w[i] > x + w - 4. {
                start = x + 10.;
            }
            let cy = y + 6. + head_h[i] / 2.;
            let mut text_x = start;
            if let Some(icon) = group.icon.as_deref().filter(|s| !s.is_empty()) {
                head += &icon_slot(icon, start, cy - badge / 2., badge, options);
                text_x += badge + 6.;
            }
            head += &label_svg(
                label,
                text_x + label.width / 2.,
                cy,
                " class=\"pele-cluster-label\" fill=\"var(--_m)\"",
            );
        }
        clusters += &format!(
            "<g class=\"pele-cluster\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{}/>{head}</g>",
            esc_text(&group.id),
            num(x),
            num(y),
            num(w),
            num(bottom(i) - y),
            rounded("")
        );
    }
    let mut nodes_out = String::new();
    let glyph = icon_size * 0.5;
    let text_size = 6f64.max(js_round(size * 0.75));
    for i in 0..n {
        let node = nodes[i];
        let at = format!(
            "data-id=\"{}\" transform=\"translate({},{})\"",
            esc_text(&node.id),
            num(center_x(i)),
            num(y[node_row[i]])
        );
        if node.junction {
            nodes_out += &format!(
                "<g class=\"pele-node pele-junction\" {at}><circle cy=\"{}\" r=\"3\" fill=\"var(--_l)\"/></g>",
                num(half)
            );
            continue;
        }
        let mut inner = String::new();
        if let Some(icon) = node.icon.as_deref().filter(|s| !s.is_empty()) {
            inner = icon_slot(icon, -glyph / 2., (icon_size - glyph) / 2., glyph, options);
        } else if let Some(text) = node.icon_text.as_deref().filter(|s| !s.is_empty()) {
            let text = layout_label(Some(text), false, m, text_size, icon_size - 8., 0);
            let lines = ((icon_size - 4.) / if text.line_height == 0. { 1. } else { text.line_height })
                .floor()
                .max(1.) as usize;
            inner = label_svg(
                &fit(text, icon_size - 8., lines, m),
                0.,
                half,
                &format!(" class=\"pele-icon-text\" font-size=\"{}\"", js_num(text_size)),
            );
        }
        nodes_out += &format!(
            "<g class=\"pele-node pele-service\" {at}><rect x=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}/>{}{}</g>",
            num(-half),
            num(icon_size),
            num(icon_size),
            rounded(""),
            inner,
            label_svg(&labels[i], 0., icon_size + 4. + labels[i].height / 2., " class=\"pele-label\"")
        );
    }
    let title = title_label(model.title.as_deref(), m, size, if tight { title_room(options) } else { 4000. });
    let title_height = if title.height > 0. { title.height + 12. } else { 0. };
    let width = ((max_x - min_x).max(title.width) + 2. * pad).ceil();
    let height = (max_y - min_y + title_height + 2. * pad).ceil();
    let ox = js_round((width - (max_x - min_x)) / 2. - min_x);
    let oy = js_round(pad + title_height - min_y);
    let mut body = String::new();
    if !clusters.is_empty() {
        body += &format!("<g class=\"pele-clusters\">{clusters}</g>");
    }
    if !edges_out.is_empty() {
        body += &format!(
            "<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{edges_out}</g>"
        );
    }
    if !labels_out.is_empty() {
        body += &format!("<g class=\"pele-edge-labels\" font-size=\"{}\">{labels_out}</g>", js_num(small));
    }
    if !nodes_out.is_empty() {
        body += &format!("<g class=\"pele-nodes\">{nodes_out}</g>");
    }
    let mut content = label_svg(
        &title,
        width / 2.,
        pad + title.height / 2.,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !body.is_empty() {
        content += &format!("<g transform=\"translate({},{})\">{body}</g>", js_num(ox), js_num(oy));
    }
    let svg = svg_document(
        "architecture",
        width,
        height,
        size,
        options,
        &Accessible {
            acc_title: model.acc_title.as_deref(),
            acc_descr: model.acc_descr.as_deref(),
        },
        &content,
    );
    Rendered {
        svg,
        width,
        height,
        links: vec![],
    }
}
