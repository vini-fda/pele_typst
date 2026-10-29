use std::collections::HashMap;

use regex::Regex;

use super::builder::{esc_text, label_svg};
use super::ink::{fill_of, ink};
use crate::layout::layered::ease;
use crate::text::label::Label;
use crate::util::{js_round, js_sign, num, parse_float, trim};

pub const ARROW: f64 = 8.0;

#[derive(Debug, Clone, Default)]
pub struct EdgePath {
    pub d: String,
    pub tokens: Option<Vec<f64>>,
    pub sx: f64,
    pub sy: f64,
    pub sdx: f64,
    pub sdy: f64,
    pub ex: f64,
    pub ey: f64,
    pub edx: f64,
    pub edy: f64,
}

// The most a corner is rounded by in a stepped route.
const CORNER: f64 = 12.0;
// How far the handles of a wide turn reach along its two legs.
const HANDLE: f64 = 0.62;
// How far an edge runs straight before a marker at its end, where it has the room.
pub const LEAD: f64 = 6.0;

// How far from each side of each node the edges that meet it start to turn.
#[derive(Default)]
pub struct SideReach {
    reach: HashMap<i64, f64>,
}

impl SideReach {
    pub fn add(&mut self, node: usize, side: i64, kind: &str) {
        let trim = marker_trim(kind);
        let key = node as i64 * 4 + side;
        if side >= 0 && trim > 0.0 && trim + LEAD > self.reach.get(&key).copied().unwrap_or(0.0) {
            self.reach.insert(key, trim + LEAD);
        }
    }

    pub fn of(&self, node: usize, side: i64) -> f64 {
        if side >= 0 { self.reach.get(&(node as i64 * 4 + side)).copied().unwrap_or(0.0) } else { 0.0 }
    }
}

fn at(v: &[f64], i: isize) -> f64 {
    if i < 0 || i as usize >= v.len() { f64::NAN } else { v[i as usize] }
}

fn near(ax: f64, ay: f64, bx: f64, by: f64) -> bool {
    (ax - bx).abs() < 0.01 && (ay - by).abs() < 0.01
}

// Match the value parsed from num() output.
fn written(v: f64) -> f64 {
    let n = js_round(v * 100.0);
    if n == 0.0 { 0.0 } else { n / 100.0 }
}

struct Route {
    xs: Vec<f64>,
    ys: Vec<f64>,
    linear: bool,
}

impl Route {
    fn aligned(&self, i: isize) -> bool {
        (at(&self.xs, i) - at(&self.xs, i - 1)).abs() < 0.01 || (at(&self.ys, i) - at(&self.ys, i - 1)).abs() < 0.01
    }
    fn straight(&self, i: isize) -> bool {
        self.linear || self.aligned(i)
    }
    // The way the route heads from point i to point j: along the flow where an S joins them.
    fn unit(&self, i: isize, j: isize, axis: f64) -> (f64, f64) {
        if !self.straight(i.max(j)) {
            let or1 = |s: f64| if s == 0.0 || s.is_nan() { 1.0 } else { s };
            return if axis == 0.0 {
                (0.0, or1(js_sign(at(&self.ys, j) - at(&self.ys, i))))
            } else {
                (or1(js_sign(at(&self.xs, j) - at(&self.xs, i))), 0.0)
            };
        }
        let dx = at(&self.xs, j) - at(&self.xs, i);
        let dy = at(&self.ys, j) - at(&self.ys, i);
        let mut len = dx.hypot(dy);
        if len == 0.0 || len.is_nan() {
            len = 1.0;
        }
        (dx / len, dy / len)
    }
    fn length(&self, i: isize) -> f64 {
        (at(&self.xs, i) - at(&self.xs, i - 1)).abs() + (at(&self.ys, i) - at(&self.ys, i - 1)).abs()
    }
    // Whether the route turns a corner at point i.
    fn turns(&self, i: isize, last: isize) -> bool {
        !self.linear
            && i > 0
            && i < last
            && self.aligned(i)
            && self.aligned(i + 1)
            && self.length(i) >= 0.01
            && self.length(i + 1) >= 0.01
            && (((at(&self.xs, i) - at(&self.xs, i - 1)).abs() < 0.01) != ((at(&self.xs, i + 1) - at(&self.xs, i)).abs() < 0.01))
    }
    fn along(&self, i: isize, ux: f64, uy: f64) -> f64 {
        (at(&self.xs, i) - at(&self.xs, i - 1)).abs() * ux.abs() + (at(&self.ys, i) - at(&self.ys, i - 1)).abs() * uy.abs()
    }
}

