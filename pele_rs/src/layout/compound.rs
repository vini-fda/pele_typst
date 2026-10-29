// Layout of a graph whose nodes may be nested in groups, after pele's layout/compound.ts. Each
// group is laid out on its own, in its own direction, and then placed in its parent as a single
// box. An edge that crosses a group's border passes through a port on the side facing its other end.

use std::collections::HashMap;

use super::layered::{Groups, Kind, LEdge, LNode, LayeredOptions, LayeredResult, ease, layered, ledge, lnode, orient, rank};
use crate::util::{js_round, stable_sort_by};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    TB,
    BT,
    LR,
    RL,
}

// The direction a diagram names, or top to bottom for anything else.
pub fn direction(dir: Option<&str>) -> Dir {
    match dir {
        Some("BT") => Dir::BT,
        Some("LR") => Dir::LR,
        Some("RL") => Dir::RL,
        _ => Dir::TB,
    }
}

#[derive(Debug, Clone)]
pub struct CNode {
    pub w: f64,
    pub h: f64,
    pub parent: i64,
    pub is_group: bool,
    pub dir: Option<Dir>,
    pub pad_x: f64,
    pub pad_top: f64,
    pub pad_bottom: f64,
    pub min_w: f64,
    pub seq: f64,
    pub span: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct CEdge {
    pub src: usize,
    pub dst: usize,
    pub minlen: f64,
    pub label_w: f64,
    pub label_h: f64,
    // x, y, axis triples; axis is 0 where the edge runs vertically into the point, 1 horizontally.
    pub route: Vec<f64>,
    pub label_x: f64,
    pub label_y: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CompoundResult {
    pub width: f64,
    pub height: f64,
}

pub fn cnode(w: f64, h: f64) -> CNode {
    CNode {
        w,
        h,
        parent: -1,
        is_group: false,
        dir: None,
        pad_x: 0.0,
        pad_top: 0.0,
        pad_bottom: 0.0,
        min_w: 0.0,
        seq: 0.0,
        span: 0.0,
        x: 0.0,
        y: 0.0,
    }
}

const TOP: u8 = 0;
const RIGHT: u8 = 1;
const BOTTOM: u8 = 2;
const LEFT: u8 = 3;

#[derive(Debug, Clone)]
struct PortEdge {
    e: usize,
    inner: usize,
    out: bool,
    side: u8,
    hint: f64,
    px: f64,
    py: f64,
    route: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
struct LevelEdge {
    e: usize,
    a: usize,
    b: usize,
}

#[derive(Debug, Clone)]
struct Level {
    id: i64,
    dir: Dir,
    items: Vec<usize>,
    edges: Vec<LevelEdge>,
    ports: Vec<usize>,
    port_of: HashMap<usize, usize>,
    routes: HashMap<usize, Vec<f64>>,
    x: f64,
    y: f64,
}

const MAX_PORTS: i64 = 40000;
const MAX_ORDERED_PORTS: i64 = 400;
const MAX_ORDERED_EDGES: usize = 500;
const FLAT_SLACK: f64 = 1.5;
const FLAT_SPARE: f64 = 4.0;

// Ignore crossings within this distance of a shared node.
const FAN: f64 = 24.0;
const IN_LINE: f64 = 0.5;
const CURVE_PIECES: usize = 3;
const CORNER_PIECES: usize = 2;
const STEP_PIECES: usize = 3;
const HANDLE: f64 = 0.62;
const MAX_INSERT: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Curved,
    Stepped,
    Linear,
}

pub fn style_of(opt: &LayeredOptions) -> Style {
    if opt.tracks {
        Style::Stepped
    } else if opt.straight {
        Style::Linear
    } else {
        Style::Curved
    }
}

struct Segs {
    ax: Vec<f64>,
    ay: Vec<f64>,
    bx: Vec<f64>,
    by: Vec<f64>,
    low: Vec<f64>,
    high: Vec<f64>,
    owner: Vec<usize>,
    ends: Vec<u8>,
    top: f64,
    bottom: f64,
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
}

impl Segs {
    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, ei: usize, part: u8, mut x0: f64, mut y0: f64, x1: f64, y1: f64) {
        let on = x0 == self.to_x && y0 == self.to_y && ((y1 == y0 && self.from_y == y0) || (x1 == x0 && self.from_x == x0));
        if let Some(p) = self.owner.len().checked_sub(1)
            && on
            && self.owner[p] == ei
            && self.ends[p] == part
            && (x0 - self.from_x) * (x1 - x0) + (y0 - self.from_y) * (y1 - y0) > 0.0
        {
            self.ax.pop();
            self.ay.pop();
            self.bx.pop();
            self.by.pop();
            self.low.pop();
            self.high.pop();
            self.owner.pop();
            self.ends.pop();
            x0 = self.from_x;
            y0 = self.from_y;
        }
        self.from_x = x0;
        self.from_y = y0;
        self.to_x = x1;
        self.to_y = y1;
        self.ends.push(part);
        self.ax.push(x0.min(x1));
        self.bx.push(x0.max(x1));
        self.ay.push(if x0 <= x1 { y0 } else { y1 });
        self.by.push(if x0 <= x1 { y1 } else { y0 });
        let low = y0.min(y1);
        let high = y0.max(y1);
        self.low.push(low);
        self.high.push(high);
        if low < self.top {
            self.top = low;
        }
        if high > self.bottom {
            self.bottom = high;
        }
        self.owner.push(ei);
    }
}

pub fn route_crossings(nodes: &[CNode], edges: &[CEdge], style: Style) -> i64 {
    let _ = STEP_PIECES;
    let mut s = Segs {
        ax: Vec::new(),
        ay: Vec::new(),
        bx: Vec::new(),
        by: Vec::new(),
        low: Vec::new(),
        high: Vec::new(),
        owner: Vec::new(),
        ends: Vec::new(),
        top: f64::INFINITY,
        bottom: f64::NEG_INFINITY,
        from_x: f64::NAN,
        from_y: f64::NAN,
        to_x: f64::NAN,
        to_y: f64::NAN,
    };
    for (ei, e) in edges.iter().enumerate() {
        if e.src == e.dst {
            continue;
        }
        let r = &e.route;
        let get = |k: usize| r.get(k).copied().unwrap_or(f64::NAN);
        let aligned = |k: usize| (get(k) - get(k - 3)).abs() < 0.01 || (get(k + 1) - get(k - 2)).abs() < 0.01;
        let length = |k: usize| (get(k) - get(k - 3)).abs() + (get(k + 1) - get(k - 2)).abs();
        let turns = |k: usize| {
            style == Style::Curved
                && k > 0
                && k + 3 < r.len()
                && aligned(k)
                && aligned(k + 3)
                && length(k) >= 0.01
                && length(k + 3) >= 0.01
                && (((get(k) - get(k - 3)).abs() < 0.01) != ((get(k + 3) - get(k)).abs() < 0.01))
        };
        if r.is_empty() {
            continue;
        }
        let mut at_x = r[0];
        let mut at_y = r[1];
        let mut k = 3;
        while k < r.len() {
            let part = (if k == 3 { 1 } else { 0 }) | (if k == r.len() - 3 { 2 } else { 0 });
            let x0 = at_x;
            let y0 = at_y;
            let x1 = r[k];
            let y1 = r[k + 1];
            at_x = x1;
            at_y = y1;
            if aligned(k) || style == Style::Linear {
                if !turns(k) {
                    s.add(ei, part, x0, y0, x1, y1);
                    k += 3;
                    continue;
                }
                let before = if turns(k - 3) { length(k) / 2.0 } else { length(k) };
                let after = if turns(k + 3) { length(k + 3) / 2.0 } else { length(k + 3) };
                let ux = (x1 - r[k - 3]) / length(k);
                let uy = (y1 - r[k - 2]) / length(k);
                let vx = (r[k + 3] - x1) / length(k + 3);
                let vy = (r[k + 4] - y1) / length(k + 3);
                let sx = x1 - ux * before;
                let sy = y1 - uy * before;
                s.add(ei, part, x0, y0, sx, sy);
                let c1x = x1 - ux * before * (1.0 - HANDLE);
                let c1y = y1 - uy * before * (1.0 - HANDLE);
                let c2x = x1 + vx * after * (1.0 - HANDLE);
                let c2y = y1 + vy * after * (1.0 - HANDLE);
                at_x = x1 + vx * after;
                at_y = y1 + vy * after;
                let turn = part | if k + 3 == r.len() - 3 { 2 } else { 0 };
                let mut px = sx;
                let mut py = sy;
                for i in 1..=CORNER_PIECES {
                    let t = i as f64 / CORNER_PIECES as f64;
                    let u = 1.0 - t;
                    let x = u * u * u * sx + 3.0 * u * u * t * c1x + 3.0 * u * t * t * c2x + t * t * t * at_x;
                    let y = u * u * u * sy + 3.0 * u * u * t * c1y + 3.0 * u * t * t * c2y + t * t * t * at_y;
                    s.add(ei, turn, px, py, x, y);
                    px = x;
                    py = y;
                }
                k += 3;
                continue;
            }
            if style == Style::Stepped {
                let down = r[k + 2] == 0.0;
                let mx = if down { x0 } else { (x0 + x1) / 2.0 };
                let my = if down { (y0 + y1) / 2.0 } else { y0 };
                s.add(ei, part, x0, y0, mx, my);
                s.add(ei, part, mx, my, if down { x1 } else { mx }, if down { my } else { y1 });
                s.add(ei, part, if down { x1 } else { mx }, if down { my } else { y1 }, x1, y1);
                k += 3;
                continue;
            }
            // Approximate the rendered S-curve with straight segments.
            let down = r[k + 2] == 0.0;
            let c = if down { ease(x1 - x0, y1 - y0) } else { ease(y1 - y0, x1 - x0) };
            let mut px = x0;
            let mut py = y0;
            for i in 1..=CURVE_PIECES {
                let t = i as f64 / CURVE_PIECES as f64;
                let u = 1.0 - t;
                let across = t * t * (3.0 - 2.0 * t);
                let along = 3.0 * c * t * u * u + 3.0 * (1.0 - c) * t * t * u + t * t * t;
                let x = x0 + (x1 - x0) * (if down { across } else { along });
                let y = y0 + (y1 - y0) * (if down { along } else { across });
                s.add(ei, part, px, py, x, y);
                px = x;
                py = y;
            }
            k += 3;
        }
    }
    let count = s.owner.len();
    let (top, bottom) = (s.top, s.bottom);
    let bands = if bottom > top {
        js_round((count as f64).sqrt()) as usize
    } else {
        1
    };
    let scale = if bottom > top { bands as f64 / (bottom - top) } else { 0.0 };
    let band_of = |y: f64| -> usize {
        let b = ((y - top) * scale).floor();
        let b = if b.is_nan() || b < 0.0 { 0.0 } else { b };
        (bands as f64 - 1.0).min(b) as usize
    };
    let mut first = vec![0usize; count];
    let mut band_start = vec![0usize; bands + 1];
    for p in 0..count {
        first[p] = band_of(s.low[p]);
        let last = band_of(s.high[p]);
        for b in first[p]..=last {
            band_start[b + 1] += 1;
        }
    }
    for b in 0..bands {
        band_start[b + 1] += band_start[b];
    }
    let mut fill = band_start[..bands].to_vec();
    let mut in_band = vec![0usize; band_start[bands]];
    for p in 0..count {
        let last = band_of(s.high[p]);
        for b in first[p]..=last {
            in_band[fill[b]] = p;
            fill[b] += 1;
        }
    }
    // Edges merging into a shared endpoint run do not count as crossings.
    let mut runs = vec![0.0f64; edges.len() * 8];
    let in_line = |r: &[f64], a: usize, b: usize, c: usize| {
        ((r[a] - r[b]).abs() < IN_LINE && (r[a] - r[c]).abs() < IN_LINE)
            || ((r[a + 1] - r[b + 1]).abs() < IN_LINE && (r[a + 1] - r[c + 1]).abs() < IN_LINE)
    };
    for (ei, e) in edges.iter().enumerate() {
        let r = &e.route;
        if r.len() < 6 {
            continue;
        }
        let mut k = 3;
        while k + 3 < r.len() && in_line(r, 0, k, k + 3) {
            k += 3;
        }
        let last = r.len() - 3;
        let mut j = last as isize - 3;
        while j >= 3 && in_line(r, last, j as usize, j as usize - 3) {
            j -= 3;
        }
        let j = j.max(0) as usize;
        runs[ei * 8..ei * 8 + 8].copy_from_slice(&[r[0], r[1], r[k], r[k + 1], r[j], r[j + 1], r[last], r[last + 1]]);
    }
    let on_run = |ei: usize, end: u8, x: f64, y: f64| -> bool {
        let o = ei * 8 + if end == 1 { 0 } else { 4 };
        let x0 = runs[o];
        let y0 = runs[o + 1];
        let dx = runs[o + 2] - x0;
        let dy = runs[o + 3] - y0;
        let length = dx * dx + dy * dy;
        let t = if length > 0.0 {
            0f64.max(1f64.min(((x - x0) * dx + (y - y0) * dy) / length))
        } else {
            0.0
        };
        (x - x0 - t * dx).hypot(y - y0 - t * dy) < IN_LINE
    };
    let mut total = 0;
    for band in 0..bands {
        let band_end = band_start[band + 1];
        let bs = band_start[band];
        if band_end - bs > MAX_INSERT {
            stable_sort_by(&mut in_band[bs..band_end], |&p, &q| {
                crate::util::ord(s.ax[p] - s.ax[q]).then(p.cmp(&q))
            });
        } else {
            for i in bs + 1..band_end {
                let p = in_band[i];
                let mut j = i;
                while j > bs && s.ax[in_band[j - 1]] > s.ax[p] {
                    in_band[j] = in_band[j - 1];
                    j -= 1;
                }
                in_band[j] = p;
            }
        }
        for si in bs..band_end {
            let p = in_band[si];
            let dx = s.bx[p] - s.ax[p];
            let dy = s.by[p] - s.ay[p];
            let here = first[p] == band;
            for ti in si + 1..band_end {
                let q = in_band[ti];
                if s.ax[q] > s.bx[p] {
                    break;
                }
                if s.high[q] < s.low[p] || s.low[q] > s.high[p] || s.owner[p] == s.owner[q] || (!here && first[q] != band) {
                    continue;
                }
                let d1 = dx * (s.ay[q] - s.ay[p]) - dy * (s.ax[q] - s.ax[p]);
                let d2 = dx * (s.by[q] - s.ay[p]) - dy * (s.bx[q] - s.ax[p]);
                if (d1 > 0.0) == (d2 > 0.0) {
                    continue;
                }
                let ex = s.bx[q] - s.ax[q];
                let ey = s.by[q] - s.ay[q];
                let d3 = ex * (s.ay[p] - s.ay[q]) - ey * (s.ax[p] - s.ax[q]);
                let d4 = ex * (s.by[p] - s.ay[q]) - ey * (s.bx[p] - s.ax[q]);
                if (d3 > 0.0) == (d4 > 0.0) {
                    continue;
                }
                let ep = &edges[s.owner[p]];
                let eq = &edges[s.owner[q]];
                let at = d3 / (d3 - d4);
                let x = s.ax[p] + at * dx;
                let y = s.ay[p] + at * dy;
                let fan = |v: usize, pe: u8, qe: u8| -> bool {
                    (s.ends[p] & pe != 0 && s.ends[q] & qe != 0)
                        || ((x - nodes[v].x).abs() < nodes[v].w / 2.0 + FAN && (y - nodes[v].y).abs() < nodes[v].h / 2.0 + FAN)
                        || (on_run(s.owner[p], pe, x, y) && on_run(s.owner[q], qe, x, y))
                };
                if ep.src == eq.src && fan(ep.src, 1, 1) {
                    continue;
                }
                if ep.src == eq.dst && fan(ep.src, 1, 2) {
                    continue;
                }
                if ep.dst == eq.src && fan(ep.dst, 2, 1) {
                    continue;
                }
                if ep.dst == eq.dst && fan(ep.dst, 2, 2) {
                    continue;
                }
                total += 1;
            }
        }
    }
    total
}

// Match Mermaid: groups crossed by edges inherit their parent's direction.
pub fn inherit_crossed_directions(nodes: &mut [CNode], ends: &[(i64, i64)]) {
    let ancestors = |nodes: &[CNode], i: usize| -> Vec<usize> {
        let mut set: Vec<usize> = Vec::new();
        let mut p = nodes[i].parent;
        while p >= 0 && !set.contains(&(p as usize)) {
            set.push(p as usize);
            p = nodes[p as usize].parent;
        }
        set
    };
    for &(src, dst) in ends {
        if src < 0 || dst < 0 || src == dst {
            continue;
        }
        let a = ancestors(nodes, src as usize);
        let b = ancestors(nodes, dst as usize);
        for &gr in &a {
            if !b.contains(&gr) {
                nodes[gr].dir = None;
            }
        }
        for &gr in &b {
            if !a.contains(&gr) {
                nodes[gr].dir = None;
            }
        }
    }
}

pub fn shift_layout(nodes: &mut [CNode], edges: &mut [CEdge], ox: f64, oy: f64) {
    for c in nodes.iter_mut() {
        c.x += ox;
        c.y += oy;
    }
    for e in edges.iter_mut() {
        let mut k = 0;
        while k + 1 < e.route.len() {
            e.route[k] += ox;
            e.route[k + 1] += oy;
            k += 3;
        }
        e.label_x += ox;
        e.label_y += oy;
    }
}

fn start_side(dir: Dir) -> u8 {
    match dir {
        Dir::TB => TOP,
        Dir::BT => BOTTOM,
        Dir::LR => LEFT,
        Dir::RL => RIGHT,
    }
}

fn opposite(side: u8) -> u8 {
    (side + 2) & 3
}

fn is_vertical(dir: Dir) -> bool {
    dir == Dir::TB || dir == Dir::BT
}

struct Snapshot {
    nodes: Vec<f64>,
    routes: Vec<Vec<f64>>,
    labels: Vec<f64>,
    levels: Vec<f64>,
    total: CompoundResult,
}

struct Flat {
    ledges: Vec<LEdge>,
    ledge_of: Vec<i64>,
    last: Vec<i64>,
}

struct Laid {
    given: Vec<f64>,
    lnodes: Vec<LNode>,
    ledges: Vec<LEdge>,
    size: LayeredResult,
}

struct Compound<'a> {
    nodes: &'a mut [CNode],
    edges: &'a mut [CEdge],
    opt: &'a LayeredOptions,
    root_dir: Dir,
    levels: Vec<Level>,
    level_of: HashMap<i64, usize>,
    ports: Vec<PortEdge>,
    local: Vec<usize>,
    key: Vec<f64>,
    keyed: bool,
    flat: Option<Flat>,
    ordered: bool,
    laid: HashMap<i64, Vec<Laid>>,
    total: CompoundResult,
}

