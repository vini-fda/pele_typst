// Resolves a parsed flowchart into the nodes and edges that are drawn: group membership,
// collapsed subgraphs, class styles and edge defaults. After pele's diagrams/flowchart/graph.ts.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::db::{FlowDb, FlowNode, LabelType};

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
    pub icon: Option<String>,
    pub pos: Option<String>,
    pub img: Option<String>,
    pub asset_width: Option<f64>,
    pub asset_height: Option<f64>,
    pub constraint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub label: String,
    pub label_type: LabelType,
    pub thickness: Option<String>,
    pub minlen: Option<f64>,
    pub arrow_type_start: String,
    pub arrow_type_end: String,
    pub css_compiled_styles: Vec<String>,
    pub style: Vec<String>,
    pub classes: String,
    pub curve: Option<String>,
}

pub struct FlowGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

fn shape_of(vertex: &FlowNode) -> String {
    if vertex.img.as_deref().is_some_and(|s| !s.is_empty()) {
        return "imageSquare".into();
    }
    if vertex.icon.as_deref().is_some_and(|s| !s.is_empty()) {
        return match vertex.form.as_deref() {
            Some("circle") => "iconCircle",
            Some("square") => "iconSquare",
            Some("rounded") => "iconRounded",
            _ => "icon",
        }
        .into();
    }
    match vertex.kind.as_deref() {
        None | Some("square") => "squareRect".into(),
        Some("round") => "roundedRect".into(),
        Some(other) => other.into(),
    }
}

fn arrows(kind: Option<&str>) -> (String, String) {
    match kind {
        Some(k @ ("arrow_point" | "arrow_circle" | "arrow_cross")) => ("none".into(), k.into()),
        Some(k @ ("double_arrow_point" | "double_arrow_circle" | "double_arrow_cross")) => {
            let single = k.replacen("double_", "", 1);
            (single.clone(), single)
        }
        _ => ("none".into(), "arrow_point".into()),
    }
}

fn collapsed(meta: &Option<serde_json::Map<String, Value>>) -> bool {
    meta.as_ref().and_then(|m| m.get("view")).and_then(|v| v.as_str()) == Some("collapsed")
}

