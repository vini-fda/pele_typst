use super::model::{Class, State, Stmt, doc_direction};
use crate::{PeleError, errors::PeleErrorCode};
use indexmap::IndexMap;
#[derive(Clone, Debug, Default)]
pub struct Node {
    pub id: String,
    pub shape: Option<String>,
    pub label: Option<String>,
    pub description: Vec<String>,
    pub css_classes: String,
    pub compiled_styles: Vec<String>,
    pub styles: Vec<String>,
    pub kind: Option<String>,
    pub is_group: bool,
    pub parent_id: Option<String>,
    pub dir: Option<String>,
    pub position: Option<String>,
    pub note_for: Option<String>,
    pub color_index: Option<usize>,
}
#[derive(Clone, Debug)]
pub struct Edge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub label: String,
    pub classes: String,
    pub dashed: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}
struct Draft {
    shape: Option<String>,
    description: Vec<String>,
    array: bool,
    css_classes: String,
    styles: Vec<String>,
    kind: Option<String>,
    dir: Option<String>,
    color_index: Option<usize>,
}
struct Builder<'a> {
    states: &'a IndexMap<String, State>,
    classes: &'a IndexMap<String, Class>,
    nodes: IndexMap<String, Node>,
    edges: Vec<Edge>,
    drafts: IndexMap<String, Draft>,
    slots: IndexMap<String, Option<usize>>,
    counter: usize,
    next_slot: usize,
}
impl Builder<'_> {
    fn insert(&mut self, mut node: Node) {
        if node.id.is_empty() || node.id == "</join></fork>" || node.id == "</choice>" {
            return;
        }
        if !node.css_classes.is_empty() {
            for name in node.css_classes.split(' ') {
                if let Some(def) = self.classes.get(name) {
                    node.compiled_styles.extend(def.styles.clone());
                }
            }
        }
        if let Some(prior) = self.nodes.get(&node.id)
            && node.parent_id.is_none()
        {
            node.parent_id = prior.parent_id.clone();
        }
        self.nodes.insert(node.id.clone(), node);
    }
    fn visit(&mut self, parent: Option<&State>, item: &State, alt: bool) {
        let id = &item.id;
        if id == "root" {
            return;
        }
        let known = self.states.get(id);
        let class_str = known.map_or(String::new(), |s| s.classes.join(" "));
        let styles = known.map_or(Vec::new(), |s| s.styles.clone());
        let draft = self.drafts.entry(id.clone()).or_insert_with(|| {
            let mut shape = Some(
                if item.start == Some(true) {
                    "stateStart"
                } else if item.start == Some(false) {
                    "stateEnd"
                } else {
                    "rect"
                }
                .into(),
            );
            if item.kind.as_deref() != Some("default") {
                shape = item.kind.clone();
            }
            Draft {
                shape,
                description: vec![id.clone()],
                array: false,
                css_classes: format!("{class_str} statediagram-state"),
                styles: styles.clone(),
                kind: None,
                dir: None,
                color_index: None,
            }
        });
        let described = item.description_array || item.description.first().is_some_and(|s| !s.is_empty());
        if described {
            if draft.array {
                draft.shape = Some("rectWithTitle".into());
                draft.description.extend(item.description.clone());
            } else if draft.description.first().is_some_and(|s| !s.is_empty()) {
                draft.shape = Some("rectWithTitle".into());
                if draft.description[0] == *id {
                    draft.description = item.description.clone();
                } else {
                    draft.description.extend(item.description.clone());
                }
                draft.array = true;
            } else {
                draft.shape = Some("rect".into());
                draft.description = item.description.clone();
                draft.array = item.description_array;
            }
        }
        let len = if draft.array {
            draft.description.len()
        } else {
            draft.description.first().map_or(0, |s| s.encode_utf16().count())
        };
        if len == 1 && draft.shape.as_deref() == Some("rectWithTitle") {
            draft.shape = Some(
                if draft.kind.as_deref() == Some("group") {
                    "roundedWithTitle"
                } else {
                    "rect"
                }
                .into(),
            );
        }
        if draft.kind.is_none()
            && let Some(doc) = &item.doc
        {
            draft.kind = Some("group".into());
            draft.dir = Some(doc_direction(doc).into());
            draft.shape = Some(
                if item.kind.as_deref() == Some("divider") {
                    "divider"
                } else {
                    "roundedWithTitle"
                }
                .into(),
            );
            if draft.shape.as_deref() == Some("divider") && parent.is_some_and(|p| self.slots.contains_key(&p.id)) {
                draft.color_index = self.slots[&parent.unwrap().id];
            } else {
                let slot = self.next_slot;
                self.next_slot += 1;
                draft.color_index = if !class_str.trim().is_empty() || !styles.is_empty() {
                    None
                } else {
                    Some(slot)
                };
            }
            self.slots.insert(id.clone(), draft.color_index);
            draft.css_classes += &format!(" statediagram-cluster {}", if alt { "statediagram-cluster-alt" } else { "" });
        }
        let mut node = Node {
            id: id.clone(),
            shape: draft.shape.clone(),
            label: draft.description.first().cloned(),
            description: if draft.array {
                draft.description.iter().skip(1).cloned().collect()
            } else {
                vec![]
            },
            css_classes: draft.css_classes.clone(),
            compiled_styles: vec![],
            styles: draft.styles.clone(),
            kind: draft.kind.clone(),
            is_group: draft.kind.as_deref() == Some("group"),
            dir: draft.dir.clone(),
            color_index: draft.color_index,
            ..Default::default()
        };
        if node.shape.as_deref() == Some("divider") {
            node.label = Some(String::new());
        }
        if let Some(parent) = parent.filter(|p| p.id != "root") {
            node.parent_id = Some(parent.id.clone());
        }
        if let Some(note) = &item.note {
            let classes = draft.css_classes.clone();
            let aside = |id: String, shape: &str, css_classes: String, is_group: bool| Node {
                id,
                shape: Some(shape.into()),
                label: Some(note.text.clone()),
                css_classes,
                kind: Some(if is_group { "group" } else { "node" }.into()),
                is_group,
                position: note.position.clone(),
                ..Default::default()
            };
            let mut note_node = aside(format!("{id}----note-{}", self.counter), "note", "statediagram-note".into(), false);
            self.counter += 1;
            let parent_id = format!("{id}----parent");
            note_node.parent_id = Some(parent_id.clone());
            note_node.note_for = Some(id.clone());
            let note_id = note_node.id.clone();
            self.insert(aside(parent_id, "noteGroup", classes, true));
            self.insert(note_node);
            self.insert(node);
            let left = note.position.as_deref() == Some("left of");
            let (from, to) = if left { (note_id, id.clone()) } else { (id.clone(), note_id) };
            self.edges.push(Edge {
                id: format!("{from}-{to}"),
                start: from,
                end: to,
                label: String::new(),
                classes: "transition note-edge".into(),
                dashed: true,
            });
        } else {
            self.insert(node);
        }
    }
}
pub fn build(root: &[Stmt], states: &IndexMap<String, State>, classes: &IndexMap<String, Class>) -> Result<Graph, PeleError> {
    let mut b = Builder {
        states,
        classes,
        nodes: IndexMap::new(),
        edges: vec![],
        drafts: IndexMap::new(),
        slots: IndexMap::new(),
        counter: 0,
        next_slot: 0,
    };
    let mut pending: Vec<(Option<&State>, &Stmt, bool)> = root.iter().rev().map(|item| (None, item, true)).collect();
    while let Some((parent, item, alt)) = pending.pop() {
        match item {
            Stmt::State(state) => {
                b.visit(parent, state, alt);
                if let Some(doc) = &state.doc {
                    for item in doc.iter().rev() {
                        pending.push((Some(state), item, !alt));
                    }
                }
            }
            Stmt::Relation {
                state1,
                state2,
                description,
            } => {
                b.visit(parent, state1, alt);
                b.visit(parent, state2, alt);
                b.edges.push(Edge {
                    id: format!("edge{}", b.counter),
                    start: state1.id.clone(),
                    end: state2.id.clone(),
                    label: description.clone().unwrap_or_default(),
                    classes: "transition".into(),
                    dashed: false,
                });
                b.counter += 1;
            }
            _ => {}
        }
    }
    let nodes: Vec<_> = b.nodes.into_values().collect();
    for node in &nodes {
        if node.is_group && !node.description.is_empty() {
            return Err(PeleError::new(
                format!(
                    "Group nodes can only have label. Remove the additional description for node [{}]",
                    node.id
                ),
                PeleErrorCode::Semantic,
            )
            .of("state"));
        }
    }
    Ok(Graph { nodes, edges: b.edges })
}
