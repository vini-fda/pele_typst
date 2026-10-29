use super::layout::{Circle, Spot, find_spot, layout, region_path};
use super::{Model, normalize};
use crate::RenderOptions;
use crate::diagrams::common::Rendered;
use crate::preprocess::Config;
use crate::svg::builder::{esc_text, label_svg};
use crate::svg::ink::ink;
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{ResolvedStyle, resolve_style, series_color};
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, ITALIC, TextMeasurer};
use crate::util::{js_num, js_round, num};
use indexmap::IndexMap;
struct Row {
    id: Option<String>,
    raw: String,
    size: f64,
    base: u32,
    label: Label,
}
struct Region {
    sets: Vec<String>,
    inside: Vec<usize>,
    spot: Spot,
    lines: Vec<Row>,
    style: ResolvedStyle,
    across: bool,
}
fn key(ids: &[String]) -> String {
    if ids.len() == 1 {
        ids[0].clone()
    } else {
        serde_json::to_string(ids).unwrap()
    }
}
pub fn render(model: &Model, _c: &Config, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let tiny = js_round(size * 0.8125);
    let pad = o.padding.unwrap_or(8.0);
    let circles = layout(&model.subsets);
    let index: std::collections::HashMap<&str, usize> = circles.iter().enumerate().map(|(i, c)| (c.id.as_str(), i)).collect();
    let mut styles: IndexMap<String, IndexMap<String, String>> = IndexMap::new();
    for style in &model.styles {
        let merged = styles.entry(key(&style.targets)).or_default();
        for (k, v) in &style.styles {
            merged.insert(k.clone(), v.clone());
        }
    }
    let style_of = |k: &str| {
        resolve_style(
            &styles
                .get(k)
                .map(|s| s.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>())
                .unwrap_or_default(),
        )
    };
    let mut neighbors = vec![Vec::new(); circles.len()];
    for a in 0..circles.len() {
        for b in a + 1..circles.len() {
            if (circles[a].x - circles[b].x).hypot(circles[a].y - circles[b].y) < circles[a].r + circles[b].r {
                neighbors[a].push(b);
                neighbors[b].push(a)
            }
        }
    }
    let mut stated = IndexMap::new();
    for s in &model.subsets {
        stated.insert(key(&s.sets), s);
    }
    let mut regions: IndexMap<String, Region> = IndexMap::new();
    for (k, s) in stated {
        let mut sets = Vec::new();
        for id in &s.sets {
            if !sets.contains(id) {
                sets.push(id.clone())
            }
        }
        let inside: Vec<usize> = sets.iter().map(|id| index[id.as_str()]).collect();
        let mut smallest = inside[0];
        for &i in &inside {
            if circles[i].r < circles[smallest].r {
                smallest = i
            }
        }
        let style = style_of(&k);
        let text = s
            .label
            .clone()
            .or_else(|| if s.sets.len() == 1 { Some(s.sets[0].clone()) } else { None });
        let base = if style.bold { BOLD } else { 0 } | if style.italic { ITALIC } else { 0 };
        let others: Vec<usize> = neighbors[smallest].iter().copied().filter(|i| !inside.contains(i)).collect();
        let mut spot = find_spot(&circles, &inside, &others);
        let across = !others.is_empty() && spot.margin < circles[smallest].r * 0.2;
        if across {
            spot = find_spot(&circles, &inside, &[])
        }
        let mut region = Region {
            sets,
            inside,
            spot,
            lines: Vec::new(),
            style,
            across,
        };
        if let Some(raw) = text.filter(|s| !s.is_empty()) {
            let fs = if region.sets.len() == 1 { size } else { small };
            let label = layout_label(Some(&raw), false, m, fs, 4000.0, base);
            region.lines.push(Row {
                id: None,
                raw,
                size: fs,
                base,
                label,
            })
        }
        regions.insert(k, region);
    }
    for node in &model.text_nodes {
        let raw = node.label.as_ref().unwrap_or(&node.id);
        if let Some(region) = regions.get_mut(&key(&node.sets)) {
            region.lines.push(Row {
                id: Some(node.id.clone()),
                raw: raw.clone(),
                size: tiny,
                base: 0,
                label: layout_label(Some(raw), false, m, tiny, 4000.0, 0),
            })
        }
    }
    let largest = circles.iter().map(|c| c.r).fold(0.0_f64, f64::max);
    let mut scale = if largest > 0.0 { 100.0 / largest } else { 1.0 };
    let mut growth = 1.0_f64;
    for region in regions.values() {
        if region.spot.margin <= 0.0 {
            continue;
        }
        let width = region.lines.iter().map(|l| l.label.width).fold(0.0_f64, f64::max);
        let height: f64 = region.lines.iter().map(|l| l.label.height).sum();
        let need = width.hypot(height) / 2.0 + 4.0;
        growth = growth.max((need / (region.spot.margin * scale)).min(1.6))
    }
    scale *= growth;
    for region in regions.values_mut() {
        let room = (region.spot.margin * scale * 1.8).max(72.0);
        for line in &mut region.lines {
            if line.label.width > room {
                line.label = layout_label(Some(&line.raw), false, m, line.size, room, line.base)
            }
        }
    }
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in &circles {
        minx = minx.min((c.x - c.r) * scale);
        miny = miny.min((c.y - c.r) * scale);
        maxx = maxx.max((c.x + c.r) * scale);
        maxy = maxy.max((c.y + c.r) * scale)
    }
    for r in regions.values() {
        let wide = r.lines.iter().map(|l| l.label.width).fold(0.0_f64, f64::max);
        let tall = r.lines.iter().map(|l| l.label.height).sum::<f64>();
        minx = minx.min(r.spot.x * scale - wide / 2.0);
        maxx = maxx.max(r.spot.x * scale + wide / 2.0);
        miny = miny.min(r.spot.y * scale - tall / 2.0);
        maxy = maxy.max(r.spot.y * scale + tall / 2.0)
    }
    if circles.is_empty() {
        minx = 0.0;
        miny = 0.0;
        maxx = 0.0;
        maxy = 0.0
    }
    let raw_title = model.title.as_deref().map(normalize);
    let title = layout_label(raw_title.as_deref(), false, m, size, 4000.0, BOLD);
    let title_h = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let inner = (maxx - minx).max(title.width);
    let ox = pad + (inner - (maxx - minx)) / 2.0 - minx;
    let oy = pad + title_h - miny;
    let width = (inner + 2.0 * pad).ceil();
    let height = (maxy - miny + title_h + 2.0 * pad).ceil();
    let placed: Vec<Circle> = circles
        .iter()
        .map(|c| Circle {
            id: c.id.clone(),
            x: c.x * scale + ox,
            y: c.y * scale + oy,
            r: c.r * scale,
        })
        .collect();
    let mut sets = String::new();
    for (i, c) in placed.iter().enumerate() {
        let color = series_color(i as i64);
        let style = regions.get(&c.id).map(|r| r.style.clone()).unwrap_or_else(|| resolve_style(&[]));
        sets += &format!(
            "<circle class=\"pele-venn-set\" data-id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{color}\" fill-opacity=\"0.2\" stroke=\"{color}\"{}/>",
            esc_text(&c.id),
            num(c.x),
            num(c.y),
            num(c.r),
            style.shape
        )
    }
    let painted: Vec<_> = regions.values().filter(|r| r.sets.len() > 1 && !r.style.shape.is_empty()).collect();
    let under = |r: &Region| {
        if r.across {
            return String::new();
        }
        let mut fill = if r.sets.len() == 1 && (r.style.shape.contains("\"fill-opacity:") || r.style.shape.contains(";fill-opacity:")) {
            r.style.shape.clone()
        } else {
            String::new()
        };
        for other in &painted {
            if other.sets.iter().all(|s| r.sets.contains(s)) {
                fill = other.style.shape.clone()
            }
        }
        fill
    };
    let mut overlaps = String::new();
    let mut labels = String::new();
    for r in regions.values() {
        let id = esc_text(&r.sets.join("|"));
        if r.sets.len() > 1 && !r.style.shape.is_empty() {
            let cs: Vec<_> = r.inside.iter().map(|&i| placed[i].clone()).collect();
            let d = region_path(&cs);
            if !d.is_empty() {
                overlaps += &format!(
                    "<path class=\"pele-venn-overlap\" data-id=\"{id}\" d=\"{d}\" fill=\"none\"{}/>",
                    r.style.shape
                )
            }
        }
        let total = r.lines.iter().map(|l| l.label.height).sum::<f64>();
        if total == 0.0 {
            continue;
        }
        let x = r.spot.x * scale + ox;
        let mut y = r.spot.y * scale + oy - total / 2.0;
        let fill = under(r);
        let mut content = String::new();
        for line in &r.lines {
            let own = line
                .id
                .as_deref()
                .map(|id| style_of(id).text)
                .unwrap_or_else(|| r.style.text.clone());
            let color = ink(&fill, &own);
            let attrs = if let Some(id) = &line.id {
                format!(
                    " class=\"pele-venn-text\" data-id=\"{}\" font-size=\"{}\" fill=\"{}\"{own}",
                    esc_text(id),
                    js_num(tiny),
                    color.unwrap_or("var(--_m)")
                )
            } else {
                format!(
                    " class=\"pele-label\"{}{}{own}",
                    if line.size == size {
                        String::new()
                    } else {
                        format!(" font-size=\"{}\"", js_num(line.size))
                    },
                    color.map(|c| format!(" fill=\"{c}\"")).unwrap_or_default()
                )
            };
            content += &label_svg(&line.label, x, y + line.label.height / 2.0, &attrs);
            y += line.label.height
        }
        labels += &format!("<g class=\"pele-venn-region\" data-id=\"{id}\">{content}</g>")
    }
    let content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &format!(
        "<g class=\"pele-venn-sets\">{sets}</g>{}{}",
        if overlaps.is_empty() {
            overlaps
        } else {
            format!("<g class=\"pele-venn-overlaps\">{overlaps}</g>")
        },
        if labels.is_empty() {
            labels
        } else {
            format!("<g class=\"pele-venn-labels\">{labels}</g>")
        }
    );
    Rendered {
        svg: svg_document(
            "venn",
            width,
            height,
            size,
            o,
            &Accessible {
                acc_title: None,
                acc_descr: None,
            },
            &content,
        ),
        width,
        height,
        links: Vec::new(),
    }
}