fn same(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
}

impl Compound<'_> {
    fn level(&self, id: i64) -> &Level {
        &self.levels[self.level_of[&id]]
    }

    fn port_in(&self, group: usize, ei: usize, out: bool) -> usize {
        self.level(group as i64).port_of[&(ei * 2 + if out { 0 } else { 1 })]
    }

    fn endpoint_below(&self, item: usize, ei: usize, out: bool) -> bool {
        let e = &self.edges[ei];
        (if out { e.src } else { e.dst }) != item
    }

    fn snapshot(&self) -> Snapshot {
        let mut at = Vec::new();
        for c in self.nodes.iter() {
            at.extend_from_slice(&[c.x, c.y, c.w, c.h]);
        }
        let mut labels = Vec::new();
        for e in self.edges.iter() {
            labels.push(e.label_x);
            labels.push(e.label_y);
        }
        let mut places = Vec::new();
        for level in &self.levels {
            places.push(level.x);
            places.push(level.y);
            for &p in &level.ports {
                places.push(self.ports[p].px);
                places.push(self.ports[p].py);
            }
        }
        Snapshot {
            nodes: at,
            routes: self.edges.iter().map(|e| e.route.clone()).collect(),
            labels,
            levels: places,
            total: self.total,
        }
    }

    fn restore(&mut self, s: &Snapshot) {
        for (i, c) in self.nodes.iter_mut().enumerate() {
            c.x = s.nodes[4 * i];
            c.y = s.nodes[4 * i + 1];
            c.w = s.nodes[4 * i + 2];
            c.h = s.nodes[4 * i + 3];
        }
        for (i, e) in self.edges.iter_mut().enumerate() {
            e.route = s.routes[i].clone();
            e.label_x = s.labels[2 * i];
            e.label_y = s.labels[2 * i + 1];
        }
        let mut k = 0;
        for li in 0..self.levels.len() {
            self.levels[li].x = s.levels[k];
            self.levels[li].y = s.levels[k + 1];
            k += 2;
            for pi in 0..self.levels[li].ports.len() {
                let p = self.levels[li].ports[pi];
                self.ports[p].px = s.levels[k];
                self.ports[p].py = s.levels[k + 1];
                k += 2;
            }
        }
        self.total = s.total;
    }

    // Order outgoing ports at the group's last rank; break ties using the next rank.
    fn far_hint(&self, li: usize, port: &PortEdge) -> f64 {
        let Some(flat) = &self.flat else { return f64::NAN };
        let level = &self.levels[li];
        let le = flat.ledge_of[port.e];
        if le < 0 || level.id < 0 || !port.out || port.side != opposite(start_side(level.dir)) {
            return f64::NAN;
        }
        let points = &flat.ledges[le as usize].points;
        let last = flat.last[level.id as usize] as f64;
        let mut inside = self.key[self.edges[port.e].src];
        let mut outside = 0.0;
        let mut k = 0;
        while k < points.len() {
            if points[k] == last {
                inside = points[k + 1];
            }
            if points[k] == last + 1.0 {
                outside = points[k + 1];
            }
            k += 2;
        }
        inside + outside * 1e-6
    }

    fn build(&mut self, li: usize, sized: bool) -> (Vec<LNode>, Vec<LEdge>, Vec<i64>) {
        let dir = self.levels[li].dir;
        let vertical = is_vertical(dir);
        let start = start_side(dir);
        let mut lnodes: Vec<LNode> = Vec::new();
        let mut ledges: Vec<LEdge> = Vec::new();
        let items = self.levels[li].items.clone();
        for (i, &item) in items.iter().enumerate() {
            self.local[item] = i;
            let node = &self.nodes[item];
            let mut ln = if sized {
                lnode(
                    if vertical { node.w } else { node.h },
                    if vertical { node.h } else { node.w },
                    Kind::Node,
                )
            } else {
                lnode(0.0, 0.0, Kind::Node)
            };
            ln.span = if node.is_group {
                node.span.min(ln.w - 2.0 * node.pad_x)
            } else {
                node.span
            };
            ln.key = if self.keyed { self.key[item] } else { f64::NAN };
            lnodes.push(ln);
        }
        let held = |s: &Self, item: usize, ei: usize, out: bool| s.nodes[item].is_group && s.endpoint_below(item, ei, out);
        let offset = |s: &Self, item: usize, ei: usize, out: bool| -> f64 {
            if !sized || !s.nodes[item].is_group || !s.endpoint_below(item, ei, out) {
                return 0.0;
            }
            let port = &s.ports[s.port_in(item, ei, out)];
            if vertical {
                port.px - s.nodes[item].w / 2.0
            } else {
                port.py - s.nodes[item].h / 2.0
            }
        };
        let level_edges = self.levels[li].edges.clone();
        for le in &level_edges {
            let e = &self.edges[le.e];
            let mut edge = ledge(self.local[le.a], self.local[le.b], e.minlen, 0.0, 0.0);
            if sized {
                edge.label_w = if vertical { e.label_w } else { e.label_h };
                edge.label_h = if vertical { e.label_h } else { e.label_w };
                edge.tail_dx = offset(self, le.a, le.e, true);
                edge.head_dx = offset(self, le.b, le.e, false);
                edge.tail_fixed = held(self, le.a, le.e, true);
                edge.head_fixed = held(self, le.b, le.e, false);
            }
            ledges.push(edge);
        }
        let port_ids = self.levels[li].ports.clone();
        let mut dummies = vec![-1i64; port_ids.len()];
        for (i, &pi) in port_ids.iter().enumerate() {
            let port = self.ports[pi].clone();
            if port.side == start || port.side == opposite(start) {
                let d = lnodes.len();
                let mut ln = lnode(0.0, 0.0, if port.side == start { Kind::StartPort } else { Kind::EndPort });
                ln.key = if self.keyed {
                    self.key[if port.out { self.edges[port.e].dst } else { self.edges[port.e].src }]
                } else {
                    f64::NAN
                };
                ln.hint = if port.hint.is_nan() && self.keyed && self.flat.is_some() {
                    self.far_hint(li, &port)
                } else {
                    port.hint
                };
                lnodes.push(ln);
                dummies[i] = d as i64;
                let mut edge = if port.out {
                    ledge(self.local[port.inner], d, 1.0, 0.0, 0.0)
                } else {
                    ledge(d, self.local[port.inner], 1.0, 0.0, 0.0)
                };
                if port.out {
                    edge.tail_dx = offset(self, port.inner, port.e, true);
                    edge.tail_fixed = held(self, port.inner, port.e, true);
                } else {
                    edge.head_dx = offset(self, port.inner, port.e, false);
                    edge.head_fixed = held(self, port.inner, port.e, false);
                }
                ledges.push(edge);
            } else {
                lnodes[self.local[port.inner]].pin = if port.side == TOP || port.side == LEFT { -1.0 } else { 1.0 };
            }
        }
        (lnodes, ledges, dummies)
    }

    fn lay_out(&mut self, li: usize, mut lnodes: Vec<LNode>, mut ledges: Vec<LEdge>) -> (Vec<LNode>, Vec<LEdge>, LayeredResult) {
        if !self.ordered {
            let size = layered(&mut lnodes, &mut ledges, self.opt);
            return (lnodes, ledges, size);
        }
        let mut given: Vec<f64> = Vec::new();
        for ln in &lnodes {
            given.extend_from_slice(&[ln.w, ln.h, ln.kind as u8 as f64, ln.pin, ln.hint, ln.key, ln.span]);
        }
        for le in &ledges {
            given.extend_from_slice(&[
                le.tail as f64,
                le.head as f64,
                le.minlen,
                le.label_w,
                le.label_h,
                le.tail_dx,
                le.head_dx,
                le.tail_fixed as u8 as f64,
                le.head_fixed as u8 as f64,
            ]);
        }
        let id = self.levels[li].id;
        let past = self.laid.entry(id).or_default();
        for p in past.iter() {
            if p.given.len() == given.len() && p.given.iter().zip(&given).all(|(&a, &b)| same(a, b)) {
                return (p.lnodes.clone(), p.ledges.clone(), p.size);
            }
        }
        let size = layered(&mut lnodes, &mut ledges, self.opt);
        past.push(Laid {
            given,
            lnodes: lnodes.clone(),
            ledges: ledges.clone(),
            size,
        });
        (lnodes, ledges, size)
    }

    fn place(&mut self) {
        // Rank parents first to determine which group sides edges cross.
        for li in 0..self.levels.len() {
            for pi in self.levels[li].ports.clone() {
                let (inner, e, out, side) = {
                    let p = &self.ports[pi];
                    (p.inner, p.e, p.out, p.side)
                };
                if self.nodes[inner].is_group && self.endpoint_below(inner, e, out) {
                    let q = self.port_in(inner, e, out);
                    self.ports[q].side = side;
                }
            }
            if self.levels[li].edges.is_empty() {
                continue;
            }
            let mut needed = false;
            for le in &self.levels[li].edges {
                let e = &self.edges[le.e];
                if (self.nodes[le.a].is_group && e.src != le.a) || (self.nodes[le.b].is_group && e.dst != le.b) {
                    needed = true;
                }
            }
            if !needed {
                continue;
            }
            let (mut lnodes, mut ledges, _) = self.build(li, false);
            orient(&lnodes, &mut ledges);
            rank(&mut lnodes, &ledges, 1);
            let start = start_side(self.levels[li].dir);
            for (i, le) in self.levels[li].edges.clone().iter().enumerate() {
                let (src, dst) = (self.edges[le.e].src, self.edges[le.e].dst);
                let a_above = !ledges[i].reversed;
                if self.nodes[le.a].is_group && src != le.a {
                    let q = self.port_in(le.a, le.e, true);
                    self.ports[q].side = if a_above { opposite(start) } else { start };
                }
                if self.nodes[le.b].is_group && dst != le.b {
                    let q = self.port_in(le.b, le.e, false);
                    self.ports[q].side = if a_above { start } else { opposite(start) };
                }
            }
        }

        // Lay out children first so their sizes are known.
        for li in (0..self.levels.len()).rev() {
            let id = self.levels[li].id;
            let dir = self.levels[li].dir;
            let vertical = is_vertical(dir);
            let flip = dir == Dir::BT || dir == Dir::RL;
            let (lnodes, ledges, dummies) = self.build(li, true);
            let (lnodes, ledges, size) = self.lay_out(li, lnodes, ledges);
            let content_w = if vertical { size.width } else { size.height };
            let content_h = if vertical { size.height } else { size.width };
            let (pad_x, pad_top, pad_bottom, min_w) = if id >= 0 {
                let g = &self.nodes[id as usize];
                (g.pad_x, g.pad_top, g.pad_bottom, g.min_w)
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };
            let box_w = (content_w + 2.0 * pad_x).max(min_w);
            let box_h = content_h + pad_top + pad_bottom;
            let off_x = (box_w - content_w) / 2.0;
            let off_y = pad_top;
            let tx = |lx: f64, ly: f64| {
                off_x
                    + if vertical {
                        lx
                    } else if flip {
                        size.height - ly
                    } else {
                        ly
                    }
            };
            let ty = |lx: f64, ly: f64| off_y + if vertical { if flip { size.height - ly } else { ly } } else { lx };
            let axis = if vertical { 0.0 } else { 1.0 };
            let convert = |points: &[f64]| -> Vec<f64> {
                let mut out = Vec::new();
                let mut i = 0;
                while i + 1 < points.len() {
                    out.push(tx(points[i], points[i + 1]));
                    out.push(ty(points[i], points[i + 1]));
                    out.push(axis);
                    i += 2;
                }
                out
            };

            let items = self.levels[li].items.clone();
            for (i, &item) in items.iter().enumerate() {
                self.nodes[item].x = tx(lnodes[i].x, lnodes[i].y);
                self.nodes[item].y = ty(lnodes[i].x, lnodes[i].y);
            }
            let level_edges = self.levels[li].edges.clone();
            for (i, le) in level_edges.iter().enumerate() {
                let edge = &ledges[i];
                self.levels[li].routes.insert(le.e, convert(&edge.points));
                let e = &mut self.edges[le.e];
                if e.label_w > 0.0 {
                    e.label_x = tx(edge.label_x, edge.label_y);
                    e.label_y = ty(edge.label_x, edge.label_y);
                }
            }
            let mut k = level_edges.len();
            for (i, pi) in self.levels[li].ports.clone().into_iter().enumerate() {
                let port = self.ports[pi].clone();
                if dummies[i] >= 0 {
                    let mut route = convert(&ledges[k].points);
                    k += 1;
                    let at = if port.out { route.len() - 3 } else { 0 };
                    let mut bx = route[at];
                    let mut by = route[at + 1];
                    match port.side {
                        TOP => by = 0.0,
                        BOTTOM => by = box_h,
                        LEFT => bx = 0.0,
                        _ => bx = box_w,
                    }
                    if port.out {
                        route.extend_from_slice(&[bx, by, axis]);
                    } else {
                        route.splice(0..0, [bx, by, axis]);
                    }
                    let p = &mut self.ports[pi];
                    p.px = bx;
                    p.py = by;
                    p.route = route;
                    continue;
                }
                let item = self.nodes[port.inner].clone();
                let mut ix = item.x;
                let mut iy = item.y;
                if item.is_group && self.endpoint_below(port.inner, port.e, port.out) {
                    let inner = &self.ports[self.port_in(port.inner, port.e, port.out)];
                    ix += inner.px - item.w / 2.0;
                    iy += inner.py - item.h / 2.0;
                } else {
                    match port.side {
                        TOP => iy -= item.h / 2.0,
                        BOTTOM => iy += item.h / 2.0,
                        LEFT => ix -= item.w / 2.0,
                        _ => ix += item.w / 2.0,
                    }
                }
                let mut bx = ix;
                let mut by = iy;
                match port.side {
                    TOP => by = 0.0,
                    BOTTOM => by = box_h,
                    LEFT => bx = 0.0,
                    _ => bx = box_w,
                }
                let a = if port.side == TOP || port.side == BOTTOM { 0.0 } else { 1.0 };
                let p = &mut self.ports[pi];
                p.px = bx;
                p.py = by;
                p.route = if port.out {
                    vec![ix, iy, a, bx, by, a]
                } else {
                    vec![bx, by, a, ix, iy, a]
                };
            }

            if id >= 0 {
                self.nodes[id as usize].w = box_w;
                self.nodes[id as usize].h = box_h;
            } else {
                self.total = CompoundResult {
                    width: box_w,
                    height: box_h,
                };
            }
        }

        for li in 0..self.levels.len() {
            let ox = self.levels[li].x;
            let oy = self.levels[li].y;
            for item in self.levels[li].items.clone() {
                self.nodes[item].x += ox;
                self.nodes[item].y += oy;
                if self.nodes[item].is_group {
                    let child = self.level_of[&(item as i64)];
                    self.levels[child].x = self.nodes[item].x - self.nodes[item].w / 2.0;
                    self.levels[child].y = self.nodes[item].y - self.nodes[item].h / 2.0;
                }
            }
        }

        let append = |route: &mut Vec<f64>, part: &[f64], ox: f64, oy: f64| {
            let mut i = 0;
            while i + 2 < part.len() {
                let x = part[i] + ox;
                let y = part[i + 1] + oy;
                let len = route.len();
                if !(len >= 3 && (route[len - 3] - x).abs() < 0.01 && (route[len - 2] - y).abs() < 0.01) {
                    route.extend_from_slice(&[x, y, part[i + 2]]);
                }
                i += 3;
            }
        };

        for li in 0..self.levels.len() {
            let level_id = self.levels[li].id;
            for le in self.levels[li].edges.clone() {
                let (src, dst) = (self.edges[le.e].src, self.edges[le.e].dst);
                let mut route: Vec<f64> = Vec::new();
                let mut outward: Vec<usize> = Vec::new();
                let mut item = src;
                while self.nodes[item].parent != level_id {
                    outward.push(self.level_of[&self.nodes[item].parent]);
                    item = self.nodes[item].parent as usize;
                }
                for lv in outward {
                    let pi = self.levels[lv].port_of[&(le.e * 2)];
                    append(&mut route, &self.ports[pi].route, self.levels[lv].x, self.levels[lv].y);
                }
                append(&mut route, &self.levels[li].routes[&le.e], self.levels[li].x, self.levels[li].y);
                let mut inward: Vec<usize> = Vec::new();
                let mut item = dst;
                while self.nodes[item].parent != level_id {
                    inward.push(self.level_of[&self.nodes[item].parent]);
                    item = self.nodes[item].parent as usize;
                }
                for &lv in inward.iter().rev() {
                    let pi = self.levels[lv].port_of[&(le.e * 2 + 1)];
                    append(&mut route, &self.ports[pi].route, self.levels[lv].x, self.levels[lv].y);
                }
                let (lx, ly) = (self.levels[li].x, self.levels[li].y);
                let e = &mut self.edges[le.e];
                e.route = route;
                if e.label_w > 0.0 {
                    e.label_x += lx;
                    e.label_y += ly;
                }
            }
        }
    }
}

