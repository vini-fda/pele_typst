use super::model::{Model, Node};
use crate::diagrams::common::{Rendered, runs_across, turn_to_fit};
use crate::layout::{
    compound::{CEdge, CNode, Dir, cnode, compound_layout, direction},
    layered::LayeredOptions,
};
use crate::svg::{
    builder::{esc, esc_text, label_svg, shared_style, span_style, text_style},
    edges::{EdgePath, edge_label_svg, route_path},
    ink::ink_fill,
    root::{Accessible, svg_document},
    theme::{ResolvedStyle, class_names, resolve_style, rounded},
};
use crate::text::{
    label::{Label, has_math, label_line, layout_label, line_middle},
    measurer::{BOLD, ITALIC},
};
use crate::util::{js_num, js_round, num};
use crate::{Config, RenderOptions, TextMeasurer};
use indexmap::IndexMap;
use std::collections::HashMap;
struct Row {
    key: Label,
    value: Label,
    height: f64,
}
struct View<'a> {
    node: &'a Node,
    requirement: bool,
    style: ResolvedStyle,
    kind: Label,
    name: Label,
    rows: Vec<Row>,
    key_width: f64,
    head_height: f64,
    w: f64,
    h: f64,
}
fn restyle(mut l: Label, bits: u32, m: &dyn TextMeasurer) -> Label {
    if bits == 0 || l.lines.is_empty() {
        return l;
    }
    l.width = 0.0;
    l.widths.clear();
    for line in &mut l.lines {
        let mut total = 0.0;
        for span in line {
            span.style |= bits;
            if span.plain() {
                span.width = m.width(&span.text, l.size, span.style);
            }
            total += span.width;
        }
        l.widths.push(total);
        l.width = l.width.max(total);
    }
    l
}
fn start_text(l: &Label, x: f64, top: f64, attrs: &str) -> String {
    let shared = shared_style(l);
    let mut body = String::new();
    let mut math = String::new();
    for (i, line) in l.lines.iter().enumerate() {
        let middle = top + line_middle(l, i);
        if has_math(line) {
            math += &label_svg(&label_line(l, i), x + l.widths[i] / 2.0, middle, attrs);
            continue;
        }
        let y = middle + l.size * 0.35;
        let mut first = true;
        for span in line {
            if span.icon.is_some() || span.text.is_empty() {
                continue;
            }
            body += &format!(
                "<tspan{}{}>{}</tspan>",
                if first {
                    format!(" x=\"{}\" y=\"{}\"", num(x), num(y))
                } else {
                    String::new()
                },
                span_style(span.style & !shared),
                esc(&span.text)
            );
            first = false;
        }
    }
    (if body.is_empty() {
        String::new()
    } else {
        format!(
            "<text{} text-anchor=\"start\" xml:space=\"preserve\">{}</text>",
            text_style(attrs, shared),
            body
        )
    }) + &math
}

