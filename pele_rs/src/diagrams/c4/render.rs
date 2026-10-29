use super::{
    layout::{HEAD_LEFT, HEAD_TOP, layout},
    model::Model,
    route::{self, HEAD, LABEL_PAD, LANE, Lanes},
};
use crate::{
    Config, RenderOptions, TextMeasurer,
    diagrams::{
        common::{Rendered, tighten, title_room},
        treeview::literal::left_label as draw_left_label,
    },
    svg::{
        builder::{esc, esc_text, label_svg as draw_label_svg},
        edges::{ARROW, EdgePath, Fill, casings_svg, edge_casing, marker, marker_trim, over_fills},
        ink::{ink_fill, paints},
        root::{Accessible, svg_document},
        shapes::draw_shape,
        theme::{ResolvedStyle, resolve_style, rounded},
    },
    text::{
        label::{Label, layout_label},
        measurer::{BOLD, ITALIC},
    },
    url::{LinkInfo, linked, sanitize_url},
    util::{js_round, num},
};
use std::collections::{HashMap, HashSet};
// C4's upstream SVG calls omit the icon drawing callback while layout still
// probes icons. Preserve that observable distinction, including callback counts.
fn without_icon_drawing(label: &Label) -> Option<Label> {
    if !label.lines.iter().flatten().any(|s| s.icon.is_some()) {
        return None;
    }
    let mut out = label.clone();
    for span in out.lines.iter_mut().flatten() {
        span.icon_markup = None;
        span.icon_resolver = None;
    }
    Some(out)
}
fn label_svg(label: &Label, x: f64, y: f64, attrs: &str) -> String {
    let plain = without_icon_drawing(label);
    draw_label_svg(plain.as_ref().unwrap_or(label), x, y, attrs)
}
fn left_label(label: &Label, x: f64, y: f64, attrs: &str) -> String {
    let plain = without_icon_drawing(label);
    draw_left_label(plain.as_ref().unwrap_or(label), x, y, attrs)
}
fn user_style(fill: Option<&str>, stroke: Option<&str>, color: Option<&str>) -> ResolvedStyle {
    let mut d = vec![];
    for (field, val) in [("fill", fill), ("stroke", stroke), ("color", color)] {
        if let Some(s) = val.filter(|s| !s.is_empty()) {
            d.push(format!("{field}:{s}"));
        }
    }
    resolve_style(&d)
}
fn link(body: String, url: Option<&str>, id: &str, links: &mut Vec<LinkInfo>, options: &RenderOptions) -> String {
    let Some(url) = url.filter(|s| !s.is_empty()) else { return body };
    let url = if url.contains("¶ß") {
        url.replace("ﬂ°°", "#").replace("ﬂ°", "#").replace("¶ß", ";")
    } else {
        url.to_owned()
    };
    linked(&body, &sanitize_url(&url), id, links, options, "")
}
fn covered(taken: &[f64], x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let mut area = 0.;
    for b in taken.as_chunks::<4>().0 {
        let w = x1.min(b[2]) - x0.max(b[0]);
        if w <= 0. {
            continue;
        }
        let h = y1.min(b[3]) - y0.max(b[1]);
        if h > 0. {
            area += w * h;
        }
    }
    area
}
fn offset(v: Option<f64>) -> f64 {
    v.filter(|v| *v != 0. && !v.is_nan()).unwrap_or(0.).max(-2000.).min(2000.)
}
fn grow(bounds: &mut [f64; 4], x0: f64, y0: f64, x1: f64, y1: f64) {
    bounds[0] = bounds[0].min(x0);
    bounds[1] = bounds[1].min(y0);
    bounds[2] = bounds[2].max(x1);
    bounds[3] = bounds[3].max(y1);
}
pub fn render(db: &Model, config: &Config, options: &RenderOptions, m: &dyn TextMeasurer) -> Result<Rendered, crate::PeleError> {
    super::validate(db)?;
    Ok(tighten(options, |tight| {
        if !tight {
            return draw(db, config, options, m, false, 36.).0;
        }
        let (mut best, mut squeezed) = draw(db, config, options, m, true, 36.);
        for gap in [60., 84., 108.] {
            if squeezed == 0 {
                return best;
            }
            let (wider, left) = draw(db, config, options, m, true, gap);
            if left < squeezed {
                best = wider;
                squeezed = left;
            }
        }
        best
    }))
}
fn draw(db: &Model, _config: &Config, options: &RenderOptions, m: &dyn TextMeasurer, tight: bool, gap: f64) -> (Rendered, usize) {
    let size = options.font_size.unwrap_or(16.);
    let small = js_round(size * 0.8125);
    let pad = options.padding.unwrap_or(8.);
    let mut links = vec![];
    let mut layout = layout(
        db,
        m,
        size,
        small,
        if tight {
            Some((options.max_width.unwrap_or(320.) - 2. * pad).max(80.))
        } else {
            None
        },
        gap,
    );
    let mut bounds = [0., 0., layout.groups[0].w, layout.groups[0].h];
    let mut ends = vec![];
    let mut pairs: HashMap<usize, HashSet<usize>> = HashMap::new();
    for rel in &db.rels {
        let a = (!rel.from_attr.is_some())
            .then(|| db.get(&rel.from))
            .flatten()
            .and_then(|r| layout.boxes.get(&r));
        let b = (!rel.to_attr.is_some())
            .then(|| db.get(&rel.to))
            .flatten()
            .and_then(|r| layout.boxes.get(&r));
        let (Some(a), Some(b)) = (a, b) else { continue };
        ends.push((a.clone(), b.clone()));
        pairs.entry(a.index).or_default().insert(b.index);
    }
    let mut nodes = vec![];
    let mut taken = vec![];
    for &i in &layout.order {
        for &n in &layout.groups[i].nodes {
            let node = &layout.nodes[n];
            nodes.push(node.bx.clone());
            taken.extend_from_slice(&[node.x + 8., node.y + 8., node.x + node.w - 8., node.y + node.h - 8.]);
        }
    }
    let careful = db.rels.len() * (nodes.len() + layout.order.len() + 3 * db.rels.len()) <= 2_000_000;
    let (mut casings, mut edges_out) = (String::new(), String::new());
    let trim = marker_trim("arrow_point");
    let mut fills = vec![];
    for &i in &layout.order {
        if i == 0 {
            continue;
        }
        let g = &layout.groups[i];
        let shape = user_style(db.boundaries[g.boundary].get("bgColor"), None, None).shape;
        if paints(&shape) {
            fills.push(Fill {
                bx: [g.x, g.y, g.x + g.w, g.y + g.h],
                shape,
            });
        }
    }
    let mut lanes = Lanes::default();
    for &i in &layout.order {
        if i != 0 {
            lanes.frame(&layout.groups[i].bx);
        }
    }
    let mut routes = vec![];
    for (i, rel) in db.rels.iter().enumerate() {
        let (a, b) = &ends[i];
        let style = user_style(None, rel.get("lineColor"), None);
        let start = matches!(rel.kind.as_str(), "birel" | "rel_b");
        let end = rel.kind != "rel_b";
        let reverse = a.index != b.index && pairs.get(&b.index).is_some_and(|p| p.contains(&a.index));
        let r = route::route(
            a,
            b,
            if reverse { 7. } else { 0. },
            if careful { Some(nodes.as_slice()) } else { None },
            &mut lanes,
            if start { trim } else { 0. },
            if end { trim } else { 0. },
        );
        grow(&mut bounds, r.x0.min(r.x1), r.y0.min(r.y1), r.x0.max(r.x1), r.y0.max(r.y1));
        if start && careful {
            taken.extend_from_slice(&[r.sx - ARROW, r.sy - ARROW, r.sx + ARROW, r.sy + ARROW]);
        }
        if end && careful {
            taken.extend_from_slice(&[r.ex - ARROW, r.ey - ARROW, r.ex + ARROW, r.ey + ARROW]);
        }
        let over = over_fills(&r.d, &[r.sx, r.sy, r.ex, r.ey], &fills, &style.line, None);
        let color = over
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| style.stroke.as_ref().map(|s| esc(s)).unwrap_or_else(|| "var(--_l)".into()));
        if over.as_deref() == Some("") {
            let path = EdgePath {
                d: r.d.clone(),
                sx: r.sx,
                sy: r.sy,
                sdx: r.sdx,
                sdy: r.sdy,
                ex: r.ex,
                ey: r.ey,
                edx: r.edx,
                edy: r.edy,
                ..Default::default()
            };
            casings += &edge_casing(
                &path,
                if start { "arrow_point" } else { "none" },
                if end { "arrow_point" } else { "none" },
                &style.line,
                false,
            );
        }
        edges_out += &format!(
            "<g class=\"pele-edge\" data-id=\"{}\"{}{}><path d=\"{}\"/>{}{}</g>",
            esc_text(&format!("{}-{}", rel.from, rel.to)),
            over.as_ref()
                .filter(|s| !s.is_empty())
                .map_or(String::new(), |s| format!(" stroke=\"{s}\"")),
            style.line,
            r.d,
            if start {
                marker("arrow_point", r.sx, r.sy, r.sdx, r.sdy, &color, "none")
            } else {
                String::new()
            },
            if end {
                marker("arrow_point", r.ex, r.ey, r.edx, r.edy, &color, "none")
            } else {
                String::new()
            }
        );
        routes.push(r);
    }
    for &i in &layout.order {
        let g = &mut layout.groups[i];
        if !careful || g.head_w == 0. {
            continue;
        }
        let top = g.y + HEAD_TOP;
        let bottom = g.y + g.head - LANE;
        let mut fewest = usize::MAX;
        for x in [HEAD_LEFT, (g.w - g.head_w) / 2., g.w - HEAD_LEFT - g.head_w] {
            let count = routes
                .iter()
                .filter(|r| route::crosses(&r.points, g.x + x - 4., top, g.x + x + g.head_w + 4., bottom))
                .count();
            if count < fewest {
                fewest = count;
                g.head_x = x;
            }
            if count == 0 {
                break;
            }
        }
        taken.extend_from_slice(&[g.x + g.head_x - 4., top, g.x + g.head_x + g.head_w + 4., bottom]);
    }
    let mut labels_out = String::new();
    for (i, rel) in db.rels.iter().enumerate() {
        let r = &routes[i];
        let id = format!("{}-{}", rel.from, rel.to);
        let style = user_style(None, None, rel.get("textColor"));
        let text = if db.c4_type.as_deref() == Some("C4Dynamic") {
            format!("{}: {}", i + 1, rel.label)
        } else {
            rel.label.clone()
        };
        let label = layout_label(Some(&text), false, m, small, r.room, 0);
        let techn = rel.get("techn").filter(|s| !s.is_empty()).map(|s| format!("[{s}]"));
        let techn = layout_label(techn.as_deref(), false, m, small, r.room, ITALIC);
        let lw = label.width.max(techn.width) + LABEL_PAD;
        let lh = label.height + techn.height;
        if lh == 0. {
            continue;
        }
        let beside = r.nx.abs() * (lw / 2.) + r.ny.abs() * (lh / 2.) + 2.;
        let aside = if r.lean != 0. { r.lean.min(beside - 14.).max(0.) } else { beside };
        let mut dx = r.nx * aside + if tight { 0. } else { offset(rel.offset_x) };
        let mut dy = r.ny * aside + if tight { 0. } else { offset(rel.offset_y) };
        let mut at = 0.5;
        if careful {
            let mut least = f64::INFINITY;
            let moved = dx != r.nx * aside || dy != r.ny * aside;
            for pass in 0..if moved { 2 } else { 1 } {
                if least <= 0. {
                    break;
                }
                let (px, py) = if pass > 0 { (r.nx * aside, r.ny * aside) } else { (dx, dy) };
                for t in [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8] {
                    let cx = r.x0 + (r.x1 - r.x0) * t + px;
                    let cy = r.y0 + (r.y1 - r.y0) * t + py;
                    let area = covered(&taken, cx - lw / 2., cy - lh / 2., cx + lw / 2., cy + lh / 2.);
                    if area < least {
                        least = area;
                        at = t;
                        dx = px;
                        dy = py;
                    }
                    if area == 0. {
                        break;
                    }
                }
            }
        }
        let cx = r.x0 + (r.x1 - r.x0) * at + dx;
        let top = r.y0 + (r.y1 - r.y0) * at + dy - lh / 2.;
        if careful {
            taken.extend_from_slice(&[cx - lw / 2., top, cx + lw / 2., top + lh]);
        }
        grow(&mut bounds, cx - lw / 2., top, cx + lw / 2., top + lh);
        labels_out += &format!(
            "<g class=\"pele-edge-label\" data-id=\"{}\">{}<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\"/>{}</g>",
            esc_text(&id),
            rel.get("descr")
                .filter(|s| !s.is_empty())
                .map_or(String::new(), |s| format!("<title>{}</title>", esc_text(s))),
            num(cx - lw / 2.),
            num(top),
            num(lw),
            num(lh),
            link(
                label_svg(&label, cx, top + label.height / 2., &style.text)
                    + &label_svg(
                        &techn,
                        cx,
                        top + label.height + techn.height / 2.,
                        &format!(" class=\"pele-c4-techn\" fill=\"var(--_m)\"{}", style.text)
                    ),
                rel.get("link"),
                &id,
                &mut links,
                options
            )
        );
    }
    let (mut clusters, mut nodes_out) = (String::new(), String::new());
    for &i in &layout.order {
        let g = &layout.groups[i];
        if i != 0 {
            let boundary = &db.boundaries[g.boundary];
            let style = user_style(boundary.get("bgColor"), boundary.get("borderColor"), boundary.get("fontColor"));
            let x = g.x + g.head_x;
            let mut y = g.y + HEAD_TOP;
            let color = ink_fill(&style.shape, &style.text, None) + &style.text;
            let muted = ink_fill(&style.shape, &style.text, Some("var(--_m)")) + &style.text;
            let mut text = left_label(
                &g.labels[0],
                x,
                y,
                &format!(" class=\"pele-cluster-label\" font-weight=\"var(--_hw)\"{color}"),
            );
            y += g.labels[0].height;
            text += &left_label(
                &g.labels[1],
                x,
                y,
                &format!(" class=\"pele-c4-type\" font-size=\"{}\"{muted}", num(small)),
            );
            y += g.labels[1].height;
            text += &left_label(
                &g.labels[2],
                x,
                y,
                &format!(" class=\"pele-c4-descr\" font-size=\"{}\"{muted}", num(small)),
            );
            clusters += &format!(
                "<g class=\"pele-cluster pele-c4-{}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"var(--_b)\"{}{}/>{}</g>",
                if boundary.node_type.is_some() { "node" } else { "boundary" },
                esc_text(&boundary.alias),
                num(g.x),
                num(g.y),
                num(g.w),
                num(g.h),
                if boundary.node_type.is_some() {
                    ""
                } else {
                    " stroke-dasharray=\"6 4\""
                },
                rounded(&style.shape),
                link(text, boundary.get("link"), &boundary.alias, &mut links, options)
            );
        }
        for &n in &g.nodes {
            let node = &layout.nodes[n];
            let shape = &db.shapes[node.shape];
            let external = shape.kind.starts_with("external");
            let style = user_style(shape.get("bgColor"), shape.get("borderColor"), shape.get("fontColor"));
            let stroke = format!(" stroke=\"var(--_b)\"{}", if external { " stroke-dasharray=\"4 3\"" } else { "" });
            let (w, h, form, labels) = (node.w, node.h, node.form.as_str(), &node.labels);
            let dx = if form == "h-cyl" { -route::queue_rx(h) / 2. } else { 0. };
            let dy = if form == "person" {
                HEAD / 2.
            } else if form == "cyl" {
                route::cyl_ry(w) / 2.
            } else {
                0.
            };
            let color = ink_fill(&style.shape, &style.text, None) + &style.text;
            let muted = ink_fill(&style.shape, &style.text, Some("var(--_m)")) + &style.text;
            let mut y = dy - node.text_h / 2.;
            let mut text = label_svg(
                &labels[0],
                dx,
                y + labels[0].height / 2.,
                &format!(" class=\"pele-c4-type\" font-size=\"{}\"{muted}", num(small - 1.)),
            );
            y += labels[0].height;
            text += &label_svg(
                &labels[1],
                dx,
                y + labels[1].height / 2.,
                &format!(" class=\"pele-label\" font-weight=\"var(--_hw)\"{color}"),
            );
            y += labels[1].height;
            text += &label_svg(
                &labels[2],
                dx,
                y + labels[2].height / 2.,
                &format!(" class=\"pele-c4-techn\" font-size=\"{}\"{muted}", num(small)),
            );
            y += labels[2].height + 4.;
            text += &label_svg(
                &labels[3],
                dx,
                y + labels[3].height / 2.,
                &format!(" class=\"pele-c4-descr\" font-size=\"{}\"{color}", num(small)),
            );
            let body = draw_shape(
                form,
                w,
                h,
                &format!(" fill=\"var({})\"{stroke}{}", if external { "--_bg" } else { "--_s" }, style.shape),
                &(stroke + &style.line),
            ) + &text;
            nodes_out += &format!(
                "<g class=\"pele-node pele-shape-{form} pele-c4-{}{}\" data-id=\"{}\" transform=\"translate({},{})\">{}</g>",
                shape.kind,
                if external { " pele-c4-external" } else { "" },
                esc_text(&shape.alias),
                num(node.x + w / 2.),
                num(node.y + h / 2.),
                link(body, shape.get("link"), &shape.alias, &mut links, options)
            );
        }
    }
    let title = layout_label(
        db.title.as_deref(),
        false,
        m,
        size,
        if tight { title_room(options) } else { 4000. },
        BOLD,
    );
    let title_h = if title.height > 0. { title.height + 12. } else { 0. };
    let [minx, miny, maxx, maxy] = bounds;
    let inner = (maxx - minx).max(title.width);
    let width = (inner + 2. * pad).ceil();
    let height = (maxy - miny + title_h + 2. * pad).ceil();
    let ox = pad - minx + (inner - (maxx - minx)) / 2.;
    let oy = pad + title_h - miny;
    let mut content = label_svg(
        &title,
        width / 2.,
        pad + title.height / 2.,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &format!("<g transform=\"translate({},{})\">", num(ox), num(oy));
    if !clusters.is_empty() {
        content += &format!("<g class=\"pele-clusters\">{clusters}</g>");
    }
    content += &casings_svg(&casings);
    if !edges_out.is_empty() {
        content += &format!("<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>");
    }
    if !nodes_out.is_empty() {
        content += &format!("<g class=\"pele-nodes\">{nodes_out}</g>");
    }
    if !labels_out.is_empty() {
        content += &format!("<g class=\"pele-edge-labels\" font-size=\"{}\">{labels_out}</g>", num(small));
    }
    content += "</g>";
    let svg = svg_document(
        "c4",
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
    (Rendered { svg, width, height, links }, lanes.squeezed)
}
