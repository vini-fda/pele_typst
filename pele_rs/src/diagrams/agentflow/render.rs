use super::{
    db::{LabelType, Model},
    graph::{self, GraphEdge, GraphNode},
    shapes,
};
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, runs_across, turn_to_fit};
use crate::diagrams::flowchart::shapes::canonical_shape;
use crate::layout::compound::{CEdge, CNode, Dir, cnode, compound_layout, direction, inherit_crossed_directions, shift_layout};
use crate::layout::layered::LayeredOptions;
use crate::preprocess::Config;
use crate::svg::builder::{esc, esc_text, label_svg};
use crate::svg::cluster::{Box4, Crossings, cluster_title_x, mark_crossings};
use crate::svg::edges::{
    EdgePath, Fill, LoopLabel, casings_svg, edge_casing, edge_label_svg, loop_path, marker, marker_trim, over_fills, route_path,
};
use crate::svg::ink::{ink_fill, paints};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::shapes::{PAD_X, PAD_Y, draw_shape, inset_route, shape_filled, shape_size};
use crate::svg::theme::{ResolvedStyle, class_names, resolve_style, rounded, series_color, sets_fill};
use crate::text::entities::decode_entities;
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, ITALIC, TextMeasurer};
use crate::url::{LinkInfo, internal_link, linked, rel_attr};
use crate::util::{js_round, num};
use std::collections::HashMap;
fn setting(config: &Config, key: &str, fallback: f64) -> f64 {
    config
        .get("agentflow")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_f64())
        .filter(|&v| v > 0.0)
        .map_or(fallback, |v| v.min(2000.0))
}
fn tint(slot: usize, wash: &str, style: &ResolvedStyle) -> String {
    let color = series_color(slot as i64);
    format!(
        " fill=\"{color}\"{} stroke=\"{color}\"",
        if sets_fill(&style.shape) {
            String::new()
        } else {
            format!(" fill-opacity=\"{wash}\"")
        }
    )
}
struct View<'a> {
    node: &'a GraphNode,
    style: ResolvedStyle,
    label: Label,
    shape: String,
    w: f64,
    h: f64,
    dy: f64,
    loops: usize,
}
fn flow_of(nodes: &[CNode], i: usize, root: Dir) -> Dir {
    let mut p = nodes[i].parent;
    let mut hops = 0;
    while p >= 0 && hops < 10000 {
        if let Some(dir) = nodes[p as usize].dir {
            return dir;
        }
        p = nodes[p as usize].parent;
        hops += 1;
    }
    root
}
fn contains(nodes: &[CNode], depth: &[i64], group: usize, i: usize) -> bool {
    let mut j = i;
    for _ in depth[group]..depth[i] {
        let p = nodes[j].parent;
        if p < 0 {
            return false;
        }
        j = p as usize;
    }
    j == group && i != group
}
pub fn render(db: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    if runs_across(db.direction.as_deref()) {
        turn_to_fit(options, |down| draw(db, config, options, measurer, down))
    } else {
        draw(db, config, options, measurer, false)
    }
}
fn draw(db: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer, turned: bool) -> Rendered {
    let mut copy = db.clone();
    let graph = graph::build(
        &mut copy,
        8,
        config.get("flowchart").and_then(|v| v.get("curve")).and_then(|v| v.as_str()),
    );
    let size = options.font_size.unwrap_or(16.0);
    let edge_size = js_round(size * 0.875);
    let wrap_width = setting(config, "wrappingWidth", 120.0);
    let min_width = setting(config, "minNodeWidth", 120.0);
    let pad = options.padding.unwrap_or(8.0);
    let index: HashMap<_, _> = graph.nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let mut views = vec![];
    let mut nodes = vec![];
    for node in &graph.nodes {
        let mut styles = node.css_compiled_styles.clone();
        styles.extend(node.css_styles.iter().cloned());
        let style = resolve_style(&styles);
        let text_style = if style.bold { BOLD } else { 0 } | if style.italic { ITALIC } else { 0 };
        let markdown = node.label_type == LabelType::Markdown;
        let font_size = style.font_size.unwrap_or(size);
        let view;
        let mut c;
        if node.is_group {
            let label = layout_label(node.label.as_deref(), markdown, measurer, font_size, 4000.0, text_style);
            c = cnode(0.0, 0.0);
            c.is_group = true;
            c.dir = node.dir.as_deref().filter(|s| !s.is_empty()).map(|s| direction(Some(s)));
            c.pad_x = 20.0;
            c.pad_top = if label.height > 0.0 { label.height + 16.0 } else { 20.0 };
            c.pad_bottom = 20.0;
            c.min_w = label.width + 28.0;
            view = View {
                node,
                style,
                label,
                shape: "group".into(),
                w: 0.0,
                h: 0.0,
                dy: 0.0,
                loops: 0,
            };
        } else if node.shape == "collapsedGroup" {
            let label = layout_label(node.label.as_deref(), markdown, measurer, font_size, wrap_width, text_style);
            let w = label.width.max(min_width) + 2.0 * PAD_X;
            let h = label.height + 2.0 * PAD_Y + 14.0;
            view = View {
                node,
                style,
                label,
                shape: "collapsed".into(),
                w,
                h,
                dy: -7.0,
                loops: 0,
            };
            c = cnode(w, h);
        } else {
            let shape = canonical_shape(Some(&node.shape));
            let label = layout_label(node.label.as_deref(), markdown, measurer, font_size, wrap_width, text_style);
            let s = shape_size(
                &shape,
                if shape == "diam" { label.width } else { label.width.max(min_width) },
                label.height,
            );
            c = cnode(s.w, s.h);
            view = View {
                node,
                style,
                label,
                shape,
                w: s.w,
                h: s.h,
                dy: s.dy,
                loops: 0,
            };
        }
        c.seq = views.len() as f64;
        views.push(view);
        nodes.push(c);
    }
    for (i, n) in graph.nodes.iter().enumerate() {
        nodes[i].parent = n.parent_id.as_deref().and_then(|p| index.get(p)).map_or(-1, |&i| i as i64);
    }
    let ends = graph
        .edges
        .iter()
        .map(|e| {
            (
                index.get(e.start.as_str()).map_or(-1, |&i| i as i64),
                index.get(e.end.as_str()).map_or(-1, |&i| i as i64),
            )
        })
        .collect::<Vec<_>>();
    inherit_crossed_directions(&mut nodes, &ends);
    let root = if turned { Dir::TB } else { direction(db.direction.as_deref()) };
    let mut depth = vec![-1; nodes.len()];
    for i in 0..nodes.len() {
        let mut chain = vec![];
        let mut j = i as i64;
        while j >= 0 && depth[j as usize] < 0 && chain.len() <= nodes.len() {
            chain.push(j as usize);
            j = nodes[j as usize].parent;
        }
        let first_depth = if j >= 0 { depth[j as usize] + 1 } else { 0 };
        for (d, &k) in chain.iter().rev().enumerate() {
            depth[k] = first_depth + d as i64;
        }
    }
    let mut edges = vec![];
    let mut edge_labels = vec![];
    let mut drawn = vec![];
    let mut held = vec![];
    for edge in &graph.edges {
        let (Some(&src), Some(&dst)) = (index.get(edge.start.as_str()), index.get(edge.end.as_str())) else {
            continue;
        };
        let label = layout_label(
            Some(&edge.label),
            edge.label_type == LabelType::Markdown,
            measurer,
            edge_size,
            wrap_width,
            0,
        );
        if contains(&nodes, &depth, src, dst) || contains(&nodes, &depth, dst, src) {
            held.push((edge, label, src, dst));
            continue;
        }
        if src == dst {
            views[src].loops += 1;
            if matches!(flow_of(&nodes, src, root), Dir::LR | Dir::RL) {
                nodes[src].h += 2.0 * (26.0 + if label.height > 0.0 { label.height + 8.0 } else { 0.0 });
            } else {
                nodes[src].w += 2.0 * (26.0 + if label.width > 0.0 { label.width + 12.0 } else { 0.0 });
            }
        }
        edges.push(CEdge {
            src,
            dst,
            minlen: edge.minlen.unwrap_or(1.0),
            label_w: if label.width > 0.0 { label.width + 12.0 } else { 0.0 },
            label_h: if label.height > 0.0 { label.height + 4.0 } else { 0.0 },
            route: vec![],
            label_x: 0.0,
            label_y: 0.0,
        });
        drawn.push(edge);
        edge_labels.push(label);
    }
    let layout = compound_layout(
        &mut nodes,
        &mut edges,
        root,
        &LayeredOptions {
            node_sep: setting(config, "nodeSpacing", 40.0),
            edge_sep: 16.0,
            rank_sep: setting(config, "rankSpacing", 48.0),
            port_sep: 20.0,
            ..Default::default()
        },
    );
    let title = layout_label(db.title.as_deref(), false, measurer, size, 4000.0, BOLD);
    let title_height = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let inner = layout.width.max(title.width);
    shift_layout(&mut nodes, &mut edges, pad + (inner - layout.width) / 2.0, pad + title_height);
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + title_height + 2.0 * pad).ceil();
    let mut links = vec![];
    let mut held_paths = vec![];
    let mut held_labels = vec![];
    let mut held_routes = vec![];
    let careful = held.len() * nodes.len() < 2000000;
    for (edge, _, src, dst) in &held {
        let out = contains(&nodes, &depth, *dst, *src);
        let outer = if out { *dst } else { *src };
        let i = if out { *src } else { *dst };
        let g = &nodes[outer];
        let n = &nodes[i];
        let end = match g.dir.unwrap_or_else(|| flow_of(&nodes, outer, root)) {
            Dir::TB => 2,
            Dir::RL => 3,
            Dir::BT => 0,
            Dir::LR => 1,
        };
        let first = if out { end } else { (end + 2) & 3 };
        let toward = |side: usize| {
            let upright = side & 1 == 0;
            let sign = if side == 0 || side == 3 { -1.0 } else { 1.0 };
            let nx = if upright { n.x } else { n.x + sign * n.w / 2.0 };
            let ny = if upright { n.y + sign * n.h / 2.0 } else { n.y };
            let bx = if upright { n.x } else { g.x + sign * g.w / 2.0 };
            let by = if upright { g.y + sign * g.h / 2.0 } else { n.y };
            let axis = if upright { 0.0 } else { 1.0 };
            if out {
                vec![nx, ny, axis, bx, by, axis]
            } else {
                vec![bx, by, axis, nx, ny, axis]
            }
        };
        let blocked = |r: &[f64]| {
            nodes.iter().enumerate().any(|(k, c)| {
                k != i
                    && !c.is_group
                    && c.x + c.w / 2.0 > r[0].min(r[3]) - 1.0
                    && c.x - c.w / 2.0 < r[0].max(r[3]) + 1.0
                    && c.y + c.h / 2.0 > r[1].min(r[4]) - 1.0
                    && c.y - c.h / 2.0 < r[1].max(r[4]) + 1.0
            })
        };
        let mut route = toward(first);
        if careful && blocked(&route) {
            for side in [(first + 1) & 3, (first + 3) & 3, (first + 2) & 3] {
                let other = toward(side);
                if !blocked(&other) {
                    route = other;
                    break;
                }
            }
        }
        held_labels.push(((route[0] + route[3]) / 2.0, (route[1] + route[4]) / 2.0));
        let mut drawn_route = route.clone();
        let v = &views[i];
        inset_route(
            &mut drawn_route,
            if out { 0 } else { 3 },
            &v.shape,
            v.w,
            v.h,
            n.x,
            n.y,
            n.w,
            n.h,
            n.is_group,
        );
        held_paths.push(route_path(
            &drawn_route,
            Some("linear"),
            0.0,
            marker_trim(&edge.arrow_type_end),
            None,
            None,
        ));
        held_routes.push(route);
    }
    let mut crossings: Crossings = HashMap::new();
    if nodes.iter().any(|n| n.is_group) {
        for e in &edges {
            mark_crossings(&mut crossings, &e.route);
        }
        for r in &held_routes {
            mark_crossings(&mut crossings, r);
        }
    }
    let (mut clusters, mut nodes_out) = (String::new(), String::new());
    let mut fills = vec![];
    for (i, view) in views.iter().enumerate() {
        let c = &nodes[i];
        let node = view.node;
        let (x, y) = (c.x, c.y);
        let stripped = node
            .css_classes
            .strip_prefix("default ")
            .or_else(|| node.css_classes.strip_prefix("default"))
            .unwrap_or(&node.css_classes);
        let classes = class_names(stripped);
        let id = esc_text(&node.id);
        if c.is_group {
            if paints(&view.style.shape) {
                fills.push(Fill {
                    bx: [x - c.w / 2.0, y - c.h / 2.0, x + c.w / 2.0, y + c.h / 2.0],
                    shape: view.style.shape.clone(),
                });
            }
            let tx = cluster_title_x(&Box4 { x, y, w: c.w, h: c.h }, view.label.width, &crossings, 20.0);
            clusters += &format!(
                "<g class=\"pele-cluster pele-flow{classes}\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>{}</g>",
                num(x - c.w / 2.0),
                num(y - c.h / 2.0),
                num(c.w),
                num(c.h),
                rounded(&(tint(node.color_index.unwrap_or(7), "0.06", &view.style) + &view.style.shape)),
                label_svg(
                    &view.label,
                    tx,
                    y - c.h / 2.0 + 8.0 + view.label.height / 2.0,
                    &format!(
                        " class=\"pele-cluster-label\"{}{}",
                        ink_fill(&view.style.shape, &view.style.text, Some("var(--_m)")),
                        view.style.text
                    )
                )
            );
            continue;
        }
        let (w, h) = (view.w, view.h);
        let mut kind = String::new();
        let shape = if view.shape == "collapsed" {
            let color = series_color(node.color_index.unwrap_or(7) as i64);
            let line = h / 2.0 - 16.0;
            let dots = line + 8.0;
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/><path d=\"M{},{}H{}\" fill=\"none\" stroke=\"{color}\" stroke-dasharray=\"3 3\"{}/><path class=\"pele-collapsed-mark\" d=\"{}{}{}\" fill=\"{color}\" stroke=\"none\"/>",
                num(-w / 2.0),
                num(-h / 2.0),
                num(w),
                num(h),
                rounded(&(tint(node.color_index.unwrap_or(7), "0.06", &view.style) + &view.style.shape)),
                num(-w / 2.0 + 10.0),
                num(line),
                num(w / 2.0 - 10.0),
                view.style.line,
                dot(-9.0, dots),
                dot(0.0, dots),
                dot(9.0, dots)
            )
        } else {
            let node_kind = node.kind.as_deref().unwrap_or("task");
            let slot = shapes::kind_slot(node_kind).unwrap_or(1);
            kind = format!(" pele-kind-{node_kind}");
            draw_shape(
                &view.shape,
                w,
                h,
                &(tint(slot, "0.14", &view.style) + &view.style.shape),
                &(format!(" stroke=\"{}\"", series_color(slot as i64)) + &view.style.line),
            )
        };
        let ink = if view.shape == "collapsed" || shape_filled(&view.shape) {
            ink_fill(&view.style.shape, &view.style.text, None)
        } else {
            String::new()
        };
        let mut text = label_svg(&view.label, 0.0, view.dy, &format!(" class=\"pele-label\"{ink}{}", view.style.text));
        let internal = format!("{classes} ").contains(" internal-link ");
        if internal && node.label.as_deref().is_some_and(|s| !s.is_empty()) && options.links != Some(false) {
            let name = decode_entities(node.label.as_deref().unwrap());
            let safe = internal_link(&name, options);
            let target = esc(&name);
            text = format!(
                "<a class=\"internal-link\"{} data-href=\"{target}\">{text}</a>",
                if safe {
                    format!(" href=\"{target}\"{}", rel_attr(options))
                } else {
                    String::new()
                }
            );
            links.push(LinkInfo {
                id: node.id.clone(),
                href: name,
                internal: true,
            });
        }
        let mut body = shape + &text;
        if let Some(tooltip) = node.tooltip.as_deref().filter(|s| !s.is_empty()) {
            body = format!("<title>{}</title>{body}", esc_text(tooltip));
        }
        if let Some(link) = node.link.as_deref().filter(|s| !s.is_empty()) {
            body = linked(
                &body,
                link,
                &node.id,
                &mut links,
                options,
                &node
                    .link_target
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!(" target=\"{}\"", esc(s)))
                    .unwrap_or_default(),
            );
        }
        nodes_out += &format!(
            "<g class=\"pele-node pele-shape-{}{kind}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">{body}</g>",
            view.shape,
            num(x),
            num(y)
        );
    }
    let (mut casings, mut edges_out, mut labels_out) = (String::new(), String::new(), String::new());
    let mut put = |edge: &GraphEdge, label: &Label, path: &EdgePath, x: f64, y: f64| {
        let style = if !edge.style.is_empty() || !edge.css_compiled_styles.is_empty() {
            let mut styles = edge.css_compiled_styles.clone();
            styles.extend(edge.style.iter().cloned());
            Some(resolve_style(&styles))
        } else {
            None
        };
        let end = &edge.arrow_type_end;
        let line = style.as_ref().map(|s| s.line.as_str()).unwrap_or("");
        let over = over_fills(&path.d, &[path.ex, path.ey], &fills, line, path.tokens.as_deref());
        let color = over.as_ref().filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| {
            style
                .as_ref()
                .and_then(|s| s.stroke.as_deref())
                .filter(|s| !s.is_empty())
                .map(esc)
                .unwrap_or("var(--_l)".into())
        });
        let kind = if edge.thickness.as_deref() == Some("dotted") {
            "reference"
        } else if end == "arrow_cross" {
            "failure"
        } else {
            "sequence"
        };
        let classes = class_names(
            &edge
                .classes
                .replace("edge-thickness-normal", "")
                .replace("edge-pattern-solid", "")
                .replace("flowchart-link", ""),
        );
        if over.as_deref() == Some("") {
            casings += &edge_casing(path, "none", end, line, false);
        }
        edges_out += &format!(
            "<g class=\"pele-edge pele-edge-{kind}{classes}\" data-id=\"{}\"{}{line}><path d=\"{}\"{}/>{}</g>",
            esc_text(&edge.id),
            over.as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| format!(" stroke=\"{s}\""))
                .unwrap_or_default(),
            path.d,
            if edge.thickness.as_deref() == Some("dotted") {
                " stroke-dasharray=\"3 4\""
            } else {
                ""
            },
            marker(end, path.ex, path.ey, path.edx, path.edy, &color, "none")
        );
        if label.width > 0.0 {
            labels_out += &edge_label_svg(&edge.id, label, x, y, style.as_ref().map(|s| s.text.as_str()).unwrap_or(""));
        }
    };
    let mut loop_seen = HashMap::new();
    for (i, ce) in edges.iter_mut().enumerate() {
        let edge = drawn[i];
        let path;
        if ce.src == ce.dst {
            let view = &views[ce.src];
            let k = loop_seen.entry(ce.src).or_insert(0usize);
            let node = &nodes[ce.src];
            let mut label = LoopLabel {
                label_w: ce.label_w,
                label_h: ce.label_h,
                label_x: ce.label_x,
                label_y: ce.label_y,
            };
            path = loop_path(
                node.x,
                node.y,
                view.w,
                view.h,
                *k as f64,
                matches!(flow_of(&nodes, ce.src, root), Dir::LR | Dir::RL),
                26.0,
                marker_trim(&edge.arrow_type_end),
                &mut label,
            );
            *k += 1;
            ce.label_x = label.label_x;
            ce.label_y = label.label_y;
        } else {
            let mut route = ce.route.clone();
            let s = &nodes[ce.src];
            let d = &nodes[ce.dst];
            let vs = &views[ce.src];
            let vd = &views[ce.dst];
            let last = route.len() - 3;
            inset_route(&mut route, 0, &vs.shape, vs.w, vs.h, s.x, s.y, s.w, s.h, s.is_group);
            inset_route(&mut route, last, &vd.shape, vd.w, vd.h, d.x, d.y, d.w, d.h, d.is_group);
            path = route_path(&route, edge.curve.as_deref(), 0.0, marker_trim(&edge.arrow_type_end), None, None);
        }
        put(edge, &edge_labels[i], &path, ce.label_x, ce.label_y);
    }
    for (i, (edge, label, _, _)) in held.iter().enumerate() {
        put(edge, label, &held_paths[i], held_labels[i].0, held_labels[i].1);
    }
    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !clusters.is_empty() {
        content += &format!("<g class=\"pele-clusters\">{clusters}</g>");
    }
    content += &casings_svg(&casings);
    if !edges_out.is_empty() {
        content += &format!("<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>");
    }
    if !labels_out.is_empty() {
        content += &format!("<g class=\"pele-edge-labels\" font-size=\"{}\">{labels_out}</g>", num(edge_size));
    }
    content += &format!("<g class=\"pele-nodes\">{nodes_out}</g>");
    Rendered {
        svg: svg_document(
            "agentflow",
            width,
            height,
            size,
            options,
            &Accessible {
                acc_title: db.acc_title.as_deref(),
                acc_descr: db.acc_descr.as_deref(),
            },
            &content,
        ),
        width,
        height,
        links,
    }
}
fn dot(x: f64, y: f64) -> String {
    format!("M{},{}a1.75,1.75 0 1 0 3.5,0a1.75,1.75 0 1 0 -3.5,0Z", num(x - 1.75), num(y))
}
