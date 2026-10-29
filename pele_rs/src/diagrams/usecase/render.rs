use super::{
    Model,
    graph::{GEdge, GNode, build},
};
use crate::diagrams::common::{Rendered, runs_across, turn_to_fit};
use crate::layout::{
    compound::{CEdge, CNode, Dir, cnode, compound_layout, direction, shift_layout},
    layered::LayeredOptions,
};
use crate::svg::{
    builder::{esc, esc_text, label_svg},
    cluster::{Crossings, mark_crossings},
    edges::{EdgePath, Fill, Under, casing, casings_svg, edge_ink, edge_label_svg, fill_under, hollow_on, marker, marker_trim, route_path},
    ink::{ink_fill, paints},
    root::{Accessible, svg_document},
    theme::{ResolvedStyle, class_names, resolve_style, rounded, sets_fill},
};
use crate::text::{
    label::{Label, label_line, layout_label, line_middle},
    measurer::{BOLD, ITALIC, TextMeasurer},
};
use crate::util::{js_num, js_round, num};
use crate::{RenderOptions, preprocess::Config};
const SHAPE: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Actor,
    Ellipse,
    Box,
    Json,
    Boundary,
}
struct View {
    node: GNode,
    kind: Kind,
    style: ResolvedStyle,
    label: Label,
    stereotype: Label,
    w: f64,
    h: f64,
    rows: Vec<(Label, Label)>,
    key_w: f64,
    head_h: f64,
    loops: usize,
}
fn option(c: &Config, key: &str, f: f64) -> f64 {
    c.get("usecase")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_f64())
        .filter(|v| *v > 0.0)
        .map(|v| v.min(2000.0))
        .unwrap_or(f)
}
fn literal(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(c, '&' | '<' | '>' | '\\') {
                format!("&#{};", c as u32)
            } else {
                c.to_string()
            }
        })
        .collect()
}
fn text(raw: Option<&str>, markdown: bool, m: &dyn TextMeasurer, size: f64, room: f64, base: u32) -> Label {
    let s = raw.map(|s| if markdown { s.to_string() } else { literal(s) });
    layout_label(s.as_deref(), markdown, m, size, room, base)
}
fn anchor(v: &View, c: &CNode, side: usize, along: f64) -> (f64, f64) {
    let sign = if side == 1 || side == 2 { 1.0 } else { -1.0 };
    let upright = side == 1 || side == 3;
    let mut reach = if upright { v.w } else { v.h } / 2.0;
    let mut middle = 0.0;
    if v.kind == Kind::Actor {
        if upright {
            reach = 23.0;
            middle = (60.0 - v.h) / 2.0
        } else {
            reach += 2.0
        }
    } else if v.kind == Kind::Ellipse {
        let span = if upright { v.h } else { v.w } / 2.0;
        reach *= (1.0 - (along / span).powi(2)).max(0.0).sqrt()
    }
    if upright {
        (c.x + sign * reach, c.y + middle + along)
    } else {
        (c.x + along, c.y + sign * reach)
    }
}
fn side_length(v: &View, side: usize) -> f64 {
    let upright = side == 1 || side == 3;
    if v.kind == Kind::Actor {
        return if upright { 30.0 } else { 24.0 };
    }
    let len = if upright { v.h } else { v.w };
    if v.kind == Kind::Ellipse { len * 0.6 } else { len - 16.0 }
}
fn attach(edges: &mut [CEdge], nodes: &[CNode], views: &[View]) {
    let mut sides: indexmap::IndexMap<usize, Vec<(usize, usize, f64)>> = indexmap::IndexMap::new();
    for (ei, e) in edges.iter().enumerate() {
        if e.src == e.dst || e.route.len() < 6 {
            continue;
        }
        let last = e.route.len() - 3;
        for (index, at, next) in [(e.src, 0, 3), (e.dst, last, last - 3)] {
            let c = &nodes[index];
            if c.is_group {
                continue;
            }
            let dx = e.route[at] - c.x;
            let dy = e.route[at + 1] - c.y;
            let side = if (dy.abs() - c.h / 2.0).abs() < 0.5 {
                if dy < 0.0 { 0 } else { 2 }
            } else if (dx.abs() - c.w / 2.0).abs() < 0.5 {
                if dx < 0.0 { 3 } else { 1 }
            } else {
                continue;
            };
            sides
                .entry(index * 4 + side)
                .or_default()
                .push((ei, at, e.route[next + if side & 1 != 0 { 1 } else { 0 }]))
        }
    }
    for (key, mut ends) in sides {
        let index = key >> 2;
        let side = key & 3;
        let step = if ends.len() > 1 {
            14.0_f64.min(side_length(&views[index], side) / (ends.len() - 1) as f64)
        } else {
            0.0
        };
        ends.sort_by(|a, b| a.2.total_cmp(&b.2));
        for (i, &(ei, at, _)) in ends.iter().enumerate() {
            let (x, y) = anchor(
                &views[index],
                &nodes[index],
                side,
                (i as f64 - (ends.len() - 1) as f64 / 2.0) * step,
            );
            edges[ei].route[at] = x;
            edges[ei].route[at + 1] = y
        }
    }
}
fn trim(kind: &str) -> f64 {
    if kind == "extension" { 12.0 } else { marker_trim(kind) }
}
fn mark(kind: &str, x: f64, y: f64, dx: f64, dy: f64, color: &str, stroke: &str) -> String {
    if kind != "extension" {
        return marker(kind, x, y, dx, dy, color, stroke);
    }
    let bx = x - dx * 12.0;
    let by = y - dy * 12.0;
    format!(
        "<path class=\"pele-marker\" d=\"M{},{}L{},{}L{},{}Z\" fill=\"var(--_bg)\" stroke=\"{color}\" stroke-linejoin=\"round\"/>",
        num(x),
        num(y),
        num(bx - dy * 6.0),
        num(by + dx * 6.0),
        num(bx + dy * 6.0),
        num(by - dx * 6.0)
    )
}
fn figure(v: &View, o: &RenderOptions) -> String {
    let n = &v.node;
    let shape = &v.style.shape;
    let line = format!(" fill=\"none\" stroke=\"var(--_b)\"{}", v.style.line);
    let mut head = -21.0;
    let mut radius = 9.0;
    let mut out = match n.actor_type.as_deref() {
        Some("hollow") => {
            radius = 8.0;
            format!(
                "<circle cy=\"-21\" r=\"8\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{shape}/><path d=\"M-20,-9H20V0H5.5L20,15.5L12,25.5L0,12L-12,25.5L-20,15.5L-5.5,0H-20Z\" fill=\"var(--_bg)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"{shape}/>"
            )
        }
        Some("awesome") => format!(
            "<path d=\"M0,-30A11,11 0 1 1 0,-8A11,11 0 1 1 0,-30ZM-20,25C-20,9 -12,-3 0,-3C12,-3 20,9 20,25C20,28 18,30 15,30H-15C-18,30 -20,28 -20,25Z\" fill=\"var(--_b)\"{shape}/>"
        ),
        Some("icon") => {
            head = -14.0;
            radius = 12.0;
            let icon = n.icon.as_deref().unwrap_or("");
            format!(
                "<rect x=\"-22\" y=\"-26\" width=\"44\" height=\"44\"{}/><svg class=\"pele-icon\" data-icon=\"{}\" x=\"-14\" y=\"-18\" width=\"28\" height=\"28\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{}</svg>",
                rounded(&(SHAPE.to_string() + shape)),
                esc_text(icon),
                o.icons.as_ref().and_then(|r| r.resolve(icon)).unwrap_or_default()
            )
        }
        _ => format!("<circle cy=\"-21\" r=\"9\"{SHAPE}{shape}/><path d=\"M0,-12V10M-14,-3H14M0,10L-12,30M0,10L12,30\"{line}/>"),
    };
    if n.business {
        out += &format!(
            "<path class=\"pele-business\" d=\"M{},{}L{},{}\"{line}/>",
            num(0.12 * radius),
            num(head + 0.99 * radius),
            num(0.92 * radius),
            num(head - 0.39 * radius)
        )
    }
    out
}
pub fn render(model: &Model, c: &Config, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    if !runs_across(Some(&model.direction)) {
        draw(model, c, o, m, false)
    } else {
        turn_to_fit(o, |down| draw(model, c, o, m, down))
    }
}
fn draw(model: &Model, config: &Config, o: &RenderOptions, m: &dyn TextMeasurer, turned: bool) -> Rendered {
    let graph = build(model);
    let size = o.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let tiny = js_round(size * 0.8125);
    let pad = o.padding.unwrap_or(8.0);
    let dir = if turned { Dir::TB } else { direction(Some(&model.direction)) };
    let sideways = matches!(dir, Dir::LR | Dir::RL);
    let index: std::collections::HashMap<String, usize> = graph.nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), i)).collect();
    let mut views = Vec::new();
    let mut cnodes = Vec::new();
    for node in graph.nodes {
        let mut declarations = node.compiled.clone();
        declarations.extend(node.styles.clone());
        let style = resolve_style(&declarations);
        let base = if style.bold { BOLD } else { 0 } | if style.italic { ITALIC } else { 0 };
        let fs = style.font_size.unwrap_or(size);
        let st = node.stereotype.as_ref().map(|s| format!("«{s}»"));
        let stereotype = text(st.as_deref(), false, m, tiny, 200.0, base);
        let mut v = View {
            node,
            kind: Kind::Box,
            style,
            label: stereotype.clone(),
            stereotype,
            w: 0.0,
            h: 0.0,
            rows: Vec::new(),
            key_w: 0.0,
            head_h: 0.0,
            loops: 0,
        };
        let node = &v.node;
        let mut c;
        if node.group {
            v.kind = Kind::Boundary;
            v.label = text(Some(&node.label), node.markdown, m, fs, 4000.0, base);
            let th = v.label.height;
            c = cnode(0.0, 0.0);
            c.is_group = true;
            c.pad_x = 20.0;
            c.pad_bottom = 20.0;
            c.pad_top = if node.boundary_type.as_deref() == Some("package") {
                th + 28.0
            } else if th > 0.0 {
                th + 16.0
            } else {
                20.0
            };
            c.min_w = v.label.width + 40.0
        } else {
            if let Some(rows) = &node.rows {
                v.kind = Kind::Json;
                v.label = text(Some(&node.label), false, m, fs, 4000.0, base | BOLD);
                let mut vw = 0.0_f64;
                for (k, value) in rows {
                    let key = text(Some(k), false, m, small, 4000.0, base);
                    let value = text(Some(value), false, m, small, 260.0, base);
                    v.key_w = v.key_w.max(key.width);
                    vw = vw.max(value.width);
                    v.h += key.height.max(value.height).max(js_round(small * 1.5)) + 8.0;
                    v.rows.push((key, value))
                }
                if v.key_w > 0.0 {
                    v.key_w += 20.0
                }
                v.head_h = v.label.height.max(js_round(fs * 1.5)) + 8.0;
                v.w = (v.label.width + 20.0)
                    .max(if v.rows.is_empty() { 0.0 } else { v.key_w + vw + 20.0 })
                    .max(72.0);
                v.h += v.head_h
            } else if node.actor_type.is_some() {
                v.kind = Kind::Actor;
                v.label = text(Some(&node.label), node.markdown, m, fs, 140.0, base);
                v.w = 40.0_f64.max(v.label.width).max(v.stereotype.width);
                v.h = 66.0 + v.stereotype.height + v.label.height
            } else {
                v.label = text(
                    Some(&node.label),
                    node.markdown,
                    m,
                    if node.shape == "note" { small } else { fs },
                    180.0,
                    base,
                );
                let tw = v.label.width.max(v.stereotype.width);
                let th = v.label.height + v.stereotype.height;
                if matches!(node.shape.as_str(), "usecaseEllipse" | "usecaseBusiness") {
                    v.kind = Kind::Ellipse;
                    v.w = (tw + 2.0 * node.padding + if node.business { 16.0 } else { 4.0 }).max(88.0);
                    v.h = (th + 28.0).max(48.0)
                } else if node.shape == "note" {
                    v.w = tw + 2.0 * node.padding + 6.0;
                    v.h = th + 18.0
                } else {
                    v.w = (tw + 2.0 * node.padding + 12.0).max(80.0);
                    v.h = (th + 24.0).max(44.0)
                }
            }
            c = cnode(v.w, v.h)
        }
        c.seq = views.len() as f64;
        c.parent = node.parent.as_ref().and_then(|p| index.get(p)).map(|&i| i as i64).unwrap_or(-1);
        views.push(v);
        cnodes.push(c)
    }
    let mut cedges = Vec::new();
    let mut edge_labels = Vec::new();
    let mut drawn: Vec<GEdge> = Vec::new();
    for edge in graph.edges {
        let (Some(&src), Some(&dst)) = (index.get(&edge.start), index.get(&edge.end)) else {
            continue;
        };
        let semantic = matches!(edge.kind.as_str(), "include" | "extend");
        let raw = if semantic {
            Some(format!("«{}»", edge.label.as_deref().unwrap_or("undefined")))
        } else {
            edge.label.clone()
        };
        let label = text(raw.as_deref(), edge.markdown, m, small, 200.0, 0);
        if src == dst {
            views[src].loops += 1;
            if sideways {
                cnodes[src].h += 2.0 * (26.0 + if label.height > 0.0 { label.height + 8.0 } else { 0.0 })
            } else {
                cnodes[src].w += 2.0 * (26.0 + if label.width > 0.0 { label.width + 12.0 } else { 0.0 })
            }
        }
        cedges.push(CEdge {
            src,
            dst,
            minlen: edge.minlen,
            label_w: if label.width > 0.0 { label.width + 12.0 } else { 0.0 },
            label_h: if label.height > 0.0 { label.height + 4.0 } else { 0.0 },
            route: Vec::new(),
            label_x: 0.0,
            label_y: 0.0,
        });
        edge_labels.push(label);
        drawn.push(edge)
    }
    let layout = compound_layout(
        &mut cnodes,
        &mut cedges,
        dir,
        &LayeredOptions {
            node_sep: option(config, "nodeSpacing", 32.0),
            edge_sep: 16.0,
            rank_sep: option(config, "rankSpacing", 72.0),
            port_sep: 20.0,
            ..Default::default()
        },
    );
    let title = crate::svg::title::title_label(model.title.as_deref(), m, size, 4000.0);
    let title_h = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let inner = layout.width.max(title.width);
    shift_layout(&mut cnodes, &mut cedges, pad + (inner - layout.width) / 2.0, pad + title_h);
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + title_h + 2.0 * pad).ceil();
    let mut crossings = Crossings::new();
    for e in &cedges {
        mark_crossings(&mut crossings, &e.route)
    }
    attach(&mut cedges, &cnodes, &views);
    let mut clusters = String::new();
    let mut fills = Vec::new();
    let mut nodes_out = String::new();
    for (i, v) in views.iter().enumerate() {
        let c = &cnodes[i];
        let node = &v.node;
        let classes = class_names(node.classes.strip_prefix("default").unwrap_or(&node.classes).trim_start());
        let id = esc_text(&node.id);
        let text_attrs = &v.style.text;
        let shape = &v.style.shape;
        let color = ink_fill(shape, text_attrs, None);
        let muted = if color.is_empty() { " fill=\"var(--_m)\"" } else { &color };
        let w = v.w;
        let h = v.h;
        if v.kind == Kind::Boundary {
            let x = c.x - c.w / 2.0;
            let y = c.y - c.h / 2.0;
            if paints(shape) {
                fills.push(Fill {
                    bx: [x, y, x + c.w, y + c.h],
                    shape: shape.clone(),
                })
            }
            let bx = format!(
                " fill=\"var(--_a)\"{} stroke=\"var(--_b)\"{shape}",
                if sets_fill(shape) { "" } else { " fill-opacity=\"0.5\"" }
            );
            let attrs = format!(" class=\"pele-cluster-label\"{muted}{text_attrs}");
            let body = if node.boundary_type.as_deref() == Some("package") {
                let tw = (v.label.width + 20.0).min(c.w);
                let th = v.label.height + 12.0;
                format!(
                    "<rect class=\"pele-boundary-tab\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{bx}/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{bx}/>",
                    num(x),
                    num(y),
                    num(tw),
                    num(th),
                    num(x),
                    num(y + th),
                    num(c.w),
                    num(c.h - th)
                ) + &label_svg(&v.label, x + tw / 2.0, y + th / 2.0, &attrs)
            } else {
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
                    num(x),
                    num(y),
                    num(c.w),
                    num(c.h),
                    rounded(&bx)
                ) + &label_svg(
                    &v.label,
                    title_x(c, v.label.width, &crossings),
                    y + 8.0 + v.label.height / 2.0,
                    &attrs,
                )
            };
            clusters += &format!("<g class=\"pele-cluster pele-boundary{classes}\" data-id=\"{id}\">{body}</g>");
            continue;
        }
        let (kind, body) = if v.kind == Kind::Actor {
            let top = -h / 2.0;
            (
                "pele-actor",
                format!(
                    "<g class=\"pele-actor-figure\" transform=\"translate(0,{})\">{}</g>",
                    num(top + 30.0),
                    figure(v, o)
                ) + &label_svg(
                    &v.stereotype,
                    0.0,
                    top + 66.0 + v.stereotype.height / 2.0,
                    &format!(" class=\"pele-stereotype\" font-size=\"{}\" fill=\"var(--_m)\"", js_num(tiny)),
                ) + &label_svg(
                    &v.label,
                    0.0,
                    h / 2.0 - v.label.height / 2.0,
                    &format!(" class=\"pele-label\"{text_attrs}"),
                ),
            )
        } else if v.kind == Kind::Json {
            let x0 = -w / 2.0;
            let mut y = -h / 2.0 + v.head_h;
            let mut cells = String::new();
            let mut lines = format!("M{},{}H{}", num(x0), num(y), num(x0 + w));
            if v.key_w > 0.0 {
                lines += &format!("M{},{}V{}", num(x0 + v.key_w), num(y), num(h / 2.0))
            }
            for (key, value) in &v.rows {
                let rh = key.height.max(value.height).max(js_round(small * 1.5)) + 8.0;
                cells += &label_svg(
                    key,
                    x0 + 10.0 + key.width / 2.0,
                    y + rh / 2.0,
                    &format!(" class=\"pele-json-key\"{muted}"),
                );
                cells += &left_label(
                    value,
                    x0 + v.key_w + 10.0,
                    y + rh / 2.0,
                    &format!(" class=\"pele-json-value\"{color}{text_attrs}"),
                );
                y += rh
            }
            (
                "pele-json",
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/><path class=\"pele-json-lines\" d=\"{lines}\" fill=\"none\" stroke=\"var(--_b)\"{}/>",
                    num(x0),
                    num(-h / 2.0),
                    num(w),
                    num(h),
                    rounded(&(SHAPE.to_string() + shape)),
                    v.style.line
                ) + &label_svg(
                    &v.label,
                    0.0,
                    -h / 2.0 + v.head_h / 2.0,
                    &format!(" class=\"pele-label\" font-weight=\"var(--_hw)\"{color}{text_attrs}"),
                ) + &format!("<g font-size=\"{}\">{cells}</g>", js_num(small)),
            )
        } else {
            let (kind, outline) = if v.kind == Kind::Ellipse {
                let mut outline = format!("<ellipse rx=\"{}\" ry=\"{}\"{SHAPE}{shape}/>", num(w / 2.0), num(h / 2.0));
                if node.business {
                    let from = w / 2.0 - 15.0;
                    let to = w / 2.0 - 5.0;
                    let arc = |at: f64| h / 2.0 * (1.0 - (at / (w / 2.0)).powi(2)).sqrt();
                    outline += &format!(
                        "<path class=\"pele-business\" d=\"M{},{}L{},{}\" fill=\"none\" stroke=\"var(--_b)\"{}/>",
                        num(from),
                        num(arc(from)),
                        num(to),
                        num(-arc(to)),
                        v.style.line
                    )
                }
                ("pele-usecase", outline)
            } else if node.shape == "note" {
                let r = w / 2.0;
                let b = h / 2.0;
                (
                    "pele-note",
                    format!(
                        "<path d=\"M{},{}H{}L{},{}V{}H{}Z\" fill=\"var(--_a)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"{shape}/><path d=\"M{},{}V{}H{}\" fill=\"none\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"{}/>",
                        num(-r),
                        num(-b),
                        num(r - 9.0),
                        num(r),
                        num(9.0 - b),
                        num(b),
                        num(-r),
                        num(r - 9.0),
                        num(-b),
                        num(9.0 - b),
                        num(r),
                        v.style.line
                    ),
                )
            } else {
                (
                    "pele-usecase",
                    format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
                        num(-w / 2.0),
                        num(-h / 2.0),
                        num(w),
                        num(h),
                        rounded(&(SHAPE.to_string() + shape))
                    ),
                )
            };
            (
                kind,
                outline
                    + &label_svg(
                        &v.stereotype,
                        0.0,
                        -v.label.height / 2.0,
                        &format!(" class=\"pele-stereotype\" font-size=\"{}\"{muted}", js_num(tiny)),
                    )
                    + &label_svg(
                        &v.label,
                        0.0,
                        v.stereotype.height / 2.0,
                        &format!(
                            " class=\"pele-label\"{}{color}{text_attrs}",
                            if node.shape == "note" {
                                format!(" font-size=\"{}\"", js_num(small))
                            } else {
                                String::new()
                            }
                        ),
                    ),
            )
        };
        nodes_out += &format!(
            "<g class=\"pele-node {kind}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">{body}</g>",
            num(c.x),
            num(c.y)
        );
    }
    let mut casings = String::new();
    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut loop_seen = std::collections::HashMap::new();
    for (i, ce) in cedges.iter_mut().enumerate() {
        let edge = &drawn[i];
        let label = &edge_labels[i];
        let mut styles = edge.compiled.clone();
        styles.extend(edge.styles.clone());
        let style = if styles.is_empty() { None } else { Some(resolve_style(&styles)) };
        let st = &edge.start_marker;
        let et = &edge.end_marker;
        let path = if ce.src == ce.dst {
            let c = &cnodes[ce.src];
            let view = &views[ce.src];
            let k = *loop_seen.entry(ce.src).or_insert(0);
            loop_seen.insert(ce.src, k + 1);
            let side = if sideways { 2 } else { 1 };
            let spread = (side_length(view, side) / 2.0).min(8.0 + k as f64 * 6.0);
            let reach = 26.0 + k as f64 * 8.0;
            let (ax, ay) = anchor(view, c, side, -spread);
            let (bx, by) = anchor(view, c, side, spread);
            let len = reach.hypot(spread);
            let len = if len == 0.0 { 1.0 } else { len };
            let ux = if sideways { spread } else { reach } / len;
            let uy = if sideways { reach } else { spread } / len;
            let start = trim(st);
            let end = trim(et);
            let out = |x: f64, y: f64, sign: f64, far: f64| {
                if sideways {
                    format!("{},{}", num(x + sign * spread * if far != 0.0 { 1.0 } else { 0.0 }), num(y + far))
                } else {
                    format!("{},{}", num(x + far), num(y + sign * spread * if far != 0.0 { 1.0 } else { 0.0 }))
                }
            };
            let sx = if sideways { ax - ux * start } else { ax + ux * start };
            let sy = if sideways { ay + uy * start } else { ay - uy * start };
            let ex = bx + ux * end;
            let ey = by + uy * end;
            if sideways {
                ce.label_x = c.x;
                ce.label_y = ay.max(by) + reach * 0.75 + 6.0 + label.height / 2.0
            } else {
                ce.label_x = ax.max(bx) + reach * 0.75 + 8.0 + label.width / 2.0;
                ce.label_y = c.y
            }
            EdgePath {
                d: format!(
                    "M{},{}C{} {} {},{}",
                    num(sx),
                    num(sy),
                    out(ax, ay, -1.0, reach),
                    out(bx, by, 1.0, reach),
                    num(ex),
                    num(ey)
                ),
                sx: ax,
                sy: ay,
                sdx: if sideways { ux } else { -ux },
                sdy: if sideways { -uy } else { uy },
                ex: bx,
                ey: by,
                edx: -ux,
                edy: -uy,
                tokens: None,
            }
        } else {
            if ce.route.len() < 6 {
                continue;
            }
            route_path(&ce.route, None, trim(st), trim(et), None, None)
        };
        let under = fill_under(&path.d, &[path.sx, path.sy, path.ex, path.ey], &fills, path.tokens.as_deref());
        let line = style.as_ref().map(|s| s.line.as_str()).unwrap_or("");
        let over = match under {
            Under::Off => None,
            Under::Crosses => Some(String::new()),
            Under::On(fill) => Some(edge_ink(fill, line)),
        };
        let color = over.as_ref().filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| {
            style
                .as_ref()
                .and_then(|s| s.stroke.as_ref())
                .map(|s| esc(s))
                .unwrap_or_else(|| "var(--_l)".into())
        });
        if over.as_deref() == Some("") {
            let marks = mark(st, path.sx, path.sy, path.sdx, path.sdy, "var(--_bg)", "var(--_bg)")
                + &mark(et, path.ex, path.ey, path.edx, path.edy, "var(--_bg)", "var(--_bg)");
            casings += &casing(&path.d, &marks, line, false)
        }
        let dashes = if edge.internal {
            "2 3"
        } else if edge.pattern == "dotted" {
            "5 4"
        } else if edge.animate {
            "9 5"
        } else {
            ""
        };
        let marks = mark(st, path.sx, path.sy, path.sdx, path.sdy, &color, "none")
            + &mark(et, path.ex, path.ey, path.edx, path.edy, &color, "none");
        let marks = if let Under::On(fill) = under {
            if over.as_ref().is_some_and(|s| !s.is_empty()) {
                hollow_on(&marks, fill)
            } else {
                marks
            }
        } else {
            marks
        };
        let classes = edge
            .classes
            .strip_prefix("default relationship relationship-")
            .and_then(|s| s.split_once(' ').map(|(_, tail)| tail))
            .unwrap_or("");
        edges_out += &format!(
            "<g class=\"pele-edge pele-{}{}\" data-id=\"{}\"{}{line}><path d=\"{}\"{}/>{marks}</g>",
            edge.kind,
            class_names(classes),
            esc_text(&edge.id),
            over.as_ref()
                .filter(|s| !s.is_empty())
                .map(|s| format!(" stroke=\"{s}\""))
                .unwrap_or_default(),
            path.d,
            if dashes.is_empty() {
                String::new()
            } else {
                format!(" stroke-dasharray=\"{dashes}\"")
            }
        );
        if label.width > 0.0 {
            let attrs = if matches!(edge.kind.as_str(), "include" | "extend") {
                " fill=\"var(--_m)\""
            } else {
                ""
            }
            .to_string()
                + style.as_ref().map(|s| s.text.as_str()).unwrap_or("");
            labels_out += &edge_label_svg(&edge.id, label, ce.label_x, ce.label_y, &attrs)
        }
    }
    let content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &if clusters.is_empty() {
        clusters
    } else {
        format!("<g class=\"pele-clusters\">{clusters}</g>")
    } + &casings_svg(&casings)
        + &if edges_out.is_empty() {
            edges_out
        } else {
            format!("<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>")
        }
        + &if labels_out.is_empty() {
            labels_out
        } else {
            format!("<g class=\"pele-edge-labels\" font-size=\"{}\">{labels_out}</g>", js_num(small))
        }
        + &format!("<g class=\"pele-nodes\">{nodes_out}</g>");
    Rendered {
        svg: svg_document(
            "usecase",
            width,
            height,
            size,
            o,
            &Accessible {
                acc_title: Some(&model.acc_title),
                acc_descr: Some(&model.acc_descr),
            },
            &content,
        ),
        width,
        height,
        links: Vec::new(),
    }
}
fn title_x(c: &CNode, width: f64, crossings: &Crossings) -> f64 {
    let Some(xs) = crossings.get(&(js_round(c.y - c.h / 2.0) as i64)) else {
        return c.x;
    };
    if width == 0.0 {
        return c.x;
    }
    let left = c.x - c.w / 2.0 + 6.0;
    let right = c.x + c.w / 2.0 - 6.0;
    let mut cuts: Vec<_> = xs.iter().copied().filter(|x| *x > left && *x < right).collect();
    cuts.sort_by(f64::total_cmp);
    cuts.push(right);
    let need = width / 2.0 + 6.0;
    let mut best = c.x;
    let mut off = f64::INFINITY;
    let mut widest = 0.0;
    let mut from = left;
    for cut in cuts {
        if cut - from >= 2.0 * need {
            let x = c.x.max(from + need).min(cut - need);
            if (x - c.x).abs() < off {
                off = (x - c.x).abs();
                best = x
            }
        } else if off == f64::INFINITY && cut - from > widest {
            widest = cut - from;
            best = (from + cut) / 2.0
        }
        from = cut
    }
    best
}
fn left_label(l: &Label, x: f64, cy: f64, attrs: &str) -> String {
    if l.lines.len() < 2 {
        return label_svg(l, x + l.width / 2.0, cy, attrs);
    }
    let mut out = String::new();
    for i in 0..l.lines.len() {
        out += &label_svg(
            &label_line(l, i),
            x + l.widths[i] / 2.0,
            cy - l.height / 2.0 + line_middle(l, i),
            attrs,
        )
    }
    out
}
