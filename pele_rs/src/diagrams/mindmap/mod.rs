pub(crate) mod outline;
use crate::diagrams::common::{Rendered, title_room, turn_to_fit};
use crate::errors::PeleErrorCode;
use crate::preprocess::Config;
use crate::svg::builder::{esc_text, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::shapes::{draw_shape, shape_inset, shape_size};
use crate::svg::theme::{class_names, series_color};
use crate::text::label::{Label, layout_label, line_middle};
use crate::text::measurer::TextMeasurer;
use crate::util::{js_round, num};
use crate::{PeleError, RenderOptions};
#[derive(Clone, Debug)]
pub struct Node {
    pub id: usize,
    pub node_id: String,
    pub level: isize,
    pub descr: String,
    pub kind: usize,
    pub children: Vec<usize>,
    pub is_root: bool,
    pub section: Option<usize>,
    pub class: Option<String>,
    pub icon: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: Option<String>,
    pub nodes: Vec<Node>,
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let mut model = Model {
        title: title.map(str::to_string),
        nodes: Vec::new(),
    };
    let mut base = 0;
    let mut open: Vec<usize> = Vec::new();
    outline::parse(source, false, |op| {
        match op {
            outline::Op::Node {
                level, id, descr, kind, ..
            } => {
                let is_root = model.nodes.is_empty();
                if is_root {
                    base = level as isize
                }
                let level = level as isize - base;
                while open.last().is_some_and(|i| model.nodes[*i].level >= level) {
                    open.pop();
                }
                let parent = open.last().copied();
                if parent.is_none() && !is_root {
                    return Err(PeleError::new(
                        format!("There can be only one root. No parent could be found for (\"{descr}\")"),
                        PeleErrorCode::Semantic,
                    )
                    .of("mindmap"));
                }
                let index = model.nodes.len();
                let section = parent.and_then(|p| {
                    if model.nodes[p].is_root {
                        Some(model.nodes[p].children.len() % 11)
                    } else {
                        model.nodes[p].section
                    }
                });
                if let Some(p) = parent {
                    model.nodes[p].children.push(index)
                }
                model.nodes.push(Node {
                    id: index,
                    node_id: id,
                    level,
                    descr,
                    kind,
                    children: Vec::new(),
                    is_root,
                    section,
                    class: None,
                    icon: None,
                });
                open.push(index)
            }
            outline::Op::Icon(v) => {
                if !v.is_empty()
                    && let Some(n) = model.nodes.last_mut()
                {
                    n.icon = Some(v)
                }
            }
            outline::Op::Class(v) => {
                if !v.is_empty()
                    && let Some(n) = model.nodes.last_mut()
                {
                    n.class = Some(v)
                }
            }
        }
        Ok(())
    })?;
    Ok(model)
}
pub(crate) fn icon_markup(name: &str, o: &RenderOptions) -> String {
    o.icons.as_ref().and_then(|r| r.resolve(name)).unwrap_or_default()
}
pub(crate) fn icon_svg(name: &str, x: f64, y: f64, side: f64, o: &RenderOptions) -> String {
    format!(
        "<svg class=\"pele-icon\" data-icon=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{}</svg>",
        esc_text(name),
        num(x),
        num(y),
        num(side),
        num(side),
        icon_markup(name, o)
    )
}
pub(crate) fn label_line(l: &Label, k: usize) -> Label {
    crate::text::label::label_line(l, k)
}
fn left_label(l: &Label, left: f64, cy: f64) -> String {
    if l.lines.len() < 2 {
        return label_svg(l, left + l.width / 2.0, cy, " class=\"pele-label\"");
    }
    let top = cy - l.height / 2.0;
    let mut out = String::new();
    for k in 0..l.lines.len() {
        out += &label_svg(
            &label_line(l, k),
            left + l.widths[k] / 2.0,
            top + line_middle(l, k),
            " class=\"pele-label\"",
        )
    }
    out
}
pub fn render(model: &Model, config: &Config, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    turn_to_fit(o, |down| draw(model, config, o, m, down))
}
fn stack(
    list: &[usize],
    from: usize,
    to: usize,
    gap: f64,
    nodes: &[Node],
    extent: &[f64],
    own: &[f64],
    anchor: &[f64],
    rel: &mut [f64],
) -> (f64, f64) {
    let mut y = 0.0;
    let mut last = 0.0;
    for k in from..to {
        let c = list[k];
        rel[c] = y;
        let grouped = !nodes[c].children.is_empty() || (k + 1 < to && !nodes[list[k + 1]].children.is_empty());
        last = if grouped { gap.max(18.0) } else { gap };
        y += extent[c] + last
    }
    let a = list[from];
    let b = list[to - 1];
    ((rel[a] + own[a] + anchor[a] + rel[b] + own[b] + anchor[b]) / 2.0, y - last)
}
fn draw(model: &Model, config: &Config, o: &RenderOptions, m: &dyn TextMeasurer, down: bool) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let pad = o.padding.unwrap_or(8.0);
    let wrap = config
        .get("mindmap")
        .and_then(|v| v.get("maxNodeWidth"))
        .and_then(|v| v.as_f64())
        .filter(|v| *v > 0.0)
        .map(|v| v.min(2000.0))
        .unwrap_or(200.0);
    let glyph = size * 1.25;
    let nodes = &model.nodes;
    let n = nodes.len();
    let mut parent = vec![0; n];
    let mut depth = vec![0; n];
    for i in 0..n {
        for &c in &nodes[i].children {
            parent[c] = i;
            depth[c] = depth[i] + 1
        }
    }
    let room = if down {
        o.max_width.filter(|x| *x > 0.0).map(|x| x - 2.0 * pad).unwrap_or(f64::INFINITY)
    } else {
        f64::INFINITY
    };
    let mut labels = vec![Label::default(); n];
    let mut shapes = vec![""; n];
    let mut w = vec![0.0; n];
    let mut h = vec![0.0; n];
    let mut dy = vec![0.0; n];
    let mut anchor = vec![0.0; n];
    let mut drawn = vec![false; n];
    for i in 0..n {
        let node = &nodes[i];
        drawn[i] = node.icon.as_ref().is_some_and(|s| !m.icon_markup(s).is_empty());
        let fit = wrap.min(72.0_f64.max(room - depth[i] as f64 * 28.0 - 12.0 - if drawn[i] { glyph + 6.0 } else { 0.0 }));
        let label = layout_label(Some(&node.descr), true, m, size, fit, 0);
        let tw = label.width
            + if drawn[i] {
                glyph + if label.width > 0.0 { 6.0 } else { 0.0 }
            } else {
                0.0
            };
        let th = label.height.max(js_round(size * 1.5));
        let mut shape = ["", "rounded", "rect", "circle", "cloud", "bang", "hex"]
            .get(node.kind)
            .copied()
            .unwrap_or("");
        if shape.is_empty() && i == 0 {
            shape = "stadium"
        }
        if down && i == 0 && shape != "rect" && shape != "rounded" {
            shape = "stadium"
        }
        shapes[i] = shape;
        labels[i] = label;
        if shape.is_empty() {
            let pill = !node.children.is_empty();
            w[i] = tw + if pill { 24.0 } else { 12.0 };
            h[i] = th + if pill { 6.0 } else { 0.0 };
            anchor[i] = h[i] / 2.0
        } else {
            let s = shape_size(shape, tw, th);
            w[i] = s.w;
            h[i] = s.h;
            dy[i] = s.dy;
            anchor[i] = s.h / 2.0
        }
    }
    let mut extent = vec![0.0; n];
    let mut rel = vec![0.0; n];
    let mut own = vec![0.0; n];
    let mut kids = vec![0.0; n];
    for i in (1..n).rev() {
        let children = &nodes[i].children;
        if children.is_empty() {
            extent[i] = h[i];
            continue;
        }
        let (middle, stacked) = stack(children, 0, children.len(), 8.0, nodes, &extent, &own, &anchor, &mut rel);
        let top = middle - anchor[i];
        let low = 0.0_f64.min(top);
        extent[i] = stacked.max(top + h[i]) - low;
        own[i] = top - low;
        kids[i] = -low
    }
    let mut x = vec![0.0; n];
    let mut y = vec![0.0; n];
    let mut band = vec![0.0; n];
    let mut side = vec![0.0; n];
    let mut trunk = vec![0.0; n];
    if down {
        let mut row = 0.0;
        for i in 0..n {
            if i > 0 && parent[i] == 0 {
                row += 10.0
            }
            x[i] = depth[i] as f64 * 28.0 + w[i] / 2.0;
            y[i] = row;
            row += h[i] + 10.0
        }
    } else if n > 0 {
        let branches = &nodes[0].children;
        let count = branches.len();
        let total: f64 = branches.iter().map(|c| extent[*c]).sum();
        let mut split = count;
        let mut best = f64::INFINITY;
        let mut right = 0.0;
        for k in 1..count {
            right += extent[branches[k - 1]];
            let diff = (2.0 * right - total + (2.0 * k as f64 - count as f64) * 28.0).abs();
            if diff < best {
                best = diff;
                split = k
            }
        }
        for (from, to, dir) in [(0, split, 1.0), (split, count, -1.0)] {
            if from >= to {
                continue;
            }
            let (middle, _) = stack(branches, from, to, 28.0, nodes, &extent, &own, &anchor, &mut rel);
            for &c in &branches[from..to] {
                band[c] = rel[c] - middle;
                side[c] = dir
            }
        }
        y[0] = -h[0] / 2.0;
        for i in 0..n {
            if i > 0 {
                y[i] = band[i] + own[i]
            }
            for &c in &nodes[i].children {
                if i > 0 {
                    side[c] = side[i];
                    band[c] = band[i] + kids[i] + rel[c]
                }
                x[c] = x[i] + side[c] * (w[i] / 2.0 + if i > 0 { 36.0 } else { 52.0 } + w[c] / 2.0)
            }
        }
    }
    let mut min_x = 0.0_f64;
    let mut max_x = 0.0_f64;
    let mut min_y = 0.0_f64;
    let mut max_y = 0.0_f64;
    let mut out = String::new();
    let mut edges = String::new();
    let mut body = String::new();
    let mut color = String::new();
    for k in 1..=n {
        let i = k % n;
        let node = &nodes[i];
        let shape = shapes[i];
        let label = &labels[i];
        let cx = x[i];
        let top = y[i];
        min_x = min_x.min(cx - w[i] / 2.0);
        max_x = max_x.max(cx + w[i] / 2.0);
        min_y = min_y.min(top);
        max_y = max_y.max(top + h[i]);
        if (i == 0 || parent[i] == 0) && !body.is_empty() {
            out += &format!(
                "<g class=\"pele-branch pele-section-{}\"><g class=\"pele-edges\" fill=\"none\" stroke=\"{color}\" stroke-width=\"1.5\">{edges}</g>{body}</g>",
                nodes[k - 1].section.unwrap_or(0)
            );
            edges.clear();
            body.clear()
        }
        if i > 0 {
            if parent[i] == 0 {
                color = series_color(node.section.unwrap_or(0) as i64)
            }
            let p = parent[i];
            if down {
                let stem = if p == 0 { 12.0_f64.max(h[0] / 2.0).min(20.0) } else { 12.0 };
                let x1 = x[p] - w[p] / 2.0 + stem.min(w[p] / 2.0);
                let y1 = if trunk[p] != 0.0 { trunk[p] } else { y[p] + h[p] };
                let left = cx - w[i] / 2.0;
                let x2 = left
                    + if !shape.is_empty() {
                        shape_inset(shape, w[i], h[i], 3, 0.0)
                    } else {
                        0.0
                    };
                let y2 = top + anchor[i];
                let bend = 0.0_f64.max(8.0_f64.min(x2 - x1).min(y2 - y1));
                trunk[p] = y2 - bend;
                edges += &format!(
                    "<path class=\"pele-edge\" d=\"M{},{}V{}Q{},{} {},{}H{}\"/>",
                    num(x1),
                    num(y1),
                    num(y2 - bend),
                    num(x1),
                    num(y2),
                    num(x1 + bend),
                    num(y2),
                    num(x2)
                )
            } else {
                let dir = side[i];
                let from = if dir > 0.0 { 1 } else { 3 };
                let x1 = x[p]
                    + dir
                        * (w[p] / 2.0
                            - if !shapes[p].is_empty() {
                                shape_inset(shapes[p], w[p], h[p], from, 0.0)
                            } else {
                                0.0
                            });
                let y1 = y[p] + anchor[p];
                let near = cx - dir * w[i] / 2.0;
                let x2 = near
                    + if !shape.is_empty() {
                        dir * shape_inset(shape, w[i], h[i], 4 - from, 0.0)
                    } else {
                        0.0
                    };
                let y2 = top + anchor[i];
                let mid = num((x1 + x2) / 2.0);
                edges += &format!(
                    "<path class=\"pele-edge\" d=\"M{},{}C{mid},{} {mid},{} {},{}\"/>",
                    num(x1),
                    num(y1),
                    num(y1),
                    num(y2),
                    num(x2),
                    num(y2)
                )
            }
        }
        let pill = shape.is_empty() && !node.children.is_empty();
        let tint = format!(" fill=\"{color}\" fill-opacity=\"0.22\" stroke=\"{color}\"");
        let mut inner = if !shape.is_empty() {
            if i > 0 {
                draw_shape(shape, w[i], h[i], &tint, &format!(" stroke=\"{color}\""))
            } else {
                draw_shape(
                    shape,
                    w[i],
                    h[i],
                    " fill=\"var(--_s)\" stroke=\"var(--_b)\"",
                    " stroke=\"var(--_b)\"",
                )
            }
        } else if pill {
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{tint}/>",
                num(-w[i] / 2.0),
                num(-h[i] / 2.0),
                num(w[i]),
                num(h[i]),
                num(h[i] / 2.0)
            )
        } else {
            String::new()
        };
        let mut shift = 0.0;
        if drawn[i]
            && let Some(name) = &node.icon
        {
            shift = (glyph + if label.width > 0.0 { 6.0 } else { 0.0 }) / 2.0;
            inner += &icon_svg(name, -shift - label.width / 2.0, dy[i] - glyph / 2.0, glyph, o)
        }
        let group = format!(
            "<g class=\"pele-node pele-shape-{}{}{}\" data-id=\"{}\" transform=\"translate({},{})\">",
            if !shape.is_empty() {
                shape
            } else if pill {
                "branch"
            } else {
                "text"
            },
            if i > 0 { "" } else { " pele-root" },
            class_names(node.class.as_deref().unwrap_or("")),
            esc_text(&node.node_id),
            num(cx),
            num(top + h[i] / 2.0)
        ) + &inner
            + &if down && shape.is_empty() {
                left_label(label, shift - label.width / 2.0, dy[i])
            } else {
                label_svg(label, shift, dy[i], " class=\"pele-label\"")
            }
            + "</g>";
        if i > 0 { body += &group } else { out += &group }
    }
    let title = crate::svg::title::title_label(model.title.as_deref(), m, size, title_room(o));
    let title_h = if title.height > 0.0 { title.height + 16.0 } else { 0.0 };
    let width = (max_x - min_x).max(title.width);
    let tw = (width + 2.0 * pad).ceil();
    let th = (max_y - min_y + title_h + 2.0 * pad).ceil();
    let content = label_svg(
        &title,
        tw / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &format!(
        "<g transform=\"translate({},{})\">{out}</g>",
        num(pad - min_x + (width - max_x + min_x) / 2.0),
        num(pad + title_h - min_y)
    );
    Rendered {
        svg: svg_document(
            "mindmap",
            tw,
            th,
            size,
            o,
            &Accessible {
                acc_title: None,
                acc_descr: None,
            },
            &content,
        ),
        width: tw,
        height: th,
        links: Vec::new(),
    }
}
