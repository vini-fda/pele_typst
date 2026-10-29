use crate::{text::entities::decode_entities, url::sanitize_url};
use indexmap::IndexMap;
#[derive(Clone, Debug, Default)]
pub struct Member {
    pub id: String,
    pub method: bool,
    pub visibility: String,
    pub classifier: String,
    pub parameters: String,
    pub return_type: String,
}
impl Member {
    pub fn new(input: &str, method: bool) -> Self {
        let mut out = Self {
            method,
            ..Default::default()
        };
        if !method || !out.parse_method(input) {
            let first = input.chars().next();
            let last = input.chars().last();
            if first.is_some_and(|c| "#+~-".contains(c)) {
                out.visibility = first.unwrap().to_string();
            }
            if last.is_some_and(|c| "$*".contains(c)) {
                out.classifier = last.unwrap().to_string();
            }
            out.id = input[out.visibility.len()..input.len() - out.classifier.len()].into();
        }
        out.id = if out.id.starts_with(' ') {
            format!(" {}", out.id.trim())
        } else {
            out.id.trim().into()
        };
        out
    }
    fn parse_method(&mut self, input: &str) -> bool {
        let src: Vec<u16> = input.encode_utf16().collect();
        let n = src.len();
        let mut start = 0;
        while start < n {
            let mut end = start;
            while end < n && !matches!(src[end], 10 | 13 | 0x2028 | 0x2029) {
                end += 1;
            }
            let mut close = end.saturating_sub(1);
            while close > start && src[close] != 41 {
                close -= 1;
            }
            let mut open = close.saturating_sub(1);
            while open > start && src[open] != 40 {
                open -= 1;
            }
            if open > start {
                let visible = matches!(src[start], 35 | 43 | 126 | 45) && open > start + 1;
                if visible {
                    self.visibility = String::from_utf16_lossy(&src[start..start + 1]);
                }
                self.id = String::from_utf16_lossy(&src[if visible { start + 1 } else { start }..open]);
                self.parameters = String::from_utf16_lossy(&src[open + 1..close]).trim().into();
                let mut rest = close + 1;
                let mut classifier = String::new();
                if rest < n && (matches!(src[rest], 36 | 42) || crate::util::is_space16(src[rest] as u32)) {
                    classifier = String::from_utf16_lossy(&src[rest..rest + 1]).trim().into();
                    rest += 1;
                }
                let mut stop = rest;
                while stop < n && !matches!(src[stop], 10 | 13 | 0x2028 | 0x2029) {
                    stop += 1;
                }
                self.return_type = String::from_utf16_lossy(&src[rest..stop]).trim().into();
                if classifier.is_empty() && self.return_type.ends_with(['$', '*']) {
                    classifier = self.return_type.pop().unwrap().to_string();
                }
                self.classifier = classifier;
                return true;
            }
            start = end + 1;
        }
        self.method = false;
        false
    }
    pub fn display(&self) -> String {
        let generic = crate::diagrams::er::render::generic;
        let mut out = format!("{}{}", self.visibility, generic(&self.id));
        if self.method {
            out += &format!("({})", generic(self.parameters.trim()));
            if !self.return_type.is_empty() {
                out += &format!(" : {}", generic(&self.return_type));
            }
        }
        out.trim().into()
    }
}
#[derive(Clone, Debug, Default)]
pub struct Node {
    pub id: String,
    pub type_: String,
    pub label: String,
    pub css_classes: String,
    pub methods: Vec<Member>,
    pub members: Vec<Member>,
    pub annotations: Vec<String>,
    pub styles: Vec<String>,
    pub parent: Option<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub have_callback: bool,
    pub tooltip: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Relation {
    pub id1: String,
    pub id2: String,
    pub title1: String,
    pub title2: String,
    pub title: Option<String>,
    pub type1: i32,
    pub type2: i32,
    pub line_type: i32,
}
#[derive(Clone, Debug)]
pub struct Note {
    pub id: String,
    pub class: Option<String>,
    pub text: String,
    pub index: usize,
    pub parent: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Interface {
    pub id: String,
    pub label: String,
    pub class_id: String,
}
#[derive(Clone, Debug)]
pub struct Namespace {
    pub id: String,
    pub label: String,
    pub classes: Vec<String>,
    pub notes: Vec<String>,
    pub children: Vec<String>,
    pub parent: Option<String>,
    pub explicit: bool,
}
#[derive(Clone, Debug, Default)]
pub struct StyleClass {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Model {
    pub direction: String,
    pub classes: IndexMap<String, Node>,
    pub relations: Vec<Relation>,
    pub notes: IndexMap<String, Note>,
    pub namespaces: IndexMap<String, Namespace>,
    pub interfaces: Vec<Interface>,
    pub style_classes: IndexMap<String, StyleClass>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    namespace_stack: Vec<String>,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            direction: "TB".into(),
            classes: IndexMap::new(),
            relations: vec![],
            notes: IndexMap::new(),
            namespaces: IndexMap::new(),
            interfaces: vec![],
            style_classes: IndexMap::new(),
            title: None,
            acc_title: None,
            acc_descr: None,
            namespace_stack: vec![],
        }
    }
}
fn split_name(id: &str) -> (&str, &str) {
    if let Some(at) = id.find('~').filter(|&at| at > 0) {
        let end = id[at + 1..].find('~').map_or(id.len(), |e| at + 1 + e);
        (&id[..at], &id[at + 1..end])
    } else {
        (id, "")
    }
}
fn lookup_id(id: &str) -> String {
    split_name(&if id.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        format!("classId-{id}")
    } else {
        id.into()
    })
    .0
    .into()
}
impl Model {
    pub fn add_class(&mut self, id: &str) {
        let (name, type_) = split_name(id);
        self.classes.entry(name.into()).or_insert_with(|| Node {
            id: name.into(),
            type_: type_.into(),
            label: name.into(),
            css_classes: "default".into(),
            ..Default::default()
        });
    }
    fn ensure(&mut self, id: &str) -> &mut Node {
        self.add_class(id);
        self.classes.get_mut(split_name(id).0).unwrap()
    }
    pub fn set_label(&mut self, id: &str, label: String) {
        self.ensure(id).label = label;
    }
    pub fn add_relation(&mut self, mut r: Relation) {
        if r.type1 == 4 && r.type2 == -1 {
            self.add_class(&r.id2);
            r.id1 = self.add_interface(r.id1, r.id2.clone());
        } else if r.type2 == 4 && r.type1 == -1 {
            self.add_class(&r.id1);
            r.id2 = self.add_interface(r.id2, r.id1.clone());
        } else {
            self.add_class(&r.id1);
            self.add_class(&r.id2);
        }
        r.id1 = split_name(&r.id1).0.into();
        r.id2 = split_name(&r.id2).0.into();
        r.title1 = r.title1.trim().into();
        r.title2 = r.title2.trim().into();
        self.relations.push(r);
    }
    fn add_interface(&mut self, label: String, class_id: String) -> String {
        let id = format!("interface{}", self.interfaces.len());
        self.interfaces.push(Interface {
            id: id.clone(),
            label,
            class_id,
        });
        id
    }
    pub fn add_annotation(&mut self, id: &str, annotation: String) {
        self.ensure(id).annotations.push(annotation);
    }
    pub fn add_member(&mut self, id: &str, member: &str) {
        let node = self.ensure(id);
        let text = member.trim();
        if text.starts_with("<<") && text.ends_with(">>") {
            node.annotations.push(text[2..text.len() - 2].into());
        } else if text.find(')').is_some_and(|p| p > 0) {
            let parsed = Member::new(text, true);
            if parsed.method {
                node.methods.push(parsed)
            } else {
                node.members.push(parsed)
            }
        } else if !text.is_empty() {
            node.members.push(Member::new(text, false));
        }
    }
    pub fn add_note(&mut self, text: String, class: Option<String>) -> String {
        let index = self.notes.len();
        let id = format!("note{index}");
        self.notes.insert(
            id.clone(),
            Note {
                id: id.clone(),
                class,
                text,
                index,
                parent: None,
            },
        );
        id
    }
    pub fn set_class(&mut self, ids: &str, css: &str) {
        for id in ids.split(',') {
            if let Some(node) = self.classes.get_mut(&lookup_id(id)) {
                node.css_classes.push(' ');
                node.css_classes.push_str(css);
            }
        }
    }
    pub fn define_class(&mut self, ids: Vec<String>, style: Vec<String>) {
        for id in ids {
            let class = self.style_classes.entry(id.clone()).or_insert_with(|| StyleClass {
                id: id.clone(),
                ..Default::default()
            });
            for s in &style {
                if s.contains("color") {
                    class.text_styles.push(s.replacen("fill", "bgFill", 1));
                }
                class.styles.push(s.clone());
            }
            let parts: Vec<String> = style.iter().flat_map(|s| s.split(',').map(str::to_owned)).collect();
            for node in self.classes.values_mut() {
                if node.css_classes.split(' ').any(|s| s == id) {
                    node.styles.extend(parts.iter().cloned());
                }
            }
        }
    }
    pub fn set_tooltip(&mut self, ids: &str, tooltip: String) {
        for id in ids.split(',') {
            if let Some(node) = self.classes.get_mut(split_name(id).0) {
                node.tooltip = Some(tooltip.clone());
            }
        }
    }
    pub fn set_link(&mut self, ids: &str, link: &str, target: Option<String>) {
        let url = decode_entities(link).trim().to_owned();
        for id in ids.split(',') {
            if let Some(node) = self.classes.get_mut(&lookup_id(id)) {
                node.link = if url.is_empty() { None } else { Some(sanitize_url(&url)) };
                node.link_target = Some(target.clone().unwrap_or_else(|| "_blank".into()));
            }
        }
        self.set_class(ids, "clickable");
    }
    pub fn set_click(&mut self, ids: &str) {
        for id in ids.split(',') {
            if let Some(node) = self.classes.get_mut(split_name(id).0) {
                node.have_callback = true;
            }
        }
        self.set_class(ids, "clickable");
    }
    pub fn set_descr(&mut self, text: String) {
        let re = regex::Regex::new(r"\n\s+").unwrap();
        self.acc_descr = Some(re.replace_all(&text, "\n").into_owned());
    }
    pub fn add_namespace(&mut self, id: &str, label: Option<String>) -> String {
        let prefix = self.namespace_stack.last().cloned().unwrap_or_default();
        let qualified = if prefix.is_empty() { id.into() } else { format!("{prefix}.{id}") };
        self.namespace_stack.push(qualified.clone());
        if let Some(ns) = self.namespaces.get_mut(&qualified) {
            ns.explicit = true;
            if let Some(label) = label.filter(|s| !s.is_empty()) {
                ns.label = label;
            }
            return qualified;
        }
        let mut parent = if prefix.is_empty() { None } else { Some(prefix.clone()) };
        let mut from = if prefix.is_empty() { 0 } else { prefix.len() + 1 };
        loop {
            let dot = qualified[from..].find('.').map(|d| from + d);
            let leaf = dot.is_none();
            let end = dot.unwrap_or(qualified.len());
            let current = qualified[..end].to_owned();
            let ns = self.namespaces.entry(current.clone()).or_insert_with(|| Namespace {
                id: current.clone(),
                label: if leaf {
                    label
                        .clone()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| qualified[from..end].into())
                } else {
                    qualified[from..end].into()
                },
                classes: vec![],
                notes: vec![],
                children: vec![],
                parent: parent.clone(),
                explicit: leaf,
            });
            if leaf {
                ns.explicit = true;
            }
            if let Some(parent) = &parent
                && let Some(p) = self.namespaces.get_mut(parent)
            {
                if !p.children.contains(&current) {
                    p.children.push(current.clone());
                }
                self.namespaces.get_mut(&current).unwrap().parent.get_or_insert(parent.clone());
            }
            if leaf {
                return qualified;
            }
            parent = Some(current);
            from = end + 1;
        }
    }
    pub fn pop_namespace(&mut self) {
        self.namespace_stack.pop();
    }
    pub fn add_namespace_members(&mut self, id: &str, classes: Vec<String>, notes: Vec<String>) {
        let Some(ns) = self.namespaces.get_mut(id) else { return };
        for name in classes {
            let name = split_name(&name).0;
            if let Some(node) = self.classes.get_mut(name) {
                node.parent = Some(id.into());
                if !ns.classes.iter().any(|n| n == name) {
                    ns.classes.push(name.into());
                }
            }
        }
        for name in notes {
            if let Some(note) = self.notes.get_mut(&name) {
                note.parent = Some(id.into());
                if !ns.notes.contains(&name) {
                    ns.notes.push(name);
                }
            }
        }
    }
    pub fn set_style(&mut self, id: &str, style: Vec<String>) {
        if let Some(node) = self.classes.get_mut(id) {
            node.styles.extend(style.iter().flat_map(|s| s.split(',').map(str::to_owned)));
        }
    }
}
