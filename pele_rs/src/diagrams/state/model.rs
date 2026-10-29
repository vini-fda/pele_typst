use crate::PeleError;
use indexmap::IndexMap;
#[derive(Clone, Debug, Default)]
pub struct Note {
    pub position: Option<String>,
    pub text: String,
}
#[derive(Debug, Default)]
pub struct State {
    pub id: String,
    pub kind: Option<String>,
    pub description: Vec<String>,
    pub description_array: bool,
    pub descriptions: Vec<String>,
    pub doc: Option<Document>,
    pub note: Option<Note>,
    pub start: Option<bool>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

// Nested state documents own their children, but clone and release them on heap
// stacks so the public model supports the upstream deep-composite cases.
#[derive(Debug, Default)]
pub struct Document(pub Vec<Stmt>);
impl std::ops::Deref for Document {
    type Target = Vec<Stmt>;
    fn deref(&self) -> &Vec<Stmt> {
        &self.0
    }
}
impl std::ops::DerefMut for Document {
    fn deref_mut(&mut self) -> &mut Vec<Stmt> {
        &mut self.0
    }
}
impl From<Vec<Stmt>> for Document {
    fn from(value: Vec<Stmt>) -> Self {
        Self(value)
    }
}
impl Document {
    pub fn into_vec(mut self) -> Vec<Stmt> {
        std::mem::take(&mut self.0)
    }
}
impl State {
    fn shallow_clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            kind: self.kind.clone(),
            description: self.description.clone(),
            description_array: self.description_array,
            descriptions: self.descriptions.clone(),
            doc: None,
            note: self.note.clone(),
            start: self.start,
            classes: self.classes.clone(),
            styles: self.styles.clone(),
            text_styles: self.text_styles.clone(),
        }
    }
}
fn clone_document(source: &[Stmt]) -> Document {
    let mut result = Document::default();
    let mut pending = vec![(source, &mut result)];
    while let Some((source, target)) = pending.pop() {
        target.0 = source
            .iter()
            .map(|item| match item {
                Stmt::State(state) => Stmt::State(Box::new(state.shallow_clone())),
                Stmt::Relation {
                    state1,
                    state2,
                    description,
                } => Stmt::Relation {
                    state1: Box::new(state1.shallow_clone()),
                    state2: Box::new(state2.shallow_clone()),
                    description: description.clone(),
                },
                other => other.clone(),
            })
            .collect();
        for (source, target) in source.iter().zip(target.0.iter_mut()) {
            let states = match (source, target) {
                (Stmt::State(source), Stmt::State(target)) => vec![(source, target)],
                (Stmt::Relation { state1: a, state2: b, .. }, Stmt::Relation { state1: c, state2: d, .. }) => vec![(a, c), (b, d)],
                _ => vec![],
            };
            for (source, target) in states {
                if let Some(doc) = &source.doc {
                    target.doc = Some(Document::default());
                    pending.push((doc.as_slice(), target.doc.as_mut().unwrap()));
                }
            }
        }
    }
    result
}
impl Clone for State {
    fn clone(&self) -> Self {
        let mut out = self.shallow_clone();
        out.doc = self.doc.as_ref().map(|doc| clone_document(doc));
        out
    }
}
impl Clone for Document {
    fn clone(&self) -> Self {
        clone_document(self)
    }
}
impl Drop for Document {
    fn drop(&mut self) {
        let mut pending = std::mem::take(&mut self.0);
        while let Some(mut item) = pending.pop() {
            match &mut item {
                Stmt::State(state) => {
                    if let Some(doc) = state.doc.take() {
                        pending.extend(doc.into_vec());
                    }
                }
                Stmt::Relation { state1, state2, .. } => {
                    if let Some(doc) = state1.doc.take() {
                        pending.extend(doc.into_vec());
                    }
                    if let Some(doc) = state2.doc.take() {
                        pending.extend(doc.into_vec());
                    }
                }
                _ => {}
            }
        }
    }
}
#[derive(Clone, Debug)]
pub enum Stmt {
    Text(String),
    State(Box<State>),
    Relation {
        state1: Box<State>,
        state2: Box<State>,
        description: Option<String>,
    },
    ClassDef {
        id: String,
        classes: String,
    },
    Style {
        id: String,
        style_class: String,
    },
    ApplyClass {
        id: String,
        style_class: String,
    },
    Direction(String),
    Click {
        id: String,
        url: String,
        tooltip: String,
    },
}
#[derive(Clone, Debug)]
pub struct Relation {
    pub id1: String,
    pub id2: String,
    pub title: String,
}
#[derive(Clone, Debug, Default)]
pub struct Class {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Link {
    pub url: String,
    pub tooltip: String,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub root_doc: Vec<Stmt>,
    pub states: IndexMap<String, State>,
    pub relations: Vec<Relation>,
    pub classes: IndexMap<String, Class>,
    pub links: IndexMap<String, Link>,
    pub graph: super::graph::Graph,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub divider_count: usize,
}
impl Model {
    pub fn direction(&self) -> &str {
        doc_direction(&self.root_doc)
    }
    pub fn divider(&mut self) -> String {
        self.divider_count += 1;
        format!("divider-id-{}", self.divider_count)
    }
    pub fn set_root(&mut self, doc: Vec<Stmt>) -> Result<(), PeleError> {
        self.root_doc = doc;
        self.translate();
        self.extract()?;
        Ok(())
    }
    pub fn set_descr(&mut self, text: String) {
        self.acc_descr = Some(regex::Regex::new(r"\n\s+").unwrap().replace_all(&text, "\n").into_owned());
    }
    fn translate(&mut self) {
        let mut root = Stmt::State(Box::new(State {
            id: "root".into(),
            doc: Some(std::mem::take(&mut self.root_doc).into()),
            ..Default::default()
        }));
        let mut pending = vec![("root".to_owned(), &mut root, true)];
        while let Some((parent, item, first)) = pending.pop() {
            match item {
                Stmt::Relation { state1, state2, .. } => {
                    for (node, first) in [(state2, false), (state1, true)] {
                        if node.id == "[*]" {
                            node.id = format!("{parent}_{}", if first { "start" } else { "end" });
                            node.start = Some(first);
                        } else {
                            node.id = node.id.trim().into();
                        }
                    }
                }
                Stmt::State(node) => {
                    if node.id == "[*]" {
                        node.id = format!("{parent}_{}", if first { "start" } else { "end" });
                        node.start = Some(first);
                    } else {
                        node.id = node.id.trim().into();
                    }
                    let Some(doc) = &mut node.doc else { continue };
                    let mut regions = Vec::new();
                    let mut current = Vec::new();
                    for item in std::mem::take(doc).into_vec() {
                        match item {
                            Stmt::State(mut state) if state.kind.as_deref() == Some("divider") && state.doc.is_none() => {
                                state.doc = Some(std::mem::take(&mut current).into());
                                regions.push(Stmt::State(state));
                            }
                            item => current.push(item),
                        }
                    }
                    if regions.is_empty() {
                        *doc = current.into();
                    } else {
                        if !current.is_empty() {
                            self.divider_count += 1;
                            regions.push(Stmt::State(Box::new(State {
                                id: format!("divider-id-{}", self.divider_count),
                                kind: Some("divider".into()),
                                doc: Some(current.into()),
                                ..Default::default()
                            })));
                        }
                        *doc = regions.into();
                    }
                    let id = node.id.clone();
                    for item in doc.iter_mut().rev() {
                        pending.push((id.clone(), item, true));
                    }
                }
                _ => {}
            }
        }
        if let Stmt::State(mut root) = root {
            self.root_doc = root.doc.take().unwrap().into_vec();
        }
    }
    fn add_state(&mut self, item: &State, classes: bool) {
        let id = item.id.trim().to_owned();
        let found = self.states.contains_key(&id);
        let state = self.states.entry(id.clone()).or_insert_with(|| State {
            id: id.clone(),
            kind: Some(item.kind.clone().unwrap_or_else(|| "default".into())),
            doc: item.doc.clone(),
            note: item.note.clone(),
            ..Default::default()
        });
        if found {
            if state.doc.is_none() {
                state.doc = item.doc.clone();
            }
            if state.kind.is_none() {
                state.kind = Some(item.kind.clone().unwrap_or_else(|| "default".into()));
            }
        }
        for text in &item.description {
            let text = text.trim();
            state.descriptions.push(if let Some(rest) = text.strip_prefix(':') {
                rest.trim().into()
            } else {
                text.into()
            });
        }
        if item.note.is_some() {
            state.note = item.note.clone();
        }
        if classes {
            for class in &item.classes {
                state.classes.push(class.trim().into());
            }
        }
    }
    fn named(&mut self, ids: &str) -> Vec<String> {
        let mut out = vec![];
        for id in ids.split(',') {
            let key = if self.states.contains_key(id) {
                id.to_owned()
            } else {
                self.add_state(
                    &State {
                        id: id.into(),
                        ..Default::default()
                    },
                    false,
                );
                id.trim().into()
            };
            if self.states.contains_key(&key) {
                out.push(key);
            }
        }
        out
    }
    fn extract(&mut self) -> Result<(), PeleError> {
        self.states.clear();
        self.relations.clear();
        self.classes.clear();
        for item in self.root_doc.clone() {
            match item {
                Stmt::State(state) => self.add_state(&state, false),
                Stmt::Relation {
                    state1,
                    state2,
                    description,
                } => {
                    self.add_state(&state1, true);
                    self.add_state(&state2, true);
                    self.relations.push(Relation {
                        id1: state1.id.trim().into(),
                        id2: state2.id.trim().into(),
                        title: description.unwrap_or_default(),
                    });
                }
                Stmt::ClassDef { id, classes } => {
                    let id = id.trim().to_owned();
                    let class = self.classes.entry(id.clone()).or_insert_with(|| Class { id, ..Default::default() });
                    if !classes.is_empty() {
                        for attr in classes.split(',') {
                            let fixed = attr.replacen(';', "", 1).trim().to_owned();
                            if attr.contains("color") {
                                class
                                    .text_styles
                                    .push(fixed.replacen("fill", "bgFill", 1).replacen("color", "fill", 1));
                            }
                            class.styles.push(fixed);
                        }
                    }
                }
                Stmt::Style { id, style_class } => {
                    let styles: Vec<_> = style_class.split(',').map(|s| s.replace(';', "").trim().to_owned()).collect();
                    for key in self.named(id.trim()) {
                        self.states[&key].styles = styles.clone();
                    }
                }
                Stmt::ApplyClass { id, style_class } => {
                    for key in self.named(id.trim()) {
                        self.states[&key].classes.push(style_class.clone());
                    }
                }
                Stmt::Click { id, url, tooltip } => {
                    self.links.insert(id, Link { url, tooltip });
                }
                _ => {}
            }
        }
        self.graph = super::graph::build(&self.root_doc, &self.states, &self.classes)?;
        Ok(())
    }
}
pub fn doc_direction(doc: &[Stmt]) -> &str {
    let mut dir = "TB";
    for item in doc {
        if let Stmt::Direction(value) = item {
            dir = value;
        }
    }
    dir
}
