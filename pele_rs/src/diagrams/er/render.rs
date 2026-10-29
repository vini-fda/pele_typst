use super::{
    markers::{MARKER_LENGTH, marker},
    model::{Model, Node, graph},
};
use crate::diagrams::common::{Rendered, runs_across, turn_to_fit};
use crate::layout::{
    compound::{CEdge, CNode, Dir, cnode, compound_layout, direction, shift_layout},
    layered::LayeredOptions,
};
use crate::svg::{
    builder::{esc, esc_text, label_svg, shared_style, span_style, text_style},
    cluster::{Box4, cluster_title_x, mark_crossings},
    edges::{Fill, Under, casing, casings_svg, edge_ink, edge_label_svg, fill_under, hollow_on, route_path},
    ink::{ON_DARK, ON_LIGHT, ink, ink_fill, paints},
    root::{Accessible, svg_document},
    theme::{ResolvedStyle, class_names, resolve_style, rounded, sets_fill},
};
use crate::text::{
    entities::decode_entities,
    label::{Label, has_math, label_line, layout_label, line_middle},
    measurer::{BOLD, ITALIC, MONO},
};
use crate::url::{LinkInfo, internal_link, rel_attr};
use crate::util::{js_num, js_round, num};
use crate::{Config, MonospacePart, RenderOptions, TextMeasurer};
use indexmap::IndexMap;
use std::collections::HashMap;
struct Row {
    cells: [Label; 4],
    h: f64,
}
struct View<'a> {
    node: &'a Node,
    style: ResolvedStyle,
    label: Label,
    w: f64,
    h: f64,
    head_h: f64,
    rows: Vec<Row>,
    cols: Vec<f64>,
    row_size: f64,
    key_size: f64,
    mono: u32,
    below: bool,
}
struct Loop {
    label: Label,
    band: f64,
    at: f64,
}
fn option(config: &Config, key: &str, fallback: f64) -> f64 {
    config
        .get("er")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_f64())
        .filter(|&v| v > 0.0)
        .map(|v| v.min(2000.0))
        .unwrap_or(fallback)
}
pub fn text_left(label: &Label, x: f64, mut y: f64, attrs: &str, mono: u32) -> String {
    if label.lines.is_empty() {
        return String::new();
    }
    if label.lines.len() == 1 && label.lines[0].len() == 1 {
        let span = &label.lines[0][0];
        if span.plain() && span.style == mono && !(mono != 0 && span.text.contains("  ")) {
            return format!(
                "<text{} x=\"{}\" y=\"{}\">{}</text>",
                text_style(attrs, mono),
                num(x),
                num(y),
                esc(&span.text)
            );
        }
    }
    let shared = shared_style(label);
    let shift = label.size * 0.35;
    let top = y - shift - label.line_height / 2.0;
    let mut body = String::new();
    let mut extra = String::new();
    for (i, line) in label.lines.iter().enumerate() {
        let middle = top + line_middle(label, i);
        if has_math(line) {
            extra += &label_svg(&label_line(label, i), x + label.widths[i] / 2.0, middle, attrs);
            continue;
        }
        y = middle + shift;
        let has_icon = line.iter().any(|s| s.icon.is_some());
        let mut sx = x;
        let mut placed = false;
        for span in line {
            if let Some(icon) = &span.icon {
                let side = label.size * 1.1;
                extra += &format!(
                    "<svg class=\"pele-icon\" data-icon=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{}</svg>",
                    esc_text(icon),
                    num(sx),
                    num(y - label.size * 0.35 - side / 2.0),
                    num(side),
                    num(side),
                    span.draw_icon()
                );
            } else if !span.text.is_empty() {
                let pos = if has_icon || !placed {
                    format!(" x=\"{}\" y=\"{}\"", num(sx), num(y))
                } else {
                    String::new()
                };
                body += &format!("<tspan{}{}>{}</tspan>", pos, span_style(span.style & !shared), esc(&span.text));
                placed = true;
            }
            sx += span.width;
        }
    }
    format!("<text{} xml:space=\"preserve\">{}</text>{}", text_style(attrs, shared), body, extra)
}

