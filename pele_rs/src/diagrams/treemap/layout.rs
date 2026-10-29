use super::Model;
use crate::util::js_round;
#[derive(Clone, Debug, Default)]
pub struct Box {
    pub node: usize,
    pub parent: Option<usize>,
    pub series: usize,
    pub value: f64,
    pub children: Vec<usize>,
    pub headed: bool,
    pub header: f64,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
fn make(model: &Model, node: usize, parent: Option<usize>, series: usize) -> Box {
    let n = &model.nodes[node];
    Box {
        node,
        parent,
        series,
        value: if n.children.is_none() {
            n.value.filter(|v| v.is_finite() && *v > 0.0).unwrap_or(0.0)
        } else {
            0.0
        },
        ..Default::default()
    }
}
fn squarify(all: &mut [Box], list: &[usize], total: f64, mut x: f64, mut y: f64, mut w: f64, mut h: f64, gap: f64) {
    let mut rest = total;
    let mut from = 0;
    while from < list.len() {
        let scale = w * h / rest;
        let side = w.min(h);
        let mut sum = all[list[from]].value * scale;
        let largest = sum;
        let mut worst = (side * side * largest / (sum * sum)).max(sum * sum / (side * side * largest));
        let mut to = from + 1;
        while to < list.len() {
            let smallest = all[list[to]].value * scale;
            let next = sum + smallest;
            let ratio = (side * side * largest / (next * next)).max(next * next / (side * side * smallest));
            if ratio > worst {
                break;
            }
            worst = ratio;
            sum = next;
            to += 1
        }
        let thickness = sum / side;
        let mut offset = 0.0;
        for i in from..to {
            let b = &mut all[list[i]];
            let length = if i == to - 1 { side - offset } else { b.value * scale / thickness };
            let (bx, by, bw, bh) = if w >= h {
                (x, y + offset, thickness, length)
            } else {
                (x + offset, y, length, thickness)
            };
            let gx = if js_round(bx + bw - gap / 2.0) > js_round(bx + gap / 2.0) {
                gap / 2.0
            } else {
                0.0
            };
            let gy = if js_round(by + bh - gap / 2.0) > js_round(by + gap / 2.0) {
                gap / 2.0
            } else {
                0.0
            };
            b.x = js_round(bx + gx);
            b.y = js_round(by + gy);
            b.w = js_round(bx + bw - gx) - b.x;
            b.h = js_round(by + bh - gy) - b.y;
            offset += length;
            rest -= b.value
        }
        if w >= h {
            x += thickness;
            w -= thickness
        } else {
            y += thickness;
            h -= thickness
        }
        from = to
    }
}
fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v >= lo { v.min(hi) } else { lo }
}
fn place(all: &mut [Box], placed: &mut Vec<usize>, children: &[usize], x: f64, y: f64, w: f64, h: f64, gap: f64) {
    let mut shown: Vec<usize> = children.iter().copied().filter(|i| all[*i].value > 0.0).collect();
    shown.sort_by(|a, b| all[*b].value.total_cmp(&all[*a].value));
    let total: f64 = shown.iter().map(|i| all[*i].value).sum();
    if shown.is_empty() || !total.is_finite() {
        return;
    }
    let w = w.max(0.0);
    let h = h.max(0.0);
    if w > 0.0 && h > 0.0 {
        squarify(all, &shown, total, x - gap / 2.0, y - gap / 2.0, w + gap, h + gap, gap)
    }
    for i in shown {
        let b = &mut all[i];
        let x0 = clamp(b.x, x, x + w);
        let y0 = clamp(b.y, y, y + h);
        b.w = clamp(b.x + b.w, x0, x + w) - x0;
        b.h = clamp(b.y + b.h, y0, y + h) - y0;
        b.x = x0;
        b.y = y0;
        placed.push(i)
    }
}
pub fn layout(model: &Model, width: f64, height: f64, gap: f64, side: f64) -> (Vec<Box>, Vec<usize>) {
    let mut all: Vec<Box> = model.roots.iter().enumerate().map(|(i, n)| make(model, *n, None, i)).collect();
    let top: Vec<usize> = (0..all.len()).collect();
    let mut i = 0;
    while i < all.len() {
        let n = all[i].node;
        if let Some(children) = &model.nodes[n].children {
            for &node in children {
                let index = all.len();
                let child = make(model, node, Some(i), all[i].series);
                all[i].children.push(index);
                all.push(child)
            }
        }
        i += 1
    }
    for i in (0..all.len()).rev() {
        if let Some(p) = all[i].parent {
            all[p].value += all[i].value
        }
    }
    let mut placed = Vec::new();
    place(&mut all, &mut placed, &top, 0.0, 0.0, width, height, gap);
    let mut i = 0;
    while i < placed.len() {
        let index = placed[i];
        let b = &mut all[index];
        if !b.children.is_empty() {
            let top = if b.h - 28.0 - side >= 12.0 { 28.0 } else { 18.0 };
            b.headed = b.w > 24.0_f64.max(2.0 * side) && b.h - top - side >= 8.0;
            b.header = if b.headed { top } else { 0.0 };
            let (x, y, w, h) = if b.headed {
                (b.x + side, b.y + top, b.w - 2.0 * side, b.h - top - side)
            } else {
                (b.x, b.y, b.w, b.h)
            };
            let children = b.children.clone();
            place(&mut all, &mut placed, &children, x, y, w, h, gap)
        }
        i += 1
    }
    (all, placed)
}
