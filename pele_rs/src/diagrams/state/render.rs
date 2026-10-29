use super::{graph::Node, model::Model};
use crate::{
    Config, RenderOptions, TextMeasurer,
    diagrams::common::{Rendered, runs_across, turn_to_fit},
    layout::{
        compound::{CEdge, CNode, Dir, cnode, compound_layout, direction},
        layered::LayeredOptions,
    },
    svg::{
        builder::{esc, esc_text, label_svg},
        edges::{EdgePath, Fill, SideReach, casings_svg, edge_casing, edge_label_svg, marker, marker_trim, over_fills, route_path},
        ink::{ink_fill, paints},
        root::{Accessible, svg_document},
        shapes::end_side,
        theme::{ResolvedStyle, class_names, resolve_style, sets_fill},
        title::with_title,
    },
    text::{
        entities::decode_entities,
        label::{Label, layout_label},
        measurer::{BOLD, ITALIC},
    },
    url::{linked, safe_url},
    util::{js_num, js_round, js_sign, num},
};
use std::collections::HashMap;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum K {
    State,
    Described,
    Start,
    End,
    Choice,
    Fork,
    Join,
    Note,
    Composite,
    Region,
    Hidden,
}
const CLASSES: [&str; 9] = [
    "pele-node pele-state",
    "pele-node pele-state",
    "pele-node pele-state-start",
    "pele-node pele-state-end",
    "pele-node pele-state-choice",
    "pele-node pele-state-fork",
    "pele-node pele-state-join",
    "pele-note",
    "pele-cluster pele-state-composite",
];
const SURFACE: &str = " fill=\"var(--_s)\" stroke=\"var(--_b)\"";
const RULE: &str = " fill=\"none\" stroke=\"var(--_b)\"";
struct View<'a> {
    node: Option<&'a Node>,
    kind: K,
    style: Option<ResolvedStyle>,
    label: Label,
    lines: Vec<Label>,
    w: f64,
    h: f64,
    title_h: f64,
}
fn view(node: Option<&Node>, kind: K) -> View<'_> {
    View {
        node,
        kind,
        style: None,
        label: Label::default(),
        lines: vec![],
        w: 0.,
        h: 0.,
        title_h: 0.,
    }
}
fn vertical(dir: Option<Dir>) -> bool {
    matches!(dir, Some(Dir::TB | Dir::BT))
}
fn number(config: &Config, key: &str, fallback: f64) -> f64 {
    config
        .get("state")
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_f64())
        .filter(|&v| v > 0.)
        .map_or(fallback, |v| v.min(2000.))
}
fn rect(x: f64, y: f64, w: f64, h: f64, rest: &str) -> String {
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rest}/>",
        num(x),
        num(y),
        num(w),
        num(h)
    )
}
fn group(dir: Dir, pad_x: f64, pad_top: f64, pad_bottom: f64) -> CNode {
    let mut c = cnode(0., 0.);
    c.is_group = true;
    c.dir = Some(dir);
    c.pad_x = pad_x;
    c.pad_top = pad_top;
    c.pad_bottom = pad_bottom;
    c
}
fn title_x(c: &CNode, width: f64, crossings: &HashMap<i64, Vec<f64>>) -> f64 {
    let Some(xs) = crossings.get(&(js_round(c.y - c.h / 2.) as i64)) else {
        return c.x;
    };
    if width == 0. {
        return c.x;
    }
    let left = c.x - c.w / 2.;
    let half = width / 2.;
    let mut best = c.x;
    let mut best_gap = -1.;
    for x in [c.x, left + 16. + half, left + c.w - 16. - half] {
        let mut gap = f64::INFINITY;
        for &cx in xs {
            if cx > left && cx < left + c.w {
                gap = gap.min((cx - x).abs() - half);
            }
        }
        if gap >= 6. {
            return x;
        }
        if gap > best_gap + 0.5 {
            best_gap = gap;
            best = x;
        }
    }
    best
}
fn divide(items: &[usize], views: &[View], cnodes: &[CNode], pad: f64, left: f64, top: f64, right: f64, bottom: f64) -> String {
    let regions: Vec<_> = items.iter().copied().filter(|&i| views[i].kind == K::Region).collect();
    if regions.len() < 2 {
        return String::new();
    }
    let lo = |i: usize, x: bool| {
        if x {
            cnodes[i].x - cnodes[i].w / 2.
        } else {
            cnodes[i].y - cnodes[i].h / 2.
        }
    };
    let hi = |i: usize, x: bool| {
        if x {
            cnodes[i].x + cnodes[i].w / 2.
        } else {
            cnodes[i].y + cnodes[i].h / 2.
        }
    };
    let mut out = String::new();
    for x in [true, false] {
        let mut sorted = regions.clone();
        sorted.sort_by(|&a, &b| lo(a, x).total_cmp(&lo(b, x)).then(a.cmp(&b)));
        if sorted.windows(2).any(|p| hi(p[0], x) > lo(p[1], x) + 0.5) {
            continue;
        }
        for pair in sorted.windows(2) {
            let at = num((hi(pair[0], x) + lo(pair[1], x)) / 2. + pad);
            out += &format!(
                "<path class=\"pele-divider\" d=\"M{}\"/>",
                if x {
                    format!("{at},{}V{}", num(top), num(bottom))
                } else {
                    format!("{},{at}H{}", num(left), num(right))
                }
            );
        }
        return out;
    }
    for i in regions {
        let c = &cnodes[i];
        out += &rect(
            c.x - c.w / 2. + pad,
            c.y - c.h / 2. + pad,
            c.w,
            c.h,
            " class=\"pele-divider\" rx=\"10\"",
        );
    }
    out
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    if runs_across(Some(model.direction())) {
        turn_to_fit(options, |down| draw(model, config, options, m, down))
    } else {
        draw(model, config, options, m, false)
    }
}
fn draw(model: &Model, config: &Config, options: &RenderOptions, m: &dyn TextMeasurer, turned: bool) -> Rendered {
    let graph = &model.graph;
    let curve = config.get("flowchart").and_then(|c| c.get("curve")).and_then(|v| v.as_str());
    let size = options.font_size.unwrap_or(16.);
    let small = js_round(size * 0.875);
    let wrap_width = number(config, "wrappingWidth", 200.);
    let pad = options.padding.unwrap_or(8.);
    let root_dir = if turned { Dir::TB } else { direction(Some(model.direction())) };
    let text = |raw: Option<&str>, size: f64, wrap: f64, style: u32| layout_label(raw, true, m, size, wrap, style);
    let mut index = HashMap::new();
    let mut views = Vec::new();
    let mut cnodes = Vec::new();
    for node in &graph.nodes {
        if node.shape.as_deref() == Some("noteGroup") {
            continue;
        }
        let kind = if node.is_group {
            if node.shape.as_deref() == Some("divider") {
                K::Region
            } else {
                K::Composite
            }
        } else {
            match node.shape.as_deref() {
                Some("rectWithTitle") => K::Described,
                Some("stateStart") => K::Start,
                Some("stateEnd") => K::End,
                Some("choice") => K::Choice,
                Some("fork") => K::Fork,
                Some("join") => K::Join,
                Some("note") => K::Note,
                Some("divider") => K::Hidden,
                _ => K::State,
            }
        };
        index.insert(node.id.as_str(), views.len());
        views.push(view(Some(node), kind));
    }
    let states = views.len();
    let mut logical = Vec::new();
    for i in 0..states {
        let parent = views[i].node.unwrap().parent_id.as_deref().and_then(|id| index.get(id).copied());
        logical.push(
            parent
                .filter(|&p| p != i && views[p].node.unwrap().is_group)
                .map_or(-1, |p| p as i64),
        );
    }
    {
        let mut kids = vec![Vec::new(); states];
        let mut queue = Vec::new();
        let mut placed = vec![false; states];
        for i in 0..states {
            if logical[i] == -1 {
                placed[i] = true;
                queue.push(i);
            } else {
                kids[logical[i] as usize].push(i);
            }
        }
        let mut done = 0;
        while done < queue.len() {
            for &kid in &kids[queue[done]] {
                if !placed[kid] {
                    placed[kid] = true;
                    queue.push(kid);
                }
            }
            done += 1;
        }
        let mut seen = vec![-1i64; states];
        for i in 0..states {
            if placed[i] {
                continue;
            }
            let mut ring = i;
            while seen[ring] != i as i64 {
                seen[ring] = i as i64;
                ring = logical[ring] as usize;
            }
            logical[ring] = -1;
            placed[ring] = true;
            queue.push(ring);
            while done < queue.len() {
                for &kid in &kids[queue[done]] {
                    if !placed[kid] {
                        placed[kid] = true;
                        queue.push(kid);
                    }
                }
                done += 1;
            }
        }
    }
    let mut owners = vec![-1i64; states];
    for i in 0..states {
        if let Some(&owner) = views[i].node.unwrap().note_for.as_deref().and_then(|id| index.get(id)) {
            owners[i] = owner as i64;
            logical[i] = logical[owner];
        }
    }
    let inner_dirs: Vec<_> = views.iter().map(|v| direction(v.node.unwrap().dir.as_deref())).collect();
    let across: Vec<_> = (0..states)
        .map(|i| {
            vertical(Some(if logical[i] == -1 {
                root_dir
            } else {
                inner_dirs[logical[i] as usize]
            }))
        })
        .collect();
    let (mut degree, mut noted, mut appear, mut counts) = (
        vec![0usize; states],
        vec![false; states],
        vec![f64::INFINITY; states],
        vec![0usize; states * 2],
    );
    let mut appeared = 0.;
    for edge in &graph.edges {
        let (Some(&src), Some(&dst)) = (index.get(edge.start.as_str()), index.get(edge.end.as_str())) else {
            continue;
        };
        if edge.dashed {
            noted[if views[src].kind == K::Note { dst } else { src }] = true;
            continue;
        }
        counts[src * 2] += 1;
        degree[src] = degree[src].max(counts[src * 2]);
        counts[dst * 2 + 1] += 1;
        degree[dst] = degree[dst].max(counts[dst * 2 + 1]);
        for i in [src, dst] {
            if appear[i] == f64::INFINITY {
                appear[i] = appeared;
                appeared += 1.;
            }
        }
    }
    for i in 0..states {
        let v = &mut views[i];
        let node = v.node.unwrap();
        let styles: Vec<_> = node.compiled_styles.iter().chain(&node.styles).cloned().collect();
        let style = resolve_style(&styles);
        let text_style = if style.bold { BOLD } else { 0 } | if style.italic { ITALIC } else { 0 };
        let font_size = style.font_size.unwrap_or(size);
        v.style = Some(style);
        let mut c = None;
        match v.kind {
            K::Composite => {
                v.label = text(node.label.as_deref(), font_size, 4000., text_style);
                v.title_h = if v.label.height > 0. { v.label.height + 12. } else { 0. };
                let mut group = group(inner_dirs[i], 16., v.title_h + 16., 16.);
                group.min_w = v.label.width + 2. * 16.;
                c = Some(group);
            }
            K::Region => c = Some(group(inner_dirs[i], 10., 10., 10.)),
            K::Start => {
                v.w = 14.;
                v.h = 14.;
            }
            K::End => {
                v.w = 18.;
                v.h = 18.;
            }
            K::Choice => {
                v.w = 30.;
                v.h = 30.;
            }
            K::Fork | K::Join => {
                let length = (40. * degree[i] as f64 + 16.).clamp(72., 320.);
                v.w = if across[i] { length } else { 8. };
                v.h = if across[i] { 8. } else { length };
            }
            K::Hidden => {}
            K::Note => {
                v.label = text(node.label.as_deref(), small, wrap_width.max(320.), 0);
                v.w = v.label.width + 24.;
                v.h = v.label.height + 16.;
            }
            _ => {
                v.label = text(node.label.as_deref(), font_size, wrap_width, text_style);
                let mut width = v.label.width;
                let mut height = v.label.height + 2. * 10.;
                if v.kind == K::Described {
                    height += 14.;
                    for line in &node.description {
                        let label = text(Some(if line.is_empty() { " " } else { line }), small, wrap_width, text_style);
                        width = width.max(label.width);
                        height += label.height;
                        v.lines.push(label);
                    }
                }
                v.w = (width + 2. * 16.).max(56.);
                v.h = height;
            }
        }
        let mut c = c.unwrap_or_else(|| cnode(v.w, v.h));
        if matches!(v.kind, K::State | K::Described) {
            c.span = if across[i] { v.w } else { v.h } - 24.;
        }
        cnodes.push(c);
    }
    let (mut cedges, mut edge_labels, mut drawn) = (Vec::new(), Vec::new(), Vec::new());
    let (mut loop_count, mut loop_width, mut loop_height) = (vec![0usize; states], vec![0f64; states], vec![0f64; states]);
    let mut anchors = HashMap::new();
    for edge in &graph.edges {
        let (Some(&src), Some(&dst)) = (index.get(edge.start.as_str()), index.get(edge.end.as_str())) else {
            continue;
        };
        let (mut src, mut dst) = (src, dst);
        let label = text(Some(&edge.label), small, wrap_width, 0);
        let label_w = if label.width > 0. { label.width + 12. } else { 0. };
        let encloses = |composite: usize, item: usize| {
            if !cnodes[composite].is_group {
                return false;
            }
            let mut p = logical[item];
            while p != -1 {
                if p == composite as i64 {
                    return true;
                }
                p = logical[p as usize];
            }
            false
        };
        if src == dst {
            loop_count[src] += 1;
            loop_width[src] = loop_width[src].max(label_w);
            loop_height[src] += label.height;
        } else {
            let composite = if encloses(src, dst) {
                Some((src, true))
            } else if encloses(dst, src) {
                Some((dst, false))
            } else {
                None
            };
            if let Some((composite, first)) = composite {
                anchors.insert(cedges.len(), composite);
                appear.push(if first { -1. } else { f64::INFINITY });
                let at = views.len();
                views.push(view(None, K::Hidden));
                let mut c = cnode(0., 0.);
                c.parent = composite as i64;
                cnodes.push(c);
                if first { src = at } else { dst = at }
            }
        }
        drawn.push(edge);
        cedges.push(CEdge {
            src,
            dst,
            minlen: 1.,
            label_w,
            label_h: if label.height > 0. { label.height + 4. } else { 0. },
            route: vec![],
            label_x: 0.,
            label_y: 0.,
        });
        edge_labels.push(label);
    }
    for i in 0..states {
        if owners[i] != -1 {
            continue;
        }
        let item = &mut cnodes[i];
        let mut room = 0.;
        if loop_count[i] > 0 {
            let reach = 26. + (loop_count[i] - 1) as f64 * 8.;
            room = if across[i] {
                reach + loop_width[i]
            } else {
                reach * 0.75 + 4. + loop_height[i]
            };
            if !item.is_group {
                if across[i] {
                    item.w += 2. * room;
                    item.h = item.h.max(loop_height[i]);
                } else {
                    item.h += 2. * room;
                    item.w = item.w.max(loop_width[i]);
                }
                room = 0.;
            }
        }
        if !noted[i] && room == 0. {
            item.parent = logical[i];
            continue;
        }
        let mut c = if across[i] {
            group(Dir::LR, room, 0., 0.)
        } else {
            group(Dir::TB, 0., room, room)
        };
        c.parent = logical[i];
        appear.push(appear[i]);
        cnodes[i].parent = views.len() as i64;
        views.push(view(None, K::Hidden));
        cnodes.push(c);
    }
    for i in 0..states {
        if owners[i] != -1 {
            cnodes[i].parent = cnodes[owners[i] as usize].parent;
        }
    }
    {
        let mut members = vec![vec![]; cnodes.len()];
        let mut top = Vec::new();
        for (i, c) in cnodes.iter().enumerate() {
            if c.parent >= 0 {
                members[c.parent as usize].push(i)
            } else {
                top.push(i)
            }
        }
        let by_flow = |a: &usize, b: &usize| {
            let initial = |i: usize| if views[i].kind == K::Start { 0 } else { 1 };
            initial(*a).cmp(&initial(*b)).then(appear[*a].total_cmp(&appear[*b])).then(a.cmp(b))
        };
        top.sort_by(by_flow);
        top.reverse();
        let mut seq = 0.;
        while let Some(i) = top.pop() {
            cnodes[i].seq = seq;
            seq += 1.;
            members[i].sort_by(by_flow);
            for &kid in members[i].iter().rev() {
                top.push(kid);
            }
        }
    }
    let layout = compound_layout(
        &mut cnodes,
        &mut cedges,
        root_dir,
        &LayeredOptions {
            node_sep: number(config, "nodeSpacing", 40.),
            edge_sep: 16.,
            rank_sep: number(config, "rankSpacing", 44.),
            port_sep: 20.,
            tracks: matches!(curve, Some("step" | "stepBefore" | "stepAfter")),
            straight: curve == Some("linear"),
            head_room: marker_trim("arrow_point"),
            ..Default::default()
        },
    );
    let width = (layout.width + 2. * pad).ceil();
    let height = (layout.height + 2. * pad).ceil();
    let mut links = Vec::new();
    let mut crossings: HashMap<i64, Vec<f64>> = HashMap::new();
    for e in &cedges {
        for k in (3..e.route.len().saturating_sub(3)).step_by(3) {
            crossings.entry(js_round(e.route[k + 1]) as i64).or_default().push(e.route[k]);
        }
    }
    let mut children = vec![vec![]; cnodes.len()];
    let mut order = Vec::new();
    for (i, c) in cnodes.iter().enumerate() {
        if c.parent >= 0 {
            children[c.parent as usize].push(i)
        } else {
            order.push(i)
        }
    }
    let roots = order.clone();
    let mut q = 0;
    while q < order.len() {
        for &child in &children[order[q]] {
            order.push(child);
        }
        q += 1;
    }
    let (mut clusters, mut dividers, mut nodes_out, mut notes_out) = (
        String::new(),
        divide(
            &roots,
            &views,
            &cnodes,
            pad,
            pad / 2.,
            pad / 2.,
            width - pad / 2.,
            height - pad / 2.,
        ),
        String::new(),
        String::new(),
    );
    let mut fills = Vec::new();
    for i in order {
        let v = &views[i];
        let (Some(node), Some(style)) = (v.node, &v.style) else { continue };
        if v.kind >= K::Region {
            continue;
        }
        let c = &cnodes[i];
        let (x, y, w, h) = (c.x + pad, c.y + pad, v.w, v.h);
        let ink = ink_fill(&style.shape, &style.text, None);
        let label_attrs = format!(" class=\"pele-label\"{ink}{}", style.text);
        let mut body = match v.kind {
            K::Composite => {
                let (left, top) = (x - c.w / 2., y - c.h / 2.);
                if paints(&style.shape) {
                    fills.push(Fill {
                        bx: [left, top, left + c.w, top + c.h],
                        shape: style.shape.clone(),
                    });
                }
                let mut body = rect(
                    left,
                    top,
                    c.w,
                    c.h,
                    &format!(
                        " rx=\"10\" fill=\"var(--_a)\"{} stroke=\"var(--_b)\"{}",
                        if sets_fill(&style.shape) { "" } else { " fill-opacity=\"0.5\"" },
                        style.shape
                    ),
                );
                if v.title_h > 0. {
                    body += &format!(
                        "<path d=\"M{},{}H{}\"{RULE}{}/>",
                        num(left),
                        num(top + v.title_h),
                        num(left + c.w),
                        style.line
                    );
                    body += &label_svg(
                        &v.label,
                        title_x(c, v.label.width, &crossings) + pad,
                        top + v.title_h / 2.,
                        &format!(" class=\"pele-cluster-label\"{ink}{}", style.text),
                    );
                }
                dividers += &divide(&children[i], &views, &cnodes, pad, left, top + v.title_h, left + c.w, top + c.h);
                body
            }
            K::Start => format!("<circle r=\"7\" fill=\"var(--_l)\"{}/>", style.shape),
            K::End => format!(
                "<circle r=\"8.5\" fill=\"var(--_bg)\" stroke=\"var(--_l)\"{}/><circle r=\"5\" fill=\"var(--_l)\"{}/>",
                style.line, style.shape
            ),
            K::Choice => format!(
                "<polygon points=\"0,{} {},0 0,{} {},0\"{SURFACE}{}/>",
                num(-h / 2.),
                num(w / 2.),
                num(h / 2.),
                num(-w / 2.),
                style.shape
            ),
            K::Fork | K::Join => rect(-w / 2., -h / 2., w, h, &format!(" rx=\"2\" fill=\"var(--_l)\"{}", style.shape)),
            K::Note => {
                let (r, b, fold, corner) = (num(w / 2.), num(h / 2.), num(w / 2. - 8.), num(8. - h / 2.));
                format!(
                    "<path d=\"M-{r},-{b}H{fold}L{r},{corner}V{b}H-{r}Z\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"/><path d=\"M{fold},-{b}V{corner}H{r}\"{RULE}/>{}",
                    label_svg(
                        &v.label,
                        0.,
                        0.,
                        &format!(" class=\"pele-label\" font-size=\"{}\" fill=\"var(--_m)\"", js_num(small))
                    )
                )
            }
            _ => {
                let mut body = rect(
                    -w / 2.,
                    -h / 2.,
                    w,
                    h,
                    &format!(" rx=\"{}\"{SURFACE}{}", num(10f64.min(h / 2.)), style.shape),
                );
                if v.kind == K::State {
                    body += &label_svg(&v.label, 0., 0., &label_attrs);
                } else {
                    let mut at = 10. - h / 2. + v.label.height;
                    body += &format!(
                        "<path d=\"M{},{}H{}\"{RULE}{}/>{}",
                        num(-w / 2.),
                        num(at + 6.),
                        num(w / 2.),
                        style.line,
                        label_svg(&v.label, 0., at - v.label.height / 2., &label_attrs)
                    );
                    at += 14.;
                    for line in &v.lines {
                        body += &label_svg(
                            line,
                            0.,
                            at + line.height / 2.,
                            &format!(
                                " class=\"pele-state-description\" font-size=\"{}\"{ink}{}",
                                js_num(small),
                                style.text
                            ),
                        );
                        at += line.height;
                    }
                }
                body
            }
        };
        if let Some(link) = model.links.get(&node.id) {
            let tooltip = decode_entities(link.tooltip.trim_matches('"'));
            if !tooltip.is_empty() {
                body = format!("<title>{}</title>{body}", esc(&tooltip));
            }
            body = linked(
                &body,
                &safe_url(link.url.trim_matches('"')),
                &node.id,
                &mut links,
                options,
                " target=\"_blank\"",
            );
        }
        let css = node
            .css_classes
            .split(' ')
            .filter(|s| {
                !matches!(
                    *s,
                    "statediagram-state" | "statediagram-cluster-alt" | "statediagram-cluster" | "statediagram-note"
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        let open = format!(
            "<g class=\"{}{}\" data-id=\"{}\"",
            CLASSES[v.kind as usize],
            class_names(&css),
            esc_text(&node.id)
        );
        if v.kind == K::Composite {
            clusters += &format!("{open}>{body}</g>");
        } else if v.kind == K::Note {
            notes_out += &format!("{open} transform=\"translate({},{})\">{body}</g>", num(x), num(y));
        } else {
            nodes_out += &format!("{open} transform=\"translate({},{})\">{body}</g>", num(x), num(y));
        }
    }
    let attach = |route: &mut Vec<f64>, at: usize, i: usize| {
        let c = &cnodes[i];
        if c.is_group {
            return;
        }
        let v = &views[i];
        let bar = matches!(v.kind, K::Fork | K::Join);
        let other = if at == 0 { 3 } else { at - 3 };
        for axis in [1, 0] {
            let (extent, drawn_extent, breadth, center, cross) = if axis != 0 {
                (c.h, v.h, v.w, c.y + pad, c.x + pad)
            } else {
                (c.w, v.w, v.h, c.x + pad, c.y + pad)
            };
            let delta = route[at + axis] - center;
            if (delta.abs() - extent / 2.).abs() >= 0.5 {
                continue;
            }
            route[at + axis] -= js_sign(delta) * (extent - drawn_extent) / 2.;
            if bar && breadth > drawn_extent {
                route[at + 1 - axis] = route[other + 1 - axis].clamp(cross - breadth / 2. + 6., cross + breadth / 2. - 6.);
            }
            return;
        }
    };
    let mut reach = SideReach::default();
    for (i, ce) in cedges.iter().enumerate() {
        if ce.src == ce.dst || ce.route.len() < 6 || drawn[i].dashed {
            continue;
        }
        let c = &cnodes[ce.dst];
        reach.add(ce.dst, end_side(&ce.route, ce.route.len() - 3, c.x, c.y, c.w, c.h), "arrow_point");
    }
    let (mut casings, mut edges_out, mut labels_out) = (String::new(), String::new(), String::new());
    let (mut loop_seen, mut loop_text) = (vec![0usize; states], vec![0f64; states]);
    for (i, ce) in cedges.iter_mut().enumerate() {
        let edge = drawn[i];
        let label = &edge_labels[i];
        let note = edge.dashed;
        let end_type = if note { "none" } else { "arrow_point" };
        let path = if ce.src == ce.dst {
            let a = &cnodes[ce.src];
            let v = &views[ce.src];
            let k = loop_seen[ce.src] as f64;
            loop_seen[ce.src] += 1;
            let side = across[ce.src];
            let (w, h) = if a.is_group { (a.w, a.h) } else { (v.w, v.h) };
            let out = if side { w } else { h } / 2.;
            let spread = ((if side { h } else { w }) / 2. - 4.).min(8. + k * 6.).max(2.);
            let reach = 26. + k * 8.;
            let len = reach.hypot(spread);
            let trim = marker_trim(end_type) / if len == 0. { 1. } else { len };
            let px = |dx: f64, dy: f64| a.x + pad + if side { out + dx } else { -dy };
            let py = |dx: f64, dy: f64| a.y + pad + if side { dy } else { out + dx };
            let at = |dx: f64, dy: f64| format!("{},{}", num(px(dx, dy)), num(py(dx, dy)));
            let mut path = EdgePath {
                d: format!(
                    "M{}C{} {} {}",
                    at(0., -spread),
                    at(reach, -2. * spread),
                    at(reach, 2. * spread),
                    at(reach * trim, spread + spread * trim)
                ),
                ex: px(0., spread),
                ey: py(0., spread),
                edx: px(-reach, -spread) - px(0., 0.),
                edy: py(-reach, -spread) - py(0., 0.),
                ..Default::default()
            };
            let len = path.edx.hypot(path.edy);
            let len = if len == 0. { 1. } else { len };
            path.edx /= len;
            path.edy /= len;
            let furthest = 26. + (loop_count[ce.src] - 1) as f64 * 8.;
            let below = loop_text[ce.src] + label.height / 2.;
            loop_text[ce.src] += label.height;
            ce.label_x = if side { a.x + out + furthest + 4. + ce.label_w / 2. } else { a.x };
            ce.label_y = if side {
                a.y - loop_height[ce.src] / 2. + below
            } else {
                a.y + out + furthest * 0.75 + 4. + below
            };
            path
        } else {
            let mut route = ce.route.clone();
            let a = &cnodes[ce.src];
            let b = &cnodes[ce.dst];
            let leaves = reach.of(ce.src, end_side(&route, 0, a.x, a.y, a.w, a.h));
            let arrives = reach.of(ce.dst, end_side(&route, route.len() - 3, b.x, b.y, b.w, b.h));
            for p in route.as_chunks_mut::<3>().0 {
                p[0] += pad;
                p[1] += pad;
            }
            attach(&mut route, 0, ce.src);
            let at = route.len() - 3;
            attach(&mut route, at, ce.dst);
            if let Some(&composite) = anchors.get(&i).filter(|_| route.len() >= 6) {
                let g = &cnodes[composite];
                let inward = views[ce.src].kind == K::Hidden;
                let at = if inward { 0 } else { route.len() - 3 };
                let near = matches!(g.dir, Some(Dir::TB | Dir::LR)) == inward;
                let point = if vertical(g.dir) {
                    [
                        route[at],
                        g.y + pad + if near { views[composite].title_h - g.h / 2. } else { g.h / 2. },
                        0.,
                    ]
                } else {
                    [g.x + pad + if near { -g.w / 2. } else { g.w / 2. }, route[at + 1], 1.]
                };
                if inward {
                    route.splice(..0, point);
                } else {
                    route.extend_from_slice(&point);
                }
            }
            route_path(&route, curve, 0., marker_trim(end_type), Some(leaves), Some(arrives))
        };
        let over_result = over_fills(&path.d, &[path.ex, path.ey], &fills, "", path.tokens.as_deref());
        let over = over_result.as_deref().unwrap_or("");
        if over_result.as_deref() == Some("") {
            casings += &edge_casing(&path, "none", end_type, "", false);
        }
        let kind = if note {
            format!(
                "pele-note-edge\" stroke=\"{}\" stroke-dasharray=\"3 4",
                if over.is_empty() { "var(--_b)" } else { over }
            )
        } else if !over.is_empty() {
            format!("pele-transition\" stroke=\"{over}")
        } else {
            "pele-transition".into()
        };
        edges_out += &format!(
            "<g class=\"pele-edge {kind}\" data-id=\"{}\"><path d=\"{}\"/>{}</g>",
            esc_text(&edge.id),
            path.d,
            marker(
                end_type,
                path.ex,
                path.ey,
                path.edx,
                path.edy,
                if over.is_empty() { "var(--_l)" } else { over },
                "none"
            )
        );
        if label.width > 0. {
            labels_out += &edge_label_svg(&edge.id, label, ce.label_x + pad, ce.label_y + pad, "");
        }
    }
    let mut content = String::new();
    if !clusters.is_empty() {
        content += &format!("<g class=\"pele-clusters\">{clusters}</g>");
    }
    if !dividers.is_empty() {
        content += &format!("<g class=\"pele-dividers\" stroke-dasharray=\"4 4\"{RULE}>{dividers}</g>");
    }
    content += &casings_svg(&casings);
    if !edges_out.is_empty() {
        content += &format!("<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>");
    }
    if !labels_out.is_empty() {
        content += &format!("<g class=\"pele-edge-labels\" font-size=\"{}\">{labels_out}</g>", js_num(small));
    }
    content += &format!("<g class=\"pele-nodes\">{nodes_out}{notes_out}</g>");
    let titled = with_title(model.title.as_deref(), m, size, pad, width, height, content);
    let svg = svg_document(
        "state",
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