struct Writer {
    d: String,
    tokens: Vec<f64>,
}

impl Writer {
    fn tok(&mut self, v: f64) -> String {
        self.tokens.push(written(v));
        num(v)
    }
}

fn lead(reach: f64, trim: f64, run: f64) -> f64 {
    let room = (reach - LEAD - trim).max(0.0).min(run / 4.0);
    room + (reach - trim - room).min((run - room) / 4.0)
}

// Builds the path for a route of x, y, axis triples, trimming both ends to leave room for markers.
pub fn route_path(
    route: &[f64],
    curve: Option<&str>,
    start_trim: f64,
    end_trim: f64,
    start_reach: Option<f64>,
    end_reach: Option<f64>,
) -> EdgePath {
    let start_reach = start_reach.unwrap_or(if start_trim > 0.0 { start_trim + LEAD } else { 0.0 });
    let end_reach = end_reach.unwrap_or(if end_trim > 0.0 { end_trim + LEAD } else { 0.0 });
    let linear = curve == Some("linear");
    let stepped = matches!(curve, Some("step") | Some("stepBefore") | Some("stepAfter"));
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    let mut axes: Vec<f64> = Vec::new();
    let mut i = 0;
    while i + 2 < route.len() + 2 && i < route.len() {
        let x = route[i];
        let y = route.get(i + 1).copied().unwrap_or(f64::NAN);
        let axis = route.get(i + 2).copied().unwrap_or(f64::NAN);
        // A stepped route turns at right angles where a smooth one would curve.
        if stepped && !xs.is_empty() {
            let last = xs.len() - 1;
            if (x - xs[last]).abs() >= 0.01 && (y - ys[last]).abs() >= 0.01 {
                let px = xs[last];
                let py = ys[last];
                let my = if curve == Some("stepBefore") { py } else if curve == Some("stepAfter") { y } else { (py + y) / 2.0 };
                let mx = if curve == Some("stepBefore") { px } else if curve == Some("stepAfter") { x } else { (px + x) / 2.0 };
                let turns = if axis == 0.0 { [px, my, x, my] } else { [mx, py, mx, y] };
                for k in [0, 2] {
                    if near(turns[k], turns[k + 1], px, py) || near(turns[k], turns[k + 1], x, y) {
                        continue;
                    }
                    xs.push(turns[k]);
                    ys.push(turns[k + 1]);
                    axes.push(axis);
                }
            }
        }
        xs.push(x);
        ys.push(y);
        axes.push(axis);
        i += 3;
    }
    if xs.len() < 2 {
        let (x, y) = (xs.first().copied().unwrap_or(0.0), ys.first().copied().unwrap_or(0.0));
        return EdgePath { d: format!("M{},{}", num(x), num(y)), tokens: Some(vec![77.0, 2.0, written(x), written(y)]), sx: x, sy: y, ex: x, ey: y, ..Default::default() };
    }
    let mut r = Route { xs, ys, linear };
    let mut last = r.xs.len() as isize - 1;
    let (sdx, sdy) = r.unit(0, 1, axes[1]);
    let (edx, edy) = r.unit(last - 1, last, axes[last as usize]);
    let sx = r.xs[0];
    let sy = r.ys[0];
    let ex = r.xs[last as usize];
    let ey = r.ys[last as usize];
    r.xs[0] += sdx * start_trim;
    r.ys[0] += sdy * start_trim;
    r.xs[last as usize] -= edx * end_trim;
    r.ys[last as usize] -= edy * end_trim;
    let arrives = if end_reach > end_trim && (!r.straight(last) || r.turns(last - 1, last)) {
        lead(end_reach, end_trim, r.along(last, edx, edy))
    } else {
        0.0
    };
    let leaves = if start_reach > start_trim && (!r.straight(1) || r.turns(1, last)) {
        lead(start_reach, start_trim, r.along(1, sdx, sdy))
    } else {
        0.0
    };
    if arrives >= 1.0 {
        let l = last as usize;
        let (vx, vy, va) = (r.xs[l] - edx * arrives, r.ys[l] - edy * arrives, axes[l]);
        r.xs.insert(l, vx);
        r.ys.insert(l, vy);
        axes.insert(l, va);
        last += 1;
    }
    if leaves >= 1.0 {
        let (vx, vy, va) = (r.xs[0] + sdx * leaves, r.ys[0] + sdy * leaves, axes[1]);
        r.xs.insert(1, vx);
        r.ys.insert(1, vy);
        axes.insert(1, va);
        last += 1;
    }

    // Keep tokens to avoid parsing d when checking fills.
    let mut w = Writer { d: String::new(), tokens: vec![77.0, 2.0] };
    let a = w.tok(r.xs[0]);
    let b = w.tok(r.ys[0]);
    w.d = format!("M{},{}", a, b);
    let mut at_x = r.xs[0];
    let mut at_y = r.ys[0];
    let line_to = |w: &mut Writer, x: f64, y: f64, at_x: &mut f64, at_y: &mut f64| {
        if near(x, y, *at_x, *at_y) {
            return;
        }
        w.tokens.push(76.0);
        w.tokens.push(2.0);
        let a = w.tok(x);
        let b = w.tok(y);
        w.d += &format!("L{},{}", a, b);
        *at_x = x;
        *at_y = y;
    };
    for i in 1..=last {
        let iu = i as usize;
        let x = r.xs[iu];
        let y = r.ys[iu];
        if !r.straight(i) {
            let down = axes[iu] == 0.0;
            let along = if down { y - at_y } else { x - at_x };
            let k = along.abs() * ease(if down { x - at_x } else { y - at_y }, along);
            let ux = if down { 0.0 } else { js_sign(along) * k };
            let uy = if down { js_sign(along) * k } else { 0.0 };
            w.tokens.push(67.0);
            w.tokens.push(6.0);
            let p = [w.tok(at_x + ux), w.tok(at_y + uy), w.tok(x - ux), w.tok(y - uy), w.tok(x), w.tok(y)];
            w.d += &format!("C{},{} {},{} {},{}", p[0], p[1], p[2], p[3], p[4], p[5]);
            at_x = x;
            at_y = y;
            continue;
        }
        if !r.turns(i, last) {
            // A point part of the way along one straight run adds nothing to the path.
            if i < last
                && r.aligned(i)
                && r.aligned(i + 1)
                && (x - r.xs[iu - 1]) * (r.xs[iu + 1] - x) + (y - r.ys[iu - 1]) * (r.ys[iu + 1] - y) > 0.0
            {
                continue;
            }
            line_to(&mut w, x, y, &mut at_x, &mut at_y);
            continue;
        }
        // How much of the leg before the turn and of the leg after it the turn takes.
        let mut before = if r.turns(i - 1, last) { r.length(i) / 2.0 } else { r.length(i) };
        let mut after = if r.turns(i + 1, last) { r.length(i + 1) / 2.0 } else { r.length(i + 1) };
        if stepped {
            let m = CORNER.min(before).min(after);
            before = m;
            after = m;
        }
        let (ux, uy) = r.unit(i - 1, i, axes[iu]);
        let (vx, vy) = r.unit(i, i + 1, axes[iu + 1]);
        line_to(&mut w, x - ux * before, y - uy * before, &mut at_x, &mut at_y);
        at_x = x + vx * after;
        at_y = y + vy * after;
        if stepped {
            w.tokens.push(81.0);
            w.tokens.push(4.0);
            let p = [w.tok(x), w.tok(y), w.tok(at_x), w.tok(at_y)];
            w.d += &format!("Q{},{} {},{}", p[0], p[1], p[2], p[3]);
        } else {
            w.tokens.push(67.0);
            w.tokens.push(6.0);
            let p = [
                w.tok(x - ux * before * (1.0 - HANDLE)),
                w.tok(y - uy * before * (1.0 - HANDLE)),
                w.tok(x + vx * after * (1.0 - HANDLE)),
                w.tok(y + vy * after * (1.0 - HANDLE)),
                w.tok(at_x),
                w.tok(at_y),
            ];
            w.d += &format!("C{},{} {},{} {},{}", p[0], p[1], p[2], p[3], p[4], p[5]);
        }
    }
    EdgePath { d: w.d, tokens: Some(w.tokens), sx, sy, sdx: -sdx, sdy: -sdy, ex, ey, edx, edy }
}

