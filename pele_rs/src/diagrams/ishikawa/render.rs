use super::Model;
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, title_room, turn_to_fit};
use crate::diagrams::treeview::literal::break_label;
use crate::preprocess::Config;
use crate::svg::builder::{esc_text, label_svg};
use crate::svg::edges::{ARROW, marker};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::ROUNDED;
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{js_num, js_round, num};
struct Bone {
    label: Label,
    depth: usize,
    parent: Option<usize>,
    slot: usize,
    kids: usize,
    rise: f64,
    at: f64,
    text_end: f64,
}
struct Category {
    node: usize,
    label: Label,
    box_w: f64,
    box_h: f64,
    bones: Vec<Bone>,
    rise: f64,
    reach: f64,
}
fn arrange(model: &Model, node: usize, up: bool, m: &dyn TextMeasurer, size: f64, small: f64) -> Category {
    let label = layout_label(Some(&model.nodes[node].text), false, m, size, 180.0, 0);
    let line_height = js_round(small * 1.5);
    let mut bones: Vec<Bone> = Vec::new();
    let mut rows = Vec::new();
    type Frame = (usize, usize, Option<usize>, usize, Option<usize>);
    let mut stack: Vec<Frame> = Vec::new();
    let push = |stack: &mut Vec<Frame>, children: &[usize], depth: usize, parent: Option<usize>| {
        let n = children.len();
        for slot in (0..n).rev() {
            stack.push((children[if up { n - 1 - slot } else { slot }], depth, parent, slot, None))
        }
    };
    push(&mut stack, &model.nodes[node].children, 2, None);
    while let Some((node, depth, parent, slot, bone)) = stack.pop() {
        if let Some(bone) = bone {
            rows.push(bone);
            continue;
        }
        let index = bones.len();
        bones.push(Bone {
            label: layout_label(Some(&model.nodes[node].text), false, m, small, 200.0, 0),
            depth,
            parent,
            slot,
            kids: model.nodes[node].children.len(),
            rise: 0.0,
            at: 0.0,
            text_end: 0.0,
        });
        if depth % 2 == 0 {
            rows.push(index)
        } else {
            stack.push((node, depth, parent, slot, Some(index)))
        }
        push(&mut stack, &model.nodes[node].children, depth + 1, Some(index))
    }
    let mut rise = 14.0;
    for index in rows {
        let height = bones[index].label.height.max(line_height) + 6.0;
        bones[index].rise = rise + height / 2.0;
        rise += height
    }
    let box_w = label.width + 20.0;
    let mut reach = box_w / 2.0;
    for i in 0..bones.len() {
        let parent = bones[i].parent;
        let parent_at = parent.map(|p| bones[p].at).unwrap_or(0.0);
        if bones[i].depth.is_multiple_of(2) {
            bones[i].at = parent_at;
            bones[i].text_end = parent_at
                - if bones[i].kids > 0 {
                    (bones[i].kids + 1) as f64 * 22.0
                } else {
                    18.0
                }
                - 5.0
        } else {
            bones[i].at = parent_at - (bones[parent.unwrap()].kids - bones[i].slot) as f64 * 22.0;
            bones[i].text_end = bones[i].at - 5.0
        }
        reach = reach.max(bones[i].label.width - bones[i].text_end)
    }
    let box_h = label.height + 10.0;
    Category {
        node,
        label,
        box_w,
        box_h,
        bones,
        rise: rise + 8.0,
        reach,
    }
}
pub fn render(model: &Model, _config: &Config, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    turn_to_fit(o, |down| if down { vertical(model, o, m) } else { draw(model, o, m) })
}
fn finish(o: &RenderOptions, size: f64, width: f64, height: f64, content: String) -> Rendered {
    Rendered {
        svg: svg_document(
            "ishikawa",
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
fn draw(model: &Model, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let pad = o.padding.unwrap_or(8.0);
    let title = layout_label(model.title.as_deref(), false, m, size, 4000.0, BOLD);
    let title_h = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let mut body = String::new();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    if let Some(root) = model.nodes.first() {
        let head = layout_label(Some(&root.text), false, m, size, 200.0, BOLD);
        let head_h = (head.height + 24.0).max(48.0);
        let head_w = head.width + 24.0 + head_h / 2.0;
        let mut categories: Vec<Category> = root
            .children
            .iter()
            .enumerate()
            .map(|(i, node)| arrange(model, *node, i % 2 == 0, m, size, small))
            .collect();
        for c in &mut categories {
            c.rise = c.rise.max(head_h / 2.0 + 14.0)
        }
        let mut xs = Vec::new();
        let mut x = -34.0;
        for i in (0..categories.len()).step_by(2) {
            if i > 0 {
                let mut room = 0.0_f64;
                for side in 0..2 {
                    if let Some(before) = categories.get(i - 2 + side) {
                        room = room.max(before.reach + categories.get(i + side).map(|c| c.box_w / 2.0).unwrap_or(0.0))
                    }
                }
                x -= room + 26.0
            }
            xs.push(x);
            xs.push(x)
        }
        let tail = if categories.is_empty() { -34.0 } else { x - 28.0 };
        max_x = head_w;
        min_x = tail;
        min_y = -head_h / 2.0;
        max_y = head_h / 2.0;
        let mut groups = String::new();
        let length = 0.5_f64.hypot(1.0);
        for (i, c) in categories.iter().enumerate() {
            let sign = if i % 2 == 0 { -1.0 } else { 1.0 };
            let ax = xs[i];
            let px = |at: f64, rise: f64| ax + at - rise * 0.5;
            let end_x = px(0.0, c.rise);
            let end_y = sign * c.rise;
            let trim = ARROW - 1.0;
            let mut d = format!(
                "M{},{}L{},{}",
                num(ax - 0.5 / length * trim),
                num(sign * trim / length),
                num(end_x),
                num(end_y)
            );
            let mut texts = String::new();
            for bone in &c.bones {
                let y = sign * bone.rise;
                if bone.depth % 2 == 0 {
                    d += &format!(
                        "M{},{}H{}",
                        num(px(bone.at, bone.rise)),
                        num(y),
                        num(px(bone.text_end + 5.0, bone.rise))
                    )
                } else {
                    let from = c.bones[bone.parent.unwrap()].rise;
                    d += &format!(
                        "M{},{}L{},{}",
                        num(px(bone.at, from)),
                        num(sign * from),
                        num(px(bone.at, bone.rise)),
                        num(y)
                    )
                }
                let right = px(bone.text_end, bone.rise);
                texts += &label_svg(
                    &bone.label,
                    right - bone.label.width / 2.0,
                    y,
                    if bone.depth > 2 { " fill=\"var(--_m)\"" } else { "" },
                );
                min_x = min_x.min(right - bone.label.width)
            }
            let box_y = end_y + sign * c.box_h / 2.0;
            min_x = min_x.min(end_x - c.box_w / 2.0);
            max_x = max_x.max(end_x + c.box_w / 2.0);
            min_y = min_y.min(box_y - c.box_h / 2.0);
            max_y = max_y.max(box_y + c.box_h / 2.0);
            groups += &(format!(
                "<g class=\"pele-ishikawa-category\" data-id=\"{}\"><path class=\"pele-ishikawa-bones\" d=\"{d}\" fill=\"none\" stroke=\"var(--_l)\"/>",
                esc_text(&model.nodes[c.node].text)
            ) + &marker("arrow_point", ax, 0.0, 0.5 / length, -sign / length, "var(--_l)", "none")
                + &format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{ROUNDED}/>",
                    num(end_x - c.box_w / 2.0),
                    num(box_y - c.box_h / 2.0),
                    num(c.box_w),
                    num(c.box_h)
                )
                + &label_svg(&c.label, end_x, box_y, " class=\"pele-label\"")
                + &if texts.is_empty() {
                    String::new()
                } else {
                    format!("<g class=\"pele-ishikawa-causes\" font-size=\"{}\">{texts}</g>", js_num(small))
                }
                + "</g>")
        }
        let r = head_h / 2.0;
        body = format!(
            "<path class=\"pele-ishikawa-spine\" d=\"M{},0H{}\" fill=\"none\" stroke=\"var(--_l)\"/>",
            num(tail),
            num(-(ARROW - 1.0))
        ) + &marker("arrow_point", 0.0, 0.0, 1.0, 0.0, "var(--_l)", "none")
            + &groups
            + &format!(
                "<g class=\"pele-ishikawa-head\" data-id=\"{}\"><path d=\"M0,{}H{}A{},{} 0 0 1 {},{}H0Z\" fill=\"var(--_s)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"/>",
                esc_text(&root.text),
                num(-r),
                num(head_w - r),
                num(r),
                num(r),
                num(head_w - r),
                num(r)
            )
            + &label_svg(
                &head,
                12.0 + head.width / 2.0,
                0.0,
                " class=\"pele-label\" font-weight=\"var(--_hw)\"",
            )
            + "</g>";
    }
    let inner = (max_x - min_x).max(title.width);
    let width = (inner + 2.0 * pad).ceil();
    let height = (max_y - min_y + title_h + 2.0 * pad).ceil();
    let ox = pad + (inner - (max_x - min_x)) / 2.0 - min_x;
    let oy = pad + title_h - min_y;
    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !body.is_empty() {
        content += &format!("<g transform=\"translate({},{})\">{body}</g>", num(ox), num(oy))
    }
    finish(o, size, width, height, content)
}
fn vertical(model: &Model, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let pad = o.padding.unwrap_or(8.0);
    let room = 140.0_f64.max(o.max_width.unwrap_or(320.0) - 2.0 * pad);
    let title = layout_label(model.title.as_deref(), false, m, size, title_room(o), BOLD);
    struct Entry {
        node: usize,
        depth: usize,
        parent: Option<usize>,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }
    let mut order: Vec<Entry> = Vec::new();
    let mut pending = if model.nodes.is_empty() { Vec::new() } else { vec![(0, 0, None)] };
    let mut depth = 0;
    while let Some((node, d, parent)) = pending.pop() {
        let index = order.len();
        order.push(Entry {
            node,
            depth: d,
            parent,
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        });
        depth = depth.max(d);
        for &child in model.nodes[node].children.iter().rev() {
            pending.push((child, d + 1, Some(index)))
        }
    }
    let indent = 20.0_f64.min(2.0_f64.max((room - 120.0) / depth.max(1) as f64));
    let mut y = pad + if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    let mut width = title.width;
    let mut nodes = String::new();
    let mut edges = String::new();
    for i in 0..order.len() {
        let entry = &mut order[i];
        entry.x = pad + entry.depth as f64 * indent;
        let label_room = 60.0_f64.max(room - entry.depth as f64 * indent - 20.0);
        let label = break_label(
            layout_label(
                Some(&model.nodes[entry.node].text),
                false,
                m,
                if entry.depth < 2 { size } else { small },
                label_room,
                if entry.depth < 2 { BOLD } else { 0 },
            ),
            m,
            label_room,
        );
        entry.w = label.width + 20.0;
        entry.h = label.height + 12.0;
        entry.y = y;
        y += entry.h + 12.0;
        width = width.max(entry.x - pad + entry.w);
        let framed = entry.depth < 2;
        nodes += &format!(
            "<g class=\"pele-node {}\" data-id=\"{}\">",
            if entry.depth == 0 {
                "pele-ishikawa-head"
            } else if entry.depth == 1 {
                "pele-ishikawa-category"
            } else {
                "pele-ishikawa-cause"
            },
            esc_text(&model.nodes[entry.node].text)
        );
        if framed {
            nodes += &format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"/>",
                num(entry.x),
                num(entry.y),
                num(entry.w),
                num(entry.h),
                if entry.depth == 0 { 12 } else { 4 }
            )
        }
        nodes += &label_svg(
            &label,
            entry.x + entry.w / 2.0,
            entry.y + entry.h / 2.0,
            &format!(
                " class=\"pele-label\"{}{}",
                if entry.depth < 2 { " font-weight=\"var(--_hw)\"" } else { "" },
                if entry.depth > 1 {
                    format!(" font-size=\"{}\"", js_num(small))
                } else {
                    String::new()
                }
            ),
        );
        nodes += "</g>";
        let ex = entry.x;
        let ey = entry.y;
        let eh = entry.h;
        if let Some(p) = entry.parent {
            let parent = &order[p];
            let gx = parent.x + indent / 2.0;
            edges += &format!(
                "<path class=\"pele-edge\" d=\"M{},{}V{}H{}\" fill=\"none\" stroke=\"var(--_b)\"/>",
                num(gx),
                num(parent.y + parent.h),
                num(ey + eh / 2.0),
                num(ex)
            )
        }
    }
    let tw = (width + 2.0 * pad).ceil();
    let height = ((pad + title.height).max(y - 12.0) + pad).ceil();
    let content = label_svg(
        &title,
        tw / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &edges
        + &nodes;
    finish(o, size, tw, height, content)
}
