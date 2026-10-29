use super::{
    graph::{GraphNode, graph},
    markers,
    model::{Member, Model, Node},
};
use crate::{
    Config, MonospacePart, RenderOptions, TextMeasurer,
    diagrams::common::{Rendered, runs_across, tighten, turn_to_fit},
    layout::{
        components::wrap_components,
        compound::{CEdge, Dir, cnode, compound_layout, direction},
        layered::LayeredOptions,
    },
    svg::{
        builder::{esc, esc_text, label_svg},
        cluster::{Box4, Crossings, cluster_title_x, mark_crossings, struck_title, title_crossed},
        edges::{EdgePath, edge_label_svg, route_path},
        ink::ink_fill,
        root::{Accessible, svg_document},
        shapes::end_side,
        theme::{FONT_MONO, ResolvedStyle, class_names, resolve_style, rounded},
        title::with_title,
    },
    text::{
        entities::decode_entities,
        label::{Label, has_math, layout_label},
        measurer::{BOLD, ITALIC, MONO},
    },
    url::{LinkInfo, linked},
    util::{js_num, js_round, num},
};
use indexmap::IndexMap;
use std::collections::HashMap;
struct Line {
    text: String,
    width: f64,
    classifier: String,
}
enum Annotation {
    Text(String),
    Math(Label),
}
struct View<'a> {
    node: &'a GraphNode<'a>,
    style: ResolvedStyle,
    label: Label,
    w: f64,
    h: f64,
    loops: usize,
    annotations: Vec<Annotation>,
    attributes: Vec<Line>,
    methods: Vec<Line>,
    title_h: f64,
    attributes_h: f64,
    font_size: f64,
}
#[derive(Clone, Copy)]
struct End {
    edge: usize,
    at: usize,
    toward: f64,
    order: usize,
    room: f64,
    side: usize,
    after: bool,
}
struct Card {
    text: String,
    math: Option<Label>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    parent: usize,
}
fn number(conf: &Config, key: &str, fallback: f64) -> f64 {
    conf.get(key)
        .and_then(|v| v.as_f64())
        .filter(|n| *n > 0.)
        .map_or(fallback, |n| n.min(2000.))
}
fn mono(style: &ResolvedStyle, options: &RenderOptions) -> bool {
    options.monospace.as_ref().is_some_and(|m| m.contains(&MonospacePart::Members))
        && !style.text.contains("\"font-family:")
        && !style.text.contains(";font-family:")
}
fn member_line(member: &Member, m: &dyn TextMeasurer, size: f64, mono: bool) -> Line {
    let text = decode_entities(&member.display()).trim().to_owned();
    let style = if member.classifier == "*" { ITALIC } else { 0 } | if mono { MONO } else { 0 };
    Line {
        width: m.width(&text, size, style),
        text,
        classifier: member.classifier.clone(),
    }
}
fn math_text(raw: &str, size: f64, m: &dyn TextMeasurer) -> Option<Label> {
    if !raw.contains("$$") {
        return None;
    }
    let l = layout_label(Some(raw), false, m, size, 4000., 0);
    if l.lines.iter().any(|line| has_math(line)) { Some(l) } else { None }
}
fn class_style(cls: &Node, model: &Model) -> ResolvedStyle {
    let mut kept = IndexMap::new();
    let mut keep = |decl: &str| {
        let one = resolve_style(&[decl.to_owned()]);
        if !one.shape.is_empty() || !one.text.is_empty() {
            let prop = decl.split_once(':').map_or("", |p| p.0).trim().to_lowercase();
            kept.insert(prop, decl.to_owned());
        }
    };
    for name in cls.css_classes.split(' ') {
        if let Some(def) = model.style_classes.get(name) {
            for s in &def.styles {
                for part in s.split(',') {
                    keep(part);
                }
            }
        }
    }
    for s in &cls.styles {
        keep(s);
    }
    resolve_style(&kept.into_values().collect::<Vec<_>>())
}
fn base<'a>(node: &'a GraphNode<'a>, label: Label, w: f64, h: f64, size: f64) -> View<'a> {
    View {
        node,
        style: resolve_style(&[]),
        label,
        w,
        h,
        loops: 0,
        annotations: vec![],
        attributes: vec![],
        methods: vec![],
        title_h: 0.,
        attributes_h: 0.,
        font_size: size,
    }
}
fn class_view<'a>(
    node: &'a GraphNode<'a>,
    cls: &Node,
    model: &Model,
    options: &RenderOptions,
    m: &dyn TextMeasurer,
    size: f64,
    tiny: f64,
    tiny_height: f64,
    hide_empty: bool,
) -> View<'a> {
    let style = class_style(cls, model);
    let font_size = style.font_size.unwrap_or(size);
    let member_size = js_round(font_size * 0.875);
    let line_height = js_round(member_size * 1.5);
    let generic = if cls.type_.is_empty() {
        String::new()
    } else {
        format!(
            "&lt;{}&gt;",
            super::super::er::render::generic(&cls.type_)
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        )
    };
    let label = layout_label(
        Some(&format!("{}{generic}", cls.label)),
        false,
        m,
        font_size,
        4000.,
        BOLD | if style.italic { ITALIC } else { 0 },
    );
    let annotations: Vec<_> = cls
        .annotations
        .iter()
        .map(|a| {
            math_text(&format!("«{}»", a.trim()), tiny, m)
                .map_or_else(|| Annotation::Text(format!("«{}»", decode_entities(a).trim())), Annotation::Math)
        })
        .collect();
    let attributes: Vec<_> = cls
        .members
        .iter()
        .map(|line| member_line(line, m, member_size, mono(&style, options)))
        .collect();
    let methods: Vec<_> = cls
        .methods
        .iter()
        .map(|line| member_line(line, m, member_size, mono(&style, options)))
        .collect();
    let mut inner = label.width;
    let mut annotations_h = 0.;
    for a in &annotations {
        match a {
            Annotation::Text(s) => {
                inner = inner.max(m.width(s, tiny, 0));
                annotations_h += tiny_height;
            }
            Annotation::Math(l) => {
                inner = inner.max(l.width);
                annotations_h += tiny_height.max(l.height);
            }
        }
    }
    for line in attributes.iter().chain(&methods) {
        inner = inner.max(line.width);
    }
    let sections = !(hide_empty && attributes.is_empty() && methods.is_empty());
    let section = |count: usize| {
        if !sections {
            0.
        } else if count > 0 {
            12. + count as f64 * line_height
        } else {
            10.
        }
    };
    let title_h = 16. + annotations_h + label.height.max(js_round(font_size * 1.5));
    let attributes_h = section(attributes.len());
    View {
        node,
        style,
        label,
        w: (inner + 24.).max(64.),
        h: title_h + attributes_h + section(methods.len()),
        loops: 0,
        annotations,
        attributes,
        methods,
        title_h,
        attributes_h,
        font_size,
    }
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    let fitted = |down| tighten(options, |tight| draw(model, config, options, m, down, tight));
    if runs_across(Some(&model.direction)) {
        turn_to_fit(options, fitted)
    } else {
        fitted(false)
    }
}
fn draw(model: &Model, config: &Config, options: &RenderOptions, m: &dyn TextMeasurer, turned: bool, tight: bool) -> Rendered {
    let empty = Config::new();
    let conf = config.get("class").and_then(|v| v.as_object()).unwrap_or(&empty);
    let graph = graph(model, conf.get("hierarchicalNamespaces").and_then(|v| v.as_bool()) != Some(false));
    let hide_empty = conf.get("hideEmptyMembersBox").and_then(|v| v.as_bool()) == Some(true);
    let size = options.font_size.unwrap_or(16.);
    let small = js_round(size * 0.875);
    let tiny = js_round(size * 0.8);
    let tiny_height = js_round(tiny * 1.4);
    let pad = options.padding.unwrap_or(8.);
    let card_width = |raw: &str| {
        if raw.is_empty() {
            0.
        } else {
            math_text(raw, tiny, m).map_or_else(|| m.width(&decode_entities(raw), tiny, 0), |l| l.width)
        }
    };
    let root_dir = if turned { Dir::TB } else { direction(Some(&model.direction)) };
    let mut children = vec![0; graph.nodes.len()];
    for node in &graph.nodes {
        if node.parent >= 0 {
            children[node.parent as usize] += 1;
        }
    }
    let mut crossed = vec![false; graph.nodes.len()];
    for edge in &graph.edges {
        if edge.source < 0 || edge.target < 0 {
            continue;
        }
        let (a, b) = (graph.nodes[edge.source as usize].parent, graph.nodes[edge.target as usize].parent);
        if a == b {
            continue;
        }
        for mut g in [a, b] {
            while g >= 0 && !crossed[g as usize] {
                crossed[g as usize] = true;
                g = graph.nodes[g as usize].parent;
            }
        }
    }
    let (mut views, mut cnodes) = (Vec::new(), Vec::new());
    for (i, node) in graph.nodes.iter().enumerate() {
        let (view, mut c) = if node.is_group {
            let label = layout_label(Some(&node.label), false, m, size, 4000., 0);
            let mut view = base(node, label, 0., 0., size);
            let c = if children[i] > 0 {
                let mut c = cnode(0., 0.);
                c.is_group = true;
                c.pad_x = 20.;
                c.pad_top = if view.label.height > 0. { view.label.height + 16. } else { 20. };
                c.pad_bottom = 20.;
                c.min_w = view.label.width + 2.0 * 20.0 - 12.0
                    + if crossed[i] && !tight {
                        (view.label.width + 24.).min(240.)
                    } else {
                        0.
                    };
                c
            } else {
                view.w = (view.label.width + 40.).max(80.);
                view.h = view.label.height + 40.;
                cnode(view.w, view.h)
            };
            (view, c)
        } else if let Some(cls) = node.class_node {
            let view = class_view(node, cls, model, options, m, size, tiny, tiny_height, hide_empty);
            let c = cnode(view.w, view.h);
            (view, c)
        } else if node.shape == "note" {
            let label = layout_label(Some(&node.label), true, m, small, 260., 0);
            let view = base(node, label.clone(), (label.width + 24.).max(40.), label.height + 16., small);
            let c = cnode(view.w, view.h);
            (view, c)
        } else {
            let label = layout_label(Some(&node.label), false, m, small, 200., 0);
            let view = base(node, label.clone(), label.width.max(12.) + 4., label.height.max(12.), small);
            let c = cnode(view.w, view.h);
            (view, c)
        };
        c.parent = node.parent as i64;
        c.seq = i as f64;
        views.push(view);
        cnodes.push(c);
    }
    let (mut cedges, mut drawn, mut edge_labels, mut start_room, mut end_room) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut crowd = vec![0f64; views.len() * 4];
    for edge in &graph.edges {
        if edge.source < 0 || edge.target < 0 {
            continue;
        }
        let (src, dst) = (edge.source as usize, edge.target as usize);
        let label = layout_label(edge.label.as_deref(), true, m, small, 200., 0);
        let (start_w, end_w) = (card_width(&edge.start_label), card_width(&edge.end_label));
        if src == dst {
            views[src].loops += 1;
            cnodes[src].w += 2. * (26. + if label.width > 0. { label.width + 12. } else { 0. });
        } else {
            for (item, slot, room) in [(src, 0, start_w), (dst, 2, end_w)] {
                crowd[item * 4 + slot] += 1.;
                crowd[item * 4 + slot + 1] = crowd[item * 4 + slot + 1].max(room);
            }
        }
        start_room.push(start_w);
        end_room.push(end_w);
        drawn.push(edge);
        cedges.push(CEdge {
            src,
            dst,
            minlen: 1.,
            label_w: if label.width > 0. { label.width + 12. } else { 0. },
            label_h: if label.height > 0. { label.height + 4. } else { 0. },
            route: vec![],
            label_x: 0.,
            label_y: 0.,
        });
        edge_labels.push(label);
    }
    if matches!(root_dir, Dir::TB | Dir::BT) {
        for i in 0..views.len() {
            if views[i].node.class_node.is_none() {
                continue;
            }
            let mut need = 0f64;
            for slot in [0, 2] {
                let (count, room) = (crowd[i * 4 + slot], crowd[i * 4 + slot + 1]);
                if count > 1. {
                    need = need.max((count - 1.) * if room > 0. { room + 24. } else { 14. } + 24.);
                }
            }
            need = need.min(480.);
            if need > views[i].w {
                cnodes[i].w += need - views[i].w;
                views[i].w = need;
            }
        }
    }
    let lettered = start_room.iter().zip(&end_room).any(|(&a, &b)| a > 0. || b > 0.);
    let mut tall_cards = 0f64;
    if matches!(root_dir, Dir::TB | Dir::BT) {
        let height = |raw: &str| {
            if raw.is_empty() {
                0.
            } else {
                tiny_height.max(math_text(raw, tiny, m).map_or(0., |l| l.height))
            }
        };
        for edge in &drawn {
            tall_cards = tall_cards.max(height(&edge.start_label) + height(&edge.end_label) - 2. * tiny_height);
        }
    }
    let layout = compound_layout(
        &mut cnodes,
        &mut cedges,
        root_dir,
        &LayeredOptions {
            node_sep: number(conf, "nodeSpacing", 40.),
            edge_sep: 16.,
            rank_sep: number(conf, "rankSpacing", if lettered { 72. } else { 56. }) + tall_cards,
            port_sep: 20.,
            apart: true,
            ..Default::default()
        },
    );
    let mut sides: IndexMap<usize, Vec<End>> = IndexMap::new();
    let mut starts = vec![None; cedges.len()];
    let mut ends = vec![None; cedges.len()];
    for (i, ce) in cedges.iter_mut().enumerate() {
        if ce.src == ce.dst || ce.route.len() < 6 {
            continue;
        }
        for (first, item, room) in [(true, ce.src, start_room[i]), (false, ce.dst, end_room[i])] {
            let at = if first { 0 } else { ce.route.len() - 3 };
            let c = &cnodes[item];
            let side = end_side(&ce.route, at, c.x, c.y, c.w, c.h);
            if side < 0 {
                continue;
            }
            let side = side as usize;
            let slack = (c.w - views[item].w) / 2.;
            if side == 1 {
                ce.route[at] -= slack;
            } else if side == 3 {
                ce.route[at] += slack;
            }
            let across = side & 1;
            let toward = ce.route[if at == 0 { 3 + across } else { at - 3 + across }];
            let bend = toward - ce.route[at + across];
            let end = End {
                edge: i,
                at,
                toward,
                order: i,
                room,
                side,
                after: if across != 0 { bend > 0. } else { bend >= 0. },
            };
            sides.entry(item * 4 + side).or_default().push(end);
            if first { starts[i] = Some(end) } else { ends[i] = Some(end) }
        }
    }
    let on_border = |item: usize, x: f64, y: f64| {
        let mut g = cnodes[item].parent;
        let mut hops = 0;
        while g >= 0 && hops < 10000 {
            let c = &cnodes[g as usize];
            let (dx, dy) = ((x - c.x).abs(), (y - c.y).abs());
            if ((dy - c.h / 2.).abs() < 0.5 && dx <= c.w / 2. + 0.5) || ((dx - c.w / 2.).abs() < 0.5 && dy <= c.h / 2. + 0.5) {
                return true;
            }
            g = c.parent;
            hops += 1;
        }
        false
    };
    for (key, list) in &mut sides {
        if list.len() < 2 {
            continue;
        }
        let item = key >> 2;
        let across = key & 1;
        let length = if across != 0 { views[item].h } else { views[item].w };
        list.sort_by(|a, b| a.toward.total_cmp(&b.toward).then(a.order.cmp(&b.order)));
        let mut total = 0.;
        let mut gaps = Vec::new();
        for pair in list.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let mut gap = if across != 0 { 20. } else { 24. };
            if a.room > 0. && a.after {
                gap += if across != 0 { tiny_height + 6. } else { a.room };
            }
            if b.room > 0. && !b.after {
                gap += if across != 0 { tiny_height + 6. } else { b.room };
            }
            gaps.push(gap);
            total += gap;
        }
        let scale = 1f64.min((length - 24.) / total);
        if scale <= 0. {
            continue;
        }
        let mut at = if across != 0 { cnodes[item].y } else { cnodes[item].x } - total * scale / 2.;
        for (j, end) in list.iter().enumerate() {
            let route = &mut cedges[end.edge].route;
            let step = if end.at == 0 { 3 } else { -3 };
            let first = end.at as i32 + step;
            let mut border = None;
            let mut k = first;
            while k >= 0 && (k as usize) < route.len() && (route[k as usize + across] - route[first as usize + across]).abs() < 0.5 {
                if on_border(item, route[k as usize], route[k as usize + 1]) {
                    border = Some(k);
                    break;
                }
                k += step;
            }
            if let Some(border) = border
                && (route[first as usize + across] - at).abs() <= 24.
            {
                let mut k = first;
                while k != border + step {
                    route[k as usize + across] = at;
                    k += step;
                }
            }
            route[end.at + across] = at;
            if let Some(gap) = gaps.get(j) {
                at += gap * scale;
            }
        }
    }
    for i in 0..cedges.len() {
        for (first, end, room) in [(true, starts[i], start_room[i]), (false, ends[i], end_room[i])] {
            let Some(end) = end.filter(|_| room > 0.) else { continue };
            let route = &mut cedges[i].route;
            let across = end.side & 1;
            let at = if first { 0 } else { route.len() - 3 };
            let along = at + 1 - across;
            let reach = (route[if first { 3 + 1 - across } else { at - 3 + 1 - across }] - route[along]).abs();
            let length = (if across != 0 { room + 10. } else { tiny_height + 6. }).min(reach * 0.35);
            if length < 4. {
                continue;
            }
            let mut point = [route[at], route[at + 1], route[at + 2]];
            point[1 - across] += if end.side == 0 || end.side == 3 { -length } else { length };
            route.splice(if first { 3..3 } else { at..at }, point);
        }
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (0f64, 0f64, layout.width, layout.height);
    let mut cards = Vec::new();
    let mut card = |parent: usize, raw: &str, width: f64, x: f64, y: f64, side: usize, after: bool| {
        if raw.is_empty() {
            return;
        }
        let math = math_text(raw, tiny, m);
        let h = math.as_ref().map_or(tiny_height, |l| tiny_height.max(l.height));
        let (mut cx, mut cy) = (x, y);
        if side & 1 != 0 {
            cx += if side == 3 { -(5. + width / 2.) } else { 5. + width / 2. };
            cy += if after { 7. + h / 2. } else { -(7. + h / 2.) };
        } else {
            cx += if after { 9. + width / 2. } else { -(9. + width / 2.) };
            cy += if side == 0 { -(3. + h / 2.) } else { 3. + h / 2. };
        }
        cards.push(Card {
            text: decode_entities(raw),
            math,
            x: cx,
            y: cy,
            w: width,
            h,
            parent,
        });
        min_x = min_x.min(cx - width / 2.);
        max_x = max_x.max(cx + width / 2.);
        min_y = min_y.min(cy - h / 2.);
        max_y = max_y.max(cy + h / 2.);
    };
    let mut loop_seen: HashMap<usize, usize> = HashMap::new();
    let mut loop_at = vec![0; cedges.len()];
    for (i, ce) in cedges.iter_mut().enumerate() {
        let edge = drawn[i];
        if ce.src == ce.dst {
            let k = loop_seen.entry(ce.src).or_default();
            loop_at[i] = *k;
            let kf = *k as f64;
            *k += 1;
            let c = &cnodes[ce.src];
            let view = &views[ce.src];
            let x = c.x + view.w / 2.;
            let spread = (view.h / 2. - 4.).min(8. + kf * 6.);
            card(ce.src, &edge.start_label, start_room[i], x, c.y - spread, 1, false);
            card(ce.dst, &edge.end_label, end_room[i], x, c.y + spread, 1, true);
            ce.label_x = c.x + view.w / 2. + 26. + kf * 8. + 4. + ce.label_w / 2.;
            ce.label_y = c.y;
        } else if ce.route.len() >= 6 {
            let last = ce.route.len() - 3;
            card(
                ce.src,
                &edge.start_label,
                start_room[i],
                ce.route[0],
                ce.route[1],
                starts[i].map_or(2, |e| e.side),
                starts[i].is_none_or(|e| e.after),
            );
            card(
                ce.dst,
                &edge.end_label,
                end_room[i],
                ce.route[last],
                ce.route[last + 1],
                ends[i].map_or(0, |e| e.side),
                ends[i].is_none_or(|e| e.after),
            );
        }
    }
    drop(card);
    if let Some(available) = options.max_width.filter(|&w| w > 0. && max_x - min_x + 2. * pad > w) {
        let mut all = cnodes.clone();
        for item in &cards {
            let mut c = cnode(item.w, item.h);
            c.parent = item.parent as i64;
            c.x = item.x;
            c.y = item.y;
            all.push(c);
        }
        if let Some(wrapped) = wrap_components(
            &mut all,
            &mut cedges,
            (available - 2. * pad).max(1.),
            number(conf, "nodeSpacing", 40.),
        ) {
            let count = cnodes.len();
            cnodes.clone_from_slice(&all[..count]);
            for (item, c) in cards.iter_mut().zip(&all[count..]) {
                item.x = c.x;
                item.y = c.y;
            }
            min_x = 0.;
            min_y = 0.;
            max_x = wrapped.width;
            max_y = wrapped.height;
        }
    }
    let (ox, oy) = (pad - min_x, pad - min_y);
    let width = (max_x - min_x + 2. * pad).ceil();
    let height = (max_y - min_y + 2. * pad).ceil();
    let mut links = Vec::new();
    let mut crossings = Crossings::new();
    if graph.nodes.first().is_some_and(|n| n.is_group) {
        for edge in &cedges {
            mark_crossings(&mut crossings, &edge.route);
        }
    }
    let (mut clusters, mut struck_titles, mut nodes_out) = (String::new(), String::new(), String::new());
    for (i, view) in views.iter().enumerate() {
        let c = &cnodes[i];
        let node = view.node;
        let (x, y) = (c.x + ox, c.y + oy);
        let id = esc_text(&node.id);
        let (w, h) = if c.is_group { (c.w, c.h) } else { (view.w, view.h) };
        if node.is_group {
            let b = Box4 {
                x: c.x,
                y: c.y,
                w: c.w,
                h: c.h,
            };
            let title_x = if c.is_group {
                cluster_title_x(&b, view.label.width, &crossings, 20.)
            } else {
                c.x
            };
            let title_y = y - h / 2. + 8. + view.label.height / 2.;
            let title_attrs = " class=\"pele-cluster-label\" fill=\"var(--_m)\"";
            let struck = c.is_group && title_crossed(&b, view.label.width, title_x, &crossings);
            if struck {
                let mut depth = 1;
                let mut p = node.parent;
                while p >= 0 {
                    depth += 1;
                    p = graph.nodes[p as usize].parent;
                }
                struck_titles += &struck_title(&view.label, title_x + ox, title_y, title_attrs, &id, depth);
            }
            clusters += &format!(
                "<g class=\"pele-cluster pele-namespace\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\" fill-opacity=\"0.5\" stroke=\"var(--_b)\"{}/>{}</g>",
                num(x - w / 2.),
                num(y - h / 2.),
                num(w),
                num(h),
                rounded(""),
                if struck {
                    String::new()
                } else {
                    label_svg(&view.label, title_x + ox, title_y, title_attrs)
                }
            );
            continue;
        }
        let at = format!(" transform=\"translate({},{})\"", num(x), num(y));
        if let Some(cls) = node.class_node {
            nodes_out += &class_svg(view, cls, &id, &at, options, size, tiny, tiny_height, &mut links);
        } else if node.shape == "note" {
            let (r, t) = (w / 2., -h / 2.);
            nodes_out += &format!(
                "<g class=\"pele-node pele-note\" data-id=\"{id}\"{at}><path d=\"M{},{}H{}L{},{}V{}H{}Z\" fill=\"var(--_a)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"/><path d=\"M{},{}V{}H{}\" fill=\"none\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"/>{}</g>",
                num(-r),
                num(t),
                num(r - 8.),
                num(r),
                num(t + 8.),
                num(-t),
                num(-r),
                num(r - 8.),
                num(t),
                num(t + 8.),
                num(r),
                label_svg(
                    &view.label,
                    0.,
                    0.,
                    &format!(" class=\"pele-label\" font-size=\"{}\"", js_num(small))
                )
            );
        } else {
            nodes_out += &format!(
                "<g class=\"pele-node pele-interface\" data-id=\"{id}\"{at}>{}</g>",
                label_svg(
                    &view.label,
                    0.,
                    0.,
                    &format!(" class=\"pele-label\" font-size=\"{}\"", js_num(small))
                )
            );
        }
    }
    let (mut edges_out, mut labels_out) = (String::new(), String::new());
    for (i, ce) in cedges.iter_mut().enumerate() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let (start_trim, end_trim) = (markers::trim(edge.start_marker), markers::trim(edge.end_marker));
        let path = if ce.src == ce.dst {
            let c = &cnodes[ce.src];
            let view = &views[ce.src];
            let k = loop_at[i] as f64;
            let (x, y) = (c.x + ox + view.w / 2., c.y + oy);
            let spread = (view.h / 2. - 4.).min(8. + k * 6.);
            let reach = 26. + k * 8.;
            EdgePath {
                d: format!(
                    "M{},{}C{},{} {},{} {},{}",
                    num(x + start_trim),
                    num(y - spread),
                    num(x + reach + start_trim),
                    num(y - spread),
                    num(x + reach + end_trim),
                    num(y + spread),
                    num(x + end_trim),
                    num(y + spread)
                ),
                sx: x,
                sy: y - spread,
                sdx: -1.,
                sdy: 0.,
                ex: x,
                ey: y + spread,
                edx: -1.,
                edy: 0.,
                ..Default::default()
            }
        } else {
            for p in ce.route.as_chunks_mut::<3>().0 {
                p[0] += ox;
                p[1] += oy;
            }
            route_path(&ce.route, None, start_trim, end_trim, None, None)
        };
        let attrs = match edge.pattern {
            "dashed" => " stroke-dasharray=\"6 4\"",
            "dotted" => " stroke-dasharray=\"2 4\"",
            _ => "",
        };
        edges_out += &format!(
            "<g class=\"pele-edge{}\" data-id=\"{}\"><path d=\"{}\"{attrs}/>{}{}</g>",
            if edge.pattern == "dotted" {
                " pele-note-edge"
            } else {
                " pele-relation"
            },
            esc_text(&edge.id),
            path.d,
            markers::draw(edge.start_marker, path.sx, path.sy, path.sdx, path.sdy, "var(--_l)"),
            markers::draw(edge.end_marker, path.ex, path.ey, path.edx, path.edy, "var(--_l)")
        );
        if label.width > 0. {
            labels_out += &edge_label_svg(&edge.id, label, ce.label_x + ox, ce.label_y + oy, "");
        }
    }
    let mut cards_out = String::new();
    for item in cards {
        cards_out += &if let Some(math) = item.math {
            label_svg(&math, item.x + ox, item.y + oy, " class=\"pele-cardinality\"")
        } else {
            format!(
                "<text class=\"pele-cardinality\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
                num(item.x + ox),
                num(item.y + oy + tiny * 0.35),
                esc(&item.text)
            )
        };
    }
    let mut content = String::new();
    if !clusters.is_empty() {
        content += &format!("<g class=\"pele-clusters\">{clusters}</g>");
    }
    if !edges_out.is_empty() {
        content += &format!("<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>");
    }
    if !struck_titles.is_empty() {
        content += &format!("<g class=\"pele-cluster-titles\">{struck_titles}</g>");
    }
    if !labels_out.is_empty() {
        content += &format!("<g class=\"pele-edge-labels\" font-size=\"{}\">{labels_out}</g>", js_num(small));
    }
    if !cards_out.is_empty() {
        content += &format!(
            "<g class=\"pele-cardinalities\" font-size=\"{}\" fill=\"var(--_m)\">{cards_out}</g>",
            js_num(tiny)
        );
    }
    content += &format!("<g class=\"pele-nodes\">{nodes_out}</g>");
    let titled = with_title(model.title.as_deref(), m, size, pad, width, height, content);
    let svg = svg_document(
        "class",
        titled.width,
        titled.height,
        size,
        options,
        &Accessible {
            acc_title: model.acc_title.as_deref(),
            acc_descr: model.acc_descr.as_deref(),
        },
        &titled.content,
    );
    Rendered {
        svg,
        width: titled.width,
        height: titled.height,
        links,
    }
}
fn class_svg(
    view: &View,
    cls: &Node,
    id: &str,
    at: &str,
    options: &RenderOptions,
    size: f64,
    tiny: f64,
    tiny_height: f64,
    links: &mut Vec<LinkInfo>,
) -> String {
    let (w, h, style) = (view.w, view.h, &view.style);
    let (left, top) = (-w / 2., -h / 2.);
    let member_size = js_round(view.font_size * 0.875);
    let line_height = js_round(member_size * 1.5);
    let mut body = format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}/>",
        num(left),
        num(top),
        num(w),
        num(h),
        rounded(&style.shape)
    );
    let ink = ink_fill(&style.shape, &style.text, None);
    let first = top + view.title_h;
    let second = first + view.attributes_h;
    if h > view.title_h {
        let stroke = style
            .stroke
            .as_deref()
            .filter(|s| !s.is_empty())
            .map_or(String::new(), |s| format!(" style=\"stroke:{}\"", esc(s)));
        body += &format!(
            "<path class=\"pele-divider\" d=\"M{},{}H{}M{},{}H{}\" fill=\"none\" stroke=\"var(--_b)\"{stroke}/>",
            num(left),
            num(first),
            num(-left),
            num(left),
            num(second),
            num(-left)
        );
    }
    let mut y = top + 8.;
    for annotation in &view.annotations {
        let muted = if ink.is_empty() { " fill=\"var(--_m)\"" } else { &ink };
        match annotation {
            Annotation::Text(text) => {
                body += &format!(
                    "<text class=\"pele-annotation\" x=\"0\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\"{muted}>{}</text>",
                    num(y + tiny_height / 2. + tiny * 0.35),
                    js_num(tiny),
                    esc(text)
                );
                y += tiny_height;
            }
            Annotation::Math(label) => {
                let h = tiny_height.max(label.height);
                body += &label_svg(
                    label,
                    0.,
                    y + h / 2.,
                    &format!(" class=\"pele-annotation\" font-size=\"{}\"{muted}", js_num(tiny)),
                );
                y += h;
            }
        }
    }
    let title_size = if view.font_size == size {
        String::new()
    } else {
        format!(" font-size=\"{}\"", num(view.font_size))
    };
    body += &label_svg(
        &view.label,
        0.,
        (y + first - 8.) / 2.,
        &format!(" class=\"pele-label\"{title_size} font-weight=\"var(--_hw)\"{ink}{}", style.text),
    );
    let lines = |list: &[Line], kind: &str, from: f64| {
        let mut out = String::new();
        let mut baseline = from + 6. + line_height / 2. + member_size * 0.35;
        for line in list {
            let mark = if line.classifier == "$" {
                format!(
                    " pele-static\"{}",
                    if style.text.is_empty() {
                        " style=\"text-decoration:underline\"".into()
                    } else {
                        format!("{}text-decoration:underline\"", &style.text[..style.text.len() - 1])
                    }
                )
            } else if line.classifier == "*" {
                format!(" pele-abstract\" font-style=\"italic\"{}", style.text)
            } else {
                format!("\"{}", style.text)
            };
            out += &format!(
                "<text class=\"pele-member pele-{kind}{mark} x=\"{}\" y=\"{}\">{}</text>",
                num(left + 12.),
                num(baseline),
                esc(&line.text)
            );
            baseline += line_height;
        }
        out
    };
    if !view.attributes.is_empty() || !view.methods.is_empty() {
        body += &format!(
            "<g class=\"pele-members\" font-size=\"{}\"{ink}{}>{}{}</g>",
            js_num(member_size),
            if mono(style, options) {
                format!(" font-family=\"{FONT_MONO}\"")
            } else {
                String::new()
            },
            lines(&view.attributes, "attribute", first),
            lines(&view.methods, "method", second)
        );
    }
    if let Some(tip) = cls.tooltip.as_deref().filter(|s| !s.is_empty()) {
        body = format!("<title>{}</title>{body}", esc_text(tip));
    }
    if let Some(link) = cls.link.as_deref().filter(|s| !s.is_empty()) {
        let target = cls
            .link_target
            .as_deref()
            .filter(|s| matches!(*s, "_self" | "_blank" | "_parent" | "_top"))
            .map_or(String::new(), |s| format!(" target=\"{s}\""));
        body = linked(&body, link, &cls.id, links, options, &target);
    }
    let classes = cls
        .css_classes
        .strip_prefix("default")
        .map(|s| s.strip_prefix(char::is_whitespace).unwrap_or(s))
        .unwrap_or(&cls.css_classes);
    format!(
        "<g class=\"pele-node pele-class{}\" data-id=\"{id}\"{at}>{body}</g>",
        class_names(classes)
    )
}