pub fn marker_trim(kind: &str) -> f64 {
    match kind {
        "arrow_point" => ARROW - 1.0,
        "arrow_circle" => ARROW,
        "arrow_cross" => 4.0,
        _ => 0.0,
    }
}

// Draws a marker whose tip is at (x, y), pointing along (dx, dy).
pub fn marker(kind: &str, x: f64, y: f64, dx: f64, dy: f64, color: &str, stroke: &str) -> String {
    let nx = -dy;
    let ny = dx;
    match kind {
        "arrow_point" => {
            let bx = x - dx * ARROW;
            let by = y - dy * ARROW;
            let half = ARROW * 0.42;
            format!(
                "<path class=\"pele-marker\" d=\"M{},{}L{},{}L{},{}Z\" fill=\"{}\" stroke=\"{}\"/>",
                num(x),
                num(y),
                num(bx + nx * half),
                num(by + ny * half),
                num(bx - nx * half),
                num(by - ny * half),
                color,
                stroke
            )
        }
        "arrow_circle" => format!(
            "<circle class=\"pele-marker\" cx=\"{}\" cy=\"{}\" r=\"3.5\" fill=\"{}\" stroke=\"{}\"/>",
            num(x - dx * 4.0),
            num(y - dy * 4.0),
            color,
            stroke
        ),
        "arrow_cross" => {
            let cx = x - dx * 4.0;
            let cy = y - dy * 4.0;
            let a = 3.5;
            let ux = (dx + nx) * a;
            let uy = (dy + ny) * a;
            let vx = (dx - nx) * a;
            let vy = (dy - ny) * a;
            format!(
                "<path class=\"pele-marker\" d=\"M{},{}L{},{}M{},{}L{},{}\"/>",
                num(cx - ux),
                num(cy - uy),
                num(cx + ux),
                num(cy + uy),
                num(cx - vx),
                num(cy - vy),
                num(cx + vx),
                num(cy + vy)
            )
        }
        _ => String::new(),
    }
}

