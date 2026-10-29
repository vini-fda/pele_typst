// Layered (Sugiyama-style) layout of one flat graph, after pele's layout/layered.ts. Ranks run
// down the y axis; callers rotate the result for other directions. See the TypeScript source for
// the aims each step serves; the comments here keep only what the code does not say.

use std::collections::HashSet;

use indexmap::IndexMap;

use crate::util::{ord, stable_sort_by};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Node = 0,
    Dummy = 1,
    Label = 2,
    StartPort = 3,
    EndPort = 4,
}

#[derive(Debug, Clone)]
pub struct LNode {
    pub w: f64,
    pub h: f64,
    pub kind: Kind,
    pub pin: f64,
    // Port's cross-flow destination; NaN if unknown.
    pub hint: f64,
    // Initial cross-flow position; NaN if unknown.
    pub key: f64,
    pub span: f64,
    pub x: f64,
    pub y: f64,
    pub rank: i64,
}

#[derive(Debug, Clone)]
pub struct LEdge {
    pub tail: usize,
    pub head: usize,
    pub minlen: f64,
    pub label_w: f64,
    pub label_h: f64,
    pub tail_dx: f64,
    pub head_dx: f64,
    pub tail_fixed: bool,
    pub head_fixed: bool,
    pub reversed: bool,
    pub points: Vec<f64>,
    pub label_x: f64,
    pub label_y: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Groups {
    pub of: Vec<i64>,
    pub parent: Vec<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct LayeredOptions {
    pub node_sep: f64,
    pub edge_sep: f64,
    pub rank_sep: f64,
    pub port_sep: f64,
    pub tracks: bool,
    pub straight: bool,
    pub apart: bool,
    pub head_room: f64,
    pub groups: Option<Groups>,
    pub order_only: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LayeredResult {
    pub width: f64,
    pub height: f64,
    pub crossings: Option<f64>,
    pub declared: Option<f64>,
}

pub fn lnode(w: f64, h: f64, kind: Kind) -> LNode {
    LNode {
        w,
        h,
        kind,
        pin: 0.0,
        hint: f64::NAN,
        key: f64::NAN,
        span: 0.0,
        x: 0.0,
        y: 0.0,
        rank: 0,
    }
}

pub fn ledge(tail: usize, head: usize, minlen: f64, label_w: f64, label_h: f64) -> LEdge {
    LEdge {
        tail,
        head,
        minlen,
        label_w,
        label_h,
        tail_dx: 0.0,
        head_dx: 0.0,
        tail_fixed: false,
        head_fixed: false,
        reversed: false,
        points: Vec::new(),
        label_x: 0.0,
        label_y: 0.0,
    }
}

const MAX_SWEEPS: usize = 24;
const MAX_DUMMIES: i64 = 20000;
const PORT_PITCH: f64 = 16.0;
const MIN_PITCH: f64 = 8.0;
const MIN_SLOPE: f64 = 0.125;
const MAX_GAP: f64 = 3.0;
const TRACK_MARGIN: f64 = 18.0;
const TRACK_PITCH: f64 = 10.0;
const TRACK_SPREAD: f64 = 14.0;
const LEVEL_WIDE: f64 = 2.0;
const LEVEL_MARGIN: f64 = 28.0;
const LEVEL_PITCH: f64 = 12.0;
const LEVEL_LEAD: f64 = 12.0;
const LEVEL_CLEAR: f64 = 8.0;
const LEVEL_TOUCH: f64 = 4.0;
const CROSSED_SLOPE: f64 = 0.5;
const IN_LINE: f64 = 0.5;
const TWIN: f64 = 1.0;
const SNAP: f64 = 12.0;
const BEND_SNAP: f64 = 24.0;
const LEAN_CLEAR: f64 = 40.0;
const MAX_WEIGHED: usize = 400;
const SAMPLES: usize = 16;

const HELD: u8 = 0;
const LEAVES: u8 = 1;
const ARRIVES: u8 = 2;

fn is_dummy(kind: u8) -> bool {
    kind == Kind::Dummy as u8 || kind == Kind::Label as u8
}

fn sort_range(list: &mut [usize], from: usize, to: usize, key: &[i64]) {
    for i in from + 1..to {
        let item = list[i];
        let k = key[item];
        let mut j = i as isize - 1;
        while j >= from as isize && (key[list[j as usize]] > k || (key[list[j as usize]] == k && list[j as usize] > item)) {
            list[(j + 1) as usize] = list[j as usize];
            j -= 1;
        }
        list[(j + 1) as usize] = item;
    }
}

struct Graph {
    n: usize,
    real: usize,
    max_rank: usize,
    w: Vec<f64>,
    kind: Vec<u8>,
    rank: Vec<usize>,
    ef: Vec<usize>,
    et: Vec<usize>,
    efd: Vec<f64>,
    etd: Vec<f64>,
    succ_start: Vec<usize>,
    succ_e: Vec<usize>,
    pred_start: Vec<usize>,
    pred_e: Vec<usize>,
    layer_start: Vec<usize>,
    order: Vec<usize>,
    pos: Vec<usize>,
    offsets: bool,
    fixed: Vec<u8>,
    grp: Option<Vec<i64>>,
    group_parent: Option<Vec<i64>>,
}

impl Graph {
    fn set_positions(&mut self) {
        for r in 0..=self.max_rank {
            let s = self.layer_start[r];
            for i in s..self.layer_start[r + 1] {
                self.pos[self.order[i]] = i - s;
            }
        }
    }
}

#[derive(Default)]
struct Pieces {
    from: Vec<f64>,
    to: Vec<f64>,
    rank: Vec<usize>,
    lower: Vec<i64>,
    track: Vec<i64>,
    levels: Vec<i64>,
    wide: Vec<u8>,
}

pub fn layered(nodes: &mut [LNode], edges: &mut [LEdge], opt: &LayeredOptions) -> LayeredResult {
    let real = nodes.len();
    if real == 0 {
        return LayeredResult::default();
    }
    let m = edges.len();
    if real == 1 && m == 0 && nodes[0].kind == Kind::Node {
        let node = &mut nodes[0];
        node.rank = 0;
        node.x = node.w / 2.0;
        node.y = node.h / 2.0;
        return LayeredResult {
            width: node.w,
            height: node.h,
            ..Default::default()
        };
    }

    // Every edge gets a middle rank of its own when labels or parallel edges need the room.
    let mut step: i64 = 1;
    if m > 0 {
        let mut seen = HashSet::new();
        for e in edges.iter() {
            let a = e.tail.min(e.head);
            let b = e.tail.max(e.head);
            let key = a * real + b;
            if e.label_w > 0.0 || seen.contains(&key) {
                step = 2;
                break;
            }
            seen.insert(key);
        }
    }

    orient(nodes, edges);
    rank(nodes, edges, step);

    let mut span = vec![0i64; m];
    let mut dummies: i64 = 0;
    for ei in 0..m {
        let e = &edges[ei];
        span[ei] = (nodes[e.head].rank - nodes[e.tail].rank).abs();
        if span[ei] > 1 {
            dummies += span[ei] - 1;
        }
    }
    let mut direct = vec![false; m];
    if dummies > MAX_DUMMIES {
        let mut longest: Vec<usize> = (0..m).filter(|&ei| span[ei] > 1).collect();
        stable_sort_by(&mut longest, |&a, &b| span[b].cmp(&span[a]).then(a.cmp(&b)));
        for ei in longest {
            if dummies <= MAX_DUMMIES {
                break;
            }
            direct[ei] = true;
            dummies -= span[ei] - 1;
        }
    }

    let n = real + dummies as usize;
    let mut w = vec![0.0f64; n];
    let mut h = vec![0.0f64; n];
    let mut kind = vec![0u8; n];
    let mut rank_of = vec![0usize; n];
    let mut max_rank = 0usize;
    for i in 0..real {
        let node = &nodes[i];
        w[i] = node.w;
        h[i] = node.h;
        kind[i] = node.kind as u8;
        rank_of[i] = node.rank as usize;
        if node.rank as usize > max_rank {
            max_rank = node.rank as usize;
        }
    }

    let mut segments = 0usize;
    for ei in 0..m {
        if !direct[ei] {
            segments += span[ei] as usize;
        }
    }
    let mut ef = vec![0usize; segments];
    let mut et = vec![0usize; segments];
    let mut efd = vec![0.0f64; segments];
    let mut etd = vec![0.0f64; segments];
    let mut first_dummy = vec![-1i64; m];
    // Which way the edges with a label in each rank head: 1 down, 2 up, 3 both.
    let mut heads = vec![0u8; max_rank + 1];
    let mut label_node = vec![-1i64; m];
    let mut spans = false;
    for node in nodes.iter() {
        if node.span > 0.0 && node.kind == Kind::Node {
            spans = true;
        }
    }
    let mut efk: Option<Vec<u8>> = if spans { Some(vec![0u8; segments]) } else { None };
    let mut etk: Option<Vec<u8>> = if spans { Some(vec![0u8; segments]) } else { None };
    let mut first_seg = vec![-1i64; m];
    let mut last_seg = vec![-1i64; m];
    let mut key = vec![f64::NAN; n];
    for i in 0..real {
        key[i] = nodes[i].key;
    }
    let mut offsets = false;
    {
        let mut next = real;
        let mut ce = 0usize;
        for ei in 0..m {
            if direct[ei] {
                continue;
            }
            let e = &edges[ei];
            let a = if e.reversed { e.head } else { e.tail };
            let b = if e.reversed { e.tail } else { e.head };
            let a_dx = if e.reversed { e.head_dx } else { e.tail_dx };
            let b_dx = if e.reversed { e.tail_dx } else { e.head_dx };
            if a_dx != 0.0 || b_dx != 0.0 {
                offsets = true;
            }
            let ra = rank_of[a] as i64;
            let rb = rank_of[b] as i64;
            let mut label_rank: i64 = -1;
            if e.label_w > 0.0 {
                label_rank = ra + ((rb - ra) >> 1);
                if step == 2 && rb - ra >= 4 {
                    if label_rank & 1 == 1 {
                        label_rank += if e.reversed { 1 } else { -1 };
                    }
                } else if step == 2 && label_rank & 1 == 0 {
                    label_rank += if e.reversed { 1 } else { -1 };
                }
            }
            let mut prev = a;
            if rb - ra > 1 {
                first_dummy[ei] = next as i64;
            }
            first_seg[ei] = ce as i64;
            if let Some(efk) = efk.as_mut()
                && !(if e.reversed { e.head_fixed } else { e.tail_fixed })
            {
                efk[ce] = if e.reversed { ARRIVES } else { LEAVES };
            }
            for r in ra + 1..rb {
                let d = next;
                next += 1;
                rank_of[d] = r as usize;
                key[d] = key[a] + ((key[b] - key[a]) * (r - ra) as f64) / (rb - ra) as f64;
                if r == label_rank {
                    w[d] = e.label_w;
                    h[d] = e.label_h;
                    kind[d] = Kind::Label as u8;
                    label_node[ei] = d as i64;
                    heads[r as usize] |= if e.reversed { 2 } else { 1 };
                } else {
                    kind[d] = Kind::Dummy as u8;
                }
                ef[ce] = prev;
                et[ce] = d;
                efd[ce] = if prev == a { a_dx } else { 0.0 };
                ce += 1;
                prev = d;
            }
            ef[ce] = prev;
            et[ce] = b;
            efd[ce] = if prev == a { a_dx } else { 0.0 };
            etd[ce] = b_dx;
            if let Some(etk) = etk.as_mut()
                && !(if e.reversed { e.tail_fixed } else { e.head_fixed })
            {
                etk[ce] = if e.reversed { LEAVES } else { ARRIVES };
            }
            last_seg[ei] = ce as i64;
            ce += 1;
        }
    }

    // Bends belong to the innermost group containing both edge endpoints.
    let mut grp: Option<Vec<i64>> = None;
    if let Some(groups) = &opt.groups {
        let of = &groups.of;
        let parent = &groups.parent;
        let mut g = vec![0i64; n];
        g[..real].copy_from_slice(&of[..real]);
        let depth = |mut gr: i64| {
            let mut d = 0;
            while gr >= 0 {
                d += 1;
                gr = parent[gr as usize];
            }
            d
        };
        let lca = |mut x: i64, mut y: i64| {
            let mut dx = depth(x);
            let mut dy = depth(y);
            while dx > dy {
                x = parent[x as usize];
                dx -= 1;
            }
            while dy > dx {
                y = parent[y as usize];
                dy -= 1;
            }
            while x != y {
                x = parent[x as usize];
                y = parent[y as usize];
            }
            x
        };
        for ei in 0..m {
            if first_dummy[ei] < 0 {
                continue;
            }
            let e = &edges[ei];
            let gr = lca(of[e.tail], of[e.head]);
            let d0 = first_dummy[ei] as usize;
            for d in d0..d0 + span[ei] as usize - 1 {
                g[d] = gr;
            }
        }
        grp = Some(g);
    }

    let mut succ_start = vec![0usize; n + 1];
    let mut pred_start = vec![0usize; n + 1];
    for e in 0..segments {
        succ_start[ef[e] + 1] += 1;
        pred_start[et[e] + 1] += 1;
    }
    for i in 0..n {
        succ_start[i + 1] += succ_start[i];
        pred_start[i + 1] += pred_start[i];
    }
    let mut succ_e = vec![0usize; segments];
    let mut pred_e = vec![0usize; segments];
    {
        let mut sf = succ_start[..n].to_vec();
        let mut pf = pred_start[..n].to_vec();
        for e in 0..segments {
            succ_e[sf[ef[e]]] = e;
            sf[ef[e]] += 1;
            pred_e[pf[et[e]]] = e;
            pf[et[e]] += 1;
        }
    }

    let mut layer_start = vec![0usize; max_rank + 2];
    for i in 0..n {
        layer_start[rank_of[i] + 1] += 1;
    }
    for r in 0..=max_rank {
        layer_start[r + 1] += layer_start[r];
    }
    let mut order = vec![0usize; n];
    let pos = vec![0usize; n];

    // Initial order: depth-first from the top so that subtrees start out contiguous.
    {
        let mut by_rank = vec![0usize; n];
        let mut fill = layer_start[..=max_rank].to_vec();
        for i in 0..n {
            by_rank[fill[rank_of[i]]] = i;
            fill[rank_of[i]] += 1;
        }
        fill[..=max_rank].copy_from_slice(&layer_start[..=max_rank]);
        let mut visited = vec![false; n];
        let mut stack: Vec<usize> = Vec::new();
        for k in 0..n {
            if visited[by_rank[k]] {
                continue;
            }
            stack.push(by_rank[k]);
            while let Some(v) = stack.pop() {
                if visited[v] {
                    continue;
                }
                visited[v] = true;
                order[fill[rank_of[v]]] = v;
                fill[rank_of[v]] += 1;
                let mut j = succ_start[v + 1];
                while j > succ_start[v] {
                    j -= 1;
                    let wv = et[succ_e[j]];
                    if !visited[wv] {
                        stack.push(wv);
                    }
                }
            }
        }
    }

    let sort_rank = |order: &mut Vec<usize>, r: usize, key: &dyn Fn(usize) -> f64| -> bool {
        let s = layer_start[r];
        let e = layer_start[r + 1];
        if e - s < 2 {
            return false;
        }
        for &v in &order[s..e] {
            if key(v).is_nan() {
                return false;
            }
        }
        let mut layer: Vec<(usize, usize)> = order[s..e].iter().copied().enumerate().map(|(i, v)| (v, i)).collect();
        stable_sort_by(&mut layer, |a, b| ord(key(a.0) - key(b.0)).then(a.1.cmp(&b.1)));
        for (i, (v, _)) in layer.into_iter().enumerate() {
            order[s + i] = v;
        }
        true
    };
    for r in 0..=max_rank {
        sort_rank(&mut order, r, &|v| key[v]);
    }

    let pinned = nodes.iter().any(|node| node.pin != 0.0);
    if pinned {
        for r in 0..=max_rank {
            let s = layer_start[r];
            let e = layer_start[r + 1];
            let pin_of = |v: usize| if v < real { nodes[v].pin } else { 0.0 };
            let mut layer: Vec<(usize, usize)> = order[s..e].iter().copied().enumerate().map(|(i, v)| (v, i)).collect();
            stable_sort_by(&mut layer, |a, b| ord(pin_of(a.0) - pin_of(b.0)).then(a.1.cmp(&b.1)));
            for (i, (v, _)) in layer.into_iter().enumerate() {
                order[s + i] = v;
            }
        }
    }
    // Preserve port order to avoid crossings outside the group.
    let mut fixed = vec![0u8; max_rank + 1];
    {
        let hint = |v: usize| {
            if v < real && (nodes[v].kind == Kind::StartPort || nodes[v].kind == Kind::EndPort) {
                nodes[v].hint
            } else {
                f64::NAN
            }
        };
        for r in [0, max_rank] {
            if sort_rank(&mut order, r, &hint) {
                fixed[r] = 1;
            }
        }
    }

    let mut g = Graph {
        n,
        real,
        max_rank,
        w: w.clone(),
        kind: kind.clone(),
        rank: rank_of.clone(),
        ef,
        et,
        efd,
        etd,
        succ_start,
        succ_e,
        pred_start,
        pred_e,
        layer_start,
        order,
        pos,
        offsets,
        fixed,
        grp,
        group_parent: opt.groups.as_ref().map(|gr| gr.parent.clone()),
    };
    g.set_positions();

    let mut crossings = 0.0;
    let mut declared = 0.0;
    if segments > 0 && max_rank > 0 {
        let (c, d) = reduce_crossings(&mut g, nodes);
        crossings = c as f64;
        declared = d as f64;
        g.set_positions();
    }
    if opt.order_only {
        // Points are (rank, cross-flow position) pairs.
        let at = position(&mut g, opt);
        for i in 0..real {
            nodes[i].x = at[i];
        }
        for ei in 0..m {
            let e = &mut edges[ei];
            let a = if e.reversed { e.head } else { e.tail };
            let b = if e.reversed { e.tail } else { e.head };
            let mut pts = vec![g.rank[a] as f64, at[a]];
            if !direct[ei] && first_dummy[ei] >= 0 {
                let d0 = first_dummy[ei] as usize;
                for d in d0..d0 + span[ei] as usize - 1 {
                    pts.push(g.rank[d] as f64);
                    pts.push(at[d]);
                }
            }
            pts.push(g.rank[b] as f64);
            pts.push(at[b]);
            e.points = pts;
        }
        return LayeredResult {
            width: 0.0,
            height: 0.0,
            crossings: Some(crossings),
            declared: Some(declared),
        };
    }

    if let (Some(efk), Some(etk)) = (&efk, &etk) {
        spread(&mut g, nodes, efk, etk);
    }

    let mut xs = position(&mut g, opt);

    let empty = empty_ranks(&g);
    if let (Some(efk), Some(etk)) = (&efk, &etk) {
        let free = free_ends(&g, efk, etk);
        align_labels(&g, nodes, &mut xs, &free, opt, empty.as_deref());
        settle(&mut g, nodes, &xs, efk, etk, &free, empty.as_deref());
        snap(&g, &mut xs, empty.as_deref(), opt);
        for ei in 0..m {
            if direct[ei] {
                continue;
            }
            let e = &mut edges[ei];
            let a_dx = g.efd[first_seg[ei] as usize];
            let b_dx = g.etd[last_seg[ei] as usize];
            e.tail_dx = if e.reversed { b_dx } else { a_dx };
            e.head_dx = if e.reversed { a_dx } else { b_dx };
        }
    }
    let _ = (&mut efk, &mut etk);

    align_ports(&g, &mut xs, empty.as_deref());

    let mut on_line = vec![0u8; n];
    if let Some(empty) = &empty
        && !opt.tracks
    {
        straighten(
            &g,
            &mut xs,
            empty,
            &first_dummy,
            &span,
            if opt.apart { 0.0 } else { opt.edge_sep },
            &mut on_line,
        );
    }
    if !opt.tracks {
        past_labels(&g, &mut xs, &first_dummy, &span, opt.edge_sep, &mut on_line);
    }

    let mut piece_start = vec![0usize; m + 1];
    let mut pieces: Option<Pieces> = if opt.tracks || (efk.is_some() && !opt.straight) {
        Some(Pieces::default())
    } else {
        None
    };
    let mut track_count = vec![0i64; max_rank + 1];
    if let Some(pieces) = pieces.as_mut() {
        for ei in 0..m {
            piece_start[ei] = pieces.from.len();
            if direct[ei] {
                continue;
            }
            let d0 = first_dummy[ei];
            let range = if d0 < 0 {
                0..0
            } else {
                d0 as usize..d0 as usize + span[ei] as usize - 1
            };
            // An edge that keeps a bend to stay clear of its twin is drawn through the bend, as it is.
            let mut twin = false;
            for d in range.clone() {
                let bare = empty.as_ref().is_some_and(|e| e[g.rank[d]] == 1);
                if !opt.tracks && bare != (on_line[d] == 1) {
                    twin = true;
                }
            }
            if twin {
                continue;
            }
            let fs = first_seg[ei] as usize;
            let mut x = xs[g.ef[fs]] + g.efd[fs];
            let mut rank = g.rank[g.ef[fs]];
            for d in range {
                if empty.as_ref().is_some_and(|e| e[g.rank[d]] == 1) {
                    continue;
                }
                pieces.from.push(x);
                pieces.to.push(xs[d]);
                pieces.rank.push(rank);
                pieces.lower.push(d as i64);
                x = xs[d];
                rank = g.rank[d];
            }
            let ls = last_seg[ei] as usize;
            pieces.from.push(x);
            pieces.to.push(xs[g.et[ls]] + g.etd[ls]);
            pieces.rank.push(rank);
            pieces.lower.push(-1);
        }
        piece_start[m] = pieces.from.len();
    }

    let port_only = |r: usize, k: Kind| -> bool {
        let mut found = false;
        for i in g.layer_start[r]..g.layer_start[r + 1] {
            let kk = g.kind[g.order[i]];
            if kk == k as u8 {
                found = true;
            } else if kk != Kind::Dummy as u8 {
                return false;
            }
        }
        found
    };
    let start_ranks = if port_only(0, Kind::StartPort) { step as usize } else { 0 };
    let end_ranks = if port_only(max_rank, Kind::EndPort) { step as usize } else { 0 };
    let mut run = vec![0.0f64; max_rank + 1];
    for e in 0..segments {
        let across = (xs[g.ef[e]] + g.efd[e] - xs[g.et[e]] - g.etd[e]).abs();
        if across > run[g.rank[g.ef[e]]] {
            run[g.rank[g.ef[e]]] = across;
        }
    }
    let mut band_top = vec![0.0f64; max_rank + 1];
    let mut band_bottom = vec![0.0f64; max_rank + 1];
    let usual = |r: usize| -> f64 {
        (if r < start_ranks || r as i64 >= max_rank as i64 - end_ranks as i64 {
            opt.port_sep
        } else {
            opt.rank_sep
        }) / step as f64
    };
    let mut gap_under = vec![0.0f64; max_rank + 1];
    for r in 0..max_rank {
        gap_under[r] = if pieces.is_some() {
            usual(r)
        } else {
            usual(r).max(((MAX_GAP * opt.rank_sep) / step as f64).min(run[r] * MIN_SLOPE))
        };
    }
    let mut between = vec![1u8; max_rank + 1];
    for i in 0..real {
        between[g.rank[i]] = 0;
    }
    for r in 1..max_rank {
        if opt.tracks || between[r] == 0 || (heads[r] != 1 && heads[r] != 2) {
            continue;
        }
        let shift = (opt.head_room / 2.0).min(gap_under[if heads[r] == 1 { r - 1 } else { r }] / 2.0);
        gap_under[r - 1] += if heads[r] == 1 { -shift } else { shift };
        gap_under[r] += if heads[r] == 1 { shift } else { -shift };
    }
    let margin = if opt.tracks { TRACK_MARGIN } else { LEVEL_MARGIN };
    let least_pitch = if opt.tracks { TRACK_PITCH } else { LEVEL_PITCH };
    let most_pitch = if opt.tracks { TRACK_SPREAD } else { LEVEL_PITCH };
    let mut below = vec![0usize; max_rank + 1];
    if let Some(pieces) = pieces.as_mut() {
        let mut room = vec![0.0f64; max_rank + 1];
        for r in 0..max_rank {
            if empty.as_ref().is_some_and(|e| e[r] == 1) {
                continue;
            }
            let mut next = r + 1;
            room[r] = gap_under[r];
            while next < max_rank && empty.as_ref().is_some_and(|e| e[next] == 1) {
                room[r] += gap_under[next];
                next += 1;
            }
            below[r] = next;
        }
        for k in 0..pieces.from.len() {
            let wide = (pieces.from[k] - pieces.to[k]).abs() > LEVEL_WIDE * room[pieces.rank[k]];
            pieces.wide.push(wide as u8);
        }
        let mut tall = vec![0.0f64; max_rank + 1];
        assign_tracks(pieces, &mut track_count, !opt.tracks, &room, &mut tall);
        for r in 0..max_rank {
            if track_count[r] > 0 {
                gap_under[r] += (2.0 * margin + (track_count[r] - 1) as f64 * least_pitch - room[r]).max(0.0);
            } else if tall[r] > 0.0 {
                gap_under[r] += ((MAX_GAP * opt.rank_sep).min(tall[r]) - room[r]).max(0.0);
            }
        }
    }
    let mut y = 0.0;
    for r in 0..=max_rank {
        let mut hh: f64 = 0.0;
        for i in g.layer_start[r]..g.layer_start[r + 1] {
            if h[g.order[i]] > hh {
                hh = h[g.order[i]];
            }
        }
        band_top[r] = y;
        band_bottom[r] = y + hh;
        y += hh + gap_under[r];
    }
    let height = y;
    let inset = |d: usize| -> f64 {
        if g.kind[d] == Kind::Dummy as u8 && between[g.rank[d]] == 1 && !opt.tracks {
            (band_bottom[g.rank[d]] - band_top[g.rank[d]]) / 4.0
        } else {
            0.0
        }
    };
    let track_y = |pieces: &Pieces, piece: usize, on_track: i64| -> f64 {
        let r = pieces.rank[piece];
        let top = band_bottom[r];
        let bottom = band_top[below[r]];
        let count = if opt.tracks { track_count[r] } else { pieces.levels[piece] };
        let pitch = if count > 1 {
            most_pitch.min((bottom - top - 2.0 * margin) / (count - 1) as f64)
        } else {
            0.0
        };
        (top + bottom) / 2.0 + (on_track as f64 - (count - 1) as f64 / 2.0) * pitch
    };

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    for i in 0..n {
        let half = w[i] / 2.0;
        if xs[i] - half < min_x {
            min_x = xs[i] - half;
        }
        if xs[i] + half > max_x {
            max_x = xs[i] + half;
        }
    }
    let mut arrives = vec![0u8; max_rank + 1];
    let mut leaves = vec![0u8; max_rank + 1];
    for i in 0..real {
        if g.kind[i] != Kind::Node as u8 || efk.is_none() {
            continue;
        }
        if g.pred_start[i + 1] > g.pred_start[i] {
            arrives[g.rank[i]] = 1;
        }
        if g.succ_start[i + 1] > g.succ_start[i] {
            leaves[g.rank[i]] = 1;
        }
    }
    for i in 0..real {
        let node = &mut nodes[i];
        let r = node.rank as usize;
        node.x = xs[i] - min_x;
        if g.kind[i] == Kind::Node as u8 && leaves[r] == 1 && arrives[r] == 0 {
            node.y = band_bottom[r] - node.h / 2.0;
        } else if g.kind[i] == Kind::Node as u8 && arrives[r] == 1 && leaves[r] == 0 {
            node.y = band_top[r] + node.h / 2.0;
        } else {
            node.y = (band_top[r] + band_bottom[r]) / 2.0;
        }
    }

    for ei in 0..m {
        let (first_x, first_y, first_h, last_x, last_y, last_h) = {
            let e = &edges[ei];
            let first = &nodes[if e.reversed { e.head } else { e.tail }];
            let last = &nodes[if e.reversed { e.tail } else { e.head }];
            (first.x, first.y, first.h, last.x, last.y, last.h)
        };
        let e = &mut edges[ei];
        let mut pts: Vec<f64> = vec![first_x + if e.reversed { e.head_dx } else { e.tail_dx }, first_y + first_h / 2.0];
        let d0 = first_dummy[ei];
        if pieces.as_ref().is_some_and(|_| piece_start[ei + 1] > piece_start[ei]) {
            let pieces = pieces.as_ref().unwrap();
            for k in piece_start[ei]..piece_start[ei + 1] {
                if pieces.track[k] >= 0 && (opt.tracks || pieces.wide[k] == 1) {
                    let across = track_y(pieces, k, pieces.track[k]);
                    pts.extend_from_slice(&[pieces.from[k] - min_x, across, pieces.to[k] - min_x, across]);
                } else if pieces.track[k] > 0 {
                    pts.push(pieces.from[k] - min_x);
                    pts.push(track_y(pieces, k, pieces.track[k] - 1));
                }
                let d = pieces.lower[k];
                if d < 0 {
                    continue;
                }
                let d = d as usize;
                pts.push(xs[d] - min_x);
                pts.push(band_top[g.rank[d]] + inset(d));
                if band_bottom[g.rank[d]] > band_top[g.rank[d]] {
                    pts.push(xs[d] - min_x);
                    pts.push(band_bottom[g.rank[d]] - inset(d));
                }
            }
        } else if d0 >= 0 {
            let d0 = d0 as usize;
            for d in d0..d0 + span[ei] as usize - 1 {
                if on_line[d] == 1 {
                    continue;
                }
                let x = xs[d] - min_x;
                let top = band_top[g.rank[d]];
                let bottom = band_bottom[g.rank[d]];
                pts.push(x);
                pts.push(top + inset(d));
                if bottom > top {
                    pts.push(x);
                    pts.push(bottom - inset(d));
                }
            }
        }
        pts.push(last_x + if e.reversed { e.tail_dx } else { e.head_dx });
        pts.push(last_y - last_h / 2.0);
        if e.reversed {
            let mut i = 0;
            let mut j = pts.len() - 2;
            while i < j {
                pts.swap(i, j);
                pts.swap(i + 1, j + 1);
                i += 2;
                j -= 2;
            }
        }
        let ln = label_node[ei];
        if ln >= 0 {
            let ln = ln as usize;
            e.label_x = xs[ln] - min_x;
            e.label_y = (band_top[g.rank[ln]] + band_bottom[g.rank[ln]]) / 2.0;
        } else if e.label_w > 0.0 {
            e.label_x = (pts[0] + pts[pts.len() - 2]) / 2.0;
            e.label_y = (pts[1] + pts[pts.len() - 1]) / 2.0;
        }
        e.points = pts;
    }

    LayeredResult {
        width: max_x - min_x,
        height,
        ..Default::default()
    }
}

// Reverses the edges that close a cycle, found by depth-first search in declaration order.
pub fn orient(nodes: &[LNode], edges: &mut [LEdge]) {
    let n = nodes.len();
    let m = edges.len();
    let mut start = vec![0usize; n + 1];
    for e in edges.iter_mut() {
        e.reversed = nodes[e.head].kind == Kind::StartPort || nodes[e.tail].kind == Kind::EndPort;
        if !e.reversed {
            start[e.tail + 1] += 1;
        }
    }
    for i in 0..n {
        start[i + 1] += start[i];
    }
    let mut out = vec![0usize; start[n]];
    {
        let mut fill = start[..n].to_vec();
        for i in 0..m {
            if !edges[i].reversed {
                out[fill[edges[i].tail]] = i;
                fill[edges[i].tail] += 1;
            }
        }
    }
    let mut state = vec![0u8; n];
    let mut cursor = start[..n].to_vec();
    let mut stack: Vec<usize> = Vec::new();
    for root in 0..n {
        if state[root] != 0 {
            continue;
        }
        stack.push(root);
        state[root] = 1;
        while let Some(&v) = stack.last() {
            if cursor[v] < start[v + 1] {
                let ei = out[cursor[v]];
                cursor[v] += 1;
                let w = edges[ei].head;
                if state[w] == 1 {
                    edges[ei].reversed = true;
                } else if state[w] == 0 {
                    state[w] = 1;
                    stack.push(w);
                }
            } else {
                state[v] = 2;
                stack.pop();
            }
        }
    }
}

pub fn rank(nodes: &mut [LNode], edges: &[LEdge], step: i64) {
    let n = nodes.len();
    let m = edges.len();
    let mut from = vec![0usize; m];
    let mut to = vec![0usize; m];
    let mut len = vec![0i64; m];
    let mut out_start = vec![0usize; n + 1];
    let mut in_start = vec![0usize; n + 1];
    for (i, e) in edges.iter().enumerate() {
        let a = if e.reversed { e.head } else { e.tail };
        let b = if e.reversed { e.tail } else { e.head };
        from[i] = a;
        to[i] = b;
        len[i] = (1f64.max(e.minlen) as i64) * step;
        out_start[a + 1] += 1;
        in_start[b + 1] += 1;
    }
    for i in 0..n {
        out_start[i + 1] += out_start[i];
        in_start[i + 1] += in_start[i];
    }
    let mut out_e = vec![0usize; m];
    let mut in_e = vec![0usize; m];
    {
        let mut of = out_start[..n].to_vec();
        let mut inf = in_start[..n].to_vec();
        for i in 0..m {
            out_e[of[from[i]]] = i;
            of[from[i]] += 1;
            in_e[inf[to[i]]] = i;
            inf[to[i]] += 1;
        }
    }

    let mut r = vec![0i64; n];
    let mut indeg: Vec<i64> = (0..n).map(|i| (in_start[i + 1] - in_start[i]) as i64).collect();
    let mut topo = vec![0usize; n];
    let mut count = 0;
    for i in 0..n {
        if indeg[i] == 0 {
            topo[count] = i;
            count += 1;
        }
    }
    let mut q = 0;
    while q < count {
        let v = topo[q];
        for k in out_start[v]..out_start[v + 1] {
            let ei = out_e[k];
            let w = to[ei];
            if r[v] + len[ei] > r[w] {
                r[w] = r[v] + len[ei];
            }
            indeg[w] -= 1;
            if indeg[w] == 0 {
                topo[count] = w;
                count += 1;
            }
        }
        q += 1;
    }

    // Move each loosely attached node toward the side that has more edges.
    for _pass in 0..8 {
        let mut changed = false;
        for q in (0..count).rev() {
            let v = topo[q];
            let ins = in_start[v + 1] - in_start[v];
            let outs = out_start[v + 1] - out_start[v];
            if ins == 0 && outs == 0 {
                continue;
            }
            let mut lo: Option<i64> = None;
            let mut hi: Option<i64> = None;
            for k in in_start[v]..in_start[v + 1] {
                let ei = in_e[k];
                let c = r[from[ei]] + len[ei];
                lo = Some(lo.map_or(c, |l| l.max(c)));
            }
            for k in out_start[v]..out_start[v + 1] {
                let ei = out_e[k];
                let c = r[to[ei]] - len[ei];
                hi = Some(hi.map_or(c, |h| h.min(c)));
            }
            let mut next = r[v];
            if let (true, Some(h)) = (outs > ins, hi) {
                next = h;
            } else if let (true, Some(l)) = (ins > outs, lo) {
                next = l;
            }
            if next != r[v] {
                r[v] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut min: Option<i64> = None;
    let mut max: Option<i64> = None;
    let mut start_ports = false;
    for i in 0..n {
        let kind = nodes[i].kind;
        if kind == Kind::StartPort {
            start_ports = true;
        }
        if kind == Kind::StartPort || kind == Kind::EndPort {
            continue;
        }
        min = Some(min.map_or(r[i], |m| m.min(r[i])));
        max = Some(max.map_or(r[i], |m| m.max(r[i])));
    }
    let mut min = min.unwrap_or(0);
    let max = max.unwrap_or(0);
    for i in 0..n {
        if nodes[i].kind == Kind::StartPort {
            r[i] = min - step;
        } else if nodes[i].kind == Kind::EndPort {
            r[i] = max + step;
        }
    }
    if start_ports {
        min -= step;
    }
    for i in 0..n {
        nodes[i].rank = r[i] - min;
    }
}

// The ends of edges that may move along one side of a node, ordered by their opposite nodes.
#[allow(clippy::too_many_arguments)]
fn side_ends(g: &Graph, v: usize, side: usize, span: f64, kinds: &[u8], ids: &mut Vec<usize>, room: &mut Vec<f64>) -> f64 {
    let start = if side == 0 { &g.pred_start } else { &g.succ_start };
    let list = if side == 0 { &g.pred_e } else { &g.succ_e };
    let other = if side == 0 { &g.ef } else { &g.et };
    for k in start[v]..start[v + 1] {
        if kinds[list[k]] != HELD {
            ids.push(list[k]);
        }
    }
    if ids.len() < 2 {
        return 0.0;
    }
    stable_sort_by(ids, |&a, &b| g.pos[other[a]].cmp(&g.pos[other[b]]).then(a.cmp(&b)));
    room.resize(ids.len(), 0.0);
    let fill = |room: &mut Vec<f64>, share: &dyn Fn(bool, bool) -> f64| -> f64 {
        let mut total = 0.0;
        room[0] = 0.0;
        for k in 1..ids.len() {
            let same = kinds[ids[k]] == kinds[ids[k - 1]];
            room[k] = share(same && kinds[ids[k]] == LEAVES, same);
            total += room[k];
        }
        total
    };
    let mut total = fill(room, &|_, _| 1.0);
    if span / total < MIN_PITCH {
        total = fill(room, &|leaves, _| if leaves { 0.5 } else { 1.0 });
    }
    if span / total < MIN_PITCH {
        total = fill(room, &|_, same| if same { 0.0 } else { 1.0 });
    }
    if total == 0.0 {
        return 0.0;
    }
    let pitch = PORT_PITCH.min(span / total);
    for r in room.iter_mut().skip(1) {
        *r *= pitch;
    }
    total * pitch
}

// Gives the edges that meet one side of a node places of their own along it, around its middle.
fn spread(g: &mut Graph, nodes: &[LNode], efk: &[u8], etk: &[u8]) {
    for v in 0..g.real {
        let span = nodes[v].span.min(g.w[v]);
        if span <= 0.0 || nodes[v].kind != Kind::Node {
            continue;
        }
        for side in 0..2 {
            let mut ids = Vec::new();
            let mut room = Vec::new();
            let total = side_ends(g, v, side, span, if side == 0 { etk } else { efk }, &mut ids, &mut room);
            if total == 0.0 {
                continue;
            }
            let dx = if side == 0 { &mut g.etd } else { &mut g.efd };
            let mut at = -total / 2.0;
            for k in 0..ids.len() {
                at += room[k];
                dx[ids[k]] = at;
            }
        }
    }
}

// The segment of an edge that reaches the next point held in place, past the bends in ranks that hold nothing else.
fn follow(g: &Graph, empty: Option<&[u8]>, mut e: usize, side: usize) -> usize {
    let Some(empty) = empty else { return e };
    for _ in 0..100000 {
        let u = if side == 0 { g.ef[e] } else { g.et[e] };
        if g.kind[u] != Kind::Dummy as u8 || empty[g.rank[u]] == 0 {
            break;
        }
        e = if side == 0 {
            g.pred_e[g.pred_start[u]]
        } else {
            g.succ_e[g.succ_start[u]]
        };
    }
    e
}

// Aligns border ports with the ends on their nodes so edges run straight across the gap.
fn align_ports(g: &Graph, xs: &mut [f64], empty: Option<&[u8]>) {
    for (r, kind) in [(0usize, Kind::StartPort), (g.max_rank, Kind::EndPort)] {
        let s = g.layer_start[r];
        let end = g.layer_start[r + 1];
        let mut to: Vec<f64> = Vec::new();
        for i in s..end {
            let v = g.order[i];
            let start = kind == Kind::StartPort;
            let degree = if start {
                g.succ_start[v + 1] - g.succ_start[v]
            } else {
                g.pred_start[v + 1] - g.pred_start[v]
            };
            if g.kind[v] != kind as u8 || degree != 1 {
                break;
            }
            let e = follow(
                g,
                empty,
                if start {
                    g.succ_e[g.succ_start[v]]
                } else {
                    g.pred_e[g.pred_start[v]]
                },
                if start { 1 } else { 0 },
            );
            let at = if start { xs[g.et[e]] + g.etd[e] } else { xs[g.ef[e]] + g.efd[e] };
            if let Some(&last) = to.last()
                && at < last + TWIN
            {
                break;
            }
            to.push(at);
        }
        if to.is_empty() || to.len() != end - s {
            continue;
        }
        for i in s..end {
            xs[g.order[i]] = to[i - s];
        }
    }
}

fn free_ends(g: &Graph, efk: &[u8], etk: &[u8]) -> [Vec<i64>; 2] {
    let mut free = [vec![0i64; g.n], vec![0i64; g.n]];
    for e in 0..g.ef.len() {
        if etk[e] != HELD {
            free[0][g.et[e]] += 1;
        }
        if efk[e] != HELD {
            free[1][g.ef[e]] += 1;
        }
    }
    free
}

// How far along the flow the handles of a plain S reach, as a share of the way it travels along it.
pub fn ease(across: f64, along: f64) -> f64 {
    let a = across.abs();
    let a = if a == 0.0 || a.is_nan() { 1.0 } else { a };
    0.25f64.max(0.5f64.min((4.0 * along.abs()) / a))
}

// Brings the labels and bends of each edge into line with an end of the edge.
fn align_labels(g: &Graph, nodes: &[LNode], xs: &mut [f64], free: &[Vec<i64>; 2], opt: &LayeredOptions, empty: Option<&[u8]>) {
    let sep = opt.edge_sep;
    let placed = |v: usize| -> bool {
        g.kind[v] == Kind::Label as u8 || (g.kind[v] == Kind::Dummy as u8 && !empty.is_some_and(|e| e[g.rank[v]] == 1))
    };
    let end = |xs: &[f64], e: usize, side: usize| -> (f64, i64) {
        let u = if side == 0 { g.ef[e] } else { g.et[e] };
        let x = if side == 0 { xs[u] + g.efd[e] } else { xs[u] + g.etd[e] };
        (
            x,
            if u < g.real && nodes[u].kind == Kind::Node {
                free[1 - side][u]
            } else {
                0
            },
        )
    };
    let room = |xs: &[f64], v: usize| -> (f64, f64) {
        let s = g.layer_start[g.rank[v]];
        let i = s + g.pos[v];
        let clear = |other: usize| -> f64 {
            g.w[other] / 2.0
                + g.w[v] / 2.0
                + if g.kind[other] == Kind::Node as u8 {
                    opt.node_sep / 2.0 + sep / 2.0
                } else if g.kind[other] == g.kind[v] {
                    sep
                } else {
                    sep / 2.0
                }
        };
        (
            if i > s {
                xs[g.order[i - 1]] + clear(g.order[i - 1])
            } else {
                f64::NEG_INFINITY
            },
            if i + 1 < g.layer_start[g.rank[v] + 1] {
                xs[g.order[i + 1]] - clear(g.order[i + 1])
            } else {
                f64::INFINITY
            },
        )
    };
    let choose = |x: f64, up: (f64, i64), down: (f64, i64), lo: f64, hi: f64| -> f64 {
        let up_first = if up.1 != down.1 {
            up.1 < down.1
        } else {
            (up.0 - x).abs() <= (down.0 - x).abs()
        };
        let choices = if up_first { [up.0, down.0] } else { [down.0, up.0] };
        for c in choices {
            if c >= lo && c <= hi {
                return c;
            }
        }
        lo.max(hi.min(choices[0]))
    };
    let mut done = vec![false; g.n];
    let mut chain: Vec<usize> = Vec::new();
    for r in 1..g.max_rank {
        for i in g.layer_start[r]..g.layer_start[r + 1] {
            if !placed(g.order[i]) || done[g.order[i]] {
                continue;
            }
            let mut top = g.order[i];
            while is_dummy(g.kind[g.ef[g.pred_e[g.pred_start[top]]]]) {
                top = g.ef[g.pred_e[g.pred_start[top]]];
            }
            let mut bottom = top;
            let mut lo = f64::NEG_INFINITY;
            let mut hi = f64::INFINITY;
            chain.clear();
            let mut d = top;
            while is_dummy(g.kind[d]) {
                bottom = d;
                if placed(d) {
                    done[d] = true;
                    chain.push(d);
                    let (least, most) = room(xs, d);
                    if least > lo {
                        lo = least;
                    }
                    if most < hi {
                        hi = most;
                    }
                }
                d = g.et[g.succ_e[g.succ_start[d]]];
            }
            if lo <= hi {
                let x = choose(
                    xs[chain[0]],
                    end(xs, g.pred_e[g.pred_start[top]], 0),
                    end(xs, g.succ_e[g.succ_start[bottom]], 1),
                    lo,
                    hi,
                );
                for &d in &chain {
                    xs[d] = x;
                }
                continue;
            }
            for &d in &chain {
                let (least, most) = room(xs, d);
                if least > most {
                    continue;
                }
                let up = end(xs, follow(g, empty, g.pred_e[g.pred_start[d]], 0), 0);
                let down = end(xs, follow(g, empty, g.succ_e[g.succ_start[d]], 1), 1);
                xs[d] = choose(xs[d], up, down, least, most);
            }
        }
    }
}

struct SideInfo {
    v: usize,
    side: usize,
    span: f64,
    ids: Vec<usize>,
    room: Vec<f64>,
    all: Vec<usize>,
}

// Once the nodes have their places, moves an end along its side when that lets its edge run straight.
fn settle(g: &mut Graph, nodes: &[LNode], xs: &[f64], efk: &[u8], etk: &[u8], free: &[Vec<i64>; 2], empty: Option<&[u8]>) {
    let wanted = |g: &Graph, e: usize, side: usize, v: usize, span: f64, again: bool| -> f64 {
        if free[side][v] < 2 {
            return 0.0;
        }
        let e = follow(g, empty, e, side);
        let u = if side == 0 { g.ef[e] } else { g.et[e] };
        let point = if side == 0 { xs[u] + g.efd[e] } else { xs[u] + g.etd[e] };
        if again && (point - xs[v]).abs() <= span / 2.0 + IN_LINE {
            return (-span / 2.0).max((span / 2.0).min(point - xs[v]));
        }
        let mut middle = point;
        let mut width = 0.0;
        if u < g.real && nodes[u].kind == Kind::Node && (if side == 0 { efk } else { etk })[e] != HELD && free[1 - side][u] >= 2 {
            width = 0f64.max(nodes[u].span.min(g.w[u]));
            if width > 0.0 {
                middle = xs[u];
            }
        }
        let lo = (xs[v] - span / 2.0).max(middle - width / 2.0);
        let hi = (xs[v] + span / 2.0).min(middle + width / 2.0);
        if lo > hi + IN_LINE {
            let lean = 0f64.max(span / 2.0 - LEAN_CLEAR);
            return (-lean).max(lean.min(point - xs[v]));
        }
        let place = (xs[v] * width + middle * span) / (width + span);
        lo.max(hi.min(place)) - xs[v]
    };
    let mut sides: Vec<SideInfo> = Vec::new();
    for v in 0..g.real {
        let span = nodes[v].span.min(g.w[v]);
        if span <= 0.0 || nodes[v].kind != Kind::Node {
            continue;
        }
        for side in 0..2 {
            let mut ids = Vec::new();
            let mut room = Vec::new();
            side_ends(g, v, side, span, if side == 0 { etk } else { efk }, &mut ids, &mut room);
            if ids.is_empty() {
                continue;
            }
            if ids.len() == 1 {
                room.resize(1, 0.0);
                room[0] = 0.0;
            }
            let start = if side == 0 { &g.pred_start } else { &g.succ_start };
            let list = if side == 0 { &g.pred_e } else { &g.succ_e };
            let other = if side == 0 { &g.ef } else { &g.et };
            let mut all: Vec<usize> = (start[v]..start[v + 1]).map(|k| list[k]).collect();
            stable_sort_by(&mut all, |&a, &b| g.pos[other[a]].cmp(&g.pos[other[b]]).then(a.cmp(&b)));
            sides.push(SideInfo {
                v,
                side,
                span,
                ids,
                room,
                all,
            });
        }
    }
    let mut moved = true;
    let mut pass = 0;
    while moved && pass < 6 {
        moved = false;
        for s in &sides {
            let kinds = if s.side == 0 { etk } else { efk };
            let mut k = 0;
            let mut lo = -s.span / 2.0;
            for i in 0..=s.all.len() {
                let held = i < s.all.len() && kinds[s.all[i]] == HELD;
                if i < s.all.len() && !held {
                    continue;
                }
                let mut size = 0;
                let mut j = i as isize - 1;
                while j >= 0 && kinds[s.all[j as usize]] != HELD {
                    size += 1;
                    j -= 1;
                }
                let hi = if held {
                    (if s.side == 0 { &g.etd } else { &g.efd })[s.all[i]] - PORT_PITCH
                } else {
                    s.span / 2.0
                };
                if size > 0 {
                    let mut sum: Vec<f64> = Vec::new();
                    let mut count: Vec<f64> = Vec::new();
                    let mut before = 0.0;
                    for j in 0..size {
                        if j > 0 {
                            before += s.room[k + j];
                        }
                        sum.push(wanted(g, s.ids[k + j], s.side, s.v, s.span, pass > 0) - before);
                        count.push(1.0);
                        let mut together = j > 0 && s.room[k + j] == 0.0;
                        while sum.len() > 1
                            && (together || sum[sum.len() - 2] / count[count.len() - 2] > sum[sum.len() - 1] / count[count.len() - 1])
                        {
                            together = false;
                            let top = sum.pop().unwrap();
                            let pooled = count.pop().unwrap();
                            *sum.last_mut().unwrap() += top;
                            *count.last_mut().unwrap() += pooled;
                        }
                    }
                    let total = before;
                    let mut j = 0;
                    before = 0.0;
                    let dx = if s.side == 0 { &mut g.etd } else { &mut g.efd };
                    for b in 0..sum.len() {
                        let level = if hi - total < lo {
                            (lo + hi - total) / 2.0
                        } else {
                            lo.max((hi - total).min(sum[b] / count[b]))
                        };
                        let mut c = 0.0;
                        while c < count[b] {
                            if j > 0 {
                                before += s.room[k + j];
                            }
                            if (dx[s.ids[k + j]] - level - before).abs() > 0.01 {
                                moved = true;
                            }
                            dx[s.ids[k + j]] = level + before;
                            c += 1.0;
                            j += 1;
                        }
                    }
                    k += size;
                }
                if held {
                    lo = (if s.side == 0 { &g.etd } else { &g.efd })[s.all[i]] + PORT_PITCH;
                }
            }
        }
        pass += 1;
    }
}

fn half_room(g: &Graph, opt: &LayeredOptions, v: usize) -> f64 {
    g.w[v] / 2.0
        + (if g.kind[v] == Kind::Node as u8 {
            opt.node_sep
        } else {
            opt.edge_sep
        }) / 2.0
}

// Brings edges that are nearly straight into line, by moving a node or bend a little across the flow.
fn snap(g: &Graph, xs: &mut [f64], empty: Option<&[u8]>, opt: &LayeredOptions) {
    let passed = |u: usize| empty.is_some_and(|e| g.kind[u] == Kind::Dummy as u8 && e[g.rank[u]] == 1);
    let mut a: Vec<usize> = Vec::new();
    let mut b: Vec<usize> = Vec::new();
    let mut da: Vec<f64> = Vec::new();
    let mut db: Vec<f64> = Vec::new();
    for e in 0..g.ef.len() {
        if passed(g.ef[e]) {
            continue;
        }
        let f = follow(g, empty, e, 1);
        a.push(g.ef[e]);
        b.push(g.et[f]);
        da.push(g.efd[e]);
        db.push(g.etd[f]);
    }
    let off = |xs: &[f64], k: usize| xs[b[k]] + db[k] - xs[a[k]] - da[k];
    let mut block: Vec<usize> = (0..g.n).collect();
    let mut members: Vec<Vec<usize>> = (0..g.n).map(|i| vec![i]).collect();
    let join = |block: &mut Vec<usize>, members: &mut Vec<Vec<usize>>, x: usize, y: usize| {
        let mut keep = block[x];
        let mut drop = block[y];
        if keep == drop {
            return;
        }
        if members[keep].len() < members[drop].len() {
            std::mem::swap(&mut keep, &mut drop);
        }
        let moving = std::mem::take(&mut members[drop]);
        for v in moving {
            block[v] = keep;
            members[keep].push(v);
        }
    };
    let mut near: Vec<usize> = Vec::new();
    for k in 0..a.len() {
        let d = off(xs, k).abs();
        if d < 0.01 {
            join(&mut block, &mut members, a[k], b[k]);
        } else if d <= BEND_SNAP {
            near.push(k);
        }
    }
    if near.is_empty() {
        return;
    }
    {
        let xs_ro: &[f64] = xs;
        stable_sort_by(&mut near, |&x, &y| ord(off(xs_ro, x).abs() - off(xs_ro, y).abs()).then(x.cmp(&y)));
    }
    for k in near {
        let d = off(xs, k);
        if d.abs() < 0.01 {
            join(&mut block, &mut members, a[k], b[k]);
            continue;
        }
        if d.abs() > BEND_SNAP || block[a[k]] == block[b[k]] {
            continue;
        }
        let upper = block[a[k]];
        let lower = block[b[k]];
        let first = if members[upper].len() <= members[lower].len() {
            upper
        } else {
            lower
        };
        for id in [first, if first == upper { lower } else { upper }] {
            let shift = if id == upper { d } else { -d };
            if d.abs() > SNAP && !members[id].iter().all(|&v| g.kind[v] == Kind::Dummy as u8) {
                continue;
            }
            let free = members[id].iter().all(|&v| {
                let s = g.layer_start[g.rank[v]];
                let end = g.layer_start[g.rank[v] + 1];
                let i = s + g.pos[v];
                if i > s
                    && block[g.order[i - 1]] != id
                    && xs[v] + shift - xs[g.order[i - 1]] < half_room(g, opt, v) + half_room(g, opt, g.order[i - 1]) - 0.01
                {
                    return false;
                }
                if i + 1 < end
                    && block[g.order[i + 1]] != id
                    && xs[g.order[i + 1]] - xs[v] - shift < half_room(g, opt, v) + half_room(g, opt, g.order[i + 1]) - 0.01
                {
                    return false;
                }
                true
            });
            if !free {
                continue;
            }
            for &v in &members[id] {
                xs[v] += shift;
            }
            join(&mut block, &mut members, a[k], b[k]);
            break;
        }
    }
}

// Marks the ranks that hold nothing but the bends of edges, if there are any.
fn empty_ranks(g: &Graph) -> Option<Vec<u8>> {
    let mut empty = vec![0u8; g.max_rank + 1];
    let mut any = false;
    for r in 1..g.max_rank {
        let mut only = g.layer_start[r + 1] > g.layer_start[r];
        let mut i = g.layer_start[r];
        while only && i < g.layer_start[r + 1] {
            if g.kind[g.order[i]] != Kind::Dummy as u8 {
                only = false;
            }
            i += 1;
        }
        if only {
            empty[r] = 1;
            any = true;
        }
    }
    if any { Some(empty) } else { None }
}

// Gives every piece that does not run straight down a track across its gap, and counts the tracks of each gap.
fn assign_tracks(pieces: &mut Pieces, track_count: &mut [i64], curved: bool, room: &[f64], tall: &mut [f64]) {
    let total = pieces.from.len();
    let mut by_rank: IndexMap<usize, Vec<usize>> = IndexMap::new();
    for k in 0..total {
        pieces.track.push(-1);
        pieces.levels.push(0);
        if (pieces.from[k] - pieces.to[k]).abs() < IN_LINE {
            continue;
        }
        by_rank.entry(pieces.rank[k]).or_default().push(k);
    }
    if by_rank.is_empty() {
        return;
    }
    let from = pieces.from.clone();
    let to = pieces.to.clone();
    let wide = pieces.wide.clone();
    let lo_of: Vec<f64> = (0..total).map(|k| from[k].min(to[k])).collect();
    let hi_of: Vec<f64> = (0..total).map(|k| from[k].max(to[k])).collect();
    let within = |x: f64, k: usize| -> i32 { (x >= lo_of[k] - IN_LINE && x <= hi_of[k] + IN_LINE) as i32 };
    let mates = |a: usize, b: usize| (from[a] - from[b]).abs() < IN_LINE || (to[a] - to[b]).abs() < IN_LINE;
    let overlap = |a: usize, b: usize| lo_of[a] < hi_of[b] - IN_LINE && lo_of[b] < hi_of[a] - IN_LINE;
    let clash = |a: usize, b: usize| overlap(a, b) && !mates(a, b);
    let meet = |a: usize, b: usize| overlap(a, b) || within(to[a], b) + within(from[b], a) + within(to[b], a) + within(from[a], b) > 0;
    let crossings = |a: usize, b: usize| within(to[a], b) + within(from[b], a);
    let near_pairs = |list: &[usize]| -> Vec<usize> {
        let mut by_lo: Vec<usize> = (0..list.len()).collect();
        stable_sort_by(&mut by_lo, |&a, &b| ord(lo_of[list[a]] - lo_of[list[b]]));
        let mut pairs = Vec::new();
        for x in 0..by_lo.len() {
            let i = by_lo[x];
            let reach = hi_of[list[i]] + 4.0 * IN_LINE;
            let mut y = x + 1;
            while y < by_lo.len() && lo_of[list[by_lo[y]]] <= reach {
                let j = by_lo[y];
                pairs.push(i.min(j));
                pairs.push(i.max(j));
                y += 1;
            }
        }
        pairs
    };

    let lay = |pieces: &mut Pieces, list: &mut Vec<usize>| -> i64 {
        stable_sort_by(list, |&a, &b| {
            ord(hi_of[b] - lo_of[b] - (hi_of[a] - lo_of[a]))
                .then(ord(lo_of[a] - lo_of[b]))
                .then(a.cmp(&b))
        });
        let count = list.len();
        let mut above: Vec<Vec<usize>> = Vec::new();
        let mut group: Vec<usize> = (0..count).collect();
        fn find(group: &mut [usize], mut i: usize) -> usize {
            while group[i] != i {
                group[i] = group[group[i]];
                i = group[i];
            }
            i
        }
        let weighed = count <= MAX_WEIGHED;
        let pairs = if weighed { near_pairs(list) } else { Vec::new() };
        let mut clashes: Vec<Vec<usize>> = if weighed { vec![Vec::new(); count] } else { Vec::new() };
        let mut p = 0;
        while p < pairs.len() {
            let (i, j) = (pairs[p], pairs[p + 1]);
            p += 2;
            if !clash(list[i], list[j]) {
                continue;
            }
            clashes[i].push(j);
            clashes[j].push(i);
        }
        let mut sequence: Vec<usize> = (0..count).collect();
        if weighed {
            let mut waiting = vec![0i64; count];
            let mut after: Vec<Vec<usize>> = vec![Vec::new(); count];
            above = vec![Vec::new(); count];
            let mut p = 0;
            while p < pairs.len() {
                let (i, j) = (pairs[p], pairs[p + 1]);
                p += 2;
                let a = list[i];
                let b = list[j];
                if !meet(a, b) {
                    continue;
                }
                let fi = find(&mut group, i);
                let fj = find(&mut group, j);
                group[fi] = fj;
                let ab = crossings(a, b);
                let ba = crossings(b, a);
                if ab == ba {
                    continue;
                }
                let first = if ab < ba { i } else { j };
                let second = if ab < ba { j } else { i };
                above[second].push(first);
                after[first].push(second);
                waiting[second] += 1;
            }
            let mut placed = vec![false; count];
            sequence = Vec::with_capacity(count);
            for _ in 0..count {
                let mut pick: isize = -1;
                for i in 0..count {
                    if placed[i] {
                        continue;
                    }
                    if pick < 0 || waiting[i] < waiting[pick as usize] {
                        pick = i as isize;
                    }
                    if waiting[i] == 0 {
                        break;
                    }
                }
                let pick = pick as usize;
                placed[pick] = true;
                sequence.push(pick);
                for &j in &after[pick] {
                    waiting[j] -= 1;
                }
            }
        }
        let mut taken: HashSet<i64> = HashSet::new();
        let mut on_track: Vec<Vec<usize>> = Vec::new();
        let mut rows: i64 = 0;
        for &at in &sequence {
            let k = list[at];
            let mut t: i64 = 0;
            if let Some(ab) = above.get(at) {
                for &i in ab {
                    if pieces.track[list[i]] >= t {
                        t = pieces.track[list[i]] + 1;
                    }
                }
            }
            if weighed {
                taken.clear();
                for &i in &clashes[at] {
                    taken.insert(pieces.track[list[i]]);
                }
                while taken.contains(&t) {
                    t += 1;
                }
            } else {
                while on_track.get(t as usize).is_some_and(|row| row.iter().any(|&other| clash(k, other))) {
                    t += 1;
                }
                while on_track.len() <= t as usize {
                    on_track.push(Vec::new());
                }
                on_track[t as usize].push(k);
            }
            pieces.track[k] = t;
            if t >= rows {
                rows = t + 1;
            }
        }
        let mut first = vec![rows; count];
        let mut last = vec![-1i64; count];
        for i in 0..count {
            let root = find(&mut group, i);
            first[root] = first[root].min(pieces.track[list[i]]);
            last[root] = last[root].max(pieces.track[list[i]]);
        }
        for i in 0..count {
            let root = find(&mut group, i);
            pieces.levels[list[i]] = last[root] - first[root] + 1;
            if curved {
                pieces.track[list[i]] -= first[root];
            }
        }
        rows
    };

    let sample = |k: usize, h: f64, out: &mut [f64]| {
        let c = ease(to[k] - from[k], h);
        for i in 0..=SAMPLES {
            let t = i as f64 / SAMPLES as f64;
            let u = 1.0 - t;
            out[2 * i] = from[k] + (to[k] - from[k]) * t * t * (3.0 - 2.0 * t);
            out[2 * i + 1] = h * (3.0 * c * t * u * u + 3.0 * (1.0 - c) * t * t * u + t * t * t);
        }
    };
    let reaches = |points: &[f64], line: &[f64], limit: f64| -> bool {
        let bound = limit * limit * (1.0 + 1e-9);
        for i in 0..=SAMPLES {
            let px = points[2 * i];
            let py = points[2 * i + 1];
            for j in 0..SAMPLES {
                let dx = line[2 * j + 2] - line[2 * j];
                let dy = line[2 * j + 3] - line[2 * j + 1];
                let mut den = dx * dx + dy * dy;
                if den == 0.0 || den.is_nan() {
                    den = 1.0;
                }
                let t = 0f64.max(1f64.min(((px - line[2 * j]) * dx + (py - line[2 * j + 1]) * dy) / den));
                let ex = px - line[2 * j] - t * dx;
                let ey = py - line[2 * j + 1] - t * dy;
                if ex * ex + ey * ey < bound && ex.hypot(ey) < limit {
                    return true;
                }
            }
        }
        false
    };
    let mut line_a = vec![0.0f64; 2 * SAMPLES + 2];
    let mut line_b = vec![0.0f64; 2 * SAMPLES + 2];

    let ranks: Vec<usize> = by_rank.keys().copied().collect();
    for r in ranks {
        let mut all = by_rank.get(&r).unwrap().clone();
        if !curved {
            track_count[r] = lay(pieces, &mut all);
            continue;
        }
        if all.len() > MAX_WEIGHED {
            continue;
        }
        let mut far: Vec<usize> = all.iter().copied().filter(|&k| wide[k] == 1).collect();
        lay(pieces, &mut far);
        let mut level: HashSet<usize> = far.iter().copied().filter(|&k| pieces.levels[k] > 1).collect();
        let pairs = near_pairs(&all);
        let mut p = 0;
        while p < pairs.len() {
            let a = all[pairs[p]];
            let b = all[pairs[p + 1]];
            p += 2;
            if (level.contains(&a) && level.contains(&b)) || !clash(a, b) || (from[a] < to[a]) != (from[b] < to[b]) {
                continue;
            }
            let height = room[r];
            let hh = (height / 2.0).max(height - 2.0 * LEVEL_LEAD);
            sample(a, hh, &mut line_a);
            sample(b, hh, &mut line_b);
            let limit = if wide[a] == 1 || wide[b] == 1 { LEVEL_CLEAR } else { LEVEL_TOUCH };
            if reaches(&line_a, &line_b, limit) || reaches(&line_b, &line_a, limit) {
                level.insert(a);
                level.insert(b);
            }
        }
        for &k in &far {
            pieces.track[k] = -1;
            pieces.levels[k] = 0;
        }
        if level.is_empty() {
            continue;
        }
        let mut crossing = 0;
        let mut alongside = 0;
        let mut beside: HashSet<usize> = HashSet::new();
        let mut p = 0;
        while p < pairs.len() {
            let a = all[pairs[p]];
            let b = all[pairs[p + 1]];
            p += 2;
            if level.contains(&a) && level.contains(&b) {
                if !overlap(a, b) {
                    continue;
                }
                if (from[a] < to[a]) != (from[b] < to[b]) {
                    crossing += 1;
                } else if !mates(a, b) {
                    alongside += 1;
                }
            } else if (level.contains(&a) || level.contains(&b)) && meet(a, b) {
                beside.insert(a);
                beside.insert(b);
            }
        }
        if crossing > alongside {
            for &k in &all {
                if (hi_of[k] - lo_of[k]) * CROSSED_SLOPE > tall[r] {
                    tall[r] = (hi_of[k] - lo_of[k]) * CROSSED_SLOPE;
                }
            }
            continue;
        }
        let mut members: Vec<usize> = all
            .iter()
            .copied()
            .filter(|&k| level.contains(&k) || (wide[k] != 1 && beside.contains(&k)))
            .collect();
        lay(pieces, &mut members);
        let mut most = 0;
        for &k in &members {
            if pieces.levels[k] > most {
                most = pieces.levels[k];
            }
        }
        track_count[r] = most;
        let _ = &mut all;
    }
}

// Moves the bends in ranks that hold only bends onto the straight line between the nearest held points.
fn straighten(g: &Graph, xs: &mut [f64], empty: &[u8], first_dummy: &[i64], span: &[i64], sep: f64, on_line: &mut [u8]) {
    let len = xs.len();
    let mut want = vec![0.0f64; len];
    let mut from = vec![0.0f64; len];
    let mut to = vec![0.0f64; len];
    for ei in 0..first_dummy.len() {
        let d0 = first_dummy[ei];
        if d0 < 0 {
            continue;
        }
        let d0 = d0 as usize;
        let end = d0 + span[ei] as usize - 1;
        let base = g.rank[d0];
        let mut d = d0;
        while d < end {
            if empty[base + d - d0] == 0 {
                d += 1;
                continue;
            }
            let mut last = d;
            while last + 1 < end && empty[base + last + 1 - d0] == 1 {
                last += 1;
            }
            let above = g.pred_e[g.pred_start[d]];
            let below = g.succ_e[g.succ_start[last]];
            let x0 = xs[g.ef[above]] + g.efd[above];
            let x1 = xs[g.et[below]] + g.etd[below];
            let steps = (last - d + 2) as f64;
            for k in d..=last {
                want[k] = x0 + ((x1 - x0) * (k - d + 1) as f64) / steps;
                from[k] = x0;
                to[k] = x1;
            }
            d = last + 1;
        }
    }
    let mut twins: Vec<usize> = Vec::new();
    for r in 1..g.max_rank {
        if empty[r] == 0 {
            continue;
        }
        let s = g.layer_start[r];
        let end = g.layer_start[r + 1];
        let mut row: Vec<usize> = g.order[s..end].to_vec();
        stable_sort_by(&mut row, |&a, &b| ord(from[a] - from[b]).then(ord(to[a] - to[b])).then(a.cmp(&b)));
        let mut i = 0;
        while i < row.len() {
            twins.clear();
            let mut j = i;
            while j < row.len() && (from[row[j]] - from[row[i]]).abs() < TWIN && (to[row[j]] - to[row[i]]).abs() < TWIN {
                twins.push(row[j]);
                j += 1;
            }
            stable_sort_by(&mut twins, |&a, &b| g.pos[a].cmp(&g.pos[b]));
            let count = twins.len();
            for (k, &d) in twins.iter().enumerate() {
                xs[d] = want[d] + (k as f64 - (count as f64 - 1.0) / 2.0) * sep;
                if count == 1 || sep == 0.0 {
                    on_line[d] = 1;
                }
            }
            i = j;
        }
    }
}

// In a rank that holds labels and bends but no nodes, a bend that would pass clear of every label is left out.
fn past_labels(g: &Graph, xs: &mut [f64], first_dummy: &[i64], span: &[i64], clear: f64, on_line: &mut [u8]) {
    let mut labelled = vec![0u8; g.max_rank + 1];
    let mut any = false;
    for r in 1..g.max_rank {
        let mut labels = false;
        let mut others = false;
        for i in g.layer_start[r]..g.layer_start[r + 1] {
            let k = g.kind[g.order[i]];
            if k == Kind::Label as u8 {
                labels = true;
            } else if k != Kind::Dummy as u8 {
                others = true;
            }
        }
        if labels && !others {
            labelled[r] = 1;
            any = true;
        }
    }
    if !any {
        return;
    }
    for ei in 0..first_dummy.len() {
        let d0 = first_dummy[ei];
        if d0 < 0 {
            continue;
        }
        let d0 = d0 as usize;
        let end = d0 + span[ei] as usize - 1;
        for d in d0..end {
            if g.kind[d] != Kind::Dummy as u8 || labelled[g.rank[d]] == 0 {
                continue;
            }
            let mut first = d;
            let mut last = d;
            while first > d0 && on_line[first - 1] == 1 {
                first -= 1;
            }
            while last + 1 < end && on_line[last + 1] == 1 {
                last += 1;
            }
            let above = g.pred_e[g.pred_start[first]];
            let below = g.succ_e[g.succ_start[last]];
            let from = xs[g.ef[above]] + g.efd[above];
            let to = xs[g.et[below]] + g.etd[below];
            let lo = from.min(to) - clear;
            let hi = from.max(to) + clear;
            let r = g.rank[d];
            let mut free = true;
            let mut i = g.layer_start[r];
            while free && i < g.layer_start[r + 1] {
                let label = g.order[i];
                if g.kind[label] == Kind::Label as u8 && xs[label] + g.w[label] / 2.0 > lo && xs[label] - g.w[label] / 2.0 < hi {
                    free = false;
                }
                i += 1;
            }
            if !free {
                continue;
            }
            on_line[d] = 1;
            let steps = (last - first + 2) as f64;
            for k in first..=last {
                xs[k] = from + ((to - from) * (k - first + 1) as f64) / steps;
            }
        }
    }
}

// Returns [final crossings, crossings before grouping or reordering].
fn reduce_crossings(g: &mut Graph, nodes: &[LNode]) -> (i64, i64) {
    let n = g.n;
    let real = g.real;
    let max_rank = g.max_rank;
    let mut widest = 0;
    for r in 0..=max_rank {
        widest = widest.max(g.layer_start[r + 1] - g.layer_start[r]);
    }
    if widest < 2 {
        return (0, 0);
    }
    let segs = g.ef.len();
    let offsets = g.offsets;
    let mut place: Option<Vec<usize>> = if offsets { Some(vec![0; segs]) } else { None };
    let mut place_in = vec![0usize; if offsets { segs } else { 0 }];
    let mut places = vec![0usize; if offsets { n } else { 0 }];
    let mut leaving = g.succ_e.clone();
    let mut mixed = vec![false; if offsets { n } else { 0 }];
    if offsets {
        let mut arriving = g.pred_e.clone();
        let by_offset = |list: &mut [usize], from: usize, to: usize, dx: &[f64]| -> bool {
            for k in from + 1..to {
                if dx[list[k]] == dx[list[from]] {
                    continue;
                }
                stable_sort_by(&mut list[from..to], |&a, &b| ord(dx[a] - dx[b]));
                return true;
            }
            false
        };
        for v in 0..n {
            places[v] = 1;
            if by_offset(&mut arriving, g.pred_start[v], g.pred_start[v + 1], &g.etd) {
                for k in g.pred_start[v] + 1..g.pred_start[v + 1] {
                    if g.etd[arriving[k]] != g.etd[arriving[k - 1]] {
                        places[v] += 1;
                    }
                    place_in[arriving[k]] = places[v] - 1;
                }
            }
            if by_offset(&mut leaving, g.succ_start[v], g.succ_start[v + 1], &g.efd) {
                mixed[v] = true;
            }
        }
    }
    let mut size = 1;
    while size < (if offsets { widest + segs } else { widest }) {
        size <<= 1;
    }
    let mut tree = vec![0i64; 2 * size];
    let mut targets = vec![0usize; segs];

    let count = |g: &Graph, place: &mut Option<Vec<usize>>, tree: &mut Vec<i64>, targets: &mut Vec<usize>| -> i64 {
        let mut total = 0i64;
        for r in 0..max_rank {
            let mut south_len = g.layer_start[r + 2] - g.layer_start[r + 1];
            if let Some(place) = place.as_mut() {
                let mut next = 0;
                for i in g.layer_start[r + 1]..g.layer_start[r + 2] {
                    let v = g.order[i];
                    for k in g.pred_start[v]..g.pred_start[v + 1] {
                        place[g.pred_e[k]] = next + place_in[g.pred_e[k]];
                    }
                    next += places[v];
                }
                south_len = next;
            }
            if south_len < 2 {
                continue;
            }
            let mut t = 0;
            for i in g.layer_start[r]..g.layer_start[r + 1] {
                let v = g.order[i];
                let from = g.succ_start[v];
                let to = g.succ_start[v + 1];
                let t0 = t;
                for k in from..to {
                    targets[t] = match place.as_ref() {
                        Some(p) => p[leaving[k]],
                        None => g.pos[g.et[g.succ_e[k]]],
                    };
                    t += 1;
                }
                let d = t - t0;
                if d < 2 {
                    continue;
                }
                if place.is_some() && mixed[v] {
                    for a in t0 + 1..t {
                        let item = targets[a];
                        let dx = g.efd[leaving[from + a - t0]];
                        let mut b = a as isize - 1;
                        while b >= t0 as isize && g.efd[leaving[from + b as usize - t0]] == dx && targets[b as usize] > item {
                            targets[(b + 1) as usize] = targets[b as usize];
                            b -= 1;
                        }
                        targets[(b + 1) as usize] = item;
                    }
                } else {
                    targets[t0..t].sort_unstable();
                }
            }
            if t < 2 {
                continue;
            }
            let mut leaves = 1;
            while leaves < south_len {
                leaves <<= 1;
            }
            for x in tree.iter_mut().take(2 * leaves) {
                *x = 0;
            }
            for &target in targets.iter().take(t) {
                let mut i = target + leaves;
                tree[i] += 1;
                while i > 1 {
                    if i & 1 == 0 {
                        total += tree[i + 1];
                    }
                    i >>= 1;
                    tree[i] += 1;
                }
            }
        }
        total
    };

    let declared = count(g, &mut place, &mut tree, &mut targets);
    let mut bary = vec![0.0f64; n];

    // Keep groups contiguous, ordering each by its mean key.
    let group_parent = g.group_parent.clone();
    let groups = group_parent.as_ref().map_or(0, |p| p.len());
    let mut depth = vec![0i64; groups];
    if let Some(gp) = &group_parent {
        for gr in 0..groups {
            let mut p = gr as i64;
            while p >= 0 {
                depth[gr] += 1;
                p = gp[p as usize];
            }
        }
    }
    let arrange = |g: &mut Graph, r: usize, key: &[f64]| {
        let grp = g.grp.as_ref().unwrap();
        let gp = group_parent.as_ref().unwrap();
        let s = g.layer_start[r];
        let e = g.layer_start[r + 1];
        let mut group_sum = vec![0.0f64; groups];
        let mut group_count = vec![0i64; groups];
        let mut group_first = vec![0usize; groups];
        for i in s..e {
            let v = g.order[i];
            let mut gr = grp[v];
            while gr >= 0 {
                if group_count[gr as usize] == 0 {
                    group_first[gr as usize] = i;
                }
                group_sum[gr as usize] += key[v];
                group_count[gr as usize] += 1;
                gr = gp[gr as usize];
            }
        }
        let value = |item: usize| {
            if item >= n {
                group_sum[item - n] / group_count[item - n] as f64
            } else {
                key[item]
            }
        };
        let first = |item: usize| if item >= n { group_first[item - n] } else { s + g.pos[item] };
        let compare = |&a: &usize, &b: &usize| {
            let mut ia = a;
            let mut ib = b;
            let mut ca = grp[a];
            let mut cb = grp[b];
            let mut da = if ca < 0 { 0 } else { depth[ca as usize] };
            let mut db = if cb < 0 { 0 } else { depth[cb as usize] };
            while da > db {
                ia = n + ca as usize;
                ca = gp[ca as usize];
                da -= 1;
            }
            while db > da {
                ib = n + cb as usize;
                cb = gp[cb as usize];
                db -= 1;
            }
            while ca != cb {
                ia = n + ca as usize;
                ca = gp[ca as usize];
                ib = n + cb as usize;
                cb = gp[cb as usize];
            }
            ord(value(ia) - value(ib)).then(ord(first(ia) as f64 - first(ib) as f64))
        };
        let mut row: Vec<usize> = g.order[s..e].to_vec();
        stable_sort_by(&mut row, compare);
        g.order[s..e].copy_from_slice(&row);
        for i in s..e {
            g.pos[g.order[i]] = i - s;
        }
    };
    if g.grp.is_some() {
        let mut at = vec![0.0f64; n];
        for r in 0..=max_rank {
            for i in g.layer_start[r]..g.layer_start[r + 1] {
                at[g.order[i]] = (i - g.layer_start[r]) as f64;
            }
            arrange(g, r, &at);
        }
    }
    let mut best = if g.grp.is_some() {
        count(g, &mut place, &mut tree, &mut targets)
    } else {
        declared
    };
    if best == 0 {
        return (0, declared);
    }
    let mut best_order = g.order.clone();
    let start = best.min(declared);

    let pin_of = |v: usize| if v < real { nodes[v].pin } else { 0.0 };
    let sweep = |g: &mut Graph, bary: &mut Vec<f64>, down: bool, bias_right: bool| {
        for step in 1..=max_rank {
            let r = if down { step } else { max_rank - step };
            if g.fixed[r] == 1 {
                continue;
            }
            let s = g.layer_start[r];
            let e = g.layer_start[r + 1];
            let mut moving: Vec<usize> = Vec::new();
            for i in s..e {
                let v = g.order[i];
                let pin = pin_of(v);
                if pin != 0.0 {
                    bary[v] = pin * 1e9;
                    moving.push(v);
                    continue;
                }
                let (start, list, other, other_dx) = if down {
                    (&g.pred_start, &g.pred_e, &g.ef, &g.efd)
                } else {
                    (&g.succ_start, &g.succ_e, &g.et, &g.etd)
                };
                let from = start[v];
                let to = start[v + 1];
                if from == to {
                    if g.grp.is_some() {
                        let across = if down {
                            g.layer_start[r] - g.layer_start[r - 1]
                        } else {
                            g.layer_start[r + 2] - g.layer_start[r + 1]
                        };
                        bary[v] = ((i - s) as f64 * across as f64) / (e - s) as f64;
                        continue;
                    }
                    bary[v] = -1.0;
                    continue;
                }
                let mut sum = 0.0;
                for k in from..to {
                    let u = other[list[k]];
                    sum += g.pos[u] as f64;
                    if g.offsets && g.w[u] > 0.0 {
                        sum += other_dx[list[k]] / g.w[u];
                    }
                }
                bary[v] = sum / (to - from) as f64;
                moving.push(v);
            }
            if g.grp.is_some() {
                arrange(g, r, bary);
                continue;
            }
            if moving.len() < 2 {
                continue;
            }
            {
                let pos = &g.pos;
                let bary: &Vec<f64> = bary;
                stable_sort_by(&mut moving, |&a, &b| {
                    let before = |a: usize, b: usize| {
                        bary[a] < bary[b] || (bary[a] == bary[b] && if bias_right { pos[a] > pos[b] } else { pos[a] < pos[b] })
                    };
                    if before(a, b) {
                        std::cmp::Ordering::Less
                    } else if before(b, a) {
                        std::cmp::Ordering::Greater
                    } else {
                        std::cmp::Ordering::Equal
                    }
                });
            }
            let mut k = 0;
            for i in s..e {
                if bary[g.order[i]] != -1.0 {
                    g.order[i] = moving[k];
                    k += 1;
                }
            }
            for i in s..e {
                g.pos[g.order[i]] = i - s;
            }
        }
    };

    let out_of_order = |g: &Graph| -> i64 {
        let mut total = 0;
        for r in 0..=max_rank {
            for i in g.layer_start[r]..g.layer_start[r + 1] {
                if g.order[i] >= real {
                    continue;
                }
                for j in i + 1..g.layer_start[r + 1] {
                    if g.order[j] < g.order[i] {
                        total += 1;
                    }
                }
            }
        }
        total
    };

    let at = |g: &Graph, e: usize, up: bool| -> f64 {
        let u = if up { g.ef[e] } else { g.et[e] };
        if g.offsets && g.w[u] > 0.0 {
            g.pos[u] as f64 + (if up { g.efd[e] } else { g.etd[e] }) / g.w[u]
        } else {
            g.pos[u] as f64
        }
    };
    let between = |g: &Graph, a: usize, b: usize| -> i64 {
        let mut c = 0;
        for k in g.pred_start[a]..g.pred_start[a + 1] {
            let x = at(g, g.pred_e[k], true);
            for j in g.pred_start[b]..g.pred_start[b + 1] {
                if x > at(g, g.pred_e[j], true) {
                    c += 1;
                }
            }
        }
        for k in g.succ_start[a]..g.succ_start[a + 1] {
            let x = at(g, g.succ_e[k], false);
            for j in g.succ_start[b]..g.succ_start[b + 1] {
                if x > at(g, g.succ_e[j], false) {
                    c += 1;
                }
            }
        }
        c
    };
    let transpose = |g: &mut Graph| {
        for _pass in 0..8 {
            let mut improved = false;
            for r in 0..=max_rank {
                if g.fixed[r] == 1 {
                    continue;
                }
                let s = g.layer_start[r];
                let mut i = s;
                while i + 1 < g.layer_start[r + 1] {
                    let a = g.order[i];
                    let b = g.order[i + 1];
                    let same_group = g.grp.as_ref().is_none_or(|grp| grp[a] == grp[b]);
                    if pin_of(a) != pin_of(b) || !same_group || between(g, b, a) >= between(g, a, b) {
                        i += 1;
                        continue;
                    }
                    g.order[i] = b;
                    g.order[i + 1] = a;
                    g.pos[b] = i - s;
                    g.pos[a] = i + 1 - s;
                    improved = true;
                    i += 1;
                }
            }
            if !improved {
                break;
            }
        }
    };

    let initial = g.order.clone();
    let small = n <= 600;
    let mut chosen = best_order.clone();
    let mut chosen_out = i64::MAX;
    for attempt in 0..(if small { 2 } else { 1 }) {
        if attempt > 0 {
            g.order.copy_from_slice(&initial);
            g.set_positions();
        }
        let mut found = start;
        let mut found_order = initial.clone();
        let mut stale = 0;
        let mut i = 0;
        while stale < 4 && i < MAX_SWEEPS {
            sweep(g, &mut bary, (i + attempt) % 2 == 0, i % 4 >= 2);
            let c = count(g, &mut place, &mut tree, &mut targets);
            if c < found {
                found = c;
                found_order.copy_from_slice(&g.order);
                stale = 0;
                if c == 0 {
                    break;
                }
            }
            i += 1;
            stale += 1;
        }
        g.order.copy_from_slice(&found_order);
        if small && found > 0 {
            g.set_positions();
            transpose(g);
            let c = count(g, &mut place, &mut tree, &mut targets);
            if c < found {
                found = c;
                found_order.copy_from_slice(&g.order);
            }
        }
        let out = if small { out_of_order(g) } else { 0 };
        if attempt == 0 || found < best || (found == best && out < chosen_out) {
            best = found;
            chosen = found_order;
            chosen_out = out;
        }
    }
    g.order.copy_from_slice(&chosen);
    g.set_positions();
    best_order.copy_from_slice(&chosen);
    if g.grp.is_some() {
        best = count(g, &mut place, &mut tree, &mut targets);
    }

    // A drawing and its mirror image cross as often too. Of those two, likewise.
    if !small {
        return (best, declared);
    }
    if nodes.iter().any(|node| node.pin != 0.0) {
        return (best, declared);
    }
    if chosen_out == 0 || g.fixed[0] == 1 || g.fixed[max_rank] == 1 {
        return (best, declared);
    }
    for r in 0..=max_rank {
        let (s, e) = (g.layer_start[r], g.layer_start[r + 1]);
        g.order[s..e].reverse();
    }
    g.set_positions();
    if count(g, &mut place, &mut tree, &mut targets) > best || out_of_order(g) >= chosen_out {
        g.order.copy_from_slice(&best_order);
    }
    (best, declared)
}

// Brandes-Köpf horizontal coordinate assignment: four aligned layouts, balanced.
fn position(g: &mut Graph, opt: &LayeredOptions) -> Vec<f64> {
    let n = g.n;
    let max_rank = g.max_rank;
    let segments = g.ef.len();
    let gap: Vec<f64> = (0..n).map(|i| half_room(g, opt, i)).collect();

    let mut key = vec![0i64; segments];
    for e in 0..segments {
        key[e] = g.pos[g.et[e]] as i64;
    }
    for v in 0..n {
        if g.succ_start[v + 1] - g.succ_start[v] > 1 {
            let (a, b) = (g.succ_start[v], g.succ_start[v + 1]);
            sort_range(&mut g.succ_e, a, b, &key);
        }
    }
    for e in 0..segments {
        key[e] = g.pos[g.ef[e]] as i64;
    }
    for v in 0..n {
        if g.pred_start[v + 1] - g.pred_start[v] > 1 {
            let (a, b) = (g.pred_start[v], g.pred_start[v + 1]);
            sort_range(&mut g.pred_e, a, b, &key);
        }
    }

    // An edge between two real nodes may not cross a dummy-to-dummy segment; mark those that would.
    let mut conflict = vec![false; segments];
    for r in 1..=max_rank {
        let prev_len = g.layer_start[r] - g.layer_start[r - 1];
        let s = g.layer_start[r];
        let end = g.layer_start[r + 1];
        let mut k0 = 0usize;
        let mut scan = s;
        for i in s..end {
            let v = g.order[i];
            let mut inner: isize = -1;
            if is_dummy(g.kind[v]) {
                let u = g.ef[g.pred_e[g.pred_start[v]]];
                if is_dummy(g.kind[u]) {
                    inner = u as isize;
                }
            }
            let k1 = if inner >= 0 { g.pos[inner as usize] } else { prev_len };
            if inner >= 0 || i == end - 1 {
                for j in scan..=i {
                    let wv = g.order[j];
                    let w_dummy = is_dummy(g.kind[wv]);
                    for k in g.pred_start[wv]..g.pred_start[wv + 1] {
                        let e = g.pred_e[k];
                        let up = g.pos[g.ef[e]];
                        if (up < k0 || k1 < up) && !(w_dummy && is_dummy(g.kind[g.ef[e]])) {
                            conflict[e] = true;
                        }
                    }
                }
                scan = i + 1;
                k0 = k1;
            }
        }
    }

    let mut results: Vec<Vec<f64>> = Vec::new();
    let mut root = vec![0usize; n];
    let mut align = vec![0usize; n];
    let mut shift = vec![0.0f64; n];
    let mut e_from = vec![0usize; n];
    let mut e_to = vec![0usize; n];
    let mut e_w = vec![0.0f64; n];
    let mut indeg = vec![0i64; n];
    let mut out_start = vec![0usize; n + 1];
    let mut out_e = vec![0usize; n];
    let mut fill = vec![0usize; n];
    let mut topo = vec![0usize; n];
    let mut x = vec![0.0f64; n];

    for variant in 0..4 {
        let up = variant < 2;
        let right = variant & 1 == 1;
        let (start, list, other, own_dx, other_dx) = if up {
            (&g.pred_start, &g.pred_e, &g.ef, &g.etd, &g.efd)
        } else {
            (&g.succ_start, &g.succ_e, &g.et, &g.efd, &g.etd)
        };
        for i in 0..n {
            root[i] = i;
            align[i] = i;
            shift[i] = 0.0;
            indeg[i] = 0;
            out_start[i + 1] = 0;
            x[i] = 0.0;
        }
        out_start[0] = 0;

        for step in 0..=max_rank {
            let r = if up { step } else { max_rank - step };
            let s = g.layer_start[r];
            let end = g.layer_start[r + 1];
            let adjacent: isize = if up { r as isize - 1 } else { r as isize + 1 };
            let adjacent_last: isize = if adjacent >= 0 && adjacent <= max_rank as isize {
                (g.layer_start[adjacent as usize + 1] - g.layer_start[adjacent as usize]) as isize - 1
            } else {
                0
            };
            let mut prev_idx: isize = -1;
            let idxs: Vec<usize> = if right { (s..end).rev().collect() } else { (s..end).collect() };
            for i in idxs {
                let v = g.order[i];
                let from = start[v];
                let d = start[v + 1] - from;
                if d == 0 {
                    continue;
                }
                let lo = from + ((d - 1) >> 1);
                let hi = from + (d >> 1);
                for pass in 0..2 {
                    if pass == 1 && lo == hi {
                        break;
                    }
                    let e = list[if (pass == 0) != right { lo } else { hi }];
                    let wv = other[e];
                    let idx = if right {
                        adjacent_last - g.pos[wv] as isize
                    } else {
                        g.pos[wv] as isize
                    };
                    if align[v] == v && prev_idx < idx && !conflict[e] {
                        align[wv] = v;
                        root[v] = root[wv];
                        align[v] = root[v];
                        shift[v] = shift[wv] + other_dx[e] - own_dx[e];
                        prev_idx = idx;
                    }
                }
            }
        }

        let mut block_edges = 0;
        for r in 0..=max_rank {
            let s = g.layer_start[r];
            let end = g.layer_start[r + 1];
            for i in s + 1..end {
                let a = if right { g.order[i] } else { g.order[i - 1] };
                let b = if right { g.order[i - 1] } else { g.order[i] };
                let sep = gap[a] + gap[b];
                e_from[block_edges] = root[a];
                e_to[block_edges] = root[b];
                e_w[block_edges] = if right {
                    sep - shift[a] + shift[b]
                } else {
                    sep + shift[a] - shift[b]
                };
                indeg[root[b]] += 1;
                out_start[root[a] + 1] += 1;
                block_edges += 1;
            }
        }
        for i in 0..n {
            out_start[i + 1] += out_start[i];
        }
        fill[..n].copy_from_slice(&out_start[..n]);
        for e in 0..block_edges {
            out_e[fill[e_from[e]]] = e;
            fill[e_from[e]] += 1;
        }

        let mut count = 0;
        for i in 0..n {
            if root[i] == i && indeg[i] == 0 {
                topo[count] = i;
                count += 1;
            }
        }
        let mut q = 0;
        while q < count {
            let v = topo[q];
            for k in out_start[v]..out_start[v + 1] {
                let e = out_e[k];
                let wv = e_to[e];
                if x[v] + e_w[e] > x[wv] {
                    x[wv] = x[v] + e_w[e];
                }
                indeg[wv] -= 1;
                if indeg[wv] == 0 {
                    topo[count] = wv;
                    count += 1;
                }
            }
            q += 1;
        }
        for q in (0..count).rev() {
            let v = topo[q];
            let mut min = f64::INFINITY;
            for k in out_start[v]..out_start[v + 1] {
                let e = out_e[k];
                let cand = x[e_to[e]] - e_w[e];
                if cand < min {
                    min = cand;
                }
            }
            if min != f64::INFINITY && min > x[v] {
                x[v] = min;
            }
        }
        let xs: Vec<f64> = (0..n).map(|i| (if right { -x[root[i]] } else { x[root[i]] }) + shift[i]).collect();
        results.push(xs);
    }

    let mut narrowest = 0;
    let mut best_width = f64::INFINITY;
    let mut mins = Vec::new();
    let mut maxs = Vec::new();
    for (k, xs) in results.iter().enumerate() {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for i in 0..n {
            let half = g.w[i] / 2.0;
            if xs[i] - half < lo {
                lo = xs[i] - half;
            }
            if xs[i] + half > hi {
                hi = xs[i] + half;
            }
        }
        mins.push(lo);
        maxs.push(hi);
        if hi - lo < best_width {
            best_width = hi - lo;
            narrowest = k;
        }
    }
    for k in 0..4 {
        let delta = if k & 1 == 0 {
            mins[narrowest] - mins[k]
        } else {
            maxs[narrowest] - maxs[k]
        };
        if delta != 0.0 {
            for v in results[k].iter_mut() {
                *v += delta;
            }
        }
    }

    let mut out = vec![0.0f64; n];
    for i in 0..n {
        let mut a = results[0][i];
        let mut b = results[1][i];
        let mut c = results[2][i];
        let mut d = results[3][i];
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        if c > d {
            std::mem::swap(&mut c, &mut d);
        }
        if a > c {
            std::mem::swap(&mut a, &mut c);
        }
        if b > d {
            std::mem::swap(&mut b, &mut d);
        }
        if b > c {
            std::mem::swap(&mut b, &mut c);
        }
        out[i] = (b + c) / 2.0;
    }
    out
}
