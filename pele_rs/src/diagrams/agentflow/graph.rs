use super::{
    db::{LabelType, Model},
    shapes,
};
use crate::util::trim;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub label: Option<String>,
    pub label_type: LabelType,
    pub shape: String,
    pub is_group: bool,
    pub parent_id: Option<String>,
    pub css_styles: Vec<String>,
    pub css_compiled_styles: Vec<String>,
    pub css_classes: String,
    pub dir: Option<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub tooltip: Option<String>,
    pub color_index: Option<usize>,
    pub kind: Option<String>,
    pub metadata: Option<Map<String, Value>>,
}
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub id: String,
    pub is_user_defined_id: bool,
    pub start: String,
    pub end: String,
    pub kind: String,
    pub label: String,
    pub label_type: LabelType,
    pub thickness: Option<String>,
    pub pattern: Option<String>,
    pub minlen: Option<f64>,
    pub arrow_type_start: String,
    pub arrow_type_end: String,
    pub css_compiled_styles: Vec<String>,
    pub style: Vec<String>,
    pub classes: String,
    pub animate: Option<Value>,
    pub animation: Option<Value>,
    pub curve: Option<String>,
    pub metadata: Option<Map<String, Value>>,
}
#[derive(Debug, Clone)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}
fn compiled(db: &Model, classes: &[String]) -> Vec<String> {
    let mut out = vec![];
    for name in classes {
        if let Some(class) = db.classes.get(name) {
            out.extend(class.styles.iter().chain(&class.text_styles).map(|s| trim(s).to_string()));
        }
    }
    out
}
pub fn build(db: &mut Model, palette: usize, default_curve: Option<&str>) -> Graph {
    let mut nodes = vec![];
    let mut edges = vec![];
    let subgraphs = db.subgraphs.clone();
    let mut hidden = HashSet::new();
    let mut owner = HashMap::new();
    let mut walked = HashSet::new();
    for sg in &subgraphs {
        if sg.metadata.as_ref().and_then(|m| m.get("view")).and_then(Value::as_str) != Some("collapsed") || hidden.contains(&sg.id) {
            continue;
        }
        let mut pending = vec![sg.id.clone()];
        while let Some(id) = pending.pop() {
            let Some(group) = db.subgraph(&id) else {
                continue;
            };
            for child in &group.nodes {
                if child == &sg.id || hidden.contains(child) {
                    continue;
                }
                hidden.insert(child.clone());
                owner.insert(child.clone(), sg.id.clone());
                if !walked.contains(child) {
                    pending.push(child.clone());
                }
            }
        }
        walked.insert(sg.id.clone());
    }
    let mut collapsed_into: HashMap<String, String> = HashMap::new();
    for id in &hidden {
        let mut path = vec![id.clone()];
        let mut top = owner[id].clone();
        loop {
            let known = collapsed_into.get(&top);
            let Some(up) = known.or_else(|| owner.get(&top)) else {
                break;
            };
            if known.is_none() {
                path.push(top.clone());
            }
            top = up.clone();
            if known.is_some() {
                break;
            }
        }
        for id in path {
            collapsed_into.insert(id, top.clone());
        }
    }
    let position: HashMap<_, _> = subgraphs.iter().enumerate().map(|(i, sg)| (sg.id.clone(), i)).collect();
    let mut parent_of: HashMap<String, String> = HashMap::new();
    for (i, sg) in subgraphs.iter().enumerate().rev() {
        if hidden.contains(&sg.id) {
            continue;
        }
        for id in &sg.nodes {
            let mut closes = false;
            if position.get(id).is_some_and(|&at| at >= i) {
                let mut current = Some(sg.id.as_str());
                for _ in 0..=subgraphs.len() {
                    let Some(next) = current else {
                        break;
                    };
                    if next == id {
                        closes = true;
                        break;
                    }
                    current = parent_of.get(next).map(String::as_str);
                }
            }
            if closes {
                db.diagnostic("CONTAINMENT_VIOLATION","warning",format!("Container \"{}\" cannot contain \"{id}\" because \"{id}\" already contains it. The nesting that would close the loop is dropped.",sg.id),Some(id),None);
                continue;
            }
            parent_of.insert(id.clone(), sg.id.clone());
        }
    }
    let mut by_id: HashMap<String, usize> = HashMap::new();
    for sg in subgraphs.iter().rev() {
        if hidden.contains(&sg.id) {
            continue;
        }
        let collapsed = sg.metadata.as_ref().and_then(|m| m.get("view")).and_then(Value::as_str) == Some("collapsed");
        let mut metadata = sg.metadata.clone();
        if collapsed {
            metadata
                .get_or_insert_with(Map::new)
                .insert("containerType".into(), Value::String("flow".into()));
        }
        let node = GraphNode {
            id: sg.id.clone(),
            label: Some(sg.title.clone()),
            label_type: sg.label_type,
            parent_id: parent_of.get(&sg.id).cloned(),
            css_styles: vec![],
            css_compiled_styles: compiled(db, &sg.classes),
            css_classes: sg.classes.join(" "),
            shape: if collapsed { "collapsedGroup" } else { "flowGroup" }.into(),
            dir: sg.dir.clone(),
            is_group: !collapsed,
            metadata,
            link: None,
            link_target: None,
            tooltip: None,
            color_index: None,
            kind: None,
        };
        by_id.insert(sg.id.clone(), nodes.len());
        nodes.push(node);
    }
    for vertex in db.nodes.values() {
        if hidden.contains(&vertex.id) {
            continue;
        }
        if let Some(&i) = by_id.get(&vertex.id) {
            let node = &mut nodes[i];
            node.css_styles = vertex.styles.clone();
            node.css_compiled_styles = compiled(db, &vertex.classes);
            node.css_classes = vertex.classes.join(" ");
            continue;
        }
        let shape = vertex.kind.as_deref().map(shapes::resolve);
        let classes: Vec<_> = ["default".to_string(), "node".to_string()]
            .into_iter()
            .chain(vertex.classes.iter().cloned())
            .collect();
        let node = GraphNode {
            id: vertex.id.clone(),
            label: vertex.text.clone(),
            label_type: vertex.label_type,
            parent_id: parent_of.get(&vertex.id).cloned(),
            css_styles: vertex.styles.clone(),
            css_compiled_styles: compiled(db, &classes),
            css_classes: format!("default {}", vertex.classes.join(" ")),
            dir: vertex.dir.clone(),
            link: vertex.link.clone(),
            link_target: vertex.link_target.clone(),
            tooltip: db.tooltips.get(&vertex.id).cloned(),
            metadata: vertex.metadata.clone(),
            is_group: false,
            shape: match shape {
                None | Some("square") => "squareRect",
                Some(s) => s,
            }
            .into(),
            color_index: None,
            kind: None,
        };
        by_id.insert(vertex.id.clone(), nodes.len());
        nodes.push(node);
    }
    for (index, raw) in db.edges.iter().enumerate() {
        let start = collapsed_into.get(&raw.start).unwrap_or(&raw.start);
        let end = collapsed_into.get(&raw.end).unwrap_or(&raw.end);
        if start == end && raw.start != raw.end {
            continue;
        }
        let mut styles = db.default_edge_style.clone().unwrap_or_default();
        if let Some(style) = &raw.style {
            styles.extend(style.iter().cloned());
        }
        let arrow = match raw.kind.as_deref() {
            Some("arrow_open") => "none",
            Some("arrow_cross") => "arrow_cross",
            _ => "arrow_point",
        };
        edges.push(GraphEdge {
            id: if raw.id.is_empty() {
                format!("L_{start}_{end}_{index}")
            } else {
                raw.id.clone()
            },
            is_user_defined_id: raw.is_user_defined_id,
            start: start.clone(),
            end: end.clone(),
            kind: raw.kind.clone().unwrap_or("normal".into()),
            label: raw.text.clone(),
            label_type: raw.label_type,
            thickness: raw.stroke.clone(),
            pattern: raw.stroke.clone(),
            minlen: raw.length,
            classes: "edge-thickness-normal edge-pattern-solid flowchart-link".into(),
            arrow_type_start: "none".into(),
            arrow_type_end: arrow.into(),
            css_compiled_styles: compiled(db, &raw.classes),
            style: styles,
            animate: raw.animate.clone(),
            animation: raw.animation.clone(),
            curve: raw
                .interpolate
                .clone()
                .filter(|s| !s.is_empty())
                .or(db.default_interpolate.clone().filter(|s| !s.is_empty()))
                .or_else(|| default_curve.map(str::to_string)),
            metadata: raw.metadata.clone(),
        });
    }
    for node in &mut nodes {
        if node.is_group {
            continue;
        }
        if node.shape.is_empty() || matches!(node.shape.as_str(), "squareRect" | "rect") {
            node.shape = "roundedRect".into();
        }
        if shapes::removed(&node.shape) {
            db.diagnostic(
                "SHAPE_REMOVED",
                "error",
                format!("shape \"{}\" was removed in v0.8.1, using \"roundedRect\"", node.shape),
                Some(&node.id),
                None,
            );
            node.shape = "roundedRect".into();
        } else if !shapes::allowed(&node.shape) {
            db.diagnostic(
                "SHAPE_UNSUPPORTED",
                "warning",
                format!("shape \"{}\" is not supported, using \"roundedRect\"", node.shape),
                Some(&node.id),
                None,
            );
            node.shape = "roundedRect".into();
        }
    }
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for sg in &subgraphs {
        if let Some(parent) = parent_of.get(&sg.id) {
            children.entry(parent.clone()).or_default().push(sg.id.clone());
        }
    }
    let mut order = HashMap::new();
    for sg in &subgraphs {
        if parent_of.contains_key(&sg.id) {
            continue;
        }
        let mut pending = vec![sg.id.clone()];
        while let Some(id) = pending.pop() {
            if order.contains_key(&id) {
                continue;
            }
            order.insert(id.clone(), order.len());
            if let Some(kids) = children.get(&id) {
                pending.extend(kids.iter().rev().cloned());
            }
        }
    }
    let mut fallback = order.len();
    for node in &mut nodes {
        if node.is_group || node.shape == "collapsedGroup" {
            let n = order.get(&node.id).copied().unwrap_or_else(|| {
                let n = fallback;
                fallback += 1;
                n
            });
            let slot = 7 + n % palette.saturating_sub(7).max(1);
            node.color_index = Some(if palette > 0 { slot % palette } else { slot });
            continue;
        }
        let kind = if db.connectors.contains_key(&node.id) {
            Some("connector")
        } else {
            db.nodes.get(&node.id).map(Model::vertex_kind)
        };
        if let Some(kind) = kind.filter(|s| shapes::kind_slot(s).is_some()) {
            node.kind = Some(kind.into());
        }
    }
    Graph { nodes, edges }
}