fn contains_marker(x: f64, y: f64, dx: f64, dy: f64) -> String {
    let (cx, cy) = (x - dx * 6.0, y - dy * 6.0);
    format!(
        "<circle class=\"pele-marker\" cx=\"{}\" cy=\"{}\" r=\"6\"/><path class=\"pele-marker\" d=\"M{},{}L{},{}M{},{}L{},{}\"/>",
        num(cx),
        num(cy),
        num(cx - dx * 6.0),
        num(cy - dy * 6.0),
        num(cx + dx * 6.0),
        num(cy + dy * 6.0),
        num(cx + dy * 6.0),
        num(cy - dx * 6.0),
        num(cx - dy * 6.0),
        num(cy + dx * 6.0)
    )
}
fn arrow_marker(x: f64, y: f64, dx: f64, dy: f64) -> String {
    let (bx, by) = (x - dx * 9.0, y - dy * 9.0);
    format!(
        "<path class=\"pele-marker\" d=\"M{},{}L{},{}L{},{}\" stroke-linejoin=\"round\"/>",
        num(bx - dy * 4.5),
        num(by + dx * 4.5),
        num(x),
        num(y),
        num(bx + dy * 4.5),
        num(by - dx * 4.5)
    )
}
pub fn render(model: &Model, _config: &Config, opt: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    if runs_across(Some(&model.direction)) {
        turn_to_fit(opt, |down| draw(model, opt, m, down))
    } else {
        draw(model, opt, m, false)
    }
}
fn draw(model: &Model, opt: &RenderOptions, m: &dyn TextMeasurer, turned: bool) -> Rendered {
    let size = opt.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let pad = opt.padding.unwrap_or(8.0);
    let mut index = HashMap::new();
    let mut views = vec![];
    let mut cnodes = vec![];
    for (node, requirement) in model
        .requirements
        .values()
        .map(|n| (n, true))
        .chain(model.elements.values().map(|n| (n, false)))
    {
        let style = resolve_style(&node.css_styles);
        let base = if style.bold { BOLD } else { 0 } | if style.italic { ITALIC } else { 0 };
        let name_size = style.font_size.unwrap_or(size);
        let body_size = style.font_size.unwrap_or(small);
        let kind = restyle(
            layout_label(
                Some(&format!("«{}»", if requirement { node.kind.as_str() } else { "Element" })),
                false,
                m,
                body_size,
                4000.0,
                0,
            ),
            base,
            m,
        );
        let name = restyle(layout_label(Some(&node.name), true, m, name_size, 280.0, 0), base | BOLD, m);
        let fields: Vec<(&str, &str)> = if requirement {
            vec![
                ("ID", &node.requirement_id),
                ("Text", &node.text),
                ("Risk", &node.risk),
                ("Verification", &node.verify_method),
            ]
        } else {
            vec![("Type", &node.kind), ("Doc Ref", &node.doc_ref)]
        };
        let mut rows = vec![];
        let mut key_width = 0.0f64;
        let mut value_width = 0.0f64;
        let mut body_height = 0.0;
        for (label, text) in fields {
            if text.is_empty() {
                continue;
            }
            let key = restyle(layout_label(Some(label), false, m, body_size, 4000.0, 0), base, m);
            let value = restyle(layout_label(Some(text), true, m, body_size, 240.0, 0), base, m);
            let height = key.height.max(value.height);
            key_width = key_width.max(key.width);
            value_width = value_width.max(value.width);
            body_height += height + 2.0;
            rows.push(Row { key, value, height });
        }
        if !rows.is_empty() {
            body_height += 18.0;
        }
        let head_height = 20.0 + kind.height + name.height;
        let w = 120.0f64.max(kind.width.max(name.width) + 28.0).max(if rows.is_empty() {
            0.0
        } else {
            key_width + 12.0 + value_width + 28.0
        });
        index.entry(node.name.as_str()).or_insert(views.len());
        let mut c = cnode(w, head_height + body_height);
        c.seq = views.len() as f64;
        views.push(View {
            node,
            requirement,
            style,
            kind,
            name,
            rows,
            key_width,
            head_height,
            w,
            h: head_height + body_height,
        });
        cnodes.push(c);
    }
    let dir = if turned { Dir::TB } else { direction(Some(&model.direction)) };
    let sideways = matches!(dir, Dir::LR | Dir::RL);
    let mut cedges = vec![];
    let mut labels = vec![];
    let mut drawn = vec![];
    for (i, e) in model.relations.iter().enumerate() {
        let (Some(&src), Some(&dst)) = (index.get(e.src.as_str()), index.get(e.dst.as_str())) else {
            continue;
        };
        let label = layout_label(Some(&format!("«{}»", e.kind)), false, m, small, 4000.0, 0);
        if src == dst {
            if sideways {
                cnodes[src].h += 2.0 * (26.0 + label.height + 8.0);
            } else {
                cnodes[src].w += 2.0 * (26.0 + label.width + 12.0);
            }
        }
        cedges.push(CEdge {
            src,
            dst,
            minlen: 1.0,
            label_w: label.width + 12.0,
            label_h: label.height + 4.0,
            route: vec![],
            label_x: 0.0,
            label_y: 0.0,
        });
        labels.push(label);
        drawn.push((e, format!("{}-{}-{}", e.src, e.dst, i)));
    }
    let layout = compound_layout(
        &mut cnodes,
        &mut cedges,
        dir,
        &LayeredOptions {
            node_sep: 40.0,
            edge_sep: 16.0,
            rank_sep: 56.0,
            port_sep: 20.0,
            ..Default::default()
        },
    );
    spread(&cnodes, &mut cedges, &views);
    let title = crate::svg::title::title_label(model.title.as_deref(), m, size, 4000.0);
    let th = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let inner = layout.width.max(title.width);
    let ox = pad + (inner - layout.width) / 2.0;
    let oy = pad + th;
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + th + 2.0 * pad).ceil();
    let mut nodes_out = String::new();
    for (i, v) in views.iter().enumerate() {
        let (w, h, style) = (v.w, v.h, &v.style);
        let color = ink_fill(&style.shape, &style.text, None);
        let muted = if color.is_empty() { " fill=\"var(--_m)\"" } else { &color };
        let (left, top) = (-w / 2.0, -h / 2.0);
        let mut body = format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}/>",
            num(left),
            num(top),
            num(w),
            num(h),
            rounded(&style.shape)
        );
        body += &label_svg(
            &v.kind,
            0.0,
            top + 10.0 + v.kind.height / 2.0,
            &format!(
                " class=\"pele-stereotype\" font-size=\"{}\"{}{}",
                js_num(v.kind.size),
                muted,
                style.text
            ),
        );
        body += &label_svg(
            &v.name,
            0.0,
            top + 10.0 + v.kind.height + v.name.height / 2.0,
            &format!(
                " class=\"pele-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"{}{}",
                js_num(v.name.size),
                color,
                style.text
            ),
        );
        if !v.rows.is_empty() {
            let divider = top + v.head_height;
            body += &format!(
                "<path class=\"pele-divider\" d=\"M{},{}H{}\" stroke=\"var(--_b)\"{}/>",
                num(left),
                num(divider),
                num(-left),
                style.line
            );
            let mut y = divider + 10.0;
            let mut rows = String::new();
            for row in &v.rows {
                rows += &start_text(&row.key, left + 14.0, y, &format!(" class=\"pele-field\"{}{}", muted, style.text));
                rows += &start_text(
                    &row.value,
                    left + 14.0 + v.key_width + 12.0,
                    y,
                    &format!(" class=\"pele-value\"{}{}", color, style.text),
                );
                y += row.height + 2.0;
            }
            body += &format!("<g class=\"pele-fields\" font-size=\"{}\">{}</g>", js_num(v.rows[0].key.size), rows);
        }
        let classes = v.node.classes.join(" ");
        let classes = classes
            .strip_prefix("default ")
            .or_else(|| classes.strip_prefix("default"))
            .unwrap_or(&classes);
        nodes_out += &format!(
            "<g class=\"pele-node {}{}\" data-id=\"{}\" transform=\"translate({},{})\">{}</g>",
            if v.requirement { "pele-requirement" } else { "pele-element" },
            class_names(classes),
            esc_text(&v.node.name),
            num(cnodes[i].x + ox),
            num(cnodes[i].y + oy),
            body
        );
    }
    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut loop_seen = HashMap::new();
    for (i, ce) in cedges.iter().enumerate() {
        let (e, id) = &drawn[i];
        let contains = e.kind == "contains";
        let trim = if contains { 12.0 } else { 0.0 };
        let path;
        let mut lx = ce.label_x + ox;
        let mut ly = ce.label_y + oy;
        if ce.src == ce.dst {
            let v = &views[ce.src];
            let (cx, cy) = (cnodes[ce.src].x + ox, cnodes[ce.src].y + oy);
            let k = *loop_seen.get(&ce.src).unwrap_or(&0);
            loop_seen.insert(ce.src, k + 1);
            let half = if sideways { v.w } else { v.h } / 2.0;
            let spread = (half - 4.0).min(12.0 + k as f64 * 6.0);
            let reach = 26.0 + k as f64 * 8.0;
            let at = |out, along| {
                if sideways {
                    format!("{},{}", num(cx + along), num(cy + v.h / 2.0 + out))
                } else {
                    format!("{},{}", num(cx + v.w / 2.0 + out), num(cy + along))
                }
            };
            let pull = reach / 0.75;
            path = EdgePath {
                d: format!(
                    "M{}C{} {} {}",
                    at(trim, -spread),
                    at(pull, -spread),
                    at(pull, spread),
                    at(0.0, spread)
                ),
                sx: if sideways { cx - spread } else { cx + v.w / 2.0 },
                sy: if sideways { cy + v.h / 2.0 } else { cy - spread },
                sdx: if sideways { 0.0 } else { -1.0 },
                sdy: if sideways { -1.0 } else { 0.0 },
                ex: if sideways { cx + spread } else { cx + v.w / 2.0 },
                ey: if sideways { cy + v.h / 2.0 } else { cy + spread },
                edx: if sideways { 0.0 } else { -1.0 },
                edy: if sideways { -1.0 } else { 0.0 },
                tokens: None,
            };
            if sideways {
                lx = cx;
                ly = cy + v.h / 2.0 + reach + 4.0 + ce.label_h / 2.0;
            } else {
                lx = cx + v.w / 2.0 + reach + 4.0 + ce.label_w / 2.0;
                ly = cy;
            }
        } else {
            let mut route = ce.route.clone();
            for k in (0..route.len()).step_by(3) {
                route[k] += ox;
                route[k + 1] += oy;
            }
            path = route_path(&route, None, trim, 0.0, None, None);
        }
        edges_out += &format!(
            "<g class=\"pele-edge pele-relationship-{}\" data-id=\"{}\"><path d=\"{}\"{}/>{}</g>",
            if contains { "contains" } else { "arrow" },
            esc_text(id),
            path.d,
            if contains { "" } else { " stroke-dasharray=\"5 4\"" },
            if contains {
                contains_marker(path.sx, path.sy, path.sdx, path.sdy)
            } else {
                arrow_marker(path.ex, path.ey, path.edx, path.edy)
            }
        );
        labels_out += &edge_label_svg(id, &labels[i], lx, ly, "");
    }
    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !edges_out.is_empty() {
        content += &format!(
            "<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{}</g>",
            edges_out
        );
    }
    if !labels_out.is_empty() {
        content += &format!(
            "<g class=\"pele-edge-labels\" font-size=\"{}\" fill=\"var(--_m)\">{}</g>",
            js_num(small),
            labels_out
        );
    }
    content += &format!("<g class=\"pele-nodes\">{}</g>", nodes_out);
    let svg = svg_document(
        "requirement",
        width,
        height,
        size,
        opt,
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
fn spread(nodes: &[CNode], edges: &mut [CEdge], views: &[View]) {
    let mut sides: IndexMap<usize, Vec<(usize, usize, f64)>> = IndexMap::new();
    for (i, e) in edges.iter().enumerate() {
        if e.src == e.dst || e.route.len() < 6 {
            continue;
        }
        let last = e.route.len() - 3;
        for at in [0, last] {
            let node = if at == 0 { e.src } else { e.dst };
            let c = &nodes[node];
            let (dx, dy) = (e.route[at] - c.x, e.route[at + 1] - c.y);
            let next = if at == 0 { 3 } else { at - 3 };
            let side = if (dy.abs() - c.h / 2.0).abs() < 0.5 {
                if dy < 0.0 { 0 } else { 2 }
            } else if (dx.abs() - c.w / 2.0).abs() < 0.5 {
                if dx < 0.0 { 3 } else { 1 }
            } else {
                continue;
            };
            sides.entry(node * 4 + side).or_default().push((i, at, e.route[next + side % 2]));
        }
    }
    for (key, mut ends) in sides {
        if ends.len() < 2 {
            continue;
        }
        let node = key >> 2;
        let vertical = key % 2 == 1;
        let span = if vertical { views[node].h } else { views[node].w } - 28.0;
        let step = (span / (ends.len() - 1) as f64).clamp(0.0, 28.0);
        ends.sort_by(|a, b| a.2.total_cmp(&b.2));
        let half = (ends.len() - 1) as f64 / 2.0;
        for (j, (ei, at, _)) in ends.into_iter().enumerate() {
            let off = (j as f64 - half) * step;
            edges[ei].route[at + usize::from(vertical)] = if vertical { nodes[node].y } else { nodes[node].x } + off;
        }
    }
}
