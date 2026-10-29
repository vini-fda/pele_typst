// Swimlane ranking, placement and routing, after pele's layout/lanes.ts.
use super::compound::{CEdge, CNode, CompoundResult, Dir};
use super::layered::{Kind, LEdge, LNode, LayeredOptions, ledge, lnode, orient, rank};
use crate::util::js_round;
use std::collections::HashSet;

const MAX_HOPS: usize = 10000;
const ROUTE_BUDGET: usize = 2000000;
const RELAY: usize = 8;
const TURN: f64 = 20.0;
const MARGIN: f64 = 20.0;
const LEG: f64 = 14.0;

struct Placement {
    width: f64,
    height: f64,
    routed: Vec<bool>,
    margins: Vec<bool>,
    work: usize,
}
struct Setup {
    lanes: Vec<usize>,
    lane_slot: Vec<isize>,
    real: Vec<usize>,
    item_of: Vec<isize>,
    item_lane: Vec<usize>,
    lane_pads: Vec<f64>,
    end_pad: f64,
    lnodes: Vec<LNode>,
    ledges: Vec<LEdge>,
    edge_of: Vec<usize>,
    step: usize,
    sideways: bool,
    flipped: bool,
}

pub fn lane_layout(nodes: &mut [CNode], edges: &mut [CEdge], dir: Dir, opt: &LayeredOptions, lane_pad: f64) -> CompoundResult {
    let n = nodes.len();
    if n == 0 {
        return CompoundResult::default();
    }
    let sideways = matches!(dir, Dir::LR | Dir::RL);
    let flipped = matches!(dir, Dir::BT | Dir::RL);
    let lanes: Vec<usize> = (0..n).rev().filter(|&i| nodes[i].parent < 0 && nodes[i].is_group).collect();
    let lane_count = lanes.len().max(1);
    let mut lane_slot = vec![-1; n];
    for (k, &i) in lanes.iter().enumerate() {
        lane_slot[i] = k as isize;
    }
    let mut has_child = vec![false; n];
    for c in nodes.iter() {
        if c.parent >= 0 {
            has_child[c.parent as usize] = true;
        }
    }
    let mut lane_pads = vec![lane_pad; lane_count];
    let mut end_pad = lane_pad;
    let mut item_of = vec![-1; n];
    let mut real = Vec::new();
    for i in 0..n {
        if lane_slot[i] < 0 && (!nodes[i].is_group || !has_child[i]) {
            item_of[i] = real.len() as isize;
            real.push(i);
        }
    }
    let mut lnodes: Vec<LNode> = real
        .iter()
        .map(|&i| {
            lnode(
                if sideways { nodes[i].h } else { nodes[i].w },
                if sideways { nodes[i].w } else { nodes[i].h },
                Kind::Node,
            )
        })
        .collect();
    let mut ledges = Vec::new();
    let mut edge_of = Vec::new();
    let mut step = 1;
    let mut seen = HashSet::new();
    for (ei, e) in edges.iter().enumerate() {
        let (a, b) = (item_of[e.src], item_of[e.dst]);
        if a < 0 || b < 0 || a == b {
            continue;
        }
        let key = (a.min(b), a.max(b));
        if e.label_w > 0.0 || seen.contains(&key) {
            step = 2;
        }
        seen.insert(key);
        ledges.push(ledge(
            a as usize,
            b as usize,
            e.minlen,
            if sideways { e.label_h } else { e.label_w },
            if sideways { e.label_w } else { e.label_h },
        ));
        edge_of.push(ei);
    }
    orient(&lnodes, &mut ledges);
    rank(&mut lnodes, &ledges, step as i64);
    let mut item_lane = Vec::new();
    for &i in &real {
        let mut t = i;
        let mut room = lane_pad;
        let mut hops = 0;
        while nodes[t].parent >= 0 && hops < MAX_HOPS {
            t = nodes[t].parent as usize;
            if lane_slot[t] < 0 {
                room += nodes[t].pad_x.max(nodes[t].pad_top).max(nodes[t].pad_bottom);
            }
            hops += 1;
        }
        let lane = lane_slot[t].max(0) as usize;
        lane_pads[lane] = lane_pads[lane].max(room);
        end_pad = end_pad.max(room);
        item_lane.push(lane);
    }
    let setup = Setup {
        lanes,
        lane_slot,
        real,
        item_of,
        item_lane,
        lane_pads,
        end_pad,
        lnodes,
        ledges,
        edge_of,
        step,
        sideways,
        flipped,
    };
    let m = setup.ledges.len();
    let mut plain = vec![false; m];
    let mut margins = vec![false; lane_count];
    let mut placed = place(nodes, edges, opt, &setup, &plain, &margins);
    for round in 0..3 {
        if placed.work > ROUTE_BUDGET / RELAY {
            break;
        }
        let next: Vec<bool> = if round == 0 {
            placed.routed.clone()
        } else {
            plain.iter().zip(&placed.routed).map(|(&a, &b)| a && b).collect()
        };
        if next == plain && placed.margins == margins {
            break;
        }
        plain = next;
        margins = placed.margins;
        placed = place(nodes, edges, opt, &setup, &plain, &margins);
    }
    if plain.iter().zip(&placed.routed).any(|(&a, &b)| a && !b) {
        placed = place(nodes, edges, opt, &setup, &vec![false; m], &vec![false; lane_count]);
    }
    CompoundResult {
        width: placed.width,
        height: placed.height,
    }
}

