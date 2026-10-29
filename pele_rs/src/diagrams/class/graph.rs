use super::model::{Model, Node};
use std::collections::HashMap;
#[derive(Clone, Debug)]
pub struct GraphNode<'a> {
    pub id: String,
    pub label: String,
    pub is_group: bool,
    pub shape: &'static str,
    pub parent: i32,
    pub class_node: Option<&'a Node>,
}
#[derive(Clone, Debug)]
pub struct Edge {
    pub id: String,
    pub source: i32,
    pub target: i32,
    pub label: Option<String>,
    pub start_marker: &'static str,
    pub end_marker: &'static str,
    pub start_label: String,
    pub end_label: String,
    pub pattern: &'static str,
}
pub struct Graph<'a> {
    pub nodes: Vec<GraphNode<'a>>,
    pub edges: Vec<Edge>,
}
fn marker(t: i32) -> &'static str {
    match t {
        0 => "aggregation",
        1 => "extension",
        2 => "composition",
        3 => "dependency",
        4 => "lollipop",
        _ => "none",
    }
}
pub fn graph(model: &Model, hierarchical: bool) -> Graph<'_> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let (mut ns_at, mut class_at, mut interface_at) = (HashMap::new(), HashMap::new(), HashMap::new());
    for ns in model.namespaces.values() {
        if !hierarchical && !ns.explicit {
            continue;
        }
        ns_at.insert(ns.id.as_str(), nodes.len() as i32);
        nodes.push(GraphNode {
            id: ns.id.clone(),
            label: if hierarchical { ns.label.clone() } else { ns.id.clone() },
            is_group: true,
            shape: "rect",
            parent: -1,
            class_node: None,
        });
    }
    for node in &mut nodes {
        node.parent = if hierarchical {
            model.namespaces[&node.id]
                .parent
                .as_deref()
                .and_then(|id| ns_at.get(id).copied())
                .unwrap_or(-1)
        } else {
            -1
        };
    }
    let parent = |id: Option<&str>| {
        let mut cur = id;
        if !hierarchical {
            while let Some(id) = cur {
                let Some(ns) = model.namespaces.get(id) else { break };
                if ns.explicit {
                    break;
                }
                cur = ns.parent.as_deref();
            }
        }
        cur.and_then(|id| ns_at.get(id).copied()).unwrap_or(-1)
    };
    for node in model.classes.values() {
        class_at.insert(node.id.as_str(), nodes.len() as i32);
        nodes.push(GraphNode {
            id: node.id.clone(),
            label: node.label.clone(),
            is_group: false,
            shape: "classBox",
            parent: parent(node.parent.as_deref()),
            class_node: Some(node),
        });
    }
    for note in model.notes.values() {
        let target = note.class.as_deref().and_then(|id| class_at.get(id).copied());
        if let Some(target) = target {
            edges.push(Edge {
                id: format!("edgeNote{}", note.index),

                source: nodes.len() as i32,
                target,
                label: None,
                start_marker: "none",
                end_marker: "none",
                start_label: String::new(),
                end_label: String::new(),
                pattern: "dotted",
            });
        }
        nodes.push(GraphNode {
            id: note.id.clone(),
            label: note.text.clone(),
            is_group: false,
            shape: "note",
            parent: parent(note.parent.as_deref()),
            class_node: None,
        });
    }
    for item in &model.interfaces {
        let owner = class_at.get(item.class_id.as_str()).copied();
        interface_at.insert(item.id.as_str(), nodes.len() as i32);
        nodes.push(GraphNode {
            id: item.id.clone(),
            label: item.label.clone(),
            is_group: false,
            shape: "interface",
            parent: owner.map_or(-1, |i| nodes[i as usize].parent),
            class_node: None,
        });
    }
    for (i, r) in model.relations.iter().enumerate() {
        let start = if r.type1 == 4 && r.type2 == -1 { &interface_at } else { &class_at };
        let end = if r.type2 == 4 && r.type1 == -1 { &interface_at } else { &class_at };
        edges.push(Edge {
            id: format!("id_{}_{}_{}", r.id1, r.id2, i + 1),
            source: start.get(r.id1.as_str()).copied().unwrap_or(-1),
            target: end.get(r.id2.as_str()).copied().unwrap_or(-1),
            label: r.title.clone(),
            start_marker: marker(r.type1),
            end_marker: marker(r.type2),
            start_label: if r.title1 == "none" { String::new() } else { r.title1.clone() },
            end_label: if r.title2 == "none" { String::new() } else { r.title2.clone() },
            pattern: if r.line_type == 1 { "dashed" } else { "solid" },
        });
    }
    Graph { nodes, edges }
}
