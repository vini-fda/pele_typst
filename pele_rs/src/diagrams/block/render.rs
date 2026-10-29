use super::{
    arrows,
    layout::{self, Cell},
    model::{Block, Model},
};
use crate::{
    Config, RenderOptions, TextMeasurer,
    diagrams::common::{Rendered, tighten, title_room},
    svg::{
        builder::{esc_text, label_svg},
        edges::{ARROW, EdgePath, Fill, casings_svg, edge_casing, edge_label_svg, marker, marker_trim, over_fills},
        ink::{ink_fill, paints},
        root::{Accessible, svg_document},
        shapes::{draw_shape, shape_filled, shape_inset, shape_size},
        theme::{ResolvedStyle, class_names, resolve_style, rounded, sets_fill},
        title::title_label,
    },
    text::{
        label::{Label, layout_label},
        measurer::{BOLD, ITALIC},
    },
    util::{js_round, js_sign, num},
};
use indexmap::IndexMap;
use std::{
    collections::{HashMap, HashSet},
    f64::consts::{FRAC_1_SQRT_2, PI},
    ops::{Deref, DerefMut},
};
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Leaf,
    Arrow,
    Composite,
    Space,
}
pub(super) struct View {
    pub cell: Cell,
    block: usize,
    kind: Kind,
    index: usize,
    shape: &'static str,
    label: Label,
    style: ResolvedStyle,
    mask: u8,
    dx: f64,
    dy: f64,
    bw: f64,
    bh: f64,
}
impl Deref for View {
    type Target = Cell;
    fn deref(&self) -> &Cell {
        &self.cell
    }
}
impl DerefMut for View {
    fn deref_mut(&mut self) -> &mut Cell {
        &mut self.cell
    }
}
#[derive(Clone)]
struct Obstacle {
    index: Option<usize>,
    x: f64,
    y: f64,
    bw: f64,
    bh: f64,
}
struct Route {
    edge: usize,
    points: Vec<f64>,
    loop_: usize,
    label: Label,
    lx: f64,
    ly: f64,
}
fn is_round(v: &View) -> bool {
    v.kind == Kind::Leaf && matches!(v.shape, "circle" | "dbl-circ")
}
fn edge_y(v: &View, x: f64, sign: f64) -> f64 {
    let (hw, hh) = (v.bw / 2., v.bh / 2.);
    let dx = (x - v.x).abs();
    let reach = if is_round(v) {
        (hw * hw - dx * dx).max(0.).sqrt()
    } else if v.kind == Kind::Leaf && v.shape == "diam" {
        hh * (1. - dx / hw).max(0.)
    } else if v.kind == Kind::Leaf {
        hh - shape_inset(v.shape, v.bw, v.bh, if sign > 0. { 2 } else { 0 }, 0.)
    } else if v.kind == Kind::Arrow && arrows::axis(v.mask) == 1 {
        hh - arrows::OVERHANG
    } else {
        hh
    };
    v.y + sign * reach
}
fn edge_x(v: &View, y: f64, sign: f64) -> f64 {
    let (hw, hh) = (v.bw / 2., v.bh / 2.);
    let dy = (y - v.y).abs();
    let reach = if is_round(v) {
        (hh * hh - dy * dy).max(0.).sqrt()
    } else if v.kind == Kind::Leaf && v.shape == "diam" {
        hw * (1. - dy / hh).max(0.)
    } else if v.kind == Kind::Leaf {
        hw - shape_inset(v.shape, v.bw, v.bh, if sign > 0. { 1 } else { 3 }, 0.)
    } else if v.kind == Kind::Arrow && arrows::axis(v.mask) == 2 {
        hw - arrows::OVERHANG
    } else {
        hw
    };
    v.x + sign * reach
}
fn border(v: &View, tx: f64, ty: f64, out: &mut Vec<f64>) {
    let (dx, dy, hw, hh) = (tx - v.x, ty - v.y, v.bw / 2., v.bh / 2.);
    let mut k = if is_round(v) {
        let h = dx.hypot(dy);
        hw / if h == 0. { 1. } else { h }
    } else if v.kind == Kind::Leaf && v.shape == "diam" {
        let d = dx.abs() / hw + dy.abs() / hh;
        1. / if d == 0. { 1. } else { d }
    } else {
        (if dx == 0. { f64::INFINITY } else { hw / dx.abs() }).min(if dy == 0. { f64::INFINITY } else { hh / dy.abs() })
    };
    if k == f64::INFINITY {
        k = 0.;
    }
    out.extend_from_slice(&[v.x + dx * k, v.y + dy * k]);
}
fn crosses_box(x1: f64, y1: f64, x2: f64, y2: f64, v: &Obstacle) -> bool {
    let (l, r, t, b) = (
        v.x - v.bw / 2. + 1.,
        v.x + v.bw / 2. - 1.,
        v.y - v.bh / 2. + 1.,
        v.y + v.bh / 2. - 1.,
    );
    if (x1 < l && x2 < l) || (x1 > r && x2 > r) || (y1 < t && y2 < t) || (y1 > b && y2 > b) {
        return false;
    }
    if x1 == x2 || y1 == y2 {
        return true;
    }
    let (dx, dy) = (x2 - x1, y2 - y1);
    let side = |x, y| js_sign(dx * (y - y1) - dy * (x - x1));
    let first = side(l, t);
    first == 0. || side(r, t) != first || side(r, b) != first || side(l, b) != first
}
fn hits(p: &[f64], s: &View, t: &View, obstacles: &[Obstacle]) -> usize {
    obstacles
        .iter()
        .filter(|v| {
            v.index != Some(s.index) && v.index != Some(t.index) && p.windows(4).step_by(2).any(|p| crosses_box(p[0], p[1], p[2], p[3], v))
        })
        .count()
}
fn route_edge(s: &View, t: &View, shift: f64, obstacles: Option<&[Obstacle]>) -> Vec<f64> {
    let (sl, sr, st, sb) = (s.x - s.bw / 2., s.x + s.bw / 2., s.y - s.bh / 2., s.y + s.bh / 2.);
    let (tl, tr, tt, tb) = (t.x - t.bw / 2., t.x + t.bw / 2., t.y - t.bh / 2., t.y + t.bh / 2.);
    let (x1, x2, y1, y2) = (sl.max(tl), sr.min(tr), st.max(tt), sb.min(tb));
    let (ox, oy) = (x2 - x1 > 6., y2 - y1 > 6.);
    if ox && oy {
        let (inner, outer) = if s.bw * s.bh <= t.bw * t.bh { (s, t) } else { (t, s) };
        let (up, down, left, right) = (
            inner.y - inner.bh / 2. - (outer.y - outer.bh / 2.),
            outer.y + outer.bh / 2. - (inner.y + inner.bh / 2.),
            inner.x - inner.bw / 2. - (outer.x - outer.bw / 2.),
            outer.x + outer.bw / 2. - (inner.x + inner.bw / 2.),
        );
        let least = up.min(down).min(left).min(right);
        let p = if least == down {
            vec![inner.x, edge_y(inner, inner.x, 1.), inner.x, outer.y + outer.bh / 2.]
        } else if least == up {
            vec![inner.x, edge_y(inner, inner.x, -1.), inner.x, outer.y - outer.bh / 2.]
        } else if least == right {
            vec![edge_x(inner, inner.y, 1.), inner.y, outer.x + outer.bw / 2., inner.y]
        } else {
            vec![edge_x(inner, inner.y, -1.), inner.y, outer.x - outer.bw / 2., inner.y]
        };
        return if inner.index == s.index { p } else { vec![p[2], p[3], p[0], p[1]] };
    }
    if ox {
        let margin = 6f64.min((x2 - x1) / 2.);
        let x = ((x1 + x2) / 2. + shift).max(x1 + margin).min(x2 - margin);
        let down = if t.y > s.y { 1. } else { -1. };
        return vec![x, edge_y(s, x, down), x, edge_y(t, x, -down)];
    }
    if oy {
        let margin = 6f64.min((y2 - y1) / 2.);
        let y = ((y1 + y2) / 2. + shift).max(y1 + margin).min(y2 - margin);
        let right = if t.x > s.x { 1. } else { -1. };
        return vec![edge_x(s, y, right), y, edge_x(t, y, -right), y];
    }
    let right = if t.x > s.x { 1. } else { -1. };
    let down = if t.y > s.y { 1. } else { -1. };
    let across = vec![edge_x(s, s.y, right), s.y, t.x, s.y, t.x, edge_y(t, t.x, -down)];
    let along = vec![s.x, edge_y(s, s.x, down), s.x, t.y, edge_x(t, t.y, -right), t.y];
    let wide = (t.x - s.x).abs() >= (t.y - s.y).abs();
    let (mut best, other) = if wide { (across, along) } else { (along, across) };
    let Some(obstacles) = obstacles else { return best };
    let mut least = hits(&best, s, t, obstacles);
    if least == 0 {
        return best;
    }
    let count = hits(&other, s, t, obstacles);
    if count < least {
        best = other;
        least = count;
    }
    if least == 0 {
        return best;
    }
    let mut direct = vec![];
    border(s, t.x, t.y, &mut direct);
    border(t, s.x, s.y, &mut direct);
    if hits(&direct, s, t, obstacles) < least { direct } else { best }
}
fn trace(p: &[f64], mut start: f64, mut end: f64) -> EdgePath {
    let n = p.len();
    let (sx, sy, ex, ey) = (p[0], p[1], p[n - 2], p[n - 1]);
    let (mut ax, mut ay) = (p[2] - sx, p[3] - sy);
    let first = ax.hypot(ay);
    let f = if first == 0. { 1. } else { first };
    ax /= f;
    ay /= f;
    let (mut bx, mut by) = (ex - p[n - 4], ey - p[n - 3]);
    let last = bx.hypot(by);
    let l = if last == 0. { 1. } else { last };
    bx /= l;
    by /= l;
    if n == 4 && start + end > first {
        let scale = first / (start + end);
        start *= scale;
        end *= scale;
    }
    start = start.min(first);
    end = end.min(last);
    let mut d = format!("M{},{}", num(sx + ax * start), num(sy + ay * start));
    if n == 6 {
        let r = 8f64.min(first - start).min(last - end).max(0.);
        d += &format!(
            "L{},{}Q{},{} {},{}",
            num(p[2] - ax * r),
            num(p[3] - ay * r),
            num(p[2]),
            num(p[3]),
            num(p[2] + bx * r),
            num(p[3] + by * r)
        );
    }
    d += &format!("L{},{}", num(ex - bx * end), num(ey - by * end));
    EdgePath {
        d,
        sx,
        sy,
        sdx: -ax,
        sdy: -ay,
        ex,
        ey,
        edx: bx,
        edy: by,
        ..Default::default()
    }
}
fn loop_arc(v: &View, k: usize) -> (f64, f64, f64, f64, f64) {
    let (hw, hh) = (v.bw / 2., v.bh / 2.);
    let r = (10. + 5. * (k as f64 - 1.)).min(hw).min(hh).max(1.);
    if is_round(v) {
        let (px, py) = (v.x + hw * FRAC_1_SQRT_2, v.y - hw * FRAC_1_SQRT_2);
        let half = 2. * (r / (2. * hw)).min(1.).asin();
        let angle = |at: f64| (v.y - hw * at.sin() - py).atan2(v.x + hw * at.cos() - px);
        return (px, py, r, angle(PI / 4. + half), angle(PI / 4. - half));
    }
    if v.kind == Kind::Leaf && v.shape == "diam" {
        let slope = hh.atan2(hw);
        return (v.x + hw / 2., v.y - hh / 2., r, slope + PI, slope);
    }
    (v.x + hw, v.y - hh, r, PI, PI / 2.)
}
fn loop_path(v: &View, k: usize, start: f64, end: f64) -> EdgePath {
    let (px, py, r, a1, a2) = loop_arc(v, k);
    let (from, to) = (a1 + start / r, a2 - end / r);
    let mut sweep = (to - from) % (2. * PI);
    if sweep <= 0. {
        sweep += 2. * PI;
    }
    let (x1, y1, x2, y2) = (px + r * from.cos(), py + r * from.sin(), px + r * to.cos(), py + r * to.sin());
    let (sx, sy, ex, ey) = (px + r * a1.cos(), py + r * a1.sin(), px + r * a2.cos(), py + r * a2.sin());
    let (sl, el) = ((sx - x1).hypot(sy - y1), (ex - x2).hypot(ey - y2));
    EdgePath {
        d: format!(
            "M{},{}A{},{} 0 {} 1 {},{}",
            num(x1),
            num(y1),
            num(r),
            num(r),
            if sweep > PI { 1 } else { 0 },
            num(x2),
            num(y2)
        ),
        sx,
        sy,
        sdx: if sl > 0. { (sx - x1) / sl } else { a1.sin() },
        sdy: if sl > 0. { (sy - y1) / sl } else { -a1.cos() },
        ex,
        ey,
        edx: if el > 0. { (ex - x2) / el } else { -a2.sin() },
        edy: if el > 0. { (ey - y2) / el } else { a2.cos() },
        ..Default::default()
    }
}
fn covers(x: f64, y: f64, w: f64, h: f64, s: usize, t: usize, obstacles: Option<&[Obstacle]>) -> bool {
    obstacles.is_some_and(|obs| {
        obs.iter()
            .any(|v| v.index != Some(s) && v.index != Some(t) && (x - v.x).abs() < (w + v.bw) / 2. && (y - v.y).abs() < (h + v.bh) / 2.)
    })
}
fn label_spot(p: &[f64], at: usize, w: f64, h: f64, s: usize, t: usize, obstacles: Option<&[Obstacle]>) -> f64 {
    if obstacles.is_some() {
        for spot in [0.5, 0.3, 0.7, 0.15, 0.85] {
            let x = p[at] + (p[at + 2] - p[at]) * spot;
            let y = p[at + 1] + (p[at + 3] - p[at + 1]) * spot;
            if !covers(x, y, w, h, s, t, obstacles) {
                return spot;
            }
        }
    }
    0.5
}
fn stack_labels(stack: &[usize], routes: &mut [Route], legs: &[Vec<f64>], s: usize, t: usize, obs: Option<&[Obstacle]>) -> f64 {
    let vertical = legs[stack[0]][0] == legs[stack[0]][2];
    let (mut lo, mut hi, mut need, mut crowded) = (f64::NEG_INFINITY, f64::INFINITY, -4., false);
    for (a, &i) in stack.iter().enumerate() {
        let leg = &legs[i];
        if if vertical { leg[0] != leg[2] } else { leg[1] != leg[3] } {
            return 0.;
        }
        lo = lo.max(if vertical { leg[1].min(leg[3]) } else { leg[0].min(leg[2]) });
        hi = hi.min(if vertical { leg[1].max(leg[3]) } else { leg[0].max(leg[2]) });
        let p = &routes[i];
        need += if vertical { p.label.height } else { p.label.width + 8. } + 4.;
        for &j in &stack[..a] {
            let q = &routes[j];
            if (p.lx - q.lx).abs() < (p.label.width + q.label.width) / 2. + 8.
                && (p.ly - q.ly).abs() < (p.label.height + q.label.height) / 2.
            {
                crowded = true;
            }
        }
    }
    if !crowded {
        return 0.;
    }
    if need > hi - lo - 2. * ARROW {
        return need - (hi - lo - 2. * ARROW);
    }
    let mut along = vec![];
    let mut next = (lo + hi - need) / 2.;
    for &i in stack {
        let p = &routes[i];
        let size = if vertical { p.label.height } else { p.label.width + 8. };
        let at = next + size / 2.;
        if covers(
            if vertical { p.lx } else { at },
            if vertical { at } else { p.ly },
            p.label.width + 8.,
            p.label.height,
            s,
            t,
            obs,
        ) {
            return 0.;
        }
        along.push(at);
        next += size + 4.;
    }
    for (&i, at) in stack.iter().zip(along) {
        if vertical { routes[i].ly = at } else { routes[i].lx = at }
    }
    0.
}
fn widen(
    v: &mut [View],
    parents: &[Option<usize>],
    s: usize,
    t: usize,
    vertical: bool,
    short: f64,
    base: &mut HashMap<(usize, bool), Vec<f64>>,
) -> bool {
    let mut chain = HashSet::new();
    let mut at = Some(s);
    while let Some(i) = at {
        chain.insert(i);
        at = parents[i];
    }
    let mut tb = None;
    let mut bx = Some(t);
    while let Some(i) = bx {
        if chain.contains(&i) {
            break;
        }
        tb = bx;
        bx = parents[i];
    }
    let (Some(bx), Some(tb)) = (bx, tb) else { return false };
    if bx == s {
        return false;
    }
    let mut sb = s;
    while parents[sb] != Some(bx) {
        sb = parents[sb].unwrap();
    }
    let at = if vertical {
        if v[sb].row == v[tb].row {
            return false;
        }
        v[sb].row.max(v[tb].row) - 1
    } else {
        let (l, r) = if v[sb].col < v[tb].col { (sb, tb) } else { (tb, sb) };
        if (v[r].col as f64) < v[l].col as f64 + v[l].span {
            return false;
        }
        v[r].col - 1
    };
    let gaps = if vertical {
        &mut v[bx].cell.row_gaps
    } else {
        &mut v[bx].cell.col_gaps
    };
    let old = base.entry((bx, vertical)).or_insert_with(|| gaps.clone());
    if gaps.len() <= at {
        gaps.resize(at + 1, 0.);
    }
    gaps[at] = gaps[at].max(old.get(at).copied().unwrap_or(0.) + short);
    true
}
fn style_of(b: &Block, m: &Model) -> ResolvedStyle {
    let mut declarations = vec![];
    let mut add = |styles: &[String]| {
        for style in styles {
            declarations.extend(style.split(';').map(str::to_owned));
        }
    };
    if b.classes.is_empty() {
        if let Some(c) = m.classes.get("default") {
            add(&c.styles);
        }
    } else {
        for name in &b.classes {
            if let Some(c) = m.classes.get(name) {
                add(&c.styles);
            }
        }
    }
    add(&b.styles);
    resolve_style(&declarations)
}
fn view(
    block: usize,
    index: usize,
    model: &Model,
    options: &RenderOptions,
    m: &dyn TextMeasurer,
    size: f64,
    gap: f64,
    wrap: f64,
    tight: bool,
) -> View {
    let b = &model.arena[block];
    let style = if b.kind == "space" {
        resolve_style(&[])
    } else {
        style_of(b, model)
    };
    let text_style = if style.bold { BOLD } else { 0 } | if style.italic { ITALIC } else { 0 };
    let font = style.font_size.unwrap_or(size);
    let (mut kind, mut shape, mut head, mut mask, mut dx, mut dy) = (Kind::Leaf, "", 0., 0, 0., 0.);
    let (label, nw, nh) = match b.kind.as_str() {
        "space" => {
            kind = Kind::Space;
            (layout_label(None, false, m, font, wrap, 0), 0., 0.)
        }
        "composite" => {
            kind = Kind::Composite;
            let l = layout_label(
                b.label.as_deref(),
                false,
                m,
                font,
                if tight { title_room(options) } else { 4000. },
                text_style,
            );
            head = if l.height > 0. { l.height + 4. } else { 0. };
            let w = l.width + 2. * gap;
            (l, w, head + 2. * gap)
        }
        "block_arrow" => {
            kind = Kind::Arrow;
            let l = layout_label(b.label.as_deref(), false, m, font, wrap, text_style);
            let blank = !l.lines.iter().flatten().any(|p| p.icon.is_some() || !p.text.trim().is_empty());
            mask = arrows::mask(&b.directions);
            let (w, h, x, y) = arrows::size(mask, if blank { 0. } else { l.width }, if blank { 0. } else { l.height });
            dx = x;
            dy = y;
            (l, w, h)
        }
        _ => {
            shape = match b.kind.as_str() {
                "round" => "rounded",
                "circle" => "circle",
                "doublecircle" => "dbl-circ",
                "diamond" => "diam",
                "hexagon" => "hex",
                "stadium" => "stadium",
                "subroutine" => "fr-rect",
                "cylinder" => "cyl",
                "rect_left_inv_arrow" => "odd",
                "lean_right" => "lean-r",
                "lean_left" => "lean-l",
                "trapezoid" => "trap-b",
                "inv_trapezoid" => "trap-t",
                _ => "rect",
            };
            let l = layout_label(b.label.as_deref(), false, m, font, wrap, text_style);
            let s = shape_size(shape, l.width, l.height);
            let h = if shape == "cyl" { s.h.max(l.height + 40.) } else { s.h };
            dy = s.dy;
            (l, s.w, h)
        }
    };
    View {
        cell: Cell {
            columns: if kind == Kind::Composite { b.columns.unwrap_or(-1.) } else { -1. },
            span: if kind == Kind::Space { 1. } else { b.span.unwrap_or(1.) },
            space: kind == Kind::Space,
            head,
            nw,
            nh,
            ..Default::default()
        },
        block,
        kind,
        index,
        shape,
        label,
        style,
        mask,
        dx,
        dy,
        bw: 0.,
        bh: 0.,
    }
}
fn place(v: &mut [View], widths: &[f64], gap: f64) -> ((f64, f64), Vec<Obstacle>, Vec<Obstacle>) {
    for (v, w) in v.iter_mut().zip(widths) {
        v.nw = *w;
    }
    let content = layout::layout(v, gap);
    let (mut obstacles, mut titles) = (vec![], vec![]);
    for v in v {
        if v.kind == Kind::Leaf && matches!(v.shape, "circle" | "dbl-circ" | "diam") {
            v.bw = v.nw;
            v.bh = v.nh;
        } else if v.kind == Kind::Arrow {
            v.bw = if v.span > 1. { v.w } else { v.nw };
            v.bh = v.nh;
        } else {
            v.bw = v.w;
            v.bh = if v.kind == Kind::Leaf && matches!(v.shape, "stadium" | "hex" | "odd" | "lean-r" | "lean-l" | "trap-b" | "trap-t") {
                v.nh
            } else {
                v.h
            };
        }
        if v.kind == Kind::Leaf && v.shape == "cyl" {
            v.dy = (v.bw / 14.).max(6.).min(12.) / 2.;
        }
        if matches!(v.kind, Kind::Leaf | Kind::Arrow) {
            obstacles.push(Obstacle {
                index: Some(v.index),
                x: v.x,
                y: v.y,
                bw: v.bw,
                bh: v.bh,
            });
        } else if v.head > 0. {
            titles.push(Obstacle {
                index: None,
                x: v.x,
                y: v.y - v.h / 2. + (v.head + gap) / 2. + 2.,
                bw: v.label.width,
                bh: v.label.height,
            });
        }
    }
    (content, obstacles, titles)
}
struct Routing<'a> {
    model: &'a Model,
    m: &'a dyn TextMeasurer,
    edge_size: f64,
    ends: Vec<(usize, usize)>,
    drawn: Vec<usize>,
    pairs: HashMap<usize, usize>,
    labelled: HashSet<usize>,
    routes: Vec<Route>,
    legs: Vec<Vec<f64>>,
    stacks: IndexMap<usize, Vec<usize>>,
    check: bool,
    tight: bool,
}
impl Routing<'_> {
    fn settle(&mut self, v: &mut [View], parents: &[Option<usize>], obstacles: &[Obstacle], titles: &[Obstacle], grow: bool) -> bool {
        let mut seen = HashMap::new();
        self.routes.clear();
        self.stacks.clear();
        self.legs.resize(self.drawn.len(), vec![]);
        let check = if self.check { Some(obstacles) } else { None };
        for (i, &edge) in self.drawn.iter().enumerate() {
            let (si, ti) = self.ends[i];
            let (s, t) = (&v[si], &v[ti]);
            let key = si.min(ti) * v.len() + si.max(ti);
            let total = self.pairs.get(&key).copied().unwrap_or(1);
            let k = seen.entry(key).or_insert(0usize);
            let count = *k;
            *k += 1;
            let label = layout_label(self.model.arena[edge].label.as_deref(), false, self.m, self.edge_size, 200., 0);
            let (points, lx, ly) = if si == ti {
                let (px, py, r, _, _) = loop_arc(s, count + 1);
                (vec![], px + r + 6. + label.width / 2., py - r / 2.)
            } else {
                let step = if self.labelled.contains(&key) {
                    js_round(self.edge_size * 1.5) + 2.
                } else {
                    14.
                };
                let p = route_edge(s, t, (count as f64 - (total as f64 - 1.) / 2.) * step, check);
                let n = p.len();
                let first = (p[2] - p[0]).abs() + (p[3] - p[1]).abs();
                let last = (p[n - 2] - p[n - 4]).abs() + (p[n - 1] - p[n - 3]).abs();
                let at = if n == 6 && last > first { 2 } else { 0 };
                let spot = if label.width > 0. {
                    label_spot(&p, at, label.width + 8., label.height, si, ti, check)
                } else {
                    0.5
                };
                let (lx, ly) = (p[at] + (p[at + 2] - p[at]) * spot, p[at + 1] + (p[at + 3] - p[at + 1]) * spot);
                if label.width > 0. && total > 1 {
                    self.legs[i] = p[at..at + 4].to_vec();
                    self.stacks.entry(key).or_default().push(i);
                }
                (p, lx, ly)
            };
            self.routes.push(Route {
                edge,
                points,
                loop_: if si == ti { count + 1 } else { 0 },
                label,
                lx,
                ly,
            });
        }
        let mut blocked = obstacles.to_vec();
        blocked.extend_from_slice(titles);
        let blocked = if self.check { Some(blocked.as_slice()) } else { None };
        let mut base = HashMap::new();
        let mut grown = false;
        for stack in self.stacks.values() {
            if stack.len() < 2 {
                continue;
            }
            let (s, t) = self.ends[stack[0]];
            let short = stack_labels(stack, &mut self.routes, &self.legs, s, t, blocked);
            let vertical = self.legs[stack[0]][0] == self.legs[stack[0]][2];
            if grow && short > 0. && (vertical || !self.tight) && widen(v, parents, s, t, vertical, short.ceil(), &mut base) {
                grown = true;
            }
        }
        grown
    }
    fn overlap(&self) -> f64 {
        let mut area = 0.;
        for (i, p) in self.routes.iter().enumerate() {
            for q in &self.routes[i + 1..] {
                let (pw, qw) = ((p.label.width + 8.) / 2., (q.label.width + 8.) / 2.);
                let w = (p.lx + pw).min(q.lx + qw) - (p.lx - pw).max(q.lx - qw);
                let h = (p.ly + p.label.height / 2.).min(q.ly + q.label.height / 2.)
                    - (p.ly - p.label.height / 2.).max(q.ly - q.label.height / 2.);
                if p.label.width > 0. && q.label.width > 0. && w > 0. && h > 0. {
                    area += w * h;
                }
            }
        }
        area
    }
    fn bound(&self, v: &[View], content: (f64, f64)) -> [f64; 4] {
        let mut b = [0f64, 0f64, content.0, content.1];
        let mut grow = |x: f64, y: f64, w: f64, h: f64| {
            b[0] = b[0].min(x - w / 2.);
            b[2] = b[2].max(x + w / 2.);
            b[1] = b[1].min(y - h / 2.);
            b[3] = b[3].max(y + h / 2.);
        };
        for (i, route) in self.routes.iter().enumerate() {
            if route.loop_ > 0 {
                let (px, py, r, _, _) = loop_arc(&v[self.ends[i].0], route.loop_);
                grow(px, py, 2. * r + 2., 2. * r + 2.);
            }
            if route.label.width > 0. {
                grow(route.lx, route.ly, route.label.width + 8., route.label.height);
            }
        }
        b
    }
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, m: &dyn TextMeasurer) -> Result<Rendered, crate::PeleError> {
    super::validate(model)?;
    Ok(tighten(options, |tight| draw(model, config, options, m, tight)))
}
fn draw(model: &Model, config: &Config, options: &RenderOptions, m: &dyn TextMeasurer, tight: bool) -> Rendered {
    let size = options.font_size.unwrap_or(16.);
    let edge_size = js_round(size * 0.875);
    let pad = options.padding.unwrap_or(8.);
    let configured = config.get("block").and_then(|v| v.get("padding")).and_then(|v| v.as_f64());
    let gap = configured
        .filter(|v| *v >= 0.)
        .map_or(12., |v| v.min(200.))
        .min(if tight { 6. } else { f64::INFINITY });
    let wrap = if tight { 80. } else { 200. };
    let mut root = view(model.root, 0, model, options, m, size, gap, wrap, tight);
    root.nw = 0.;
    root.nh = 0.;
    let mut v = vec![root];
    let mut parents = vec![None];
    let mut by_id = HashMap::new();
    let mut i = 0;
    while i < v.len() {
        if v[i].kind == Kind::Composite {
            for &child in &model.arena[v[i].block].children {
                let n = view(child, v.len(), model, options, m, size, gap, wrap, tight);
                let j = v.len();
                if n.kind != Kind::Space {
                    by_id.insert(model.arena[child].id.as_str(), j);
                }
                v[i].children.push(j);
                parents.push(Some(i));
                v.push(n);
            }
        }
        i += 1;
    }
    let widths = v.iter().map(|v| v.nw).collect::<Vec<_>>();
    let (mut content, mut obstacles, mut titles) = place(&mut v, &widths, gap);
    let (mut pairs, mut labelled, mut ends, mut drawn) = (HashMap::new(), HashSet::new(), vec![], vec![]);
    for &i in &model.edges {
        let edge = &model.arena[i];
        let (Some(&s), Some(&t)) = (
            by_id.get(edge.start.as_deref().unwrap_or("")),
            by_id.get(edge.end.as_deref().unwrap_or("")),
        ) else {
            continue;
        };
        let key = s.min(t) * v.len() + s.max(t);
        *pairs.entry(key).or_insert(0) += 1;
        if edge.label.as_ref().is_some_and(|l| !l.is_empty()) {
            labelled.insert(key);
        }
        ends.push((s, t));
        drawn.push(i);
    }
    let grow = drawn.len() * drawn.len() <= 2_000_000;
    let check = drawn.len() * obstacles.len() <= 2_000_000;
    let mut routes = Routing {
        model,
        m,
        edge_size,
        ends,
        drawn,
        pairs,
        labelled,
        routes: vec![],
        legs: vec![],
        stacks: IndexMap::new(),
        check,
        tight,
    };
    if routes.settle(&mut v, &parents, &obstacles, &titles, grow) {
        let before = routes.overlap();
        let b = routes.bound(&v, content);
        let narrow = b[2] - b[0];
        let mut grown = true;
        for round in 0..3 {
            if !grown {
                break;
            }
            (content, obstacles, titles) = place(&mut v, &widths, gap);
            grown = routes.settle(&mut v, &parents, &obstacles, &titles, round < 2);
        }
        let b = routes.bound(&v, content);
        let wide = b[2] - b[0];
        let room = options.max_width.filter(|v| *v > 0.).unwrap_or(f64::INFINITY);
        if routes.overlap() > before - 1. || (wide > narrow && (wide + 2. * pad).ceil() > room) {
            for v in &mut v {
                v.row_gaps.clear();
                v.col_gaps.clear();
            }
            (content, obstacles, titles) = place(&mut v, &widths, gap);
            routes.settle(&mut v, &parents, &obstacles, &titles, false);
        }
    }
    let [minx, miny, maxx, maxy] = routes.bound(&v, content);
    let title = title_label(model.title.as_deref(), m, size, if tight { title_room(options) } else { 4000. });
    let title_h = if title.height > 0. { title.height + 12. } else { 0. };
    let inner = (maxx - minx).max(title.width);
    let ox = pad - minx + (inner - (maxx - minx)) / 2.;
    let oy = pad + title_h - miny;
    for v in &mut v {
        v.x += ox;
        v.y += oy;
    }
    let width = (inner + 2. * pad).ceil();
    let height = (maxy - miny + title_h + 2. * pad).ceil();
    let (mut clusters, mut nodes, mut fills) = (String::new(), String::new(), vec![]);
    for v in &v[1..] {
        if v.kind == Kind::Space {
            continue;
        }
        let block = &model.arena[v.block];
        let classes = class_names(&block.classes.join(" "));
        let id = esc_text(&block.id);
        if v.kind == Kind::Composite {
            let top = v.y - v.h / 2.;
            if paints(&v.style.shape) {
                fills.push(Fill {
                    bx: [v.x - v.w / 2., top, v.x + v.w / 2., top + v.h],
                    shape: v.style.shape.clone(),
                });
            }
            clusters += &format!(
                "<g class=\"pele-cluster{classes}\" data-id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\"{} stroke=\"var(--_b)\"{}/>{}</g>",
                num(v.x - v.w / 2.),
                num(top),
                num(v.w),
                num(v.h),
                if sets_fill(&v.style.shape) { "" } else { " fill-opacity=\"0.5\"" },
                rounded(&v.style.shape),
                label_svg(
                    &v.label,
                    v.x,
                    top + (v.head + gap) / 2. + 2.,
                    &format!(
                        " class=\"pele-cluster-label\"{}{}",
                        ink_fill(&v.style.shape, &v.style.text, Some("var(--_m)")),
                        v.style.text
                    )
                )
            );
            continue;
        }
        let arrow = v.kind == Kind::Arrow;
        let shape = if arrow {
            arrows::draw(
                v.mask,
                v.bw,
                v.bh,
                &format!(" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}", v.style.shape),
            )
        } else {
            draw_shape(
                v.shape,
                v.bw,
                v.bh,
                &format!(" fill=\"var(--_s)\" stroke=\"var(--_b)\"{}", v.style.shape),
                &format!(" stroke=\"var(--_b)\"{}", v.style.line),
            )
        };
        nodes += &format!(
            "<g class=\"pele-node pele-shape-{}{classes}\" data-id=\"{id}\" transform=\"translate({},{})\">{}{}</g>",
            if arrow { "block-arrow" } else { v.shape },
            num(v.x),
            num(v.y),
            shape,
            label_svg(
                &v.label,
                v.dx,
                v.dy,
                &format!(
                    " class=\"pele-label\"{}{}",
                    if arrow || shape_filled(v.shape) {
                        ink_fill(&v.style.shape, &v.style.text, None)
                    } else {
                        String::new()
                    },
                    v.style.text
                )
            )
        );
    }
    let (mut casings, mut edges, mut labels) = (String::new(), String::new(), String::new());
    for (i, route) in routes.routes.iter_mut().enumerate() {
        let edge = &model.arena[route.edge];
        let (start, end) = (&edge.arrow_start, &edge.arrow_end);
        let path = if route.loop_ > 0 {
            loop_path(&v[routes.ends[i].0], route.loop_, marker_trim(start), marker_trim(end))
        } else {
            for p in route.points.as_chunks_mut::<2>().0 {
                p[0] += ox;
                p[1] += oy;
            }
            trace(&route.points, marker_trim(start), marker_trim(end))
        };
        let over = over_fills(&path.d, &[path.sx, path.sy, path.ex, path.ey], &fills, "", None);
        if over.as_deref() == Some("") {
            casings += &edge_casing(&path, start, end, "", edge.thickness == "thick");
        }
        let color = over.as_deref().filter(|s| !s.is_empty()).unwrap_or("var(--_l)");
        edges += &format!(
            "<g class=\"pele-edge\" data-id=\"{}\"{}><path d=\"{}\"{}{}/>{}{}</g>",
            esc_text(&edge.id),
            over.as_ref()
                .filter(|s| !s.is_empty())
                .map_or(String::new(), |s| format!(" stroke=\"{s}\"")),
            path.d,
            if edge.thickness == "thick" { " stroke-width=\"2.5\"" } else { "" },
            if edge.pattern == "dotted" {
                " stroke-dasharray=\"3 4\""
            } else {
                ""
            },
            marker(start, path.sx, path.sy, path.sdx, path.sdy, color, "none"),
            marker(end, path.ex, path.ey, path.edx, path.edy, color, "none")
        );
        if route.label.width > 0. {
            labels += &edge_label_svg(&edge.id, &route.label, route.lx + ox, route.ly + oy, "");
        }
    }
    let mut content = label_svg(
        &title,
        width / 2.,
        pad + title.height / 2.,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !clusters.is_empty() {
        content += &format!("<g class=\"pele-clusters\">{clusters}</g>");
    }
    content += &casings_svg(&casings);
    if !edges.is_empty() {
        content += &format!("<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges}</g>");
    }
    content += &format!("<g class=\"pele-nodes\">{nodes}</g>");
    if !labels.is_empty() {
        content += &format!("<g class=\"pele-edge-labels\" font-size=\"{}\">{labels}</g>", num(edge_size));
    }
    let svg = svg_document(
        "block",
        width,
        height,
        size,
        options,
        &Accessible {
            acc_title: model.acc_title.as_deref(),
            acc_descr: model.acc_descr.as_deref(),
        },
        &content,
    );
    Rendered {
        svg,
        width,
        height,
        links: vec![],
    }
}