fn runs(row: &[usize], lane: &[usize]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while from < row.len() {
        let mut to = from;
        while to + 1 < row.len() && lane[row[to + 1]] == lane[row[from]] {
            to += 1;
        }
        out.push((from, to));
        from = to + 1;
    }
    out
}
fn gap(a: usize, b: usize, dummy: &[bool], opt: &LayeredOptions) -> f64 {
    if dummy[a] || dummy[b] { opt.edge_sep } else { opt.node_sep }
}
fn run_size(row: &[usize], from: usize, to: usize, size: &[f64], dummy: &[bool], opt: &LayeredOptions) -> f64 {
    let mut v = size[row[from]];
    for k in from..to {
        v += gap(row[k], row[k + 1], dummy, opt) + size[row[k + 1]];
    }
    v
}
fn settle(row: &mut [usize], lane: &[usize], key: &[f64], pos: &mut [f64]) {
    row.sort_by(|&a, &b| lane[a].cmp(&lane[b]).then_with(|| key[a].total_cmp(&key[b])).then(a.cmp(&b)));
    for (k, &i) in row.iter().enumerate() {
        pos[i] = k as f64;
    }
}
fn grow(bounds: &mut [[f64; 4]], g: usize, c: &CNode) {
    bounds[g][0] = bounds[g][0].min(c.x - c.w / 2.0);
    bounds[g][1] = bounds[g][1].min(c.y - c.h / 2.0);
    bounds[g][2] = bounds[g][2].max(c.x + c.w / 2.0);
    bounds[g][3] = bounds[g][3].max(c.y + c.h / 2.0);
}

