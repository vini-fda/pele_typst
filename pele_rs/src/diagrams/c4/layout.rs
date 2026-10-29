use super::{
    model::{Element, Model, Ref},
    route::{Box, HEAD, LABEL_PAD, LANE, NARROW},
};
use crate::{
    TextMeasurer,
    svg::edges::ARROW,
    text::{
        label::{Label, layout_label},
        measurer::BOLD,
    },
};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};
pub const GROUP_PAD: f64 = LANE + 8.;
pub const HEAD_LEFT: f64 = 16.;
pub const HEAD_TOP: f64 = 10.;
pub struct Node {
    pub bx: Box,
    pub shape: usize,
    pub labels: Vec<Label>,
    pub text_h: f64,
}
pub struct Group {
    pub bx: Box,
    pub boundary: usize,
    pub kids: Vec<usize>,
    pub nodes: Vec<usize>,
    pub labels: Vec<Label>,
    pub head: f64,
    pub head_w: f64,
    pub head_x: f64,
}
impl Deref for Node {
    type Target = Box;
    fn deref(&self) -> &Box {
        &self.bx
    }
}
impl DerefMut for Node {
    fn deref_mut(&mut self) -> &mut Box {
        &mut self.bx
    }
}
impl Deref for Group {
    type Target = Box;
    fn deref(&self) -> &Box {
        &self.bx
    }
}
impl DerefMut for Group {
    fn deref_mut(&mut self) -> &mut Box {
        &mut self.bx
    }
}
pub struct Layout {
    pub order: Vec<usize>,
    pub groups: Vec<Group>,
    pub nodes: Vec<Node>,
    pub boxes: HashMap<Ref, Box>,
}
fn form(raw: &str) -> Option<&'static str> {
    Some(match raw {
        "person" => "person",
        "box" | "rounded" | "component" => "rect",
        "cylinder" | "database" | "db" => "cyl",
        "queue" | "pipe" => "h-cyl",
        _ => return None,
    })
}
fn form_of(shape: &Element) -> &'static str {
    if let Some(s) =
        form(&shape.get("shape").unwrap_or("").to_lowercase()).or_else(|| form(&shape.get("sprite").unwrap_or("").to_lowercase()))
    {
        return s;
    }
    if let Some(tags) = shape.get("tags") {
        for tag in tags.split(',') {
            if let Some(s) = form(&tag.trim().to_lowercase()) {
                return s;
            }
        }
    }
    let kind = &shape.kind;
    if kind.ends_with("person") {
        "person"
    } else if kind.ends_with("_db") {
        "cyl"
    } else if kind.ends_with("_queue") {
        "h-cyl"
    } else {
        "rect"
    }
}
fn measure(shape: usize, model: &Model, m: &dyn TextMeasurer, size: f64, small: f64, width: f64) -> Node {
    let s = &model.shapes[shape];
    let form = form_of(s);
    let cap_w = if form == "h-cyl" { 36. } else { 0. };
    let cap_h = if form == "person" {
        HEAD
    } else if form == "cyl" {
        24.
    } else {
        0.
    };
    let inner = width - 2. * 14. - cap_w;
    let techn = s.get("techn").filter(|v| !v.is_empty()).map(|v| format!("[{v}]"));
    let labels = vec![
        layout_label(Some(&format!("«{}»", s.kind.replace('_', " "))), false, m, small - 1., inner, 0),
        layout_label(Some(&s.label), false, m, size, inner, BOLD),
        layout_label(techn.as_deref(), false, m, small, inner, 0),
        layout_label(s.get("descr"), false, m, small, inner, 0),
    ];
    let mut tw = 0f64;
    let mut text_h = if labels[3].height > 0. { 4. } else { 0. };
    for label in &labels {
        tw = tw.max(label.width);
        text_h += label.height;
    }
    Node {
        shape,
        bx: Box {
            index: model.boundaries.len() + shape,
            x: 0.,
            y: 0.,
            w: width.max(tw + 2. * 14. + cap_w),
            h: 64f64.max(text_h + 2. * 12. + cap_h),
            form: form.into(),
        },
        labels,
        text_h,
    }
}
fn rows(boxes: &mut [Box], per_row: usize, gap_x: f64, gap_y: f64, stretch: bool) -> (f64, f64) {
    let (mut width, mut y) = (0f64, 0f64);
    for chunk in boxes.chunks_mut(per_row.max(1)) {
        let row_h = chunk.iter().map(|b| b.h).fold(0f64, f64::max);
        let mut x = 0.;
        for b in chunk {
            b.x = x;
            b.y = y;
            if stretch {
                b.h = row_h;
            }
            x += b.w + gap_x;
        }
        width = width.max(x - gap_x);
        y += row_h + gap_y;
    }
    (width, if boxes.is_empty() { 0. } else { y - gap_y })
}
pub fn layout(model: &Model, m: &dyn TextMeasurer, size: f64, small: f64, room: Option<f64>, gap: f64) -> Layout {
    let group_pad = if room.is_none() { GROUP_PAD } else { 12. };
    let mut groups = vec![];
    for (i, boundary) in model.boundaries.iter().enumerate() {
        let label_room = room.map_or(4000., |r| (r - 2. * HEAD_LEFT - 2. * GROUP_PAD * 2.).max(80.));
        let typ = boundary.get("type").filter(|v| !v.is_empty()).map(|v| format!("[{v}]"));
        let labels = if boundary.parent.is_some() {
            vec![
                layout_label(Some(&boundary.label), false, m, size, label_room, BOLD),
                layout_label(typ.as_deref(), false, m, small, label_room, 0),
                layout_label(boundary.get("descr"), false, m, small, label_room, 0),
            ]
        } else {
            vec![]
        };
        let head = labels.iter().map(|l| l.height).sum::<f64>();
        let head_w = labels.iter().map(|l| l.width).fold(0f64, f64::max);
        groups.push(Group {
            bx: Box {
                index: i,
                x: 0.,
                y: 0.,
                w: 0.,
                h: 0.,
                form: "rect".into(),
            },
            boundary: i,
            kids: vec![],
            nodes: vec![],
            labels,
            head: if head > 0. { head + HEAD_TOP + 6. + LANE } else { group_pad },
            head_w,
            head_x: HEAD_LEFT,
        });
    }
    for (i, b) in model.boundaries.iter().enumerate() {
        if let Some(p) = b.parent
            && let Some(parent) = groups.get_mut(p)
        {
            parent.kids.push(i);
        }
    }
    let mut order = vec![0];
    let mut depths = HashMap::from([(0, 0usize)]);
    let mut i = 0;
    while i < order.len() {
        let at = order[i];
        for &kid in &groups[at].kids {
            depths.insert(kid, depths[&at] + 1);
            order.push(kid);
        }
        i += 1;
    }
    let mut nodes = vec![];
    for (i, shape) in model.shapes.iter().enumerate() {
        if let Some(parent) = shape.parent {
            let depth = depths.get(&parent).copied().unwrap_or(0);
            let width = room.map_or(216., |r| (r - 2. * depth as f64 * group_pad - 32.).min(216.).max(100.));
            let at = nodes.len();
            nodes.push(measure(i, model, m, size, small, width));
            if let Some(group) = groups.get_mut(parent) {
                group.nodes.push(at);
            }
        }
    }
    let per_row = model.shape_in_row.max(1.) as usize;
    let groups_per_row = model.boundary_in_row.max(1.) as usize;
    let mut kids_top = HashMap::new();
    for &i in order.iter().rev() {
        let gap_x = if room.is_none() { NARROW + LABEL_PAD + 3. * ARROW } else { gap };
        let count = |boxes: &[Box], limit: usize, gap: f64| -> usize {
            let Some(room) = room else { return limit };
            let widest = boxes.iter().map(|b| b.w).fold(1f64, f64::max);
            ((room + gap - 2. * group_pad) / (widest + gap)).floor().min(limit as f64).max(1.) as usize
        };
        let mut b = groups[i].nodes.iter().map(|&n| nodes[n].bx.clone()).collect::<Vec<_>>();
        let count_n = count(&b, per_row, gap_x);
        let (grid_w, grid_h) = rows(&mut b, count_n, gap_x, 88., true);
        for (&n, bx) in groups[i].nodes.iter().zip(b) {
            nodes[n].bx = bx;
        }
        let mut b = groups[i].kids.iter().map(|&g| groups[g].bx.clone()).collect::<Vec<_>>();
        let count_g = count(&b, groups_per_row, 48.);
        let (kids_w, kids_h) = rows(&mut b, count_g, 48., 48., false);
        let kids = groups[i].kids.clone();
        for (g, bx) in kids.into_iter().zip(b) {
            groups[g].bx = bx;
        }
        let between = if grid_h > 0. && kids_h > 0. { 48. } else { 0. };
        let frame = if i == 0 { 0. } else { group_pad };
        kids_top.insert(i, grid_h + between);
        groups[i].w = grid_w.max(kids_w).max(groups[i].head_w + 2. * HEAD_LEFT - 2. * frame) + 2. * frame;
        groups[i].h = if frame != 0. { groups[i].head } else { 0. } + grid_h + between + kids_h + frame;
    }
    let mut boxes = HashMap::new();
    for &i in &order {
        let left = groups[i].x + if i == 0 { 0. } else { group_pad };
        let top = groups[i].y + if i == 0 { 0. } else { groups[i].head };
        for &n in &groups[i].nodes {
            nodes[n].x += left;
            nodes[n].y += top;
            boxes.insert(Ref::Shape(nodes[n].shape), nodes[n].bx.clone());
        }
        let kids = groups[i].kids.clone();
        for kid in kids {
            groups[kid].x += left;
            groups[kid].y += top + kids_top[&i];
        }
        if i != 0 {
            boxes.insert(Ref::Boundary(i), groups[i].bx.clone());
        }
    }
    Layout {
        order,
        groups,
        nodes,
        boxes,
    }
}
