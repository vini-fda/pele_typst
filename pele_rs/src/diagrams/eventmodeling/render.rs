use super::Model;
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, turn_to_fit};
use crate::diagrams::treeview::literal::{break_label, left_label, literal_label};
use crate::preprocess::Config;
use crate::svg::builder::{esc, esc_text, label_svg};
use crate::svg::edges::{marker, marker_trim};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{FONT_MONO, ROUNDED, series_color};
use crate::text::entities::decode_entities;
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, MONO, TextMeasurer};
use crate::util::{js_num, js_round, num};
fn series(kind: &str) -> Option<i64> {
    match kind {
        "command" => Some(0),
        "event" => Some(1),
        "readmodel" => Some(2),
        "processor" => Some(6),
        _ => None,
    }
}
fn hard_wrap(text: &str, columns: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in decode_entities(text).split('\n') {
        let chars: Vec<char> = line.chars().collect();
        if chars.len() <= columns {
            out.push(line.into())
        } else {
            out.extend(chars.chunks(columns).map(|c| c.iter().collect()))
        }
    }
    out
}
fn lines(rows: &[String], x: f64, y: f64, step: f64, attrs: &str) -> String {
    let mut body = String::new();
    for (i, row) in rows.iter().enumerate() {
        if !row.is_empty() {
            body += &format!("<tspan x=\"{}\" y=\"{}\">{}</tspan>", num(x), num(y + i as f64 * step), esc(row))
        }
    }
    if body.is_empty() {
        body
    } else {
        format!("<text{attrs} xml:space=\"preserve\">{body}</text>")
    }
}
fn literal_lines(l: &Label) -> Vec<String> {
    l.lines
        .iter()
        .map(|line| line.iter().map(|s| s.text.as_str()).collect::<String>())
        .collect()
}
fn finish(model: &Model, o: &RenderOptions, size: f64, width: f64, height: f64, content: String) -> Rendered {
    Rendered {
        svg: svg_document(
            "eventmodeling",
            width,
            height,
            size,
            o,
            &Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            &content,
        ),
        width,
        height,
        links: Vec::new(),
    }
}
pub fn render(model: &Model, _config: &Config, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    turn_to_fit(o, |down| if down { vertical(model, o, m) } else { draw(model, o, m) })
}
fn draw(model: &Model, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let name_size = js_round(size * 0.875);
    let small = js_round(size * 0.8125);
    let mono = js_round(size * 0.75);
    let mono_step = js_round(mono * 1.4);
    let small_step = js_round(small * 1.5);
    let pad = o.padding.unwrap_or(8.0);
    let widest = |rows: &[String], font_size: f64, style: u32| rows.iter().map(|r| m.width(r, font_size, style)).fold(0.0_f64, f64::max);
    struct Band {
        label: Label,
        right: f64,
        y: f64,
        h: f64,
    }
    let mut bands = Vec::new();
    let mut label_column = 0.0_f64;
    for lane in &model.lanes {
        let label = layout_label(Some(&lane.label), false, m, small, 4000.0, BOLD);
        label_column = label_column.max(label.width);
        bands.push(Band {
            label,
            right: f64::NEG_INFINITY,
            y: 0.0,
            h: 0.0,
        })
    }
    label_column += 24.0;
    struct Placed {
        name: Label,
        data: Vec<String>,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }
    let mut placed: Vec<Placed> = Vec::new();
    for (index, b) in model.boxes.iter().enumerate() {
        let band = &mut bands[b.lane];
        let name = layout_label(Some(&b.name), false, m, name_size, 4000.0, BOLD);
        let data = b
            .data
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| hard_wrap(s, 48))
            .unwrap_or_default();
        let w = 96.0_f64.max(name.width.max(widest(&data, mono, MONO)) + 24.0);
        let h = 20.0
            + name.height
            + if data.is_empty() {
                0.0
            } else {
                6.0 + data.len() as f64 * mono_step
            };
        let mut x = 0.0;
        if let Some(previous) = placed.last() {
            let stagger = previous.x + (previous.w / 2.0).max(previous.w - 80.0);
            x = if model.boxes[index - 1].lane == b.lane {
                band.right + 16.0
            } else {
                stagger.max(band.right + 16.0)
            }
        }
        band.right = x + w;
        band.h = band.h.max(h);
        placed.push(Placed {
            name,
            data,
            x,
            y: 0.0,
            w,
            h,
        });
    }
    let content_right = placed.iter().map(|i| i.x + i.w).fold(0.0_f64, f64::max);
    let lane_width = label_column + content_right + 12.0;
    let title = layout_label(model.title.as_deref(), false, m, size, 4000.0, BOLD);
    let title_h = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let left = pad;
    let top = pad + title_h;
    let mut y = top;
    let mut lanes = String::new();
    for (i, band) in bands.iter_mut().enumerate() {
        band.y = y;
        band.h += 24.0;
        lanes += &(format!(
            "<g class=\"pele-cluster pele-em-lane\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\"{ROUNDED}/>",
            esc_text(&model.lanes[i].label),
            num(left),
            num(y),
            num(lane_width),
            num(band.h)
        ) + &label_svg(
            &band.label,
            left + 12.0 + band.label.width / 2.0,
            y + band.h / 2.0,
            &format!(
                " class=\"pele-cluster-label\" font-size=\"{}\" font-weight=\"var(--_hw)\" fill=\"var(--_m)\"",
                js_num(small)
            ),
        ) + "</g>");
        y += band.h + 8.0
    }
    let mut bottom = if bands.is_empty() { top } else { y - 8.0 };
    let mut nodes = String::new();
    for (index, item) in placed.iter_mut().enumerate() {
        let b = &model.boxes[index];
        let band = &bands[b.lane];
        item.x += left + label_column;
        item.y = band.y + (band.h - item.h) / 2.0;
        let frame = format!(
            "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
            num(item.x),
            num(item.y),
            num(item.w),
            num(item.h)
        );
        let shape = if let Some(s) = series(&b.kind) {
            format!(
                "<rect {frame} fill=\"var(--_bg)\"{ROUNDED}/><rect {frame} fill=\"{}\" fill-opacity=\"0.22\" stroke=\"{}\"{ROUNDED}/>",
                series_color(s),
                series_color(s)
            )
        } else {
            format!("<rect {frame} fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>")
        };
        let text = label_svg(
            &item.name,
            item.x + item.w / 2.0,
            item.y + 10.0 + item.name.height / 2.0,
            &format!(
                " class=\"pele-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
                js_num(name_size)
            ),
        ) + &lines(
            &item.data,
            item.x + 12.0,
            item.y + 10.0 + item.name.height + 6.0 + mono_step / 2.0 + mono * 0.35,
            mono_step,
            &format!(" class=\"pele-em-data\" font-family=\"{FONT_MONO}\" font-size=\"{}\"", js_num(mono)),
        );
        nodes += &format!(
            "<g class=\"pele-node pele-em-{}{}\" data-id=\"{}\">{shape}{text}</g>",
            b.kind,
            if b.reset { " pele-em-reset" } else { "" },
            esc_text(&b.id)
        );
    }
    let mut edges = String::new();
    let trim = marker_trim("arrow_point");
    for &(source, target) in &model.relations {
        if source == target {
            continue;
        }
        let s = &placed[source];
        let t = &placed[target];
        let (d, head) = if model.boxes[source].lane == model.boxes[target].lane {
            let forward = t.x >= s.x;
            let sx = if forward { s.x + s.w } else { s.x };
            let tx = if forward { t.x } else { t.x + t.w };
            let sy = s.y + s.h / 2.0;
            let ty = t.y + t.h / 2.0;
            let dir = if tx >= sx { 1.0 } else { -1.0 };
            (
                format!("M{},{}L{},{}", num(sx), num(sy), num(tx - dir * trim), num(ty)),
                marker("arrow_point", tx, ty, dir, 0.0, "var(--_l)", "none"),
            )
        } else {
            let down = t.y > s.y;
            let from = s.x.max(t.x);
            let to = (s.x + s.w).min(t.x + t.w);
            let sx = if to - from >= 16.0 {
                (from + to) / 2.0
            } else if t.x > s.x {
                s.x + s.w - 24.0_f64.min(s.w / 4.0)
            } else {
                s.x + 24.0_f64.min(s.w / 4.0)
            };
            let tx = if to - from >= 16.0 {
                sx
            } else if t.x > s.x {
                t.x + 24.0_f64.min(t.w / 4.0)
            } else {
                t.x + t.w - 24.0_f64.min(t.w / 4.0)
            };
            let sy = if down { s.y + s.h } else { s.y };
            let ty = if down { t.y } else { t.y + t.h };
            let dir = if down { 1.0 } else { -1.0 };
            let end = ty - dir * trim;
            let mid = (sy + end) / 2.0;
            (
                if sx == tx {
                    format!("M{},{}V{}", num(sx), num(sy), num(end))
                } else {
                    format!(
                        "M{},{}C{},{} {},{} {},{}",
                        num(sx),
                        num(sy),
                        num(sx),
                        num(mid),
                        num(tx),
                        num(mid),
                        num(tx),
                        num(end)
                    )
                },
                marker("arrow_point", tx, ty, 0.0, dir, "var(--_l)", "none"),
            )
        };
        edges += &format!(
            "<g class=\"pele-edge\" data-id=\"{}-{}\"><path d=\"{d}\" fill=\"none\" stroke=\"var(--_l)\"/>{head}</g>",
            esc_text(&model.boxes[source].id),
            esc_text(&model.boxes[target].id)
        )
    }
    let names = |frame: &str| {
        model
            .boxes
            .iter()
            .rev()
            .find(|b| b.id == frame)
            .map(|b| format!(" {}", b.name))
            .unwrap_or_default()
    };
    let mut cards = String::new();
    let mut cards_right = 0.0_f64;
    let mut card = |kind: &str, heading: &str, frame: &str, rows: Vec<String>| {
        let head = layout_label(Some(&format!("{heading} · {frame}{}", names(frame))), false, m, small, 4000.0, BOLD);
        let w = head.width.max(widest(&rows, small, 0)) + 24.0;
        let h = 20.0 + head.height + rows.len() as f64 * small_step;
        let cy = bottom + if cards.is_empty() && bands.is_empty() { 0.0 } else { 8.0 };
        cards += &(format!(
            "<g class=\"pele-em-{kind}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>",
            esc_text(frame),
            num(left),
            num(cy),
            num(w),
            num(h)
        ) + &label_svg(
            &head,
            left + 12.0 + head.width / 2.0,
            cy + 10.0 + head.height / 2.0,
            &format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", js_num(small)),
        ) + &lines(
            &rows,
            left + 12.0,
            cy + 10.0 + head.height + small_step / 2.0 + small * 0.35,
            small_step,
            &format!(" font-size=\"{}\"", js_num(small)),
        ) + "</g>");
        bottom = cy + h;
        cards_right = cards_right.max(w);
    };
    for note in &model.notes {
        card("note", "Note", &note.frame, hard_wrap(&note.text, 80))
    }
    for spec in &model.specifications {
        let mut rows = vec![format!("Given  {}", spec.given.join(", "))];
        if !spec.when.is_empty() {
            rows.push(format!("When  {}", spec.when.join(", ")))
        }
        rows.push(format!("Then  {}", spec.then.join(", ")));
        card(
            "specification",
            "Given / When / Then",
            &spec.frame,
            rows.iter().flat_map(|r| hard_wrap(r, 80)).collect(),
        );
    }
    let width = (if bands.is_empty() { 0.0 } else { lane_width }).max(cards_right).max(title.width);
    let tw = (width + 2.0 * pad).ceil();
    let th = (bottom + pad).ceil();
    let content = label_svg(
        &title,
        tw / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &lanes
        + &edges
        + &nodes
        + &cards;
    finish(model, o, size, tw, th, content)
}
fn vertical(model: &Model, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let mono = js_round(size * 0.75);
    let pad = o.padding.unwrap_or(8.0);
    let room = 160.0_f64.max(o.max_width.unwrap_or(320.0) - 2.0 * pad);
    let mut runs: indexmap::IndexMap<usize, (usize, usize)> = indexmap::IndexMap::new();
    let mut passed = std::collections::HashSet::new();
    for &(s, t) in &model.relations {
        if s == t {
            continue;
        }
        if s.abs_diff(t) < 2 {
            passed.insert(s.max(t));
            continue;
        }
        runs.entry(t)
            .and_modify(|run| {
                run.0 = run.0.min(s);
                run.1 = run.1.max(s)
            })
            .or_insert((s.min(t), s.max(t)));
    }
    let mut rails = vec![0; model.boxes.len()];
    let mut ends = Vec::new();
    let mut ordered: Vec<_> = runs.into_iter().collect();
    ordered.sort_by_key(|(_, run)| run.0);
    for (target, (from, to)) in ordered {
        let rail = ends.iter().position(|end| *end < from).unwrap_or(ends.len());
        if rail == ends.len() {
            ends.push(to)
        } else {
            ends[rail] = to
        }
        rails[target] = rail
    }
    let gutter = 24.0 + 8.0 * ends.len().saturating_sub(1) as f64;
    let card_width = 120.0_f64.max(room - gutter);
    let text_room = card_width - 24.0;
    let title = layout_label(model.title.as_deref(), false, m, size, room, BOLD);
    let mut y = pad + if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let mut bottom = pad + title.height;
    let mut width = title.width.max(card_width + gutter);
    let mut nodes = String::new();
    let mut edges = String::new();
    let mut cards = String::new();
    struct Placed {
        y: f64,
        h: f64,
        w: f64,
        lane: f64,
    }
    let mut placed: Vec<Placed> = Vec::new();
    let text = |raw: &str, size: f64, style: u32| literal_label(&decode_entities(raw), m, size, text_room, style);
    for (index, b) in model.boxes.iter().enumerate() {
        let lane_room = if passed.contains(&index) { text_room - 20.0 } else { text_room };
        let lane = break_label(
            layout_label(Some(&model.lanes[b.lane].label), false, m, small, lane_room, BOLD),
            m,
            lane_room,
        );
        let name = layout_label(Some(&b.name), false, m, small, text_room - 36.0, BOLD);
        let data = b.data.as_ref().filter(|s| !s.is_empty()).map(|s| text(s, mono, MONO));
        let w = card_width
            .max(lane.width + 24.0)
            .max(name.width + 60.0)
            .max(data.as_ref().map(|l| l.width).unwrap_or(0.0) + 24.0);
        let head_h = lane.height + 12.0;
        let h = 20.0 + name.height + data.as_ref().map(|l| l.height + 6.0).unwrap_or(0.0);
        let cy = y + head_h;
        let frame = format!("x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"", num(pad), num(cy), num(w), num(h));
        nodes += &(format!(
            "<g class=\"pele-cluster pele-em-lane\" data-id=\"{}\">",
            esc_text(&model.lanes[b.lane].label)
        ) + &left_label(
            &lane,
            pad + 12.0,
            y,
            &format!(
                " class=\"pele-cluster-label\" font-size=\"{}\" font-weight=\"var(--_hw)\" fill=\"var(--_m)\"",
                js_num(small)
            ),
        ) + &format!(
            "<g class=\"pele-node pele-em-{}{}\" data-id=\"{}\"><rect {frame} fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>",
            b.kind,
            if b.reset { " pele-em-reset" } else { "" },
            esc_text(&b.id)
        ) + &series(&b.kind)
            .map(|s| {
                format!(
                    "<rect {frame} fill=\"{}\" fill-opacity=\"0.22\" stroke=\"{}\"{ROUNDED}/>",
                    series_color(s),
                    series_color(s)
                )
            })
            .unwrap_or_default()
            + &label_svg(
                &name,
                pad + 12.0 + name.width / 2.0,
                cy + 10.0 + name.height / 2.0,
                &format!(" class=\"pele-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"", js_num(small)),
            )
            + &format!(
                "<text class=\"pele-em-frame\" x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
                num(pad + w - 12.0),
                num(cy + 10.0 + small),
                js_num(mono),
                esc(&b.id)
            )
            + &data
                .as_ref()
                .map(|d| {
                    lines(
                        &literal_lines(d),
                        pad + 12.0,
                        cy + 10.0 + name.height + 6.0 + d.line_height / 2.0 + mono * 0.35,
                        d.line_height,
                        &format!(" class=\"pele-em-data\" font-family=\"{FONT_MONO}\" font-size=\"{}\"", js_num(mono)),
                    )
                })
                .unwrap_or_default()
            + "</g></g>");
        placed.push(Placed {
            y: cy,
            h,
            w,
            lane: lane.width,
        });
        width = width.max(w + gutter);
        bottom = cy + h;
        y = bottom + 28.0;
    }
    for &(source, target) in &model.relations {
        if source == target {
            continue;
        }
        let s = &placed[source];
        let t = &placed[target];
        let down = t.y > s.y;
        let dir = if down { 1.0 } else { -1.0 };
        let sy = if down { s.y + s.h } else { s.y };
        let ty = if down { t.y } else { t.y + t.h };
        let (d, head) = if source.abs_diff(target) == 1 {
            let cx = pad + (s.w.min(t.w) / 2.0).max(12.0 + if down { t.lane } else { s.lane } + 16.0);
            (
                format!("M{},{}V{}", num(cx), num(sy), num(ty - dir * marker_trim("arrow_point"))),
                marker("arrow_point", cx, ty, 0.0, dir, "var(--_l)", "none"),
            )
        } else {
            let rail = pad + width - gutter + 12.0 + 8.0 * rails[target] as f64;
            let from = s.y + s.h / 2.0 - 5.0;
            let to = t.y + t.h / 2.0 + 5.0;
            (
                format!(
                    "M{},{}H{}V{}H{}",
                    num(pad + s.w),
                    num(from),
                    num(rail),
                    num(to),
                    num(pad + t.w + marker_trim("arrow_point"))
                ),
                marker("arrow_point", pad + t.w, to, -1.0, 0.0, "var(--_l)", "none"),
            )
        };
        edges += &format!(
            "<g class=\"pele-edge\" data-id=\"{}-{}\"><path d=\"{d}\" fill=\"none\" stroke=\"var(--_l)\"/>{head}</g>",
            esc_text(&model.boxes[source].id),
            esc_text(&model.boxes[target].id)
        )
    }
    let names = |frame: &str| {
        model
            .boxes
            .iter()
            .rev()
            .find(|b| b.id == frame)
            .map(|b| format!(" {}", b.name))
            .unwrap_or_default()
    };
    let mut card = |kind: &str, heading: &str, frame: &str, raw: &str| {
        let head = layout_label(
            Some(&format!("{heading} · {frame}{}", names(frame))),
            false,
            m,
            small,
            text_room,
            BOLD,
        );
        let body = text(raw, small, 0);
        let w = card_width.max(head.width + 24.0).max(body.width + 24.0);
        let h = 20.0 + head.height + body.height;
        cards += &(format!(
            "<g class=\"pele-em-{kind}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>",
            esc_text(frame),
            num(pad),
            num(y),
            num(w),
            num(h)
        ) + &label_svg(
            &head,
            pad + 12.0 + head.width / 2.0,
            y + 10.0 + head.height / 2.0,
            &format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", js_num(small)),
        ) + &lines(
            &literal_lines(&body),
            pad + 12.0,
            y + 10.0 + head.height + body.line_height / 2.0 + small * 0.35,
            body.line_height,
            &format!(" font-size=\"{}\"", js_num(small)),
        ) + "</g>");
        bottom = y + h;
        y = bottom + 8.0;
        width = width.max(w);
    };
    for note in &model.notes {
        card("note", "Note", &note.frame, &note.text)
    }
    for spec in &model.specifications {
        let mut rows = vec![format!("Given  {}", spec.given.join(", "))];
        if !spec.when.is_empty() {
            rows.push(format!("When  {}", spec.when.join(", ")))
        }
        rows.push(format!("Then  {}", spec.then.join(", ")));
        card("specification", "Given / When / Then", &spec.frame, &rows.join("\n"));
    }
    let tw = (width + 2.0 * pad).ceil();
    let height = (bottom + pad).ceil();
    let content = label_svg(
        &title,
        tw / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &edges
        + &nodes
        + &cards;
    finish(model, o, size, tw, height, content)
}