pub fn compound_layout(nodes: &mut [CNode], edges: &mut [CEdge], root_dir: Dir, opt: &LayeredOptions) -> CompoundResult {
    let n = nodes.len();
    let mut levels: Vec<Level> = Vec::new();
    let mut level_of: HashMap<i64, usize> = HashMap::new();
    let make_level = |levels: &mut Vec<Level>, level_of: &mut HashMap<i64, usize>, id: i64, dir: Dir| -> usize {
        levels.push(Level {
            id,
            dir,
            items: Vec::new(),
            edges: Vec::new(),
            ports: Vec::new(),
            port_of: HashMap::new(),
            routes: HashMap::new(),
            x: 0.0,
            y: 0.0,
        });
        level_of.insert(id, levels.len() - 1);
        levels.len() - 1
    };
    let root = make_level(&mut levels, &mut level_of, -1, root_dir);

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut root_items: Vec<usize> = Vec::new();
    for i in 0..n {
        let p = nodes[i].parent;
        if p >= 0 && (p as usize) < n && p as usize != i && nodes[p as usize].is_group {
            children[p as usize].push(i);
        } else {
            nodes[i].parent = -1;
            root_items.push(i);
        }
    }
    levels[root].items = root_items;
    let mut depth = vec![0i64; n];
    let mut placed = vec![false; n];
    let descend = |levels: &mut Vec<Level>,
                   level_of: &mut HashMap<i64, usize>,
                   nodes: &[CNode],
                   children: &[Vec<usize>],
                   depth: &mut Vec<i64>,
                   placed: &mut Vec<bool>,
                   from: usize| {
        let mut q = from;
        while q < levels.len() {
            let (id, dir, items) = (levels[q].id, levels[q].dir, levels[q].items.clone());
            for item in items {
                placed[item] = true;
                depth[item] = if id < 0 { 0 } else { depth[id as usize] + 1 };
                if nodes[item].is_group {
                    let child = make_level(levels, level_of, item as i64, nodes[item].dir.unwrap_or(dir));
                    levels[child].items = children[item].clone();
                }
            }
            q += 1;
        }
    };
    descend(&mut levels, &mut level_of, nodes, &children, &mut depth, &mut placed, 0);
    // Groups that contain each other are unreachable from the root; lift them out.
    for i in 0..n {
        if placed[i] {
            continue;
        }
        let former = nodes[i].parent as usize;
        children[former].retain(|&c| c != i);
        nodes[i].parent = -1;
        levels[root].items.push(i);
        placed[i] = true;
        depth[i] = 0;
        if nodes[i].is_group {
            let from = levels.len();
            let child = make_level(&mut levels, &mut level_of, i as i64, nodes[i].dir.unwrap_or(root_dir));
            levels[child].items = children[i].clone();
            descend(&mut levels, &mut level_of, nodes, &children, &mut depth, &mut placed, from);
        }
    }

    for q in (1..levels.len()).rev() {
        let mut seq = f64::INFINITY;
        for &item in &levels[q].items {
            if nodes[item].seq < seq {
                seq = nodes[item].seq;
            }
        }
        if seq != f64::INFINITY {
            nodes[levels[q].id as usize].seq = seq;
        }
    }
    for level in levels.iter_mut() {
        stable_sort_by(&mut level.items, |&a, &b| {
            crate::util::ord(nodes[a].seq - nodes[b].seq).then(a.cmp(&b))
        });
    }

    // Every group border an edge crosses costs a port. Past the budget, an edge is drawn between
    // the groups that hold its ends, at the level they share.
    let mut ports_used: i64 = 0;
    for e in edges.iter_mut() {
        let mut a = e.src;
        let mut b = e.dst;
        let mut crossed = 0;
        while depth[a] > depth[b] {
            a = nodes[a].parent as usize;
            crossed += 1;
        }
        while depth[b] > depth[a] {
            b = nodes[b].parent as usize;
            crossed += 1;
        }
        while a != b && nodes[a].parent != nodes[b].parent {
            a = nodes[a].parent as usize;
            b = nodes[b].parent as usize;
            crossed += 2;
        }
        if a == b {
            continue;
        }
        if ports_used + crossed > MAX_PORTS {
            e.src = a;
            e.dst = b;
        } else {
            ports_used += crossed;
        }
    }

    let mut ports: Vec<PortEdge> = Vec::new();
    let mut loops: Vec<usize> = Vec::new();
    let mut stray: Vec<usize> = Vec::new();
    for ei in 0..edges.len() {
        edges[ei].route = Vec::new();
        if edges[ei].src == edges[ei].dst {
            loops.push(ei);
            continue;
        }
        let mut a = edges[ei].src;
        let mut b = edges[ei].dst;
        let mut a_path: Vec<usize> = Vec::new();
        let mut b_path: Vec<usize> = Vec::new();
        while depth[a] > depth[b] {
            a_path.push(a);
            a = nodes[a].parent as usize;
        }
        while depth[b] > depth[a] {
            b_path.push(b);
            b = nodes[b].parent as usize;
        }
        if a == b {
            stray.push(ei);
            continue;
        }
        while nodes[a].parent != nodes[b].parent {
            a_path.push(a);
            b_path.push(b);
            a = nodes[a].parent as usize;
            b = nodes[b].parent as usize;
        }
        let shared = level_of[&nodes[a].parent];
        levels[shared].edges.push(LevelEdge { e: ei, a, b });
        for (path, out) in [(&a_path, true), (&b_path, false)] {
            for &inner in path {
                let li = level_of[&nodes[inner].parent];
                ports.push(PortEdge {
                    e: ei,
                    inner,
                    out,
                    side: TOP,
                    hint: f64::NAN,
                    px: 0.0,
                    py: 0.0,
                    route: Vec::new(),
                });
                let pi = ports.len() - 1;
                levels[li].ports.push(pi);
                levels[li].port_of.insert(ei * 2 + if out { 0 } else { 1 }, pi);
            }
        }
    }

    let ordered = ports_used > 1 && ports_used <= MAX_ORDERED_PORTS && edges.len() <= MAX_ORDERED_EDGES;

    // Use a flat layout to coordinate node and port order across groups.
    let mut key = vec![f64::NAN; n];
    let mut flat_crossings = 0.0;
    let mut flat_info: Option<Flat> = None;
    let mut flat = levels.len() > 1 && ordered;
    for level in &levels {
        if level.dir != root_dir {
            flat = false;
        }
    }
    if flat {
        let vertical = is_vertical(root_dir);
        let mut leaf = vec![-1i64; n];
        let mut group = vec![-1i64; n];
        let mut lnodes: Vec<LNode> = Vec::new();
        let mut of: Vec<i64> = Vec::new();
        let mut parent: Vec<i64> = Vec::new();
        for i in 0..n {
            if !nodes[i].is_group {
                continue;
            }
            group[i] = parent.len() as i64;
            parent.push(-1);
        }
        for i in 0..n {
            let p = nodes[i].parent;
            if nodes[i].is_group {
                parent[group[i] as usize] = if p >= 0 { group[p as usize] } else { -1 };
            } else {
                leaf[i] = lnodes.len() as i64;
                lnodes.push(lnode(
                    if vertical { nodes[i].w } else { nodes[i].h },
                    if vertical { nodes[i].h } else { nodes[i].w },
                    Kind::Node,
                ));
                of.push(if p >= 0 { group[p as usize] } else { -1 });
            }
        }
        let mut ledges: Vec<LEdge> = Vec::new();
        let mut ledge_of = vec![-1i64; edges.len()];
        for (ei, e) in edges.iter().enumerate() {
            if e.src == e.dst || leaf[e.src] < 0 || leaf[e.dst] < 0 {
                continue;
            }
            ledge_of[ei] = ledges.len() as i64;
            ledges.push(ledge(
                leaf[e.src] as usize,
                leaf[e.dst] as usize,
                e.minlen,
                if vertical { e.label_w } else { e.label_h },
                if vertical { e.label_h } else { e.label_w },
            ));
        }
        let mut flat_opt = opt.clone();
        flat_opt.groups = Some(Groups { of, parent });
        flat_opt.order_only = true;
        let flat_layout = layered(&mut lnodes, &mut ledges, &flat_opt);
        flat_crossings = flat_layout.crossings.unwrap_or(0.0).min(flat_layout.declared.unwrap_or(0.0));
        let mut sum = vec![0.0f64; n];
        let mut count = vec![0i64; n];
        let mut last = vec![-1i64; n];
        for i in 0..n {
            if leaf[i] < 0 {
                continue;
            }
            key[i] = lnodes[leaf[i] as usize].x;
            let mut p = nodes[i].parent;
            while p >= 0 {
                sum[p as usize] += key[i];
                count[p as usize] += 1;
                last[p as usize] = last[p as usize].max(lnodes[leaf[i] as usize].rank);
                p = nodes[p as usize].parent;
            }
        }
        for i in 0..n {
            if count[i] > 0 {
                key[i] = sum[i] / count[i] as f64;
            }
        }
        flat_info = Some(Flat { ledges, ledge_of, last });
    }

    let mut c = Compound {
        nodes,
        edges,
        opt,
        root_dir,
        levels,
        level_of,
        ports,
        local: vec![0; n],
        key,
        keyed: false,
        flat: flat_info,
        ordered,
        laid: HashMap::new(),
        total: CompoundResult::default(),
    };
    let _ = c.root_dir;

    // The flat layout ignores group boxes, so independent ordering may yield fewer crossings.
    let style = style_of(opt);
    c.keyed = flat;
    c.place();
    let mut crossings: i64 = -1;
    if flat {
        crossings = route_crossings(c.nodes, c.edges, style);
        if crossings as f64 > flat_crossings * FLAT_SLACK + FLAT_SPARE {
            let first = c.snapshot();
            c.keyed = false;
            c.place();
            let own = route_crossings(c.nodes, c.edges, style);
            if own >= crossings {
                c.keyed = true;
                c.restore(&first);
            } else {
                crossings = own;
            }
        }
    }

    // Refine port order using routes outside each group.
    if c.keyed == flat && c.ordered {
        let mut again = false;
        for li in 0..c.levels.len() {
            let level = &c.levels[li];
            if level.id < 0 {
                continue;
            }
            let bx_node = c.nodes[level.id as usize].clone();
            let vertical = is_vertical(level.dir);
            let end = opposite(start_side(level.dir));
            let (lx, ly) = (level.x, level.y);
            let mut seen: Vec<(f64, f64)> = Vec::new();
            for pi in level.ports.clone() {
                let port = &c.ports[pi];
                if port.side != end {
                    continue;
                }
                let route = &c.edges[port.e].route;
                let bx = lx + port.px;
                let by = ly + port.py;
                let mut at: isize = -1;
                let mut k = 0;
                while k < route.len() && (at < 0 || port.out) {
                    if (route[k] - bx).abs() < 0.01 && (route[k + 1] - by).abs() < 0.01 {
                        at = k as isize;
                    }
                    k += 3;
                }
                if at < 0 {
                    continue;
                }
                let a = if vertical { 0 } else { 1 };
                let step: isize = if port.out { 3 } else { -3 };
                let mut next = at + step;
                while next >= 0 && (next as usize) < route.len() && (route[next as usize + a] - route[at as usize + a]).abs() < IN_LINE {
                    next += step;
                }
                if next < 0 || next as usize >= route.len() {
                    continue;
                }
                let hint = route[next as usize + a]
                    - if vertical {
                        bx_node.x - bx_node.w / 2.0
                    } else {
                        bx_node.y - bx_node.h / 2.0
                    };
                let px = if vertical { port.px } else { port.py };
                c.ports[pi].hint = hint;
                seen.push((px, hint));
            }
            stable_sort_by(&mut seen, |p, q| crate::util::ord(p.0 - q.0));
            for i in 1..seen.len() {
                if seen[i - 1].1 > seen[i].1 {
                    again = true;
                }
            }
        }
        if again {
            let before = if crossings >= 0 {
                crossings
            } else {
                route_crossings(c.nodes, c.edges, style)
            };
            if before > 0 {
                let first = c.snapshot();
                c.place();
                if route_crossings(c.nodes, c.edges, style) >= before {
                    c.restore(&first);
                }
            }
        }
    }

    let total = c.total;
    let edges = c.edges;
    let nodes = c.nodes;
    for ei in stray {
        let (a, b) = (&nodes[edges[ei].src], &nodes[edges[ei].dst]);
        let (ax, ay, bx, by) = (a.x, a.y, b.x, b.y);
        let e = &mut edges[ei];
        e.route = vec![ax, ay, 0.0, bx, by, 0.0];
        e.label_x = (ax + bx) / 2.0;
        e.label_y = (ay + by) / 2.0;
    }
    for ei in loops {
        let node = &nodes[edges[ei].src];
        let (x, y, w) = (node.x, node.y, node.w);
        let e = &mut edges[ei];
        e.label_x = x + w / 2.0 + 28.0 + e.label_w / 2.0;
        e.label_y = y;
    }
    total
}
