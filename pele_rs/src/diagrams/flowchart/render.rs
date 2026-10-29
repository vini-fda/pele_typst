// Draws a flowchart, after pele's diagrams/flowchart/render.ts.

use std::collections::HashMap;

use serde_json::Value;

use super::db::{FlowDb, LabelType};
use super::graph::{FlowGraph, GraphNode, build_flow_graph};
use super::shapes::canonical_shape;
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, runs_across, tighten, title_room, turn_to_fit};
use crate::layout::compound::{
    CEdge, CNode, CompoundResult, Dir, cnode, compound_layout, direction, inherit_crossed_directions, shift_layout,
};
use crate::layout::layered::LayeredOptions;
use crate::preprocess::Config;
use crate::svg::builder::{esc, esc_text, label_svg};
use crate::svg::cluster::{Box4, Crossings, cluster_title_x, mark_crossings, struck_title, title_crossed};
use crate::svg::edges::{
    EdgePath, Fill, LoopLabel, SideReach, casings_svg, edge_casing, edge_label_svg, loop_path, marker, marker_trim, over_fills, route_path,
};
use crate::svg::ink::{ink_fill, paints};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::shapes::{draw_shape, end_side, inset_route, shape_filled, shape_has_label, shape_size, shape_span};
use crate::svg::theme::{ResolvedStyle, class_names, resolve_style, rounded, sets_fill};
use crate::text::entities::decode_entities;
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, ITALIC, TextMeasurer};
use crate::url::{LinkInfo, image_url, internal_link, linked, rel_attr, safe_url};
use crate::util::{js_round, num};

const NODE_SEP: f64 = 40.0;
const EDGE_SEP: f64 = 16.0;
const RANK_SEP: f64 = 48.0;
const GROUP_PAD: f64 = 20.0;
const LOOP: f64 = 26.0;
const SHAPE_ATTRS: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
const LINE_ATTRS: &str = " stroke=\"var(--_b)\"";

fn number_option(section: &Config, key: &str, fallback: f64) -> f64 {
    match section.get(key).and_then(|v| v.as_f64()) {
        Some(v) if v > 0.0 => v.min(2000.0),
        _ => fallback,
    }
}

fn asset_size(value: Option<f64>, fallback: f64) -> f64 {
    match value {
        Some(v) if v > 0.0 && v <= 4000.0 => v,
        _ => fallback,
    }
}

fn present(s: &Option<String>) -> bool {
    s.as_deref().is_some_and(|v| !v.is_empty())
}

struct NodeView<'a> {
    node: &'a GraphNode,
    style: ResolvedStyle,
    label: Label,
    shape: String,
    w: f64,
    h: f64,
    dy: f64,
    loops: usize,
}

pub trait FlowVariant {
    fn kind(&self) -> &'static str;
    fn prepare(&self, graph: &mut FlowGraph);
    fn layout(&self, nodes: &mut [CNode], edges: &mut [CEdge], dir: Dir, opt: &LayeredOptions, tight: bool) -> CompoundResult;
    fn draw_group(&self, c: &CNode, label: &Label, style: &ResolvedStyle, classes: &str, id: &str, dir: Dir) -> String;
    fn filled_box(&self, c: &CNode, dir: Dir) -> [f64; 4];
}

pub fn render_flowchart(db: &FlowDb, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    render_flowchart_variant(db, config, options, measurer, None)
}

pub fn render_flowchart_variant(
    db: &FlowDb,
    config: &Config,
    options: &RenderOptions,
    measurer: &dyn TextMeasurer,
    variant: Option<&dyn FlowVariant>,
) -> Rendered {
    let fitted = |down: bool| tighten(options, |tight| draw(db, config, options, measurer, variant, down, tight));
    if runs_across(db.direction.as_deref()) {
        turn_to_fit(options, fitted)
    } else {
        fitted(false)
    }
}