fn place(nodes: &mut [CNode], edges: &mut [CEdge], opt: &LayeredOptions, s: &Setup, plain: &[bool], margins: &[bool]) -> Placement {
    let n = nodes.len();
    let m = s.ledges.len();
    let lane_count = s.lane_pads.len();
    let pad: Vec<f64> = s
        .lane_pads
        .iter()
        .enumerate()
        .map(|(k, &v)| if margins[k] { v.max(MARGIN) } else { v })
        .collect();
    let mut used = vec![false; lane_count];
    let mut dummies = 0;
    let mut span = vec![0; m];
    for k in 0..m {
        span[k] = (s.lnodes[s.ledges[k].head].rank - s.lnodes[s.ledges[k].tail].rank).unsigned_abs() as usize;
        if !plain[k] {
            dummies += span[k].saturating_sub(1);
        }
    }
    let direct = dummies > 20000;
    let total = s.real.len() + if direct { 0 } else { dummies };
    let mut ranks = vec![0; total];
    let mut lane = vec![0; total];
    let mut size = vec![0.0; total];
    let mut depth = vec![0.0; total];
    let mut dummy = vec![false; total];
    let mut max_rank = 0;
    for k in 0..s.real.len() {
        ranks[k] = s.lnodes[k].rank as usize;
        lane[k] = s.item_lane[k];
        size[k] = s.lnodes[k].w;
        depth[k] = s.lnodes[k].h;
        max_rank = max_rank.max(ranks[k]);
    }
    let mut first_dummy = vec![None; m];
    let mut label_item = vec![None; m];
    let mut links = Vec::new();
    let mut next = s.real.len();
    for k in 0..m {
        let e = &s.ledges[k];
        let (a, b) = if e.reversed { (e.head, e.tail) } else { (e.tail, e.head) };
        if direct || span[k] < 2 || plain[k] {
            links.push((a, b));
            continue;
        }
        let (ra, rb) = (ranks[a], ranks[b]);
        let mut label_rank = -1isize;
        if e.label_w > 0.0 {
            label_rank = (ra + ((rb - ra) >> 1)) as isize;
            if s.step == 2 && label_rank & 1 == 0 {
                label_rank -= 1;
            }
        }
        first_dummy[k] = Some(next);
        let mut prev = a;
        for r in ra + 1..rb {
            let d = next;
            next += 1;
            ranks[d] = r;
            lane[d] = (lane[a] as f64 + js_round((lane[b] as f64 - lane[a] as f64) * (r - ra) as f64 / (rb - ra) as f64)) as usize;
            dummy[d] = true;
            if r as isize == label_rank {
                size[d] = e.label_w;
                depth[d] = e.label_h;
                label_item[k] = Some(d);
            }
            links.push((prev, d));
            prev = d;
        }
        links.push((prev, b));
    }
    let mut up = vec![Vec::new(); total];
    let mut down = vec![Vec::new(); total];
    for &(a, b) in &links {
        down[a].push(b);
        up[b].push(a);
    }
    let mut rows = vec![Vec::new(); max_rank + 1];
    for i in 0..total {
        rows[ranks[i]].push(i);
    }
    let mut pos = vec![0.0; total];
    let mut key = vec![0.0; total];
    for row in &mut rows {
        for (k, &i) in row.iter().enumerate() {
            key[i] = k as f64;
        }
        settle(row, &lane, &key, &mut pos);
    }
    for sweep in 0..8 {
        if max_rank == 0 {
            break;
        }
        let downward = sweep & 1 == 0;
        for r in 1..=max_rank {
            let row = &mut rows[if downward { r } else { max_rank - r }];
            let adj = if downward { &up } else { &down };
            for &i in row.iter() {
                key[i] = if adj[i].is_empty() {
                    pos[i]
                } else {
                    adj[i].iter().map(|&k| pos[k]).sum::<f64>() / adj[i].len() as f64
                };
            }
            settle(row, &lane, &key, &mut pos);
        }
    }
    let mut breadth = vec![0.0f64; lane_count];
    let mut head: f64 = if s.lanes.is_empty() { 0.0 } else { 34.0 };
    for (k, &i) in s.lanes.iter().enumerate() {
        breadth[k] = (nodes[i].min_w - 2.0 * pad[k]).max(0.0);
        head = head.max(nodes[i].pad_top);
    }
    for row in &rows {
        for (from, to) in runs(row, &lane) {
            let v = run_size(row, from, to, &size, &dummy, opt);
            let l = lane[row[from]];
            breadth[l] = breadth[l].max(v);
        }
    }
    let mut lane_start = vec![0.0; lane_count + 1];
    for k in 0..lane_count {
        lane_start[k + 1] = lane_start[k] + breadth[k] + 2.0 * pad[k];
    }
    let mut c = vec![0.0; total];
    let mut want = vec![0.0; total];
    for row in &rows {
        for (from, to) in runs(row, &lane) {
            let l = lane[row[from]];
            let mut at = (lane_start[l] + pad[l] + lane_start[l + 1] - pad[l] - run_size(row, from, to, &size, &dummy, opt)) / 2.0;
            for k in from..=to {
                c[row[k]] = at + size[row[k]] / 2.0;
                if k < to {
                    at += size[row[k]] + gap(row[k], row[k + 1], &dummy, opt);
                }
            }
        }
    }
    for pass in 0..4 {
        if max_rank == 0 {
            break;
        }
        let downward = pass & 1 == 0;
        for r in 1..=max_rank {
            let row = &rows[if downward { r } else { max_rank - r }];
            let adj = if downward { &up } else { &down };
            for &i in row {
                let mut sum = 0.0;
                let mut count = 0;
                for &k in &adj[i] {
                    if lane[k] == lane[i] {
                        sum += c[k];
                        count += 1;
                    }
                }
                want[i] = if count > 0 { sum / count as f64 } else { c[i] };
            }
            for (from, to) in runs(row, &lane) {
                let l = lane[row[from]];
                let mut floor = lane_start[l] + pad[l];
                for k in from..=to {
                    let i = row[k];
                    c[i] = want[i].max(floor + size[i] / 2.0);
                    if k < to {
                        floor = c[i] + size[i] / 2.0 + gap(i, row[k + 1], &dummy, opt);
                    }
                }
                let mut ceil = lane_start[l + 1] - pad[l];
                for k in (from..=to).rev() {
                    let i = row[k];
                    c[i] = c[i].min(ceil - size[i] / 2.0);
                    if k > from {
                        ceil = c[i] - size[i] / 2.0 - gap(row[k - 1], i, &dummy, opt);
                    }
                }
            }
        }
    }
    let mut band_start = vec![0.0; max_rank + 1];
    let mut band_end = vec![0.0; max_rank + 1];
    let full = opt.rank_sep / s.step as f64;
    let mut sep = vec![full; max_rank + 1];
    if plain.contains(&true) {
        sep.fill(full / 3.0);
        for k in 0..m {
            let e = &s.ledges[k];
            let (a, b) = if e.reversed { (e.head, e.tail) } else { (e.tail, e.head) };
            if plain[k] {
                if e.label_h <= 0.0 || span[k] > s.step {
                    continue;
                }
                let room = full.min((e.label_h + 2.0 * TURN - depth[a].min(depth[b]) / 2.0) / span[k] as f64);
                for r in ranks[a]..ranks[b] {
                    sep[r] = sep[r].max(room);
                }
                continue;
            }
            if first_dummy[k].is_none() || span[k] as f64 <= s.step as f64 * e.minlen {
                for r in ranks[a]..ranks[b] {
                    sep[r] = full;
                }
                continue;
            }
            let d0 = first_dummy[k].unwrap();
            for d in d0 - 1..d0 + span[k] - 1 {
                let u = if d < d0 { a } else { d };
                let v = if d + 1 < d0 + span[k] - 1 { d + 1 } else { b };
                if lane[u] != lane[v] || (c[u] - c[v]).abs() >= 0.5 {
                    sep[ranks[u]] = full;
                }
            }
        }
    }
    let mut at = head + s.end_pad;
    for r in 0..=max_rank {
        let d = rows[r].iter().map(|&i| depth[i]).fold(0.0, f64::max);
        band_start[r] = at;
        band_end[r] = at + d;
        at += d + if r == max_rank { 0.0 } else { sep[r] };
    }
    let length = at + s.end_pad;
    let breadth_total = lane_start[lane_count];
    let coords = Coords {
        sideways: s.sideways,
        flipped: s.flipped,
        head,
        length,
    };
    let axis = if s.sideways { 1.0 } else { 0.0 };
    for (k, &i) in s.real.iter().enumerate() {
        let a = (band_start[ranks[k]] + band_end[ranks[k]]) / 2.0;
        let (x, y) = coords.point(c[k], a);
        nodes[i].x = x;
        nodes[i].y = y;
    }
    for (k, &i) in s.lanes.iter().enumerate() {
        let cc = (lane_start[k] + lane_start[k + 1]) / 2.0;
        let breadth = lane_start[k + 1] - lane_start[k];
        let node = &mut nodes[i];
        node.x = if s.sideways { length / 2.0 } else { cc };
        node.y = if s.sideways { cc } else { length / 2.0 };
        node.w = if s.sideways { length } else { breadth };
        node.h = if s.sideways { breadth } else { length };
        node.pad_top = head;
    }
    let mut depth_of = vec![0; n];
    let mut inner = Vec::new();
    for i in 0..n {
        if !nodes[i].is_group || s.lane_slot[i] >= 0 || s.item_of[i] >= 0 {
            continue;
        }
        let mut d = 0;
        let mut p = nodes[i].parent;
        while p >= 0 && d < MAX_HOPS {
            d += 1;
            p = nodes[p as usize].parent;
        }
        depth_of[i] = d;
        inner.push(i);
    }
    inner.sort_by(|&a, &b| depth_of[b].cmp(&depth_of[a]).then(a.cmp(&b)));
    let mut bounds = vec![[f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY]; n];
    for &i in &s.real {
        let p = nodes[i].parent;
        if p >= 0 && s.lane_slot[p as usize] < 0 {
            grow(&mut bounds, p as usize, &nodes[i]);
        }
    }
    for g in inner {
        if bounds[g][0] == f64::INFINITY {
            continue;
        }
        let b = bounds[g];
        let node = &mut nodes[g];
        node.w = (b[2] - b[0] + 2.0 * node.pad_x).max(node.min_w);
        node.x = (b[0] + b[2]) / 2.0;
        node.y = (b[1] - node.pad_top + b[3] + node.pad_bottom) / 2.0;
        node.h = b[3] - b[1] + node.pad_top + node.pad_bottom;
        if node.parent >= 0 && s.lane_slot[node.parent as usize] < 0 {
            grow(&mut bounds, node.parent as usize, node);
        }
    }
    for e in edges.iter_mut() {
        e.route.clear();
    }
    for k in 0..m {
        let e = &s.ledges[k];
        let (a, b) = if e.reversed { (e.head, e.tail) } else { (e.tail, e.head) };
        let mut pts = vec![(
            c[a],
            band_end[ranks[a]] - (band_end[ranks[a]] - band_start[ranks[a]] - depth[a]) / 2.0,
        )];
        if let Some(d0) = first_dummy[k] {
            for d in d0..d0 + span[k] - 1 {
                pts.push((c[d], band_start[ranks[d]]));
                if band_end[ranks[d]] > band_start[ranks[d]] {
                    pts.push((c[d], band_end[ranks[d]]));
                }
            }
        }
        pts.push((
            c[b],
            band_start[ranks[b]] + (band_end[ranks[b]] - band_start[ranks[b]] - depth[b]) / 2.0,
        ));
        if e.reversed {
            pts.reverse();
        }
        let out = &mut edges[s.edge_of[k]];
        for (cc, aa) in pts {
            let (x, y) = coords.point(cc, aa);
            out.route.extend([x, y, axis]);
        }
        if let Some(label) = label_item[k] {
            let (x, y) = coords.point(c[label], (band_start[ranks[label]] + band_end[ranks[label]]) / 2.0);
            out.label_x = x;
            out.label_y = y;
        } else {
            let r = &out.route;
            out.label_x = (r[0] + r[r.len() - 3]) / 2.0;
            out.label_y = (r[1] + r[r.len() - 2]) / 2.0;
        }
    }
    for e in edges.iter_mut() {
        if !e.route.is_empty() || e.src == e.dst {
            continue;
        }
        let (ax, ay) = border(&nodes[e.src], &nodes[e.dst]);
        let (bx, by) = border(&nodes[e.dst], &nodes[e.src]);
        let flat = if (bx - ax).abs() > (by - ay).abs() { 1.0 } else { 0.0 };
        e.route = vec![ax, ay, flat, bx, by, flat];
        e.label_x = (ax + bx) / 2.0;
        e.label_y = (ay + by) / 2.0;
    }
    let mut routed = vec![false; m];
    let route_points: usize = edges.iter().map(|e| e.route.len() / 3).sum();
    let work = m.saturating_mul(s.real.len() + edges.len() + route_points);
    if work <= ROUTE_BUDGET {
        let mid: Vec<f64> = ranks.iter().map(|&r| (band_start[r] + band_end[r]) / 2.0).collect();
        let mut labels: Vec<(f64, f64)> = edges
            .iter()
            .map(|e| {
                if s.sideways {
                    (e.label_y, coords.flip(e.label_x))
                } else {
                    (e.label_x, coords.flip(e.label_y))
                }
            })
            .collect();
        for k in 0..m {
            let e = &s.ledges[k];
            let ei = s.edge_of[k];
            let (a, b) = (e.tail, e.head);
            if lane[a] == lane[b] || (c[b] - c[a]).abs() < 1.0 {
                continue;
            }
            let (ac, bc, aa, ba) = (c[a], c[b], mid[a], mid[b]);
            let dc = (bc - ac).signum();
            let da = (ba - aa).signum();
            let ctx = RouteContext {
                coords: &coords,
                c: &c,
                mid: &mid,
                size: &size,
                depth: &depth,
                real_count: s.real.len(),
                breadth: breadth_total,
                length,
            };
            let p1 = vec![ac + dc * size[a] / 2.0, aa, bc, aa, bc, ba - da * depth[b] / 2.0];
            let p2 = vec![ac, aa + da * depth[a] / 2.0, ac, ba, bc - dc * size[b] / 2.0, ba];
            if attempt(&p1, 2.0, a, b, ei, e, &ctx, edges, &mut labels) || attempt(&p2, 2.0, a, b, ei, e, &ctx, edges, &mut labels) {
                routed[k] = true;
                continue;
            }
            if work > ROUTE_BUDGET / RELAY {
                continue;
            }
            let ls = if lane[a] < lane[b] {
                [lane[b], lane[a]]
            } else {
                [lane[a], lane[b]]
            };
            let mut around = Vec::new();
            for l in ls {
                let inset = (pad[l] - LEG).max((pad[l] / 2.0).min(MARGIN - LEG));
                for ch in [lane_start[l] + inset, lane_start[l + 1] - inset] {
                    if (ch - ac).abs() < size[a] / 2.0 + 4.0 || (ch - bc).abs() < size[b] / 2.0 + 4.0 {
                        continue;
                    }
                    let pts = vec![
                        ac + (ch - ac).signum() * size[a] / 2.0,
                        aa,
                        ch,
                        aa,
                        ch,
                        ba,
                        bc + (ch - bc).signum() * size[b] / 2.0,
                        ba,
                    ];
                    let (x0, y0) = coords.point(pts[0], pts[1]);
                    let (x1, y1) = coords.point(pts[6], pts[7]);
                    let mut cost = (meets(x0, y0, ei, edges) + meets(x1, y1, ei, edges)) as f64 * 1e6;
                    for p in (2..pts.len()).step_by(2) {
                        let (x0, y0) = coords.point(pts[p - 2], pts[p - 1]);
                        let (x1, y1) = coords.point(pts[p], pts[p + 1]);
                        cost += crossings(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1), ei, edges) as f64 * 1e6
                            + (x1 - x0).abs()
                            + (y1 - y0).abs();
                    }
                    around.push((pts, cost, l));
                }
            }
            around.sort_by(|a, b| a.1.total_cmp(&b.1));
            for (pts, _, l) in around {
                if attempt(&pts, 1.0, a, b, ei, e, &ctx, edges, &mut labels) {
                    routed[k] = true;
                    used[l] = true;
                    break;
                }
            }
        }
    }
    Placement {
        width: if s.sideways { length } else { breadth_total },
        height: if s.sideways { breadth_total } else { length },
        routed,
        margins: used,
        work,
    }
}

