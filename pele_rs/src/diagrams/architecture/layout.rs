use super::model::{Edge, Group, Hint, Node};
use std::collections::HashMap;
const DX: [i32; 4] = [-1, 1, 0, 0];
const DY: [i32; 4] = [0, 0, -1, 1];
pub struct Grid {
    pub col: Vec<i32>,
    pub row: Vec<i32>,
    pub group_col: Vec<i32>,
    pub group_row: Vec<i32>,
    pub group_cols: Vec<i32>,
    pub group_rows: Vec<i32>,
    pub group_parent: Vec<i32>,
    pub group_empty: Vec<bool>,
    pub node_parent: Vec<i32>,
}
struct Placement {
    n: usize,
    g: usize,
    parent: Vec<usize>,
    depth: Vec<i32>,
    kids: Vec<Vec<usize>>,
    link_a: Vec<usize>,
    link_b: Vec<usize>,
    side_a: Vec<usize>,
    side_b: Vec<usize>,
    item_a: Vec<usize>,
    item_b: Vec<usize>,
    hint_links: Vec<Vec<usize>>,
    edge_links: Vec<Vec<usize>>,
    x: Vec<i32>,
    y: Vec<i32>,
    w: Vec<i32>,
    h: Vec<i32>,
    placed: Vec<bool>,
    order: Vec<usize>,
    cells: HashMap<(i32, i32), usize>,
    large: Vec<usize>,
    reach: HashMap<[i32; 6], i32>,
    first: usize,
}
impl Placement {
    fn add_link(&mut self, a: usize, b: usize, sa: usize, sb: usize, hint: bool) {
        if a == b {
            return;
        }
        let (mut ia, mut ib, mut ca, mut cb) = (a, b, self.parent[a], self.parent[b]);
        while ca != cb {
            if self.depth[ca] >= self.depth[cb] {
                ia = self.n + ca;
                ca = self.parent[ia];
            } else {
                ib = self.n + cb;
                cb = self.parent[ib];
            }
        }
        let k = self.link_a.len();
        self.link_a.push(a);
        self.link_b.push(b);
        self.side_a.push(sa);
        self.side_b.push(sb);
        self.item_a.push(ia);
        self.item_b.push(ib);
        let lists = if hint { &mut self.hint_links } else { &mut self.edge_links };
        lists[ia].push(k);
        lists[ib].push(k);
    }
    fn anchor(&self, node: usize, item: usize) -> (i32, i32) {
        let (mut ax, mut ay, mut cur) = (0, 0, node);
        while cur != item {
            ax += self.x[cur];
            ay += self.y[cur];
            cur = self.n + self.parent[cur];
        }
        (ax, ay)
    }
    fn put(&mut self, item: usize, px: i32, py: i32) {
        self.x[item] = px;
        self.y[item] = py;
        self.placed[item] = true;
        self.order.push(item);
        if self.w[item] * self.h[item] > 64 {
            self.large.push(item);
            return;
        }
        for cy in py..py + self.h[item] {
            for cx in px..px + self.w[item] {
                self.cells.insert((cx, cy), item);
            }
        }
    }
    fn overlaps(&self, item: usize, rx: i32, ry: i32, rw: i32, rh: i32) -> bool {
        self.x[item] < rx + rw && rx < self.x[item] + self.w[item] && self.y[item] < ry + rh && ry < self.y[item] + self.h[item]
    }
    fn collide(&self, rx: i32, ry: i32, rw: i32, rh: i32) -> Option<usize> {
        if rw * rh > 64 {
            return self.order[self.first..]
                .iter()
                .find(|&&i| self.overlaps(i, rx, ry, rw, rh))
                .copied();
        }
        for cy in ry..ry + rh {
            for cx in rx..rx + rw {
                if let Some(&hit) = self.cells.get(&(cx, cy)) {
                    return Some(hit);
                }
            }
        }
        self.large.iter().find(|&&i| self.overlaps(i, rx, ry, rw, rh)).copied()
    }
    fn slide(&mut self, rx: i32, ry: i32, rw: i32, rh: i32, sx: i32, sy: i32) -> i32 {
        let key = [rx, ry, rw, rh, sx, sy];
        let mut k = self.reach.get(&key).copied().unwrap_or(0);
        while let Some(hit) = self.collide(rx + k * sx, ry + k * sy, rw, rh) {
            k = if sx > 0 {
                self.x[hit] + self.w[hit] - rx
            } else if sx < 0 {
                rx + rw - self.x[hit]
            } else if sy > 0 {
                self.y[hit] + self.h[hit] - ry
            } else {
                ry + rh - self.y[hit]
            };
        }
        self.reach.insert(key, k);
        k
    }
    fn follow(&mut self, from: usize, k: usize) {
        let forward = self.item_a[k] == from;
        let to = if forward { self.item_b[k] } else { self.item_a[k] };
        if self.placed[to] {
            return;
        }
        let sp = if forward { self.side_a[k] } else { self.side_b[k] };
        let sq = if forward { self.side_b[k] } else { self.side_a[k] };
        let (pax, pay) = self.anchor(if forward { self.link_a[k] } else { self.link_b[k] }, from);
        let (ax, ay) = self.anchor(if forward { self.link_b[k] } else { self.link_a[k] }, to);
        let (mut dx, mut dy) = (DX[sp], DY[sp]);
        let (mut shift_x, mut shift_y) = (0, 0);
        let facing = DX[sp] + DX[sq] == 0 && DY[sp] + DY[sq] == 0;
        if sp == sq {
            dx = if DX[sp] == 0 { 1 } else { 0 };
            dy = 1 - dx;
        } else if !facing {
            shift_x = -DX[sq];
            shift_y = -DY[sq];
        }
        let (mut bx, mut by) = if dx != 0 {
            (
                if dx > 0 {
                    self.x[from] + self.w[from]
                } else {
                    self.x[from] - self.w[to]
                },
                self.y[from] + pay + shift_y - ay,
            )
        } else {
            (
                self.x[from] + pax + shift_x - ax,
                if dy > 0 {
                    self.y[from] + self.h[from]
                } else {
                    self.y[from] - self.h[to]
                },
            )
        };
        if facing {
            if self.edge_links[to].len() > 1 {
                let mut wanted = Vec::new();
                for &j in &self.edge_links[to] {
                    let to_is_b = self.item_b[j] == to;
                    let other = if to_is_b { self.item_a[j] } else { self.item_b[j] };
                    if !self.placed[other]
                        || (if to_is_b { self.side_a[j] } else { self.side_b[j] }) != sp
                        || (if to_is_b { self.side_b[j] } else { self.side_a[j] }) != sq
                    {
                        continue;
                    }
                    let (ax, ay) = self.anchor(if to_is_b { self.link_a[j] } else { self.link_b[j] }, other);
                    let at = if dx != 0 { self.y[other] + ay } else { self.x[other] + ax };
                    let (ax, ay) = self.anchor(if to_is_b { self.link_b[j] } else { self.link_a[j] }, to);
                    wanted.push(at - if dx != 0 { ay } else { ax });
                }
                if wanted.len() > 1 {
                    wanted.sort();
                    if dx != 0 {
                        by = wanted[(wanted.len() - 1) / 2]
                    } else {
                        bx = wanted[(wanted.len() - 1) / 2]
                    }
                }
            }
            let px = if dx == 0 { 1 } else { 0 };
            let py = 1 - px;
            let forth = self.slide(bx, by, self.w[to], self.h[to], px, py);
            let back = if forth == 0 {
                0
            } else {
                self.slide(bx, by, self.w[to], self.h[to], -px, -py)
            };
            let near = if forth <= back { forth } else { -back };
            bx += px * near;
            by += py * near;
        } else {
            let far = self.slide(bx, by, self.w[to], self.h[to], dx, dy);
            bx += dx * far;
            by += dy * far;
        }
        self.put(to, bx, by);
    }
    fn layout(&mut self, c: usize) {
        let items = self.kids[c].clone();
        if items.is_empty() {
            return;
        }
        self.order.clear();
        let mut starts = Vec::new();
        for start in items {
            if self.placed[start] {
                continue;
            }
            self.first = self.order.len();
            starts.push(self.first);
            self.cells.clear();
            self.large.clear();
            self.reach.clear();
            self.put(start, 0, 0);
            let (mut hint_at, mut edge_at, mut link_at) = (self.first, self.first, 0);
            loop {
                if hint_at < self.order.len() {
                    let from = self.order[hint_at];
                    hint_at += 1;
                    for k in self.hint_links[from].clone() {
                        self.follow(from, k);
                    }
                    continue;
                }
                if edge_at >= self.order.len() {
                    break;
                }
                let from = self.order[edge_at];
                if link_at >= self.edge_links[from].len() {
                    edge_at += 1;
                    link_at = 0;
                    continue;
                }
                let k = self.edge_links[from][link_at];
                link_at += 1;
                self.follow(from, k);
            }
        }
        starts.push(self.order.len());
        let count = starts.len() - 1;
        let (mut min_x, mut min_y, mut part_w, mut part_h) = (vec![0; count], vec![0; count], vec![0; count], vec![0; count]);
        let (mut area, mut widest) = (0, 0);
        for p in 0..count {
            let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            for &item in &self.order[starts[p]..starts[p + 1]] {
                x0 = x0.min(self.x[item]);
                y0 = y0.min(self.y[item]);
                x1 = x1.max(self.x[item] + self.w[item]);
                y1 = y1.max(self.y[item] + self.h[item]);
            }
            min_x[p] = x0;
            min_y[p] = y0;
            part_w[p] = x1 - x0;
            part_h[p] = y1 - y0;
            area += part_w[p] * part_h[p];
            widest = widest.max(part_w[p]);
        }
        let limit = widest.max(((area * 2) as f64).sqrt().ceil() as i32);
        let (mut cx, mut cy, mut shelf, mut width) = (0, 0, 0, 0);
        for p in 0..count {
            if cx > 0 && cx + part_w[p] > limit {
                cy += shelf;
                cx = 0;
                shelf = 0;
            }
            for &i in &self.order[starts[p]..starts[p + 1]] {
                self.x[i] += cx - min_x[p];
                self.y[i] += cy - min_y[p];
            }
            cx += part_w[p];
            shelf = shelf.max(part_h[p]);
            width = width.max(cx);
        }
        if c < self.g {
            self.w[self.n + c] = width;
            self.h[self.n + c] = cy + shelf;
        }
    }
}
pub fn place(nodes: &[&Node], groups: &[&Group], edges: &[Edge], hints: &[Hint]) -> Grid {
    let n = nodes.len();
    let g = groups.len();
    let total = n + g;
    let mut p = Placement {
        n,
        g,
        parent: vec![g; total],
        depth: vec![0; g + 1],
        kids: vec![vec![]; g + 1],
        link_a: vec![],
        link_b: vec![],
        side_a: vec![],
        side_b: vec![],
        item_a: vec![],
        item_b: vec![],
        hint_links: vec![vec![]; total],
        edge_links: vec![vec![]; total],
        x: vec![0; total],
        y: vec![0; total],
        w: vec![1; total],
        h: vec![1; total],
        placed: vec![false; total],
        order: vec![],
        cells: HashMap::new(),
        large: vec![],
        reach: HashMap::new(),
        first: 0,
    };
    let (mut node_index, mut group_index) = (HashMap::new(), HashMap::new());
    for (i, group) in groups.iter().enumerate() {
        group_index.insert(group.id.as_str(), i);
        if let Some(&parent) = group
            .parent
            .as_deref()
            .and_then(|id| group_index.get(id))
            .filter(|&&parent| parent != i)
        {
            p.parent[n + i] = parent;
            p.depth[i] = p.depth[parent] + 1;
        } else {
            p.depth[i] = 1;
        }
    }
    for (i, node) in nodes.iter().enumerate() {
        node_index.insert(node.id.as_str(), i);
        if let Some(&parent) = node.parent.as_deref().and_then(|id| group_index.get(id)) {
            p.parent[i] = parent;
        }
        p.kids[p.parent[i]].push(i);
    }
    for i in 0..g {
        p.kids[p.parent[n + i]].push(n + i);
    }
    for hint in hints {
        let row = hint.direction == "row";
        for pair in hint.members.windows(2) {
            if let (Some(&a), Some(&b)) = (node_index.get(pair[0].as_str()), node_index.get(pair[1].as_str())) {
                p.add_link(a, b, if row { 1 } else { 3 }, if row { 0 } else { 2 }, true);
            }
        }
    }
    for like in [false, true] {
        for edge in edges {
            if (edge.lhs_dir == edge.rhs_dir) != like {
                continue;
            }
            if let (Some(&a), Some(&b)) = (node_index.get(edge.lhs_id.as_str()), node_index.get(edge.rhs_id.as_str())) {
                p.add_link(a, b, edge.lhs_dir, edge.rhs_dir, false);
            }
        }
    }
    for c in (0..g).rev() {
        p.layout(c);
    }
    p.layout(g);
    let mut out = Grid {
        col: vec![0; n],
        row: vec![0; n],
        group_col: vec![0; g],
        group_row: vec![0; g],
        group_cols: p.w[n..].to_vec(),
        group_rows: p.h[n..].to_vec(),
        group_parent: vec![-1; g],
        group_empty: vec![false; g],
        node_parent: vec![-1; n],
    };
    for i in 0..g {
        let parent = p.parent[n + i];
        out.group_parent[i] = if parent == g { -1 } else { parent as i32 };
        out.group_col[i] = p.x[n + i] + if parent == g { 0 } else { out.group_col[parent] };
        out.group_row[i] = p.y[n + i] + if parent == g { 0 } else { out.group_row[parent] };
        out.group_empty[i] = p.kids[i].is_empty();
    }
    for i in 0..n {
        let parent = p.parent[i];
        out.node_parent[i] = if parent == g { -1 } else { parent as i32 };
        out.col[i] = p.x[i] + if parent == g { 0 } else { out.group_col[parent] };
        out.row[i] = p.y[i] + if parent == g { 0 } else { out.group_row[parent] };
    }
    out
}
