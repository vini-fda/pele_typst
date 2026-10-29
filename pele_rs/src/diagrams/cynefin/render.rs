use super::{Model, boundaries};
use crate::RenderOptions;
use crate::diagrams::common::Rendered;
use crate::preprocess::Config;
use crate::svg::builder::{esc_text, label_svg};
use crate::svg::edges::{marker, marker_trim};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{ROUNDED, series_color};
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, ITALIC, TextMeasurer};
use crate::util::{js_num, js_round, num};
use std::collections::HashMap;

const QUADRANTS: [(&str, f64, f64, i64); 4] = [
    ("complex", 0.0, 0.0, 0),
    ("complicated", 1.0, 0.0, 1),
    ("chaotic", 0.0, 1.0, 3),
    ("clear", 1.0, 1.0, 2),
];
struct Badge {
    text: String,
    label: Label,
    w: f64,
    h: f64,
    overflow: bool,
}
struct Block {
    name: String,
    title: Label,
    subtitles: Vec<Label>,
    badges: Vec<Badge>,
    w: f64,
    h: f64,
    cx: f64,
    cy: f64,
}
fn setting(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    config
        .get("cynefin")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_f64())
        .filter(|&v| v >= min && v <= max)
        .unwrap_or(fallback)
}
fn block(name: &str, model: &Model, measurer: &dyn TextMeasurer, size: f64, small: f64, item_size: f64, show: bool) -> Block {
    let meta = match name {
        "complex" => ["Complex", "Probe → Sense → Respond", "Emergent Practices"],
        "complicated" => ["Complicated", "Sense → Analyse → Respond", "Good Practices"],
        "clear" => ["Clear", "Sense → Categorise → Respond", "Best Practices"],
        "chaotic" => ["Chaotic", "Act → Sense → Respond", "Novel Practices"],
        _ => ["Confusion", "", "Disorder"],
    };
    let title = layout_label(Some(meta[0]), false, measurer, size, 4000.0, BOLD);
    let subtitles: Vec<_> = if show {
        meta[1..]
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| layout_label(Some(s), false, measurer, small, 4000.0, ITALIC))
            .collect()
    } else {
        vec![]
    };
    let items = model.domains.get(name).map(Vec::as_slice).unwrap_or(&[]);
    let shown = if name == "confusion" { items.len().min(3) } else { items.len() };
    let badge = |text: String, overflow| {
        let label = layout_label(Some(&text), false, measurer, item_size, 200.0, 0);
        let w = label.width + 20.0;
        let h = label.height.max(js_round(item_size * 1.5)) + 8.0;
        Badge {
            text,
            label,
            w,
            h,
            overflow,
        }
    };
    let mut badges: Vec<_> = items[..shown].iter().map(|s| badge(s.clone(), false)).collect();
    if shown < items.len() {
        badges.push(badge(format!("+{} more", items.len() - shown), true));
    }
    let mut w = title.width;
    let mut h = title.height;
    for s in &subtitles {
        w = w.max(s.width);
        h += s.height;
    }
    if !badges.is_empty() {
        h += 4.0;
    }
    for b in &badges {
        w = w.max(b.w);
        h += 6.0 + b.h;
    }
    Block {
        name: name.into(),
        title,
        subtitles,
        badges,
        w,
        h,
        cx: 0.0,
        cy: 0.0,
    }
}
fn draw_block(b: &Block, small: f64, item_size: f64) -> String {
    let mut y = b.cy - b.h / 2.0;
    let mut out = label_svg(
        &b.title,
        b.cx,
        y + b.title.height / 2.0,
        " class=\"pele-label\" font-weight=\"var(--_hw)\"",
    );
    y += b.title.height;
    for s in &b.subtitles {
        out += &label_svg(
            s,
            b.cx,
            y + s.height / 2.0,
            &format!(
                " class=\"pele-cynefin-subtitle\" font-size=\"{}\" font-style=\"italic\" fill=\"var(--_m)\"",
                js_num(small)
            ),
        );
        y += s.height;
    }
    y += 4.0;
    for item in &b.badges {
        y += 6.0;
        out += &format!(
            "<g class=\"pele-node{}\"{}><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{}{ROUNDED}/>{}</g>",
            if item.overflow { " pele-cynefin-overflow" } else { "" },
            if item.overflow {
                String::new()
            } else {
                format!(" data-id=\"{}\"", esc_text(&item.text))
            },
            num(b.cx - item.w / 2.0),
            num(y),
            num(item.w),
            num(item.h),
            if item.overflow { " stroke-dasharray=\"3 2\"" } else { "" },
            label_svg(
                &item.label,
                b.cx,
                y + item.h / 2.0,
                &format!(
                    " class=\"pele-label\" font-size=\"{}\"{}",
                    js_num(item_size),
                    if item.overflow { " fill=\"var(--_m)\"" } else { "" }
                )
            )
        );
        y += item.h;
    }
    format!("<g class=\"pele-cynefin-domain\" data-id=\"{}\">{out}</g>", b.name)
}
fn at(curve: &[f64], end: bool) -> String {
    let i = if end { curve.len() - 2 } else { 0 };
    format!("{},{}", num(curve[i]), num(curve[i + 1]))
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    let size = options.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.8125);
    let item_size = js_round(size * 0.875);
    let pad = options.padding.unwrap_or(8.0);
    let show = config
        .get("cynefin")
        .and_then(|v| v.get("showDomainDescriptions"))
        .and_then(|v| v.as_bool())
        != Some(false);
    let amplitude = setting(config, "boundaryAmplitude", 8.0, 0.0, 50.0);
    let seed = boundaries::seed(
        setting(config, "seed", 0.0, -1e15, 1e15),
        options.id_prefix.as_deref().unwrap_or("pele"),
    );
    let mut blocks: Vec<_> = QUADRANTS
        .iter()
        .map(|q| block(q.0, model, measurer, size, small, item_size, show))
        .collect();
    let confusion = block("confusion", model, measurer, size, small, item_size, show);
    let rx = 88.0_f64.max(confusion.w * 0.71 + 8.0);
    let ry = 64.0_f64.max(confusion.h * 0.71 + 6.0);
    let mut qw = setting(config, "width", 640.0, 100.0, 10000.0) / 2.0;
    let mut qh = setting(config, "height", 440.0, 100.0, 10000.0) / 2.0;
    for b in &blocks {
        qw = qw.max(b.w + 40.0);
        qh = qh.max(b.h + 40.0);
    }
    for b in &blocks {
        let dx = (qw - b.w) / 2.0;
        let dy = (qh - b.h) / 2.0;
        if (dx / (rx + 10.0)).powi(2) + (dy / (ry + 10.0)).powi(2) >= 1.0 {
            continue;
        }
        let wider = b.w + 2.0 * (rx + 10.0);
        let taller = b.h + 2.0 * (ry + 10.0);
        if wider * qh <= qw * taller {
            qw = qw.max(wider);
        } else {
            qh = qh.max(taller);
        }
    }
    let title = layout_label(model.title.as_deref(), false, measurer, size, 4000.0, BOLD);
    let title_height = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let width = 2.0 * qw;
    let height = 2.0 * qh;
    let x0 = pad;
    let y0 = pad + title_height;
    let cx = x0 + qw;
    let cy = y0 + qh;
    let right = x0 + width;
    let bottom = y0 + height;
    let fold = boundaries::wave(y0, cy, cx, seed, amplitude, true, false, true);
    let west = boundaries::wave(x0, cx, cy, seed + 100.0, amplitude, false, false, true);
    let east = boundaries::wave(cx, right, cy, seed + 200.0, amplitude, false, true, false);
    let drop = boundaries::cliff(cx, cy, bottom, width * 0.03);
    let regions = [
        format!(
            "M{},{}L{}{}{}Z",
            num(x0),
            num(y0),
            at(&fold, false),
            boundaries::forward(&fold),
            boundaries::backward(&west)
        ),
        format!(
            "M{}L{},{}L{}{}{}Z",
            at(&fold, false),
            num(right),
            num(y0),
            at(&east, true),
            boundaries::backward(&east),
            boundaries::backward(&fold)
        ),
        format!(
            "M{}{}{}L{},{}Z",
            at(&west, false),
            boundaries::forward(&west),
            boundaries::forward(&drop),
            num(x0),
            num(bottom)
        ),
        format!(
            "M{}{}L{},{}L{}{}Z",
            at(&east, false),
            boundaries::forward(&east),
            num(right),
            num(bottom),
            at(&drop, true),
            boundaries::backward(&drop)
        ),
    ];
    let mut fills = String::new();
    for (i, &(name, _, _, color)) in QUADRANTS.iter().enumerate() {
        fills += &format!(
            "<path class=\"pele-cynefin-region\" data-id=\"{name}\" d=\"{}\" fill=\"{}\" fill-opacity=\"0.14\"/>",
            regions[i],
            series_color(color)
        );
    }
    let boundaries = format!(
        "<g class=\"pele-cynefin-boundaries\" fill=\"none\" stroke=\"var(--_b)\" stroke-dasharray=\"6 3\"><path d=\"{}\"/><path d=\"{}\"/><path d=\"{}\"/><path class=\"pele-cynefin-cliff\" d=\"{}\" stroke=\"var(--_l)\" stroke-width=\"2\" stroke-dasharray=\"none\" stroke-linecap=\"round\"/></g>",
        boundaries::path(&fold),
        boundaries::path(&west),
        boundaries::path(&east),
        boundaries::path(&drop)
    );
    let oval = boundaries::ellipse(cx, cy, rx, ry);
    let ellipse = format!(
        "<path d=\"{oval}\" fill=\"var(--_bg)\"/><path class=\"pele-cynefin-region\" data-id=\"confusion\" d=\"{oval}\" fill=\"{}\" fill-opacity=\"0.18\" stroke=\"var(--_b)\" stroke-dasharray=\"4 2\"/>",
        series_color(4)
    );
    let mut domains = String::new();
    for (i, &(_, col, row, _)) in QUADRANTS.iter().enumerate() {
        blocks[i].cx = x0 + qw * (col + 0.5);
        blocks[i].cy = y0 + qh * (row + 0.5);
        domains += &draw_block(&blocks[i], small, item_size);
    }
    blocks.push(Block { cx, cy, ..confusion });
    domains += &draw_block(&blocks[4], small, item_size);
    let inside = |b: &Block, x: f64, y: f64| {
        if b.name == "confusion" {
            ((x - cx) / (rx + 4.0)).powi(2) + ((y - cy) / (ry + 4.0)).powi(2) <= 1.0
        } else {
            (x - b.cx).abs() <= b.w / 2.0 + 8.0 && (y - b.cy).abs() <= b.h / 2.0 + 8.0
        }
    };
    let mut edges = String::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for transition in &model.transitions {
        let Some(from) = blocks.iter().find(|b| b.name == transition.from) else {
            continue;
        };
        let Some(to) = blocks.iter().find(|b| b.name == transition.to) else {
            continue;
        };
        if from.name == to.name {
            continue;
        }
        let key = if transition.from < transition.to {
            format!("{} {}", transition.from, transition.to)
        } else {
            format!("{} {}", transition.to, transition.from)
        };
        let repeat = *seen.get(&key).unwrap_or(&0);
        seen.insert(key, repeat + 1);
        let dx = to.cx - from.cx;
        let dy = to.cy - from.cy;
        let len = dx.hypot(dy);
        let len = if len == 0.0 { 1.0 } else { len };
        let mx = (from.cx + to.cx) / 2.0;
        let my = (from.cy + to.cy) / 2.0;
        let outward = if -dy * (mx - cx) + dx * (my - cy) >= 0.0 { 1.0 } else { -1.0 };
        let nx = (-dy / len) * outward;
        let ny = (dx / len) * outward;
        let diagonal = from.name != "confusion" && to.name != "confusion" && dx.abs() > 1.0 && dy.abs() > 1.0;
        let bow = if diagonal {
            2.0 * ((rx * nx).hypot(ry * ny) + 16.0)
        } else {
            len * 0.15
        } + repeat as f64 * 56.0;
        let px = mx + nx * bow;
        let py = my + ny * bow;
        let point = |t: f64| {
            let u = 1.0 - t;
            (
                u * u * from.cx + 2.0 * u * t * px + t * t * to.cx,
                u * u * from.cy + 2.0 * u * t * py + t * t * to.cy,
            )
        };
        let crossing = |b: &Block, mut lo: f64, mut hi: f64, leaving: bool| {
            for _ in 0..16 {
                let mid = (lo + hi) / 2.0;
                let (x, y) = point(mid);
                if inside(b, x, y) == leaving {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (lo + hi) / 2.0
        };
        let mut i = 1;
        while i <= 48 {
            let (x, y) = point(i as f64 / 48.0);
            if !inside(from, x, y) {
                break;
            }
            i += 1;
        }
        if i > 48 {
            continue;
        }
        let t0 = crossing(from, (i - 1) as f64 / 48.0, i as f64 / 48.0, true);
        while i <= 48 {
            let (x, y) = point(i as f64 / 48.0);
            if inside(to, x, y) {
                break;
            }
            i += 1;
        }
        let t1 = if i > 48 {
            1.0
        } else {
            crossing(to, (i - 1) as f64 / 48.0, i as f64 / 48.0, false)
        };
        let (sx, sy) = point(t0);
        let (mut ex, mut ey) = point(t1);
        let blossom = |a: f64, b: f64, c: f64| (1.0 - t0) * (1.0 - t1) * a + ((1.0 - t0) * t1 + t0 * (1.0 - t1)) * b + t0 * t1 * c;
        let qx = blossom(from.cx, px, to.cx);
        let qy = blossom(from.cy, py, to.cy);
        let el = (ex - qx).hypot(ey - qy);
        let el = if el == 0.0 { 1.0 } else { el };
        let ux = (ex - qx) / el;
        let uy = (ey - qy) / el;
        let head = marker("arrow_point", ex, ey, ux, uy, "var(--_l)", "none");
        ex -= ux * marker_trim("arrow_point");
        ey -= uy * marker_trim("arrow_point");
        let mut text = String::new();
        if let Some(raw) = &transition.label {
            let label = layout_label(Some(raw), false, measurer, small, 200.0, 0);
            let (lx, ly) = point((t0 + t1) / 2.0);
            let away = nx.abs() * label.width / 2.0 + ny.abs() * label.height / 2.0 + 6.0;
            let half = label.width / 2.0 + 4.0;
            let tx = (lx + nx * away).max(x0 + half).min((right - half).max(x0 + half));
            text = label_svg(
                &label,
                tx,
                ly + ny * away,
                &format!(" class=\"pele-edge-label\" font-size=\"{}\"", js_num(small)),
            );
        }
        edges += &format!(
            "<g class=\"pele-edge\" data-id=\"{}-{}\"><path d=\"M{},{}Q{},{} {},{}\" fill=\"none\" stroke=\"var(--_l)\"/>{head}{text}</g>",
            transition.from,
            transition.to,
            num(sx),
            num(sy),
            num(qx),
            num(qy),
            num(ex),
            num(ey)
        );
    }
    let total_width = (width.max(title.width) + 2.0 * pad).ceil();
    let total_height = (height + title_height + 2.0 * pad).ceil();
    let content = label_svg(
        &title,
        x0 + width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &format!("<g class=\"pele-cynefin-regions\">{fills}</g>{boundaries}{ellipse}{domains}")
        + &if edges.is_empty() {
            String::new()
        } else {
            format!("<g class=\"pele-edges\">{edges}</g>")
        };
    let acc = Accessible {
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    };
    Rendered {
        svg: svg_document("cynefin", total_width, total_height, size, options, &acc, &content),
        width: total_width,
        height: total_height,
        links: vec![],
    }
}