fn draw(
    db: &FlowDb,
    config: &Config,
    options: &RenderOptions,
    measurer: &dyn TextMeasurer,
    variant: Option<&dyn FlowVariant>,
    turned: bool,
    tight: bool,
) -> Rendered {
    let group_pad = if tight { 12.0 } else { GROUP_PAD };
    let empty = Config::new();
    let own = variant.and_then(|v| config.get(v.kind())).filter(|v| v.is_object());
    let flow = match own.or_else(|| config.get("flowchart")) {
        Some(Value::Object(o)) => o,
        _ => &empty,
    };
    let curve = flow.get("curve").and_then(|v| v.as_str());
    let mut graph = build_flow_graph(db, curve);
    if let Some(v) = variant {
        v.prepare(&mut graph);
    }
    let size = options.font_size.unwrap_or(16.0);
    let edge_size = js_round(size * 0.875);
    let wrap_width = number_option(flow, "wrappingWidth", 200.0).min(if variant.is_some() && tight { 100.0 } else { f64::INFINITY });
    let pad = options.padding.unwrap_or(8.0);

    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, node) in graph.nodes.iter().enumerate() {
        index.insert(node.id.as_str(), i);
    }
    let mut views: Vec<NodeView> = Vec::new();
    let mut cnodes: Vec<CNode> = Vec::new();

    for node in &graph.nodes {
        let decls = if !node.css_styles.is_empty() {
            let mut d = node.css_compiled_styles.clone();
            d.extend(node.css_styles.iter().cloned());
            d
        } else {
            node.css_compiled_styles.clone()
        };
        let style = resolve_style(&decls);
        let text_style = (if style.bold { BOLD } else { 0 }) | (if style.italic { ITALIC } else { 0 });
        let markdown = node.label_type == LabelType::Markdown;
        let font_size = style.font_size.unwrap_or(size);
        let view;
        let mut c;
        if node.is_group {
            let label = layout_label(
                node.label.as_deref(),
                markdown,
                measurer,
                font_size,
                if variant.is_some() && tight {
                    (title_room(options) / 2.0).max(80.0)
                } else {
                    4000.0
                },
                text_style,
            );
            c = cnode(0.0, 0.0);
            c.is_group = true;
            c.dir = node.dir.as_deref().filter(|d| !d.is_empty()).map(|d| direction(Some(d)));
            c.pad_x = group_pad;
            c.pad_top = if label.height > 0.0 { label.height + 16.0 } else { GROUP_PAD };
            c.pad_bottom = group_pad;
            c.min_w = label.width + 2.0 * group_pad - 12.0;
            view = NodeView {
                node,
                style,
                label,
                shape: "group".into(),
                w: 0.0,
                h: 0.0,
                dy: 0.0,
                loops: 0,
            };
        } else {
            let shape = canonical_shape(Some(&node.shape));
            let with_asset = present(&node.img) || present(&node.icon);
            let text = if with_asset || shape_has_label(&shape) {
                node.label.as_deref()
            } else {
                None
            };
            let label = layout_label(text, markdown, measurer, font_size, wrap_width, text_style);
            let (w, h, mut dy);
            if with_asset {
                let bx = if present(&node.img) { 80.0 } else { 48.0 };
                let aw = asset_size(node.asset_width, bx);
                let ah = asset_size(node.asset_height, bx);
                w = aw.max(label.width + 8.0);
                h = ah + if label.height > 0.0 { label.height + 6.0 } else { 0.0 };
                dy = if node.pos.as_deref() == Some("t") { -ah / 2.0 } else { ah / 2.0 };
                if label.height == 0.0 {
                    dy = 0.0;
                }
            } else {
                let s = shape_size(&shape, label.width, label.height);
                w = s.w;
                h = s.h;
                dy = s.dy;
            }
            c = cnode(w, h);
            view = NodeView {
                node,
                style,
                label,
                shape,
                w,
                h,
                dy,
                loops: 0,
            };
        }
        c.seq = views.len() as f64;
        views.push(view);
        cnodes.push(c);
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        cnodes[i].parent = match &node.parent_id {
            Some(p) => index.get(p.as_str()).map(|&x| x as i64).unwrap_or(-1),
            None => -1,
        };
    }
    let ends: Vec<(i64, i64)> = graph
        .edges
        .iter()
        .map(|e| {
            (
                index.get(e.start.as_str()).map(|&x| x as i64).unwrap_or(-1),
                index.get(e.end.as_str()).map(|&x| x as i64).unwrap_or(-1),
            )
        })
        .collect();
    inherit_crossed_directions(&mut cnodes, &ends);

    let root_dir = if turned { Dir::TB } else { direction(db.direction.as_deref()) };
    let flows_sideways = |cnodes: &[CNode], i: usize| -> bool {
        let mut dir: Option<Dir> = None;
        let mut p = cnodes[i].parent;
        let mut hops = 0;
        while p >= 0 && dir.is_none() && hops < 10000 {
            dir = cnodes[p as usize].dir;
            p = cnodes[p as usize].parent;
            hops += 1;
        }
        let dir = dir.unwrap_or(root_dir);
        dir == Dir::LR || dir == Dir::RL
    };
    // The edges on one side of a node spread along it; a picture or an icon keeps them at its middle.
    for i in 0..views.len() {
        if views[i].node.is_group {
            cnodes[i].span = f64::INFINITY;
        } else if !present(&views[i].node.img) && !present(&views[i].node.icon) {
            let sideways = flows_sideways(&cnodes, i);
            cnodes[i].span = shape_span(&views[i].shape, views[i].w, views[i].h, sideways);
        }
    }

    let mut cedges: Vec<CEdge> = Vec::new();
    let mut edge_labels: Vec<Label> = Vec::new();
    let mut drawn = Vec::new();
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
        if src == dst {
            // A self-loop sits beside the node, across the flow, and the node's box grows to hold it.
            views[src].loops += 1;
            if flows_sideways(&cnodes, src) {
                cnodes[src].h += 2.0 * (LOOP + if label.height > 0.0 { label.height + 8.0 } else { 0.0 });
            } else {
                cnodes[src].w += 2.0 * (LOOP + if label.width > 0.0 { label.width + 12.0 } else { 0.0 });
            }
        }
        cedges.push(CEdge {
            src,
            dst,
            minlen: edge.minlen.unwrap_or(1.0),
            label_w: if label.width > 0.0 { label.width + 12.0 } else { 0.0 },
            label_h: if label.height > 0.0 { label.height + 4.0 } else { 0.0 },
            route: Vec::new(),
            label_x: 0.0,
            label_y: 0.0,
        });
        drawn.push(edge);
        edge_labels.push(label);
    }

    let layout_options = LayeredOptions {
        node_sep: number_option(flow, "nodeSpacing", NODE_SEP) * if tight { 0.5 } else { 1.0 },
        edge_sep: EDGE_SEP,
        rank_sep: number_option(flow, "rankSpacing", RANK_SEP) * if tight { 0.75 } else { 1.0 },
        port_sep: 20.0,
        tracks: curve.is_some_and(|c| c.starts_with("step")),
        straight: curve == Some("linear"),
        apart: false,
        head_room: marker_trim("arrow_point"),
        groups: None,
        order_only: false,
    };
    let layout = if let Some(v) = variant {
        v.layout(&mut cnodes, &mut cedges, root_dir, &layout_options, tight)
    } else {
        compound_layout(&mut cnodes, &mut cedges, root_dir, &layout_options)
    };

    let title = crate::svg::title::title_label(
        db.title.as_deref(),
        measurer,
        size,
        if tight { title_room(options) } else { 4000.0 },
    );
    let title_height = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let inner = layout.width.max(title.width);
    let ox = pad + (inner - layout.width) / 2.0;
    let oy = pad + title_height;
    shift_layout(&mut cnodes, &mut cedges, ox, oy);
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + title_height + 2.0 * pad).ceil();
    let mut links: Vec<LinkInfo> = Vec::new();

    let mut crossings: Crossings = HashMap::new();
    if cnodes.iter().any(|c| c.is_group) {
        for e in &cedges {
            mark_crossings(&mut crossings, &e.route);
        }
    }

    let mut clusters = String::new();
    let mut struck_titles = String::new();
    let mut nodes_out = String::new();
    let mut fills: Vec<Fill> = Vec::new();
    for i in 0..views.len() {
        let view = &views[i];
        let c = &cnodes[i];
        let node = view.node;
        let (x, y) = (c.x, c.y);
        let stripped = node
            .css_classes
            .strip_prefix("default ")
            .or_else(|| node.css_classes.strip_prefix("default"))
            .unwrap_or(&node.css_classes);
        let classes = class_names(stripped);
        let id = esc_text(&node.id);
        if c.is_group
            && c.parent < 0
            && let Some(v) = variant
        {
            if paints(&view.style.shape) {
                fills.push(Fill {
                    bx: v.filled_box(c, root_dir),
                    shape: view.style.shape.clone(),
                });
            }
            clusters += &v.draw_group(c, &view.label, &view.style, &classes, &id, root_dir);
            continue;
        }
        if c.is_group {
            if paints(&view.style.shape) {
                fills.push(Fill {
                    bx: [x - c.w / 2.0, y - c.h / 2.0, x + c.w / 2.0, y + c.h / 2.0],
                    shape: view.style.shape.clone(),
                });
            }
            let b = Box4 { x, y, w: c.w, h: c.h };
            let title_x = cluster_title_x(&b, view.label.width, &crossings, group_pad);
            let title_y = y - c.h / 2.0 + 8.0 + view.label.height / 2.0;
            let struck = title_crossed(&b, view.label.width, title_x, &crossings);
            // A title drawn over the edges sits on a halo in the theme's colors.
            let title_attrs = format!(
                " class=\"pele-cluster-label\"{}{}",
                if struck {
                    " fill=\"var(--_m)\"".to_string()
                } else {
                    ink_fill(&view.style.shape, &view.style.text, Some("var(--_m)"))
                },
                view.style.text
            );
            if struck {
                let mut depth = 1;
                let mut p = cnodes[i].parent;
                while p >= 0 && depth < 8 {
                    depth += 1;
                    p = cnodes[p as usize].parent;
                }
                struck_titles += &struck_title(&view.label, title_x, title_y, &title_attrs, &id, depth);
            }
            clusters += &format!(
                "<g class=\"pele-cluster{}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\"{} stroke=\"var(--_b)\"{}/>{}</g>",
                classes,
                id,
                num(x - c.w / 2.0),
                num(y - c.h / 2.0),
                num(c.w),
                num(c.h),
                if sets_fill(&view.style.shape) {
                    ""
                } else {
                    " fill-opacity=\"0.5\""
                },
                rounded(&view.style.shape),
                if struck {
                    String::new()
                } else {
                    label_svg(&view.label, title_x, title_y, &title_attrs)
                }
            );
            continue;
        }

        let (w, h) = (view.w, view.h);
        let inner;
        let mut ink = String::new();
        if present(&node.img) {
            let aw = asset_size(node.asset_width, 80.0);
            let ah = asset_size(node.asset_height, 80.0);
            let top = if view.label.height > 0.0 {
                if node.pos.as_deref() == Some("t") { h / 2.0 - ah } else { -h / 2.0 }
            } else {
                -ah / 2.0
            };
            inner = match image_url(&safe_url(node.img.as_deref().unwrap()), options) {
                None => String::new(),
                Some(src) => format!(
                    "<image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"{}\"/>",
                    esc(&src),
                    num(-aw / 2.0),
                    num(top),
                    num(aw),
                    num(ah),
                    if node.constraint.as_deref() == Some("on") {
                        "xMidYMid meet"
                    } else {
                        "none"
                    }
                ),
            };
        } else if present(&node.icon) {
            let bx = asset_size(node.asset_height, 48.0);
            let top = if view.label.height > 0.0 {
                if node.pos.as_deref() == Some("t") { h / 2.0 - bx } else { -h / 2.0 }
            } else {
                -bx / 2.0
            };
            let frame = if node.shape == "icon" {
                String::new()
            } else if node.shape == "iconCircle" {
                format!(
                    "<circle cy=\"{}\" r=\"{}\"{}{}/>",
                    num(top + bx / 2.0),
                    num(bx / 2.0),
                    SHAPE_ATTRS,
                    view.style.shape
                )
            } else {
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{}{}/>",
                    num(-bx / 2.0),
                    num(top),
                    num(bx),
                    num(bx),
                    if node.shape == "iconRounded" { 8 } else { 0 },
                    SHAPE_ATTRS,
                    view.style.shape
                )
            };
            let glyph = bx * 0.6;
            inner = frame
                + &format!(
                    "<svg class=\"pele-icon\" data-icon=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{}</svg>",
                    esc_text(node.icon.as_deref().unwrap()),
                    num(-glyph / 2.0),
                    num(top + (bx - glyph) / 2.0),
                    num(glyph),
                    num(glyph),
                    options
                        .icons
                        .as_ref()
                        .and_then(|r| r.resolve(node.icon.as_deref().unwrap()))
                        .unwrap_or_default()
                );
        } else {
            inner = draw_shape(
                &view.shape,
                w,
                h,
                &format!("{}{}", SHAPE_ATTRS, view.style.shape),
                &format!("{}{}", LINE_ATTRS, view.style.line),
            );
            if shape_filled(&view.shape) {
                ink = ink_fill(&view.style.shape, &view.style.text, None);
            }
        }

        let mut text = label_svg(
            &view.label,
            0.0,
            view.dy,
            &format!(" class=\"pele-label\"{}{}", ink, view.style.text),
        );
        let internal = format!("{} ", classes).contains(" internal-link ");
        if internal && present(&node.label) && options.links != Some(false) {
            // The label is a note name, but it still ends up in an href, so it gets the same check as a URL.
            let name = decode_entities(node.label.as_deref().unwrap());
            let safe = internal_link(&name, options);
            let target = esc(&name);
            text = format!(
                "<a class=\"internal-link\"{} data-href=\"{}\">{}</a>",
                if safe {
                    format!(" href=\"{}\"{}", target, rel_attr(options))
                } else {
                    String::new()
                },
                target,
                text
            );
            links.push(LinkInfo {
                id: node.id.clone(),
                href: name,
                internal: true,
            });
        }
        let mut body = inner + &text;
        if let Some(tooltip) = node.tooltip.as_deref().filter(|t| !t.is_empty()) {
            body = format!("<title>{}</title>{}", esc_text(tooltip), body);
        }
        if let Some(link) = node.link.as_deref().filter(|l| !l.is_empty()) {
            let target = node
                .link_target
                .as_deref()
                .filter(|t| !t.is_empty())
                .map(|t| format!(" target=\"{}\"", esc(t)))
                .unwrap_or_default();
            body = linked(&body, link, &node.id, &mut links, options, &target);
        }
        nodes_out += &format!(
            "<g class=\"pele-node pele-shape-{}{}\" data-id=\"{}\" transform=\"translate({},{})\">{}</g>",
            view.shape,
            classes,
            id,
            num(x),
            num(y),
            body
        );
    }

    let mut reach = SideReach::default();
    for (i, ce) in cedges.iter().enumerate() {
        if ce.src == ce.dst || drawn[i].thickness.as_deref() == Some("invisible") || ce.route.len() < 6 {
            continue;
        }
        let s = &cnodes[ce.src];
        let d = &cnodes[ce.dst];
        reach.add(ce.src, end_side(&ce.route, 0, s.x, s.y, s.w, s.h), &drawn[i].arrow_type_start);
        reach.add(
            ce.dst,
            end_side(&ce.route, ce.route.len() - 3, d.x, d.y, d.w, d.h),
            &drawn[i].arrow_type_end,
        );
    }

    let mut casings = String::new();
    let mut edges_out = String::new();
    let mut labels_out = String::new();
    let mut loop_seen: HashMap<usize, usize> = HashMap::new();
    for i in 0..cedges.len() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let style = if !edge.style.is_empty() || !edge.css_compiled_styles.is_empty() {
            let mut d = edge.css_compiled_styles.clone();
            d.extend(edge.style.iter().cloned());
            Some(resolve_style(&d))
        } else {
            None
        };
        let line = style.as_ref().map(|s| s.line.as_str()).unwrap_or("");
        if edge.thickness.as_deref() != Some("invisible") {
            let start_type = edge.arrow_type_start.as_str();
            let end_type = edge.arrow_type_end.as_str();
            let path: EdgePath;
            let ce = &mut cedges[i];
            if ce.src == ce.dst {
                let view = &views[ce.src];
                let k = *loop_seen.get(&ce.src).unwrap_or(&0);
                loop_seen.insert(ce.src, k + 1);
                let node = &cnodes[ce.src];
                let mut l = LoopLabel {
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
                    k as f64,
                    flows_sideways(&cnodes, ce.src),
                    LOOP,
                    marker_trim(end_type),
                    &mut l,
                );
                ce.label_x = l.label_x;
                ce.label_y = l.label_y;
            } else {
                let mut route = ce.route.clone();
                let s = &cnodes[ce.src];
                let d = &cnodes[ce.dst];
                let last = route.len() - 3;
                let leaves = reach.of(ce.src, end_side(&route, 0, s.x, s.y, s.w, s.h));
                let arrives = reach.of(ce.dst, end_side(&route, last, d.x, d.y, d.w, d.h));
                let (vs, vd) = (&views[ce.src], &views[ce.dst]);
                inset_route(&mut route, 0, &vs.shape, vs.w, vs.h, s.x, s.y, s.w, s.h, s.is_group);
                inset_route(&mut route, last, &vd.shape, vd.w, vd.h, d.x, d.y, d.w, d.h, d.is_group);
                path = route_path(
                    &route,
                    edge.curve.as_deref(),
                    marker_trim(start_type),
                    marker_trim(end_type),
                    Some(leaves),
                    Some(arrives),
                );
            }
            let over = over_fills(&path.d, &[path.sx, path.sy, path.ex, path.ey], &fills, line, path.tokens.as_deref());
            let stroke = style.as_ref().and_then(|s| s.stroke.clone()).filter(|s| !s.is_empty());
            let color = match &over {
                Some(o) if !o.is_empty() => o.clone(),
                _ => stroke.map(|s| esc(&s)).unwrap_or_else(|| "var(--_l)".into()),
            };
            let mut attrs = "";
            if edge.thickness.as_deref() == Some("thick") {
                attrs = " stroke-width=\"2.5\"";
            } else if edge.thickness.as_deref() == Some("dotted") {
                attrs = " stroke-dasharray=\"3 4\"";
            }
            let classes = class_names(
                &edge
                    .classes
                    .replace("edge-thickness-normal", "")
                    .replace("edge-pattern-solid", "")
                    .replace("flowchart-link", ""),
            );
            if over.as_deref() == Some("") {
                casings += &edge_casing(&path, start_type, end_type, line, edge.thickness.as_deref() == Some("thick"));
            }
            edges_out += &format!(
                "<g class=\"pele-edge{}\" data-id=\"{}\"{}{}><path d=\"{}\"{}/>{}{}</g>",
                classes,
                esc_text(&edge.id),
                match &over {
                    Some(o) if !o.is_empty() => format!(" stroke=\"{}\"", o),
                    _ => String::new(),
                },
                line,
                path.d,
                attrs,
                marker(start_type, path.sx, path.sy, path.sdx, path.sdy, &color, "none"),
                marker(end_type, path.ex, path.ey, path.edx, path.edy, &color, "none")
            );
        }
        if label.width > 0.0 {
            let ce = &cedges[i];
            labels_out += &edge_label_svg(
                &edge.id,
                label,
                ce.label_x,
                ce.label_y,
                style.as_ref().map(|s| s.text.as_str()).unwrap_or(""),
            );
        }
    }

    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !clusters.is_empty() {
        content += &format!("<g class=\"pele-clusters\">{}</g>", clusters);
    }
    content += &casings_svg(&casings);
    if !edges_out.is_empty() {
        content += &format!(
            "<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{}</g>",
            edges_out
        );
    }
    if !struck_titles.is_empty() {
        content += &format!("<g class=\"pele-cluster-titles\">{}</g>", struck_titles);
    }
    if !labels_out.is_empty() {
        content += &format!(
            "<g class=\"pele-edge-labels\" font-size=\"{}\">{}</g>",
            crate::util::js_num(edge_size),
            labels_out
        );
    }
    content += &format!("<g class=\"pele-nodes\">{}</g>", nodes_out);
    let acc = Accessible {
        acc_title: db.acc_title.as_deref(),
        acc_descr: db.acc_descr.as_deref(),
    };
    let svg = svg_document(
        variant.map_or("flowchart", |v| v.kind()),
        width,
        height,
        size,
        options,
        &acc,
        &content,
    );
    Rendered { svg, width, height, links }
}