struct Coords {
    sideways: bool,
    flipped: bool,
    head: f64,
    length: f64,
}
impl Coords {
    fn flip(&self, a: f64) -> f64 {
        if self.flipped { self.head + self.length - a } else { a }
    }
    fn point(&self, c: f64, a: f64) -> (f64, f64) {
        if self.sideways { (self.flip(a), c) } else { (c, self.flip(a)) }
    }
}
struct RouteContext<'a> {
    coords: &'a Coords,
    c: &'a [f64],
    mid: &'a [f64],
    size: &'a [f64],
    depth: &'a [f64],
    real_count: usize,
    breadth: f64,
    length: f64,
}
fn overlaps(b: [f64; 4], lc: f64, la: f64, cw: f64, ah: f64) -> bool {
    b[2] > lc - cw / 2.0 && b[0] < lc + cw / 2.0 && b[3] > la - ah / 2.0 && b[1] < la + ah / 2.0
}
fn clear(b: [f64; 4], src: usize, dst: usize, ei: usize, ctx: &RouteContext, edges: &[CEdge], labels: &[(f64, f64)]) -> bool {
    for i in 0..ctx.real_count {
        if i != src && i != dst && overlaps(b, ctx.c[i], ctx.mid[i], ctx.size[i] + 8.0, ctx.depth[i] + 8.0) {
            return false;
        }
    }
    for (i, e) in edges.iter().enumerate() {
        if i != ei
            && e.label_w > 0.0
            && e.src != e.dst
            && overlaps(
                b,
                labels[i].0,
                labels[i].1,
                if ctx.coords.sideways { e.label_h } else { e.label_w },
                if ctx.coords.sideways { e.label_w } else { e.label_h },
            )
        {
            return false;
        }
    }
    true
}
fn crossings(x0: f64, y0: f64, x1: f64, y1: f64, ei: usize, edges: &[CEdge]) -> usize {
    let mut count = 0;
    for (i, e) in edges.iter().enumerate() {
        if i == ei {
            continue;
        }
        let r = &e.route;
        for j in (3..r.len()).step_by(3) {
            if r[j - 3].max(r[j]) > x0 && r[j - 3].min(r[j]) < x1 && r[j - 2].max(r[j + 1]) > y0 && r[j - 2].min(r[j + 1]) < y1 {
                count += 1;
                break;
            }
        }
    }
    count
}
fn meets(x: f64, y: f64, ei: usize, edges: &[CEdge]) -> usize {
    edges
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            if *i == ei || e.route.is_empty() {
                return false;
            }
            let r = &e.route;
            let l = r.len() - 3;
            ((r[0] - x).abs() < 1.0 && (r[1] - y).abs() < 1.0) || ((r[l] - x).abs() < 1.0 && (r[l + 1] - y).abs() < 1.0)
        })
        .count()
}
fn retraces(x0: f64, y0: f64, x1: f64, y1: f64, ei: usize, edges: &[CEdge]) -> bool {
    let upright = (x1 - x0).abs() < 0.5;
    let (lo, hi, line) = if upright {
        (y0.min(y1), y0.max(y1), x0)
    } else {
        (x0.min(x1), x0.max(x1), y0)
    };
    for (i, e) in edges.iter().enumerate() {
        if i == ei || e.src == edges[ei].src || e.dst == edges[ei].dst {
            continue;
        }
        let r = &e.route;
        for j in (3..r.len()).step_by(3) {
            let axis = if upright { 0 } else { 1 };
            let (u0, u1) = (r[j - 3 + axis], r[j + axis]);
            if (u0 - line).abs() >= 4.0 || (u1 - line).abs() >= 4.0 {
                continue;
            }
            let (v0, v1) = (r[j - 2 - axis], r[j + 1 - axis]);
            if hi.min(v0.max(v1)) - lo.max(v0.min(v1)) > 1.0 {
                return true;
            }
        }
    }
    false
}
fn attempt(
    pts: &[f64],
    margin: f64,
    a: usize,
    b: usize,
    ei: usize,
    e: &LEdge,
    ctx: &RouteContext,
    edges: &mut [CEdge],
    labels: &mut [(f64, f64)],
) -> bool {
    for p in (2..pts.len()).step_by(2) {
        if !clear(
            [
                pts[p - 2].min(pts[p]) - margin,
                pts[p - 1].min(pts[p + 1]) - margin,
                pts[p - 2].max(pts[p]) + margin,
                pts[p - 1].max(pts[p + 1]) + margin,
            ],
            a,
            b,
            ei,
            ctx,
            edges,
            labels,
        ) {
            return false;
        }
        let (x0, y0) = ctx.coords.point(pts[p - 2], pts[p - 1]);
        let (x1, y1) = ctx.coords.point(pts[p], pts[p + 1]);
        if retraces(x0, y0, x1, y1, ei, edges) {
            return false;
        }
    }
    let (mut lc, mut la, mut longest) = (0.0, 0.0, -1.0);
    let out = &edges[ei];
    if out.label_w > 0.0 {
        let (cw, ah) = (e.label_w, e.label_h);
        for p in (2..pts.len()).step_by(2) {
            let (cc, ca) = ((pts[p - 2] + pts[p]) / 2.0, (pts[p - 1] + pts[p + 1]) / 2.0);
            let (across, along) = ((pts[p] - pts[p - 2]).abs(), (pts[p + 1] - pts[p - 1]).abs());
            let run = across + along;
            if run <= longest || if across > 0.0 { across < cw + 8.0 } else { along < ah + 8.0 } {
                continue;
            }
            if cc - cw / 2.0 < 0.0 || cc + cw / 2.0 > ctx.breadth || ca - ah / 2.0 < 0.0 || ca + ah / 2.0 > ctx.length {
                continue;
            }
            if !clear(
                [cc - cw / 2.0, ca - ah / 2.0, cc + cw / 2.0, ca + ah / 2.0],
                usize::MAX,
                usize::MAX,
                ei,
                ctx,
                edges,
                labels,
            ) {
                continue;
            }
            let (lx, ly) = ctx.coords.point(cc, ca);
            if crossings(
                lx - out.label_w / 2.0,
                ly - out.label_h / 2.0,
                lx + out.label_w / 2.0,
                ly + out.label_h / 2.0,
                ei,
                edges,
            ) > 0
            {
                continue;
            }
            lc = cc;
            la = ca;
            longest = run;
        }
        if longest < 0.0 {
            return false;
        }
    }
    let mut way = vec![pts[0], pts[1]];
    for p in (2..pts.len()).step_by(2) {
        let (dx, dy) = (pts[p] - pts[p - 2], pts[p + 1] - pts[p - 1]);
        let run = dx.abs() + dy.abs();
        let first = if p > 2 { TURN } else { 0.0 };
        let last = if p + 2 < pts.len() { TURN } else { 0.0 };
        if run > first + last + 1.0 {
            if first > 0.0 {
                way.extend([pts[p - 2] + dx / run * first, pts[p - 1] + dy / run * first]);
            }
            if last > 0.0 {
                way.extend([pts[p] - dx / run * last, pts[p + 1] - dy / run * last]);
            }
        }
        way.extend([pts[p], pts[p + 1]]);
    }
    let out = &mut edges[ei];
    out.route.clear();
    let axis = if ctx.coords.sideways { 1.0 } else { 0.0 };
    for p in (0..way.len()).step_by(2) {
        let crosswise = if p == 0 { way[0] != way[2] } else { way[p] != way[p - 2] };
        let (x, y) = ctx.coords.point(way[p], way[p + 1]);
        out.route.extend([x, y, if crosswise { 1.0 - axis } else { axis }]);
    }
    if out.label_w > 0.0 {
        let (x, y) = ctx.coords.point(lc, la);
        out.label_x = x;
        out.label_y = y;
        labels[ei] = (lc, la);
    }
    true
}
fn border(from: &CNode, to: &CNode) -> (f64, f64) {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let (hw, hh) = ((from.w / 2.0).max(0.5), (from.h / 2.0).max(0.5));
    if dx.abs() < hw && dy.abs() < hh {
        let (to_side, to_end) = (hw - dx.abs(), hh - dy.abs());
        return if to_side < to_end {
            (from.x + if dx < 0.0 { -hw } else { hw }, to.y)
        } else {
            (to.x, from.y + if dy < 0.0 { -hh } else { hh })
        };
    }
    let scale = (if dx != 0.0 { hw / dx.abs() } else { f64::INFINITY }).min(if dy != 0.0 { hh / dy.abs() } else { f64::INFINITY });
    (from.x + dx * scale, from.y + dy * scale)
}
