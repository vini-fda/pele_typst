use super::compound::{CEdge, CNode, CompoundResult};
use indexmap::IndexMap;
struct Part {
    nodes: Vec<usize>,
    edges: Vec<usize>,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}
impl Part {
    fn include(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.left = self.left.min(x - w / 2.);
        self.top = self.top.min(y - h / 2.);
        self.right = self.right.max(x + w / 2.);
        self.bottom = self.bottom.max(y + h / 2.);
    }
}
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}
/// Moves disconnected parts into rows while preserving every part's internal layout.
pub fn wrap_components(nodes: &mut [CNode], edges: &mut [CEdge], width: f64, gap: f64) -> Option<CompoundResult> {
    let mut parent: Vec<usize> = (0..nodes.len()).collect();
    for i in 0..nodes.len() {
        if nodes[i].parent >= 0 {
            let a = root(&mut parent, i);
            let b = root(&mut parent, nodes[i].parent as usize);
            parent[a] = b;
        }
    }
    for edge in edges.iter() {
        let a = root(&mut parent, edge.src);
        let b = root(&mut parent, edge.dst);
        parent[a] = b;
    }
    let mut parts: IndexMap<usize, Part> = IndexMap::new();
    for (i, node) in nodes.iter().enumerate() {
        let id = root(&mut parent, i);
        let part = parts.entry(id).or_insert_with(|| Part {
            nodes: vec![],
            edges: vec![],
            left: f64::INFINITY,
            top: f64::INFINITY,
            right: f64::NEG_INFINITY,
            bottom: f64::NEG_INFINITY,
        });
        part.nodes.push(i);
        part.include(node.x, node.y, node.w, node.h);
    }
    if parts.len() < 2 {
        return None;
    }
    for (i, e) in edges.iter().enumerate() {
        let part = parts.get_mut(&root(&mut parent, e.src)).unwrap();
        part.edges.push(i);
        for p in e.route.as_chunks::<3>().0 {
            part.include(p[0], p[1], 0., 0.);
        }
        if e.label_w > 0. {
            part.include(e.label_x, e.label_y, e.label_w, e.label_h);
        }
    }
    let (mut x, mut y, mut row_height, mut right, mut from, mut to) = (0f64, 0f64, 0f64, 0f64, f64::INFINITY, f64::NEG_INFINITY);
    let mut shifts = Vec::new();
    for part in parts.values() {
        let (w, h) = (part.right - part.left, part.bottom - part.top);
        if x > 0. && x + w > width {
            y += row_height + gap;
            x = 0.;
            row_height = 0.;
        }
        shifts.push((x - part.left, y - part.top));
        from = from.min(part.left);
        to = to.max(part.right);
        right = right.max(x + w);
        row_height = row_height.max(h);
        x += w + gap;
    }
    if right >= to - from {
        return None;
    }
    for (part, (dx, dy)) in parts.values().zip(shifts) {
        for &i in &part.nodes {
            nodes[i].x += dx;
            nodes[i].y += dy;
        }
        for &i in &part.edges {
            let edge = &mut edges[i];
            edge.label_x += dx;
            edge.label_y += dy;
            for p in edge.route.as_chunks_mut::<3>().0 {
                p[0] += dx;
                p[1] += dy;
            }
        }
    }
    Some(CompoundResult {
        width: right,
        height: y + row_height,
    })
}
