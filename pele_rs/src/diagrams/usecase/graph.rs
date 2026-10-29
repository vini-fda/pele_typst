use super::json::Value;
use super::*;
use crate::util::trim;
#[derive(Clone, Debug, Default)]
pub struct GNode {
    pub id: String,
    pub label: String,
    pub markdown: bool,
    pub shape: String,
    pub group: bool,
    pub padding: f64,
    pub classes: String,
    pub styles: Vec<String>,
    pub compiled: Vec<String>,
    pub actor_type: Option<String>,
    pub business: bool,
    pub icon: Option<String>,
    pub stereotype: Option<String>,
    pub parent: Option<String>,
    pub rows: Option<Vec<(String, String)>>,
    pub boundary_type: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct GEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub kind: String,
    pub pattern: String,
    pub start_marker: String,
    pub end_marker: String,
    pub label: Option<String>,
    pub markdown: bool,
    pub classes: String,
    pub styles: Vec<String>,
    pub compiled: Vec<String>,
    pub animate: bool,
    pub minlen: f64,
    pub internal: bool,
}
pub struct Graph {
    pub nodes: Vec<GNode>,
    pub edges: Vec<GEdge>,
}
fn flatten(data: &super::json::Data) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![(data.root, String::new())];
    while let Some((i, path)) = stack.pop() {
        match &data.values[i] {
            Value::Array(arr) => {
                if arr.is_empty() {
                    out.push((path, "[]".into()))
                } else if arr.iter().all(|&i| matches!(data.values[i], Value::Scalar(_))) {
                    for (j, &i) in arr.iter().enumerate() {
                        if let Value::Scalar(v) = &data.values[i] {
                            out.push((if j == 0 { path.clone() } else { String::new() }, v.clone()))
                        }
                    }
                } else {
                    for (j, &i) in arr.iter().enumerate().rev() {
                        stack.push((i, format!("{path}[{j}]")))
                    }
                }
            }
            Value::Object(map) => {
                if map.is_empty() {
                    out.push((path, "{}".into()))
                } else {
                    for (k, &i) in map.iter().rev() {
                        stack.push((i, if path.is_empty() { k.clone() } else { format!("{path}.{k}") }))
                    }
                }
            }
            Value::Scalar(v) => out.push((path, v.clone())),
        }
    }
    out
}
fn classes(base: &str, extra: &[String]) -> String {
    std::iter::once(base)
        .chain(extra.iter().map(String::as_str).filter(|s| !s.is_empty()))
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn build(m: &Model) -> Graph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let compiled = |names: &[String]| {
        let mut out: IndexMap<String, String> = IndexMap::new();
        for name in std::iter::once("default").chain(names.iter().map(String::as_str)) {
            if let Some(def) = m.class_defs.get(name) {
                for raw in def {
                    let style = trim(raw);
                    let prop = trim(style.split(':').next().unwrap());
                    if !prop.is_empty() {
                        out.insert(prop.into(), style.into());
                    }
                }
            }
        }
        out.into_values().collect::<Vec<_>>()
    };
    for n in m.actors.values() {
        let base = format!(
            "default usecase-actor usecase-actor-{}{}",
            n.actor_type,
            if n.business { " usecase-business" } else { "" }
        );
        nodes.push(GNode {
            id: n.id.clone(),
            label: n.label.clone(),
            markdown: n.markdown,
            shape: match n.actor_type.as_str() {
                "hollow" => "usecaseActorHollow",
                "awesome" => "usecaseActorAwesome",
                "icon" => "usecaseActorIcon",
                _ => "usecaseActor",
            }
            .into(),
            padding: 10.0,
            classes: classes(&base, &n.classes),
            styles: n.styles.clone(),
            compiled: compiled(&n.classes),
            actor_type: Some(n.actor_type.clone()),
            business: n.business,
            icon: n.icon.clone(),
            stereotype: n.stereotype.clone(),
            parent: n.parent.clone(),
            ..Default::default()
        })
    }
    for n in m.usecases.values() {
        let ellipse = n.shape == "ellipse";
        let base = format!(
            "default usecase-element usecase-{}{}",
            n.shape,
            if n.business { " usecase-business" } else { "" }
        );
        nodes.push(GNode {
            id: n.id.clone(),
            label: n.label.clone(),
            markdown: n.markdown,
            shape: if !ellipse {
                "rect"
            } else if n.business {
                "usecaseBusiness"
            } else {
                "usecaseEllipse"
            }
            .into(),
            padding: if ellipse { 20.0 } else { 10.0 },
            classes: classes(&base, &n.classes),
            styles: n.styles.clone(),
            compiled: compiled(&n.classes),
            business: n.business,
            stereotype: n.stereotype.clone(),
            parent: n.parent.clone(),
            ..Default::default()
        })
    }
    for n in &m.notes {
        nodes.push(GNode {
            id: n.id.clone(),
            label: n.label.text.clone(),
            markdown: n.label.markdown,
            shape: "note".into(),
            padding: 10.0,
            classes: "default usecase-note".into(),
            compiled: compiled(&[]),
            ..Default::default()
        })
    }
    for n in m.jsons.values() {
        nodes.push(GNode {
            id: n.id.clone(),
            label: n.id.clone(),
            shape: "usecaseJsonTable".into(),
            padding: 10.0,
            classes: classes("default usecase-json-table", &n.classes),
            styles: n.styles.clone(),
            compiled: compiled(&n.classes),
            rows: Some(flatten(&n.value)),
            ..Default::default()
        })
    }
    for n in m.boundaries.values() {
        nodes.push(GNode {
            id: n.id.clone(),
            label: n.label.clone(),
            markdown: n.markdown,
            shape: "usecaseSystemBoundary".into(),
            group: true,
            padding: 20.0,
            classes: classes(&format!("default system-boundary system-boundary-{}", n.kind), &n.classes),
            styles: n.styles.clone(),
            compiled: compiled(&n.classes),
            boundary_type: Some(n.kind.clone()),
            ..Default::default()
        })
    }
    const MARKERS: [(&str, &str); 7] = [
        ("none", "arrow_point"),
        ("arrow_point", "none"),
        ("none", "none"),
        ("none", "arrow_circle"),
        ("none", "arrow_cross"),
        ("arrow_circle", "none"),
        ("arrow_cross", "none"),
    ];
    for r in &m.relationships {
        let semantic = matches!(r.kind.as_str(), "include" | "extend");
        let (a, b) = if r.kind == "generalization" {
            ("none", "extension")
        } else if r.kind != "association" {
            MARKERS[0]
        } else {
            MARKERS.get(r.arrow).copied().unwrap_or(MARKERS[2])
        };
        let mut cn = classes(&format!("default relationship relationship-{}", r.kind), &r.classes);
        if r.animate || r.animation.is_some() {
            cn += &format!(" edge-animation-{}", r.animation.as_deref().unwrap_or("fast"))
        }
        edges.push(GEdge {
            id: r.id.clone(),
            start: r.source.clone(),
            end: r.target.clone(),
            kind: r.kind.clone(),
            pattern: if semantic { "dotted" } else { "solid" }.into(),
            start_marker: a.into(),
            end_marker: b.into(),
            label: if semantic {
                Some(r.kind.clone())
            } else if r.kind == "association" {
                r.label.as_ref().map(|l| l.text.clone()).filter(|s| !s.is_empty())
            } else {
                None
            },
            markdown: r.kind == "association" && r.label.as_ref().is_some_and(|l| l.markdown),
            classes: cn,
            styles: r.styles.clone(),
            compiled: compiled(&r.classes),
            animate: r.animate,
            minlen: r.minlen,
            internal: false,
        })
    }
    for n in &m.notes {
        edges.push(GEdge {
            id: format!("{}-edge", n.id),
            start: n.id.clone(),
            end: n.target.clone(),
            kind: "note".into(),
            pattern: "dotted".into(),
            start_marker: "none".into(),
            end_marker: "none".into(),
            classes: "default relationship relationship-note".into(),
            compiled: compiled(&[]),
            minlen: 1.0,
            internal: true,
            ..Default::default()
        })
    }
    Graph { nodes, edges }
}