// Each command stores its character code, argument count, and arguments.
fn tokenize(d: &str) -> Vec<f64> {
    let b = d.as_bytes();
    let n = b.len();
    let code = |j: usize| -> i32 { if j < n { b[j] as i32 } else { -1 } };
    let mut out: Vec<f64> = Vec::new();
    let mut at: isize = -1;
    let mut i = 0;
    while i < n {
        let c = b[i] as i32;
        if (65..=90).contains(&c) && c != 69 {
            at = out.len() as isize + 1;
            out.push(c as f64);
            out.push(0.0);
            i += 1;
        } else if c == 45 || c == 46 || (48..=57).contains(&c) {
            let mut j = if c == 45 { i + 1 } else { i };
            let mut m: f64 = 0.0;
            let mut places: i32 = -1;
            loop {
                let k = code(j);
                if (48..=57).contains(&k) {
                    m = m * 10.0 + (k - 48) as f64;
                    if places >= 0 {
                        places += 1;
                    }
                } else if k == 46 && places < 0 {
                    places = 0;
                } else {
                    break;
                }
                j += 1;
            }
            let v;
            if (code(j) | 32) == 101 || m > 9007199254740991.0 || places > 22 {
                loop {
                    let k = code(j);
                    let prev = if j > 0 { code(j - 1) | 32 } else { -1 };
                    if k == 46 || k == 101 || k == 69 || (48..=57).contains(&k) || ((k == 45 || k == 43) && prev == 101) {
                        j += 1;
                    } else {
                        break;
                    }
                }
                v = d[i..j].parse::<f64>().unwrap_or(f64::NAN);
            } else {
                let mut x = if places > 0 { m / 10f64.powi(places) } else { m };
                if c == 45 {
                    x = -x;
                }
                v = x;
            }
            if at >= 0 {
                out.push(v);
                out[at as usize] += 1.0;
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

// Samples curves and arcs; NaN separates subpaths.
fn sample(tokens: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    let mut x = 0.0;
    let mut y = 0.0;
    let mut i = 0;
    while i + 1 < tokens.len() {
        let cmd = tokens[i] as i32;
        let count = tokens[i + 1] as usize;
        let s = i + 2;
        let e = (s + count).min(tokens.len());
        i = s + count;
        if cmd == 72 || cmd == 86 {
            for &t in &tokens[s..e] {
                if cmd == 72 {
                    x = t;
                } else {
                    y = t;
                }
                out.push(x);
                out.push(y);
            }
        } else if cmd == 67 || cmd == 81 {
            let cubic = cmd == 67;
            let step = if cubic { 6 } else { 4 };
            let mut k = s;
            while k + step <= e {
                let x1 = tokens[k];
                let y1 = tokens[k + 1];
                let x2 = tokens[k + 2];
                let y2 = tokens[k + 3];
                let x3 = if cubic { tokens[k + 4] } else { x2 };
                let y3 = if cubic { tokens[k + 5] } else { y2 };
                let mut t = 0.125;
                while t <= 1.0 {
                    // Preserve the original De Casteljau operation order to keep rounding unchanged.
                    let mut ax = x + (x1 - x) * t;
                    let mut ay = y + (y1 - y) * t;
                    let mut bx = x1 + (x2 - x1) * t;
                    let mut by = y1 + (y2 - y1) * t;
                    if cubic {
                        let cx = x2 + (x3 - x2) * t;
                        let cy = y2 + (y3 - y2) * t;
                        ax += (bx - ax) * t;
                        ay += (by - ay) * t;
                        bx += (cx - bx) * t;
                        by += (cy - by) * t;
                    }
                    out.push(ax + (bx - ax) * t);
                    out.push(ay + (by - ay) * t);
                    t += 0.125;
                }
                x = x3;
                y = y3;
                k += step;
            }
        } else if cmd == 65 {
            let mut k = s;
            while k + 7 <= e {
                arc(&mut out, x, y, tokens[k], tokens[k + 1], tokens[k + 3], tokens[k + 4], tokens[k + 5], tokens[k + 6]);
                x = tokens[k + 5];
                y = tokens[k + 6];
                k += 7;
            }
        } else {
            let mut k = s;
            while k + 2 <= e {
                if cmd == 77 && k == s && !out.is_empty() {
                    out.push(f64::NAN);
                    out.push(f64::NAN);
                }
                x = tokens[k];
                y = tokens[k + 1];
                out.push(x);
                out.push(y);
                k += 2;
            }
        }
    }
    out
}

// Samples an unrotated arc from (x1, y1), after SVG's conversion from end points to a center.
#[allow(clippy::too_many_arguments)]
fn arc(out: &mut Vec<f64>, x1: f64, y1: f64, rx: f64, ry: f64, large: f64, sweep: f64, x2: f64, y2: f64) {
    let hx = (x1 - x2) / 2.0;
    let hy = (y1 - y2) / 2.0;
    if rx == 0.0 || ry == 0.0 || (hx == 0.0 && hy == 0.0) {
        out.push(x2);
        out.push(y2);
        return;
    }
    let scale = 1f64.max(((hx * hx) / (rx * rx) + (hy * hy) / (ry * ry)).sqrt());
    let rx = rx * scale;
    let ry = ry * scale;
    let k = (if large == sweep { -1.0 } else { 1.0 })
        * (0f64.max((rx * rx * ry * ry - rx * rx * hy * hy - ry * ry * hx * hx) / (rx * rx * hy * hy + ry * ry * hx * hx))).sqrt();
    let cx = (k * rx * hy) / ry + (x1 + x2) / 2.0;
    let cy = (-k * ry * hx) / rx + (y1 + y2) / 2.0;
    let from = ((y1 - cy) / ry).atan2((x1 - cx) / rx);
    let mut turn = ((y2 - cy) / ry).atan2((x2 - cx) / rx) - from;
    if sweep != 0.0 && turn < 0.0 {
        turn += 2.0 * std::f64::consts::PI;
    } else if sweep == 0.0 && turn > 0.0 {
        turn -= 2.0 * std::f64::consts::PI;
    }
    for i in 1..=8 {
        let a = from + (turn * i as f64) / 8.0;
        out.push(cx + rx * a.cos());
        out.push(cy + ry * a.sin());
    }
}

// Test segment overlap with the box using Liang-Barsky clipping.
#[allow(clippy::too_many_arguments)]
fn clips(x: f64, y: f64, dx: f64, dy: f64, l: f64, t: f64, r: f64, b: f64) -> bool {
    let mut lo: f64 = 0.0;
    let mut hi: f64 = 1.0;
    for side in 0..4 {
        let p = [-dx, dx, -dy, dy][side];
        let q = [x - l, r - x, y - t, b - y][side];
        if p == 0.0 {
            if q < 0.0 {
                hi = -1.0;
            }
        } else if p < 0.0 {
            lo = crate::util::js_max(lo, q / p);
        } else {
            hi = crate::util::js_min(hi, q / p);
        }
    }
    lo < hi
}

fn enters(points: &[f64], l: f64, t: f64, r: f64, b: f64) -> bool {
    let mut k = 0;
    while k + 2 < points.len() {
        let x = points[k];
        let y = points[k + 1];
        let x2 = points[k + 2];
        let y2 = points.get(k + 3).copied().unwrap_or(f64::NAN);
        k += 2;
        if (x < l && x2 < l) || (x > r && x2 > r) || (y < t && y2 < t) || (y > b && y2 > b) {
            continue;
        }
        if clips(x, y, x2 - x, y2 - y, l, t, r, b) {
            return true;
        }
    }
    false
}

// Fill bounds are [left, top, right, bottom].
#[derive(Debug, Clone)]
pub struct Fill {
    pub bx: [f64; 4],
    pub shape: String,
}

pub enum Under<'a> {
    Off,
    Crosses,
    On(&'a Fill),
}

// The innermost fill holding the whole edge and its marker tips, Crosses when it crosses a border.
pub fn fill_under<'a>(d: &str, tips: &[f64], fills: &'a [Fill], tokens: Option<&[f64]>) -> Under<'a> {
    if fills.is_empty() {
        return Under::Off;
    }
    let points = match tokens {
        Some(t) => sample(t),
        None => sample(&tokenize(d)),
    };
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut k = 0;
    while k < points.len() + tips.len() {
        let x = if k < points.len() { points[k] } else { tips[k - points.len()] };
        let y = if k < points.len() { points[k + 1] } else { tips[k + 1 - points.len()] };
        if x < min_x {
            min_x = x;
        }
        if x > max_x {
            max_x = x;
        }
        if y < min_y {
            min_y = y;
        }
        if y > max_y {
            max_y = y;
        }
        k += 2;
    }
    let mut holder: Option<&Fill> = None;
    let mut least = f64::INFINITY;
    for fill in fills {
        let [l, t, r, b] = fill.bx;
        if max_x < l + 0.5 || min_x > r - 0.5 || max_y < t + 0.5 || min_y > b - 0.5 {
            continue;
        }
        let mut touched = enters(&points, l + 0.5, t + 0.5, r - 0.5, b - 0.5);
        let mut k = 0;
        while k < tips.len() && !touched {
            touched = tips[k] > l + 0.5 && tips[k] < r - 0.5 && tips[k + 1] > t + 0.5 && tips[k + 1] < b - 0.5;
            k += 2;
        }
        if !touched {
            continue;
        }
        if min_x < l || max_x > r || min_y < t || max_y > b {
            return Under::Crosses;
        }
        let area = (r - l) * (b - t);
        if area < least {
            least = area;
            holder = Some(fill);
        }
    }
    match holder {
        Some(f) => Under::On(f),
        None => Under::Off,
    }
}

// The fill's ink for an edge on it, or '' for a casing when unknown or when the line style sets a stroke.
pub fn edge_ink(fill: &Fill, line: &str) -> String {
    if line.contains("\"stroke:") || line.contains(";stroke:") {
        return String::new();
    }
    ink(&fill.shape, "").map(String::from).unwrap_or_default()
}

// Markers with the page color inside them, with the fill's color there instead.
pub fn hollow_on(markers: &str, fill: &Fill) -> String {
    match fill_of(&fill.shape).map(|c| trim(&c).to_string()) {
        Some(color) if !color.is_empty() => markers.replace(" fill=\"var(--_bg)\"", &format!(" fill=\"{}\"", color)),
        _ => markers.to_string(),
    }
}

// The ink of the innermost fill that holds the whole edge, Some("") for a casing where it
// crosses a border, or None off fills.
pub fn over_fills(d: &str, tips: &[f64], fills: &[Fill], line: &str, tokens: Option<&[f64]>) -> Option<String> {
    match fill_under(d, tips, fills, tokens) {
        Under::Off => None,
        Under::Crosses => Some(String::new()),
        Under::On(fill) => Some(edge_ink(fill, line)),
    }
}

const CASE: f64 = 4.0;

fn decl(line: &str, prop: &str) -> Option<String> {
    if !line.contains(prop) {
        return None;
    }
    let re = Regex::new(&format!(r#"[";]{}:([^;"]+);"#, regex::escape(prop))).unwrap();
    re.captures(line).map(|c| trim(c.get(1).unwrap().as_str()).to_string())
}

// A wider page-colored stroke under an edge, as faint as it; markers drop their class so marker styles skip it.
pub fn casing(d: &str, markers: &str, line: &str, thick: bool) -> String {
    let stroke = decl(line, "stroke");
    if stroke.as_deref() == Some("none") || stroke.as_deref() == Some("transparent") {
        return String::new();
    }
    let width = if thick { Some("2.5".to_string()) } else { decl(line, "stroke-width") };
    let mut attrs = String::new();
    let mut style = String::new();
    if let Some(width) = &width {
        let body = width.strip_suffix("px").unwrap_or(width);
        if !body.is_empty() && body.chars().all(|c| c.is_ascii_digit() || c == '.') {
            if parse_float(width) != 1.0 {
                attrs = format!(" stroke-width=\"{}\"", num(parse_float(width) + CASE));
            }
        } else {
            style += &format!("stroke-width:calc({} + {}px);", width, CASE);
        }
    }
    for prop in ["opacity", "stroke-opacity"] {
        if let Some(value) = decl(line, prop) {
            style += &format!("{}:{};", prop, value);
        }
    }
    let body = format!("<path d=\"{}\"/>{}", d, markers.replace(" class=\"pele-marker\"", ""));
    if !attrs.is_empty() || !style.is_empty() {
        let style = if style.is_empty() { String::new() } else { format!(" style=\"{}\"", style) };
        format!("<g{}{}>{}</g>", attrs, style, body)
    } else {
        body
    }
}

pub fn edge_casing(path: &EdgePath, start: &str, end: &str, line: &str, thick: bool) -> String {
    casing(
        &path.d,
        &(marker(start, path.sx, path.sy, path.sdx, path.sdy, "var(--_bg)", "var(--_bg)")
            + &marker(end, path.ex, path.ey, path.edx, path.edy, "var(--_bg)", "var(--_bg)")),
        line,
        thick,
    )
}

pub fn casings_svg(casings: &str) -> String {
    if casings.is_empty() {
        return String::new();
    }
    format!(
        "<g class=\"pele-casings\" fill=\"none\" stroke=\"var(--_bg)\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" pointer-events=\"none\">{}</g>",
        crate::util::js_num(1.0 + CASE),
        casings
    )
}

// An edge label on a patch of the background, so the line does not run through the text.
pub fn edge_label_svg(id: &str, label: &Label, x: f64, y: f64, attrs: &str) -> String {
    let w = label.width + 8.0;
    let h = label.height;
    format!(
        "<g class=\"pele-edge-label\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\"/>{}</g>",
        esc_text(id),
        num(x - w / 2.0),
        num(y - h / 2.0),
        num(w),
        num(h),
        label_svg(label, x, y, attrs)
    )
}

pub struct LoopLabel {
    pub label_w: f64,
    pub label_h: f64,
    pub label_x: f64,
    pub label_y: f64,
}

// A self-loop beside a node of drawn size w by h: out of the side that lies across the flow and back in.
#[allow(clippy::too_many_arguments)]
pub fn loop_path(cx: f64, cy: f64, w: f64, h: f64, k: f64, sideways: bool, base: f64, trim: f64, e: &mut LoopLabel) -> EdgePath {
    let half = (if sideways { w } else { h }) / 2.0;
    let spread = (half - 4.0).min(8.0 + k * 6.0);
    let reach = base + k * 8.0;
    let mut len = reach.hypot(spread);
    if len == 0.0 || len.is_nan() {
        len = 1.0;
    }
    let at = |out: f64, along: f64| -> String {
        if sideways {
            format!("{},{}", num(cx + along), num(cy + h / 2.0 + out))
        } else {
            format!("{},{}", num(cx + w / 2.0 + out), num(cy + along))
        }
    };
    let tx = (reach / len) * trim;
    let ty = (spread / len) * trim;
    if sideways {
        e.label_x = cx;
        e.label_y = cy + h / 2.0 + reach + 4.0 + e.label_h / 2.0;
    } else {
        e.label_x = cx + w / 2.0 + reach + 4.0 + e.label_w / 2.0;
        e.label_y = cy;
    }
    EdgePath {
        d: format!("M{}C{} {} {}", at(0.0, -spread), at(reach, -spread * 2.0), at(reach, spread * 2.0), at(tx, spread + ty)),
        tokens: None,
        sx: if sideways { cx - spread } else { cx + w / 2.0 },
        sy: if sideways { cy + h / 2.0 } else { cy - spread },
        sdx: if sideways { 0.0 } else { -1.0 },
        sdy: if sideways { -1.0 } else { 0.0 },
        ex: if sideways { cx + spread } else { cx + w / 2.0 },
        ey: if sideways { cy + h / 2.0 } else { cy + spread },
        edx: if sideways { -spread / len } else { -reach / len },
        edy: if sideways { -reach / len } else { -spread / len },
    }
}
