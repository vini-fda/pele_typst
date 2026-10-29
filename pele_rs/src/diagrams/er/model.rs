use indexmap::IndexMap;
use std::collections::HashSet;
#[derive(Debug, Clone, Default)]
pub struct Attribute {
    pub kind: String,
    pub name: String,
    pub keys: Vec<String>,
    pub comment: String,
}
#[derive(Debug, Clone)]
pub struct Entity {
    pub id: String,
    pub label: String,
    pub alias: String,
    pub attributes: Vec<Attribute>,
    pub css_classes: String,
    pub css_styles: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct Relationship {
    pub entity_a: String,
    pub role_a: String,
    pub entity_b: String,
    pub card_a: String,
    pub card_b: String,
    pub identifying: bool,
}
#[derive(Debug, Clone, Default)]
pub struct ClassDef {
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct Subgraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub css_styles: Vec<String>,
    pub dir: Option<String>,
}
#[derive(Debug, Clone)]
pub enum DocItem {
    Name(String),
    Dir(String),
}
#[derive(Debug, Clone)]
pub struct Model {
    pub direction: String,
    pub entities: IndexMap<String, Entity>,
    pub relationships: Vec<Relationship>,
    pub classes: IndexMap<String, ClassDef>,
    pub subgraphs: Vec<Subgraph>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    claimed: HashSet<String>,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            direction: "TB".into(),
            entities: IndexMap::new(),
            relationships: vec![],
            classes: IndexMap::new(),
            subgraphs: vec![],
            title: None,
            acc_title: None,
            acc_descr: None,
            claimed: HashSet::new(),
        }
    }
}
impl Model {
    pub fn add_entity(&mut self, name: &str, alias: &str) -> &mut Entity {
        let count = self.entities.len();
        let e = self.entities.entry(name.into()).or_insert_with(|| Entity {
            id: format!("entity-{name}-{count}"),
            label: name.into(),
            alias: alias.into(),
            attributes: vec![],
            css_classes: "default".into(),
            css_styles: vec![],
        });
        if e.alias.is_empty() && !alias.is_empty() {
            e.alias = alias.into();
        }
        e
    }
    pub fn add_relationship(&mut self, a: &str, role: String, b: &str, card_a: String, card_b: String, identifying: bool) {
        let entity_a = if self.subgraphs.iter().any(|s| s.id == a) {
            a.into()
        } else {
            self.add_entity(a, "").id.clone()
        };
        let entity_b = if self.subgraphs.iter().any(|s| s.id == b) {
            b.into()
        } else {
            self.add_entity(b, "").id.clone()
        };
        self.relationships.push(Relationship {
            entity_a,
            role_a: role,
            entity_b,
            card_a,
            card_b,
            identifying,
        });
    }
    pub fn set_class(&mut self, ids: &[String], names: &[String]) {
        for id in ids {
            if let Some(e) = self.entities.get_mut(id) {
                for n in names {
                    e.css_classes.push(' ');
                    e.css_classes.push_str(n);
                }
            }
            if let Some(s) = self.subgraphs.iter_mut().rev().find(|s| s.id == *id) {
                s.classes.extend_from_slice(names);
            }
        }
    }
    pub fn add_styles(&mut self, ids: &[String], styles: &[String]) {
        for id in ids {
            if let Some(e) = self.entities.get_mut(id) {
                e.css_styles.extend_from_slice(styles);
            }
            if let Some(s) = self.subgraphs.iter_mut().rev().find(|s| s.id == *id) {
                s.css_styles.extend_from_slice(styles);
            }
        }
    }
    pub fn add_class(&mut self, ids: &[String], styles: &[String]) {
        for id in ids {
            let d = self.classes.entry(id.clone()).or_default();
            for s in styles {
                if s.contains("color") {
                    d.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                d.styles.push(s.clone());
            }
        }
    }
    pub fn add_subgraph(&mut self, id: &str, list: Vec<DocItem>, title: &str) -> String {
        let id = id.trim().to_owned();
        let mut seen = HashSet::new();
        let mut nodes = vec![];
        let mut dir = None;
        for item in list {
            match item {
                DocItem::Dir(d) => dir = Some(d),
                DocItem::Name(name) => {
                    let t = name.trim();
                    if t.is_empty() || !seen.insert(t.to_owned()) {
                        continue;
                    }
                    if !self.claimed.contains(&name) {
                        nodes.push(name);
                    }
                }
            }
        }
        self.claimed.extend(nodes.iter().cloned());
        self.subgraphs.push(Subgraph {
            id: id.clone(),
            nodes,
            title: title.trim().into(),
            classes: vec![],
            css_styles: vec![],
            dir,
        });
        id
    }
}
#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub alias: String,
    pub attributes: Vec<Attribute>,
    pub is_group: bool,
    pub parent_id: Option<String>,
    pub css_styles: Vec<String>,
    pub css_compiled_styles: Vec<String>,
    pub css_classes: String,
    pub dir: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Edge {
    pub id: String,
    pub start: String,
    pub end: String,
    pub label: String,
    pub start_type: String,
    pub end_type: String,
    pub dashed: bool,
}
pub fn graph(model: &Model) -> (Vec<Node>, Vec<Edge>) {
    let mut nodes = vec![];
    let mut parent = IndexMap::new();
    let mut groups = HashSet::new();
    let compiled = |names: Vec<&str>| -> Vec<String> {
        let mut v = vec![];
        for n in names {
            if let Some(d) = model.classes.get(n) {
                v.extend(d.styles.iter().chain(d.text_styles.iter()).map(|s| s.trim().into()));
            }
        }
        v
    };
    for s in model.subgraphs.iter().rev() {
        groups.insert(s.id.clone());
        for n in &s.nodes {
            parent.insert(n.clone(), s.id.clone());
        }
    }
    for s in model.subgraphs.iter().rev() {
        nodes.push(Node {
            id: s.id.clone(),
            label: s.title.clone(),
            alias: String::new(),
            attributes: vec![],
            is_group: true,
            parent_id: parent.get(&s.id).cloned(),
            css_styles: s.css_styles.clone(),
            css_compiled_styles: compiled(s.classes.iter().map(String::as_str).collect()),
            css_classes: s.classes.join(" "),
            dir: s.dir.clone(),
        });
    }
    let mut moved = IndexMap::new();
    for (name, e) in &model.entities {
        if groups.contains(name) {
            moved.insert(e.id.clone(), name.clone());
            continue;
        }
        nodes.push(Node {
            id: e.id.clone(),
            label: e.label.clone(),
            alias: e.alias.clone(),
            attributes: e.attributes.clone(),
            is_group: false,
            parent_id: parent.get(name).cloned(),
            css_styles: e.css_styles.clone(),
            css_compiled_styles: compiled(e.css_classes.split(' ').collect()),
            css_classes: e.css_classes.clone(),
            dir: None,
        });
    }
    let edges = model
        .relationships
        .iter()
        .enumerate()
        .map(|(i, r)| Edge {
            id: format!("id_{}_{}_{}", r.entity_a, r.entity_b, i),
            start: moved.get(&r.entity_a).unwrap_or(&r.entity_a).clone(),
            end: moved.get(&r.entity_b).unwrap_or(&r.entity_b).clone(),
            label: r.role_a.clone(),
            start_type: r.card_b.to_ascii_lowercase(),
            end_type: r.card_a.to_ascii_lowercase(),
            dashed: !r.identifying,
        })
        .collect();
    (nodes, edges)
}
