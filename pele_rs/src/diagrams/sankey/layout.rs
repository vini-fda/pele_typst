use super::Model;
use crate::errors::{PeleError, PeleErrorCode};
use crate::util::ord;
pub struct Options<'a> {
    pub width: f64,
    pub height: f64,
    pub node_width: f64,
    pub node_padding: f64,
    pub label_height: f64,
    pub alignment: &'a str,
}
pub struct Node {
    pub layer: usize,
    pub value: f64,
    pub x: f64,
    pub y: f64,
    pub height: f64,
}
pub struct Link {
    pub source: usize,
    pub target: usize,
    pub value: f64,
    pub thickness: f64,
    pub source_y: f64,
    pub target_y: f64,
}
pub struct Layout {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
}
struct State {
    nodes: Vec<Node>,
    links: Vec<Link>,
    outgoing: Vec<Vec<usize>>,
    incoming: Vec<Vec<usize>>,
    source_offset: Vec<f64>,
    target_offset: Vec<f64>,
    margin: Vec<f64>,
    gap: f64,
    height: f64,
}
impl State {
    fn center(&self, i: usize) -> f64 {
        self.nodes[i].y + self.nodes[i].height / 2.0
    }
    fn stack_out(&mut self, i: usize) {
        let nodes = &self.nodes;
        let links = &self.links;
        self.outgoing[i].sort_by(|&a, &b| {
            let x = &nodes[links[a].target];
            let y = &nodes[links[b].target];
            ord((x.y + x.height / 2.0) - (y.y + y.height / 2.0)).then_with(|| a.cmp(&b))
        });
        let mut y = 0.0;
        for &k in &self.outgoing[i] {
            self.source_offset[k] = y + self.links[k].thickness / 2.0;
            y += self.links[k].thickness;
        }
    }
    fn stack_in(&mut self, i: usize) {
        let nodes = &self.nodes;
        let links = &self.links;
        self.incoming[i].sort_by(|&a, &b| {
            let x = &nodes[links[a].source];
            let y = &nodes[links[b].source];
            ord((x.y + x.height / 2.0) - (y.y + y.height / 2.0)).then_with(|| a.cmp(&b))
        });
        let mut y = 0.0;
        for &k in &self.incoming[i] {
            self.target_offset[k] = y + self.links[k].thickness / 2.0;
            y += self.links[k].thickness;
        }
    }
    fn stack_all(&mut self) {
        for i in 0..self.nodes.len() {
            self.stack_out(i);
            self.stack_in(i);
        }
    }
    fn settle(&mut self, column: &mut [usize], pull: f64, forward: bool) {
        for &i in column.iter() {
            if (if forward { &self.incoming[i] } else { &self.outgoing[i] }).is_empty() {
                continue;
            }
            if forward {
                self.stack_in(i);
            } else {
                self.stack_out(i);
            }
            let list = if forward { &self.incoming[i] } else { &self.outgoing[i] };
            let (mut weighted, mut weight, mut plain) = (0.0, 0.0, 0.0);
            for &k in list {
                let link = &self.links[k];
                let level = if forward {
                    self.nodes[link.source].y + self.source_offset[k] - self.target_offset[k]
                } else {
                    self.nodes[link.target].y + self.target_offset[k] - self.source_offset[k]
                };
                weighted += level * link.thickness;
                weight += link.thickness;
                plain += self.center(if forward { link.source } else { link.target }) - self.nodes[i].height / 2.0;
            }
            let wanted = if weight > 0.0 {
                weighted / weight
            } else {
                plain / list.len() as f64
            };
            self.nodes[i].y += (wanted - self.nodes[i].y) * pull;
        }
        column.sort_by(|&a, &b| ord(self.center(a) - self.center(b)).then_with(|| a.cmp(&b)));
        let mut y = 0.0;
        for &i in column.iter() {
            if self.nodes[i].y - self.margin[i] < y {
                self.nodes[i].y = y + self.margin[i];
            }
            y = self.nodes[i].y + self.nodes[i].height + self.margin[i] + self.gap;
        }
        y = self.height;
        for &i in column.iter().rev() {
            if self.nodes[i].y + self.nodes[i].height + self.margin[i] > y {
                self.nodes[i].y = y - self.nodes[i].height - self.margin[i];
            }
            y = self.nodes[i].y - self.margin[i] - self.gap;
        }
    }
}
pub fn layout(model: &Model, options: &Options) -> Result<Layout, PeleError> {
    let n = model.nodes.len();
    let mut links: Vec<_> = model
        .links
        .iter()
        .map(|l| Link {
            source: l.source,
            target: l.target,
            value: if l.value.is_finite() && l.value > 0.0 { l.value } else { 0.0 },
            thickness: 0.0,
            source_y: 0.0,
            target_y: 0.0,
        })
        .collect();
    let mut outgoing = vec![vec![]; n];
    let mut incoming = vec![vec![]; n];
    for (k, l) in links.iter().enumerate() {
        outgoing[l.source].push(k);
        incoming[l.target].push(k);
    }
    let mut depth = vec![0; n];
    let mut pending: Vec<_> = incoming.iter().map(Vec::len).collect();
    let mut order: Vec<_> = (0..n).filter(|&i| pending[i] == 0).collect();
    let mut q = 0;
    while q < order.len() {
        let i = order[q];
        for &k in &outgoing[i] {
            let t = links[k].target;
            depth[t] = depth[t].max(depth[i] + 1);
            pending[t] -= 1;
            if pending[t] == 0 {
                order.push(t);
            }
        }
        q += 1;
    }
    if order.len() < n {
        let mut stuck = pending.iter().position(|&p| p > 0).unwrap();
        let mut seen = vec![false; n];
        while !seen[stuck] {
            seen[stuck] = true;
            for &k in &incoming[stuck] {
                if pending[links[k].source] > 0 {
                    stuck = links[k].source;
                    break;
                }
            }
        }
        return Err(PeleError::new(
            format!(
                "Sankey diagrams cannot have circular links, and \"{}\" is part of one.",
                model.nodes[stuck]
            ),
            PeleErrorCode::Semantic,
        )
        .of("sankey"));
    }
    let layers = depth.iter().map(|&d| d + 1).max().unwrap_or(1);
    let mut reach = vec![0; n];
    for &i in order.iter().rev() {
        for &k in &outgoing[i] {
            reach[i] = reach[i].max(reach[links[k].target] + 1);
        }
    }
    let largest = links.iter().map(|l| l.value).fold(0.0, f64::max);
    let unit = if largest > 0.0 { 1.0 / largest } else { 0.0 };
    let mut nodes = vec![];
    let mut flow = vec![0.0; n];
    let mut columns = vec![vec![]; layers];
    let step = if layers > 1 {
        (options.width - options.node_width) / (layers - 1) as f64
    } else {
        0.0
    };
    for i in 0..n {
        let layer = match options.alignment {
            "left" => depth[i],
            "right" => layers - 1 - reach[i],
            "center" => {
                if incoming[i].is_empty() && !outgoing[i].is_empty() {
                    outgoing[i].iter().map(|&k| depth[links[k].target] - 1).min().unwrap_or(layers)
                } else {
                    depth[i]
                }
            }
            _ => {
                if outgoing[i].is_empty() {
                    layers - 1
                } else {
                    depth[i]
                }
            }
        };
        let (mut into, mut out, mut value, mut value_out) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
        for &k in &incoming[i] {
            into += links[k].value * unit;
            value += links[k].value;
        }
        for &k in &outgoing[i] {
            out += links[k].value * unit;
            value_out += links[k].value;
        }
        flow[i] = into.max(out);
        nodes.push(Node {
            layer,
            value: value.max(value_out),
            x: layer as f64 * step,
            y: 0.0,
            height: 0.0,
        });
        columns[layer].push(i);
    }
    let tallest = columns.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let gap = if tallest > 1 {
        options.node_padding.min(options.height / (tallest - 1) as f64)
    } else {
        options.node_padding
    };
    let mut scale = f64::INFINITY;
    for col in &columns {
        let sum = col.iter().map(|&i| flow[i]).sum::<f64>();
        if sum > 0.0 {
            scale = scale.min((options.height - (col.len() as f64 - 1.0) * gap) / sum);
        }
    }
    if !scale.is_finite() || scale < 0.0 {
        scale = 0.0;
    }
    let mut claim = 0.0_f64.max(options.label_height - gap);
    let fits = |s: f64, claim: f64| {
        columns.iter().all(|col| {
            let mut extent = (col.len() as f64 - 1.0) * gap;
            for &i in col {
                extent += (flow[i] * s).max(claim);
            }
            extent <= options.height + 1e-6
        })
    };
    if claim > 0.0 && !fits(scale, claim) {
        if fits(0.0, claim) {
            let mut low = 0.0;
            for _ in 0..40 {
                let mid = (low + scale) / 2.0;
                if fits(mid, claim) {
                    low = mid;
                } else {
                    scale = mid;
                }
            }
            scale = low;
        } else {
            claim = 0.0;
        }
    }
    let mut margin = vec![0.0; n];
    for l in &mut links {
        l.thickness = l.value * unit * scale;
    }
    for col in &columns {
        let mut y = 0.0;
        for &i in col {
            nodes[i].height = flow[i] * scale;
            margin[i] = 0.0_f64.max(claim - nodes[i].height) / 2.0;
            nodes[i].y = y + margin[i];
            y += nodes[i].height + 2.0 * margin[i] + gap;
        }
        let spare = (options.height - y + gap) / (col.len() + 1) as f64;
        for (k, &i) in col.iter().enumerate() {
            nodes[i].y += spare * (k + 1) as f64;
        }
    }
    let count = links.len();
    let mut state = State {
        nodes,
        links,
        outgoing,
        incoming,
        source_offset: vec![0.0; count],
        target_offset: vec![0.0; count],
        margin,
        gap,
        height: options.height,
    };
    state.stack_all();
    for pass in 0..6 {
        let pull = 0.99_f64.powi(pass);
        for l in (0..layers - 1).rev() {
            state.settle(&mut columns[l], pull, false);
        }
        state.stack_all();
        for column in columns.iter_mut().skip(1) {
            state.settle(column, pull, true);
        }
        state.stack_all();
    }
    for k in 0..state.links.len() {
        let l = &mut state.links[k];
        l.source_y = state.nodes[l.source].y + state.source_offset[k];
        l.target_y = state.nodes[l.target].y + state.target_offset[k];
    }
    Ok(Layout {
        nodes: state.nodes,
        links: state.links,
    })
}