pub fn generic(raw: &str) -> String {
    if !raw.contains('~') {
        return raw.into();
    }
    fn count(s: &str) -> usize {
        s.chars().filter(|&c| c == '~').count()
    }
    fn pair(s: &str) -> String {
        let n = count(s);
        if n <= 1 {
            return s.into();
        }
        let lead = !n.is_multiple_of(2) && s.starts_with('~');
        let mut chars: Vec<char> = s.chars().collect();
        let at: Vec<usize> = chars
            .iter()
            .enumerate()
            .filter(|&(i, &c)| c == '~' && (!lead || i > 0))
            .map(|(i, _)| i)
            .collect();
        for i in 0..at.len() / 2 {
            chars[at[i]] = '<';
            chars[at[at.len() - 1 - i]] = '>';
        }
        chars.into_iter().collect()
    }
    let mut sets = vec![];
    for (i, p) in raw.split(',').enumerate() {
        if i > 0 {
            sets.push(",");
        }
        sets.push(p);
    }
    let mut out: Vec<String> = vec![];
    let mut i = 0;
    while i < sets.len() {
        let set = if sets[i] == "," && i > 0 && i + 1 < sets.len() && count(sets[i - 1]) == 1 && count(sets[i + 1]) == 1 {
            out.pop();
            let s = format!("{},{}", sets[i - 1], sets[i + 1]);
            i += 1;
            s
        } else {
            sets[i].into()
        };
        out.push(pair(&set));
        i += 1;
    }
    out.concat()
}
fn cell(raw: &str) -> String {
    let converted = generic(raw);
    if converted == raw {
        raw.into()
    } else {
        converted.replace('<', "&lt;").replace('>', "&gt;")
    }
}
pub fn render_er(db: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    if runs_across(Some(&db.direction)) {
        turn_to_fit(options, |down| draw(db, config, options, measurer, down))
    } else {
        draw(db, config, options, measurer, false)
    }
}
fn draw(db: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer, turned: bool) -> Rendered {
    let (nodes, edges) = graph(db);
    let monospaced = options.monospace.as_ref().is_some_and(|p| p.contains(&MonospacePart::Attributes));
    let size = options.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let pad = options.padding.unwrap_or(8.0);
    let index: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let mut views = vec![];
    let mut cnodes = vec![];
    let mut cedges = vec![];
    let mut edge_labels = vec![];
    let mut drawn = vec![];
    let mut loops: Vec<Loop> = vec![];
    let mut node_loops: IndexMap<usize, Vec<usize>> = IndexMap::new();
    let mut loop_of = vec![];
    for edge in &edges {
        let (Some(&src), Some(&dst)) = (index.get(edge.start.as_str()), index.get(edge.end.as_str())) else {
            continue;
        };
        let label = layout_label(Some(&edge.label), true, measurer, small, 200.0, 0);
        let own = if src == dst {
            let i = loops.len();
            loops.push(Loop {
                label: label.clone(),
                band: 0.0,
                at: 0.0,
            });
            node_loops.entry(src).or_default().push(i);
            Some(i)
        } else {
            None
        };
        loop_of.push(own);
        drawn.push(edge);
        cedges.push(CEdge {
            src,
            dst,
            minlen: 1.0,
            label_w: if label.width > 0.0 { label.width + 12.0 } else { 0.0 },
            label_h: if label.height > 0.0 { label.height + 4.0 } else { 0.0 },
            route: vec![],
            label_x: 0.0,
            label_y: 0.0,
        });
        edge_labels.push(label);
    }
    let horizontal = |i: usize| {
        let mut at = i;
        for _ in 0..nodes.len() {
            let Some(up) = nodes[at].parent_id.as_ref().and_then(|p| index.get(p.as_str())).copied() else {
                break;
            };
            if let Some(d) = nodes[up].dir.as_deref().filter(|s| !s.is_empty()) {
                return matches!(d, "LR" | "RL");
            }
            at = up;
        }
        matches!(db.direction.as_str(), "LR" | "RL")
    };
    for (i, node) in nodes.iter().enumerate() {
        let decls: Vec<String> = node.css_compiled_styles.iter().chain(&node.css_styles).cloned().collect();
        let style = resolve_style(&decls);
        let base = if style.bold { BOLD } else { 0 } | if style.italic { ITALIC } else { 0 };
        let mono = if monospaced && !style.text.contains("\"font-family:") && !style.text.contains(";font-family:") {
            MONO
        } else {
            0
        };
        let name_size = style.font_size.unwrap_or(size);
        let row_size = style.font_size.unwrap_or(small);
        let key_size = style.font_size.unwrap_or(js_round(size * 0.75));
        let label = layout_label(
            Some(if node.is_group || node.alias.is_empty() {
                &node.label
            } else {
                &node.alias
            }),
            true,
            measurer,
            name_size,
            if node.is_group { 4000.0 } else { 240.0 },
            base,
        );
        let own = node_loops.get(&i).map(Vec::as_slice).unwrap_or(&[]);
        let below = !own.is_empty() && !node.is_group && horizontal(i);
        let mut view = View {
            node,
            style,
            label,
            w: 0.0,
            h: 0.0,
            head_h: 0.0,
            rows: vec![],
            cols: vec![],
            row_size,
            key_size,
            mono,
            below,
        };
        let mut reach: f64 = 0.0;
        let mut bands = 0.0;
        for &l in own {
            let lp = &mut loops[l];
            lp.band = 38.0f64.max(if below { lp.label.width + 12.0 } else { lp.label.height + 6.0 });
            bands += lp.band;
            reach = reach.max(if below { lp.label.height } else { lp.label.width });
        }
        let min_w = 96.0f64.max(if below { bands + 8.0 } else { 0.0 });
        let mut c;
        if node.is_group {
            c = cnode(0.0, 0.0);
            c.is_group = true;
            c.dir = node.dir.as_deref().filter(|s| !s.is_empty()).map(|d| direction(Some(d)));
            c.pad_x = 20.0;
            c.pad_top = if view.label.height > 0.0 { view.label.height + 16.0 } else { 20.0 };
            c.pad_bottom = 20.0;
            c.min_w = (view.label.width + 28.0).max(96.0);
        } else {
            let name_h = view.label.height.max(js_round(name_size * 1.5));
            if node.attributes.is_empty() {
                view.w = (view.label.width + 36.0).max(min_w);
                view.h = (name_h + 24.0).max(if below { 0.0 } else { bands + 8.0 });
            } else {
                let row_line = js_round(row_size * 1.5);
                let mut widths = [0.0f64; 4];
                for a in &node.attributes {
                    let cells = [
                        layout_label(Some(&cell(&a.kind)), true, measurer, row_size, 1e6, base | mono),
                        layout_label(Some(&cell(&a.name)), true, measurer, row_size, 1e6, base | mono),
                        layout_label(Some(&a.keys.join(", ")), false, measurer, key_size, 1e6, base | mono),
                        layout_label(Some(&cell(&a.comment)), true, measurer, row_size, 260.0, base),
                    ];
                    let mut h = row_line;
                    for k in 0..4 {
                        widths[k] = widths[k].max(cells[k].width);
                        h = h.max(cells[k].height);
                    }
                    view.rows.push(Row { cells, h: h + 9.0 });
                    view.h += h + 9.0;
                }
                let mut columns = 0;
                let mut inner = 0.0;
                for w in widths {
                    if w == 0.0 {
                        continue;
                    }
                    inner += w + if columns > 0 { 16.0 } else { 0.0 };
                    columns += 1;
                }
                view.w = (inner + 24.0).max(view.label.width + 36.0).max(min_w);
                let spare = if columns > 0 {
                    (view.w - inner - 24.0) / columns as f64
                } else {
                    0.0
                };
                let mut x = 12.0;
                for w in widths {
                    view.cols.push(x);
                    if w > 0.0 {
                        x += w + spare + 16.0;
                    }
                }
                view.head_h = name_h + 18.0;
                view.h += view.head_h;
            }
            let room = if own.is_empty() {
                0.0
            } else {
                2.0 * (MARKER_LENGTH + 28.0 * 0.75 + 10.0 + reach)
            };
            c = if below {
                cnode(view.w, view.h + room)
            } else {
                cnode(view.w + room, view.h)
            };
        }
        c.seq = views.len() as f64;
        views.push(view);
        cnodes.push(c);
    }
    for (i, node) in nodes.iter().enumerate() {
        cnodes[i].parent = node
            .parent_id
            .as_ref()
            .and_then(|p| index.get(p.as_str()))
            .map(|&i| i as i64)
            .unwrap_or(-1);
    }
    let layout = compound_layout(
        &mut cnodes,
        &mut cedges,
        if turned { Dir::TB } else { direction(Some(&db.direction)) },
        &LayeredOptions {
            node_sep: option(config, "nodeSpacing", 48.0),
            edge_sep: 20.0,
            rank_sep: option(config, "rankSpacing", 64.0),
            port_sep: 24.0,
            ..Default::default()
        },
    );
    spread_ends(&mut cedges, &cnodes, &views);
    let mut layout_width = layout.width;
    for (&i, own) in &node_loops {
        let c = &cnodes[i];
        if !c.is_group {
            continue;
        }
        let reach = own.iter().map(|&l| loops[l].label.width).fold(0.0, f64::max);
        layout_width = layout_width.max(c.x + c.w / 2.0 + MARKER_LENGTH + 28.0 * 0.75 + 10.0 + reach);
    }
    let title = crate::svg::title::title_label(db.title.as_deref(), measurer, size, 4000.0);
    let title_h = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let inner = layout_width.max(title.width);
    shift_layout(&mut cnodes, &mut cedges, pad + (inner - layout_width) / 2.0, pad + title_h);
    let width = (inner + 2.0 * pad).ceil();
    let height = (layout.height + title_h + 2.0 * pad).ceil();
    let mut links = vec![];
    let mut crossings = HashMap::new();
    if cnodes.iter().any(|c| c.is_group) {
        for e in &cedges {
            mark_crossings(&mut crossings, &e.route);
        }
    }
    let mut clusters = String::new();
    let mut fills = vec![];
    let mut nodes_out = String::new();
    for (i, view) in views.iter_mut().enumerate() {
        let c = &cnodes[i];
        let node = view.node;
        let classes = class_names(
            node.css_classes
                .strip_prefix("default ")
                .or_else(|| node.css_classes.strip_prefix("default"))
                .unwrap_or(&node.css_classes),
        );
        if c.is_group {
            view.w = c.w;
            view.h = c.h;
            let x = c.x - c.w / 2.0;
            let y = c.y - c.h / 2.0;
            let tx = cluster_title_x(
                &Box4 {
                    x: c.x,
                    y: c.y,
                    w: c.w,
                    h: c.h,
                },
                view.label.width,
                &crossings,
                20.0,
            );
            if paints(&view.style.shape) {
                fills.push(Fill {
                    bx: [x, y, x + c.w, y + c.h],
                    shape: view.style.shape.clone(),
                });
            }
            clusters += &format!(
                "<g class=\"pele-cluster{}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\"{} stroke=\"var(--_b)\"{}/>{}</g>",
                classes,
                esc_text(&node.id),
                num(x),
                num(y),
                num(c.w),
                num(c.h),
                if sets_fill(&view.style.shape) {
                    ""
                } else {
                    " fill-opacity=\"0.5\""
                },
                rounded(&view.style.shape),
                label_svg(
                    &view.label,
                    tx,
                    y + 8.0 + view.label.height / 2.0,
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
        let (x0, y0) = (-w / 2.0, -h / 2.0);
        let text = &view.style.text;
        let color = ink_fill(&view.style.shape, text, None);
        let muted = if color.is_empty() { " fill=\"var(--_m)\"" } else { &color };
        let shape = format!("x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"", num(x0), num(y0), num(w), num(h));
        let mut name = label_svg(
            &view.label,
            0.0,
            if view.rows.is_empty() { 0.0 } else { y0 + view.head_h / 2.0 },
            &format!(" class=\"pele-label\"{}{}", color, text),
        );
        let display = decode_entities(if node.alias.is_empty() { &node.label } else { &node.alias });
        if format!("{} ", classes).contains(" internal-link ") && !name.is_empty() && options.links != Some(false) {
            let safe = internal_link(&display, options);
            let target = esc(&display);
            name = format!(
                "<a class=\"internal-link\"{} data-href=\"{}\">{}</a>",
                if safe {
                    format!(" href=\"{}\"{}", target, rel_attr(options))
                } else {
                    String::new()
                },
                target,
                name
            );
            links.push(LinkInfo {
                id: node.label.clone(),
                href: display,
                internal: true,
            });
        }
        let box_attrs = format!(" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}", view.style.shape);
        let body = if view.rows.is_empty() {
            format!("<rect {}{}/>{}", shape, rounded(&box_attrs), name)
        } else {
            let row_line = js_round(view.row_size * 1.5);
            let line_color = view
                .style
                .stroke
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(esc)
                .unwrap_or_else(|| "var(--_b)".into());
            let mut stripes = String::new();
            let mut cells = String::new();
            let mut y = y0 + view.head_h;
            for (r, row) in view.rows.iter().enumerate() {
                if r % 2 == 0 {
                    let span = format!("x=\"{}\" width=\"{}\"", num(x0), num(w));
                    if r == view.rows.len() - 1 {
                        stripes += &format!(
                            "<rect {} y=\"{}\" height=\"{}\" style=\"rx:var(--_r)\"/><rect {} y=\"{}\" height=\"{}\"/>",
                            span,
                            num(y),
                            num(row.h),
                            span,
                            num(y),
                            num(row.h / 2.0)
                        );
                    } else {
                        stripes += &format!("<rect {} y=\"{}\" height=\"{}\"/>", span, num(y), num(row.h));
                    }
                }
                let baseline = y + 4.5 + row_line / 2.0 + view.row_size * 0.35;
                cells += &text_left(
                    &row.cells[0],
                    x0 + view.cols[0],
                    baseline,
                    &format!(" class=\"pele-er-type\"{}{}", muted, text),
                    view.mono,
                );
                cells += &text_left(
                    &row.cells[1],
                    x0 + view.cols[1],
                    baseline,
                    &format!(" class=\"pele-er-name\"{}{}", color, text),
                    view.mono,
                );
                cells += &text_left(
                    &row.cells[2],
                    x0 + view.cols[2],
                    baseline,
                    &format!(" class=\"pele-er-keys\" font-size=\"{}\"{}{}", js_num(view.key_size), muted, text),
                    view.mono,
                );
                cells += &text_left(
                    &row.cells[3],
                    x0 + view.cols[3],
                    baseline,
                    &format!(" class=\"pele-er-comment\"{}{}", muted, text),
                    0,
                );
                y += row.h;
            }
            let shade = match ink(&view.style.shape, "") {
                Some(ON_LIGHT) => ON_DARK,
                Some(ON_DARK) => ON_LIGHT,
                _ => "var(--_bg)",
            };
            format!(
                "<rect {}{}/><g class=\"pele-er-stripes\" fill=\"{}\"{}>{}</g><path class=\"pele-er-divider\" d=\"M{},{}H{}\" stroke=\"{}\"/><rect {} fill=\"none\" stroke=\"var(--_b)\"{}/>{}<g class=\"pele-er-attributes\" font-size=\"{}\">{}</g>",
                shape,
                rounded(&box_attrs),
                shade,
                if view.style.shape.contains("fill:") {
                    " opacity=\"0.6\""
                } else {
                    ""
                },
                stripes,
                num(x0),
                num(y0 + view.head_h),
                num(x0 + w),
                line_color,
                shape,
                rounded(&view.style.line),
                name,
                js_num(view.row_size),
                cells
            )
        };
        nodes_out += &format!(
            "<g class=\"pele-node pele-entity{}\" data-id=\"{}\" transform=\"translate({},{})\">{}</g>",
            classes,
            esc_text(&node.label),
            num(c.x),
            num(c.y),
            body
        );
    }
    for (&i, own) in &node_loops {
        let view = &views[i];
        let total: f64 = own.iter().map(|&l| loops[l].band).sum();
        let scale = (((if view.below { view.w } else { view.h }) - 8.0).max(0.0) / total).min(1.0);
        let mut at = -total * scale / 2.0;
        for &l in own {
            let lp = &mut loops[l];
            lp.band *= scale;
            lp.at = at + lp.band / 2.0;
            at += lp.band;
        }
    }
    let mut edges_out = String::new();
    let mut casings = String::new();
    let mut labels_out = String::new();
    for i in 0..cedges.len() {
        let ce = &mut cedges[i];
        let edge = drawn[i];
        let label = &edge_labels[i];
        let (d, mut start, mut end);
        if let Some(l) = loop_of[i] {
            let lp = &loops[l];
            let c = &cnodes[ce.src];
            let view = &views[ce.src];
            let spread = (lp.band / 2.0 - 6.0).clamp(1.0, 13.0);
            let far = MARKER_LENGTH + 28.0 * 0.75 + 6.0;
            if view.below {
                let x = c.x + lp.at;
                let y = c.y + view.h / 2.0;
                let turn = num(y + MARKER_LENGTH);
                let out = num(y + MARKER_LENGTH + 28.0);
                d = format!(
                    "M{},{}V{}C{},{} {},{} {},{}V{}",
                    num(x - spread),
                    num(y),
                    turn,
                    num(x - spread),
                    out,
                    num(x + spread),
                    out,
                    num(x + spread),
                    turn,
                    num(y)
                );
                start = marker(&edge.start_type, x - spread, y, 0.0, -1.0);
                end = marker(&edge.end_type, x + spread, y, 0.0, -1.0);
                ce.label_x = x;
                ce.label_y = y + far + label.height / 2.0;
            } else {
                let x = c.x + view.w / 2.0;
                let y = c.y + lp.at;
                let turn = num(x + MARKER_LENGTH);
                let out = num(x + MARKER_LENGTH + 28.0);
                d = format!(
                    "M{},{}H{}C{},{} {},{} {},{}H{}",
                    num(x),
                    num(y - spread),
                    turn,
                    out,
                    num(y - spread),
                    out,
                    num(y + spread),
                    turn,
                    num(y + spread),
                    num(x)
                );
                start = marker(&edge.start_type, x, y - spread, -1.0, 0.0);
                end = marker(&edge.end_type, x, y + spread, -1.0, 0.0);
                ce.label_x = x + far + label.width / 2.0;
                ce.label_y = y;
            }
        } else {
            let mut route = straighten(&ce.route);
            if route.len() < 6 {
                continue;
            }
            let last = route.len() - 3;
            inset(&mut route, 0, &views[ce.src], &cnodes[ce.src]);
            inset(&mut route, last, &views[ce.dst], &cnodes[ce.dst]);
            let dx = (route[last] - route[0]).abs();
            let dy = (route[last + 1] - route[1]).abs();
            let direct = route.len() == 6 && (dx < 0.01 || dy < 0.01 || dx.hypot(dy) <= 3.0 * MARKER_LENGTH);
            let stub = if direct { 0.0 } else { MARKER_LENGTH };
            let p = route_path(&route, None, stub, stub, None, None);
            d = if stub > 0.0 {
                format!("M{},{}L{}L{},{}", num(p.sx), num(p.sy), &p.d[1..], num(p.ex), num(p.ey))
            } else {
                p.d
            };
            start = marker(&edge.start_type, p.sx, p.sy, p.sdx, p.sdy);
            end = marker(&edge.end_type, p.ex, p.ey, p.edx, p.edy);
        }
        let under = fill_under(&d, &[], &fills, None);
        let over = match under {
            Under::Off => None,
            Under::Crosses => Some(String::new()),
            Under::On(f) => Some(edge_ink(f, "")),
        };
        if over.as_deref() == Some("") {
            casings += &casing(&d, &(start.clone() + &end), "", false);
        }
        if let Under::On(f) = under
            && over.as_deref().is_some_and(|v| !v.is_empty())
        {
            start = hollow_on(&start, f);
            end = hollow_on(&end, f);
        }
        edges_out += &format!(
            "<g class=\"pele-edge pele-er-{}\" data-id=\"{}\"{}><path d=\"{}\"{}/>{}{}</g>",
            if edge.dashed { "non-identifying" } else { "identifying" },
            esc_text(&edge.id),
            over.as_deref()
                .filter(|s| !s.is_empty())
                .map(|s| format!(" stroke=\"{}\"", s))
                .unwrap_or_default(),
            d,
            if edge.dashed { " stroke-dasharray=\"4 3\"" } else { "" },
            start,
            end
        );
        if label.width > 0.0 {
            labels_out += &edge_label_svg(&edge.id, label, ce.label_x, ce.label_y, "");
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
    if !labels_out.is_empty() {
        content += &format!("<g class=\"pele-edge-labels\" font-size=\"{}\">{}</g>", js_num(small), labels_out);
    }
    content += &format!("<g class=\"pele-nodes\">{}</g>", nodes_out);
    let svg = svg_document(
        "er",
        width,
        height,
        size,
        options,
        &Accessible {
            acc_title: db.acc_title.as_deref(),
            acc_descr: db.acc_descr.as_deref(),
        },
        &content,
    );
    Rendered { svg, width, height, links }
}
fn straighten(route: &[f64]) -> Vec<f64> {
    if route.len() < 3 {
        return route.to_vec();
    }
    let mut out = route[..3].to_vec();
    for k in (3..route.len()).step_by(3) {
        let prev = out.len() - 3;
        let flat = |a: usize| (out[prev + a] - route[k + a]).abs() < 0.01 && (route[k + a] - route[k + 3 + a]).abs() < 0.01;
        if k + 3 < route.len() && (flat(0) || flat(1)) {
            continue;
        }
        out.extend_from_slice(&route[k..k + 3]);
    }
    out
}
fn spread_ends(edges: &mut [CEdge], nodes: &[CNode], views: &[View]) {
    let mut sides: IndexMap<usize, Vec<(usize, usize, f64)>> = IndexMap::new();
    for (i, e) in edges.iter().enumerate() {
        if e.src == e.dst || e.route.len() < 6 {
            continue;
        }
        let last = e.route.len() - 3;
        for (node, at, next) in [(e.src, 0, 3), (e.dst, last, last - 3)] {
            let c = &nodes[node];
            let (dx, dy) = (e.route[at] - c.x, e.route[at + 1] - c.y);
            let side = if (dy.abs() - c.h / 2.0).abs() < 0.5 {
                if dy < 0.0 { 0 } else { 2 }
            } else if (dx.abs() - c.w / 2.0).abs() < 0.5 {
                if dx < 0.0 { 3 } else { 1 }
            } else {
                continue;
            };
            sides
                .entry(node * 4 + side)
                .or_default()
                .push((i, at, e.route[next + if side & 1 == 1 { 1 } else { 0 }]));
        }
    }
    for (key, mut ends) in sides {
        if ends.len() < 2 {
            continue;
        }
        let node = key >> 2;
        let vertical = key & 1 == 1;
        let c = &nodes[node];
        let v = &views[node];
        let length = if c.is_group {
            if vertical { c.h } else { c.w }
        } else if vertical {
            v.h
        } else {
            v.w
        };
        let step = ((length - 28.0) / (ends.len() - 1) as f64).clamp(0.0, 18.0);
        let center = if vertical { c.y } else { c.x };
        ends.sort_by(|a, b| a.2.total_cmp(&b.2));
        let half = (ends.len() - 1) as f64 / 2.0;
        for (i, (ei, at, _)) in ends.into_iter().enumerate() {
            edges[ei].route[at + if vertical { 1 } else { 0 }] = center + (i as f64 - half) * step;
        }
    }
}
fn inset(route: &mut [f64], at: usize, view: &View, c: &CNode) {
    if c.is_group {
        return;
    }
    let (dx, dy) = (route[at] - c.x, route[at + 1] - c.y);
    if c.w != view.w && (dx.abs() - c.w / 2.0).abs() < 0.5 {
        route[at] -= dx.signum() * (c.w - view.w) / 2.0;
    }
    if c.h != view.h && (dy.abs() - c.h / 2.0).abs() < 0.5 {
        route[at + 1] -= dy.signum() * (c.h - view.h) / 2.0;
    }
}