pub fn build_flow_graph(model: &FlowDb, default_curve: Option<&str>) -> FlowGraph {
    let subgraphs = &model.subgraphs;
    let compiled = |names: &[String]| -> Vec<String> {
        let mut out = Vec::new();
        for name in names {
            let Some(def) = model.classes.get(name) else { continue };
            for s in &def.styles {
                out.push(crate::util::trim(s).to_string());
            }
            for s in &def.text_styles {
                out.push(crate::util::trim(s).to_string());
            }
        }
        out
    };

    let mut subgraph_parent: HashMap<String, String> = HashMap::new();
    for sg in subgraphs {
        for child in &sg.nodes {
            if model.subgraph(child).is_some() {
                subgraph_parent.insert(child.clone(), sg.id.clone());
            }
        }
    }

    let is_collapsed = |id: &str| model.subgraph(id).is_some_and(|sg| collapsed(&sg.metadata));
    let any_collapsed = subgraphs.iter().any(|sg| collapsed(&sg.metadata));
    let mut outermost: HashMap<String, Option<String>> = HashMap::new();
    let mut outermost_collapsed = |id: &str| -> Option<String> {
        let mut chain: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut current: Option<String> = Some(id.to_string());
        while let Some(c) = current.clone() {
            if outermost.contains_key(&c) || seen.contains(&c) {
                break;
            }
            seen.insert(c.clone());
            chain.push(c.clone());
            current = subgraph_parent.get(&c).cloned();
        }
        let mut result = current.as_ref().and_then(|c| outermost.get(c).cloned().flatten());
        for c in chain.iter().rev() {
            if result.is_none() && is_collapsed(c) {
                result = Some(c.clone());
            }
            outermost.insert(c.clone(), result.clone());
        }
        result
    };

    let mut hidden: HashSet<String> = HashSet::new();
    let mut collapsed_into: HashMap<String, String> = HashMap::new();
    if any_collapsed {
        for sg in subgraphs {
            let Some(ancestor) = outermost_collapsed(&sg.id) else { continue };
            if sg.id != ancestor {
                hidden.insert(sg.id.clone());
                collapsed_into.insert(sg.id.clone(), ancestor.clone());
            }
            for child in &sg.nodes {
                if *child == ancestor {
                    continue;
                }
                hidden.insert(child.clone());
                collapsed_into.insert(child.clone(), ancestor.clone());
            }
        }
    }

    let mut parent_of: HashMap<String, String> = HashMap::new();
    let mut groups: HashSet<String> = HashSet::new();
    for sg in subgraphs.iter().rev() {
        if hidden.contains(&sg.id) {
            continue;
        }
        if !sg.nodes.is_empty() {
            groups.insert(sg.id.clone());
        }
        for id in &sg.nodes {
            parent_of.insert(id.clone(), sg.id.clone());
        }
    }

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut by_id: HashMap<String, usize> = HashMap::new();
    for sg in subgraphs.iter().rev() {
        if hidden.contains(&sg.id) {
            continue;
        }
        let is_collapsed = collapsed(&sg.metadata);
        nodes.push(GraphNode {
            id: sg.id.clone(),
            label: Some(sg.title.clone()),
            label_type: sg.label_type,
            parent_id: parent_of.get(&sg.id).cloned(),
            css_styles: Vec::new(),
            css_compiled_styles: compiled(&sg.classes),
            css_classes: sg.classes.join(" "),
            shape: if is_collapsed { "collapsedGroup".into() } else { "rect".into() },
            dir: sg.dir.clone(),
            is_group: !is_collapsed,
            link: None,
            link_target: None,
            tooltip: None,
            icon: None,
            pos: None,
            img: None,
            asset_width: None,
            asset_height: None,
            constraint: None,
        });
        by_id.insert(sg.id.clone(), nodes.len() - 1);
    }

    for vertex in model.nodes.values() {
        if hidden.contains(&vertex.id) {
            continue;
        }
        if let Some(&existing) = by_id.get(&vertex.id) {
            let n = &mut nodes[existing];
            n.css_styles = vertex.styles.clone();
            n.css_compiled_styles = compiled(&vertex.classes);
            n.css_classes = vertex.classes.join(" ");
            continue;
        }
        let is_group = groups.contains(&vertex.id);
        let mut names = vec!["default".to_string(), "node".to_string()];
        names.extend(vertex.classes.iter().cloned());
        nodes.push(GraphNode {
            id: vertex.id.clone(),
            label: vertex.text.clone(),
            label_type: vertex.label_type,
            parent_id: parent_of.get(&vertex.id).cloned(),
            css_styles: vertex.styles.clone(),
            css_compiled_styles: compiled(&names),
            css_classes: format!("default {}", vertex.classes.join(" ")),
            dir: vertex.dir.clone(),
            link: vertex.link.clone(),
            link_target: vertex.link_target.clone(),
            tooltip: model.tooltips.get(&vertex.id).cloned(),
            icon: vertex.icon.clone(),
            pos: vertex.pos.clone(),
            img: vertex.img.clone(),
            asset_width: vertex.asset_width,
            asset_height: vertex.asset_height,
            constraint: vertex.constraint.clone(),
            is_group,
            shape: if is_group { "rect".into() } else { shape_of(vertex) },
        });
    }

    let mut edges: Vec<GraphEdge> = Vec::new();
    for (index, raw) in model.edges.iter().enumerate() {
        let start = collapsed_into.get(&raw.start).cloned().unwrap_or_else(|| raw.start.clone());
        let end = collapsed_into.get(&raw.end).cloned().unwrap_or_else(|| raw.end.clone());
        if start == end && (collapsed_into.contains_key(&raw.start) || collapsed_into.contains_key(&raw.end)) {
            continue;
        }
        let (arrow_start, arrow_end) = arrows(raw.kind.as_deref());
        let mut styles: Vec<String> = model.default_edge_style.clone().unwrap_or_default();
        if let Some(s) = &raw.style {
            styles.extend(s.iter().cloned());
        }
        let plain = raw.stroke.as_deref() == Some("invisible") || raw.kind.as_deref() == Some("arrow_open");
        let non_empty = |s: &Option<String>| s.clone().filter(|v| !v.is_empty());
        edges.push(GraphEdge {
            id: if raw.id.is_empty() { format!("L_{}_{}_{}", start, end, index) } else { raw.id.clone() },
            start,
            end,
            label: raw.text.clone(),
            label_type: raw.label_type,
            thickness: raw.stroke.clone(),
            minlen: raw.length,
            classes: if raw.stroke.as_deref() == Some("invisible") {
                String::new()
            } else {
                "edge-thickness-normal edge-pattern-solid flowchart-link".into()
            },
            arrow_type_start: if plain { "none".into() } else { arrow_start },
            arrow_type_end: if plain { "none".into() } else { arrow_end },
            css_compiled_styles: compiled(&raw.classes),
            style: styles,
            curve: non_empty(&raw.interpolate).or_else(|| non_empty(&model.default_interpolate)).or_else(|| default_curve.map(String::from)),
        });
    }

    FlowGraph { nodes, edges }
}
