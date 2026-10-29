use indexmap::IndexMap;
use std::collections::HashMap;
#[derive(Clone, Debug)]
pub struct Named {
    pub key: String,
    pub value: String,
}
#[derive(Clone, Debug)]
pub enum Attr {
    Text(String),
    Named(Named),
}
impl Attr {
    pub fn value(&self) -> &str {
        match self {
            Self::Text(v) => v,
            Self::Named(v) => &v.value,
        }
    }
    pub fn text(&self) -> Option<&str> {
        if let Self::Text(v) = self { Some(v) } else { None }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Element {
    pub alias: String,
    pub label: String,
    pub alias_attr: Option<Named>,
    pub label_attr: Option<Named>,
    pub parent: Option<usize>,
    pub kind: String,
    pub node_type: Option<String>,
    pub fields: IndexMap<String, String>,
    pub extra: IndexMap<String, String>,
}
impl Element {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
    fn named(&mut self, n: &Named) {
        if n.key == "label" {
            self.label = n.value.clone();
        } else if [
            "type",
            "descr",
            "techn",
            "sprite",
            "tags",
            "link",
            "bgColor",
            "fontColor",
            "borderColor",
            "shadowing",
            "shape",
            "legendText",
            "legendSprite",
        ]
        .contains(&n.key.as_str())
        {
            self.fields.insert(n.key.clone(), n.value.clone());
        } else {
            self.extra.insert(n.key.clone(), n.value.clone());
        }
    }
    fn put(&mut self, field: &str, a: Option<&Attr>) {
        match a {
            Some(Attr::Text(v)) => {
                self.fields.insert(field.into(), v.clone());
            }
            Some(Attr::Named(n)) => self.named(n),
            None => {}
        }
    }
    fn text(&mut self, field: &str, a: Option<&Attr>) {
        if a.is_none() {
            self.fields.entry(field.into()).or_default();
        } else {
            self.put(field, a)
        }
    }
    fn label(&mut self, a: Option<&Attr>) {
        self.label_attr = None;
        match a {
            Some(Attr::Named(n)) => {
                self.label.clear();
                self.label_attr = Some(n.clone());
                self.named(n);
            }
            _ => self.label = a.map(|a| a.value().to_owned()).unwrap_or_default(),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Rel {
    pub kind: String,
    pub from: String,
    pub to: String,
    pub label: String,
    pub from_attr: Option<Named>,
    pub to_attr: Option<Named>,
    pub label_attr: Option<Named>,
    pub fields: IndexMap<String, String>,
    pub offset_x: Option<f64>,
    pub offset_y: Option<f64>,
    pub extra: IndexMap<String, String>,
}
impl Rel {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
    fn named(&mut self, n: &Named) {
        match n.key.as_str() {
            "offsetX" => self.offset_x = Some(parse_int(&n.value)),
            "offsetY" => self.offset_y = Some(parse_int(&n.value)),
            "label" => self.label = n.value.clone(),
            "descr" | "techn" | "sprite" | "tags" | "link" | "textColor" | "lineColor" => {
                self.fields.insert(n.key.clone(), n.value.clone());
            }
            _ => {
                self.extra.insert(n.key.clone(), n.value.clone());
            }
        }
    }
    fn put(&mut self, field: &str, a: Option<&Attr>) {
        match a {
            Some(Attr::Text(v)) => {
                self.fields.insert(field.into(), v.clone());
            }
            Some(Attr::Named(n)) => self.named(n),
            None => {}
        }
    }
    fn text(&mut self, field: &str, a: Option<&Attr>) {
        if a.is_none() {
            self.fields.entry(field.into()).or_default();
        } else {
            self.put(field, a)
        }
    }
    fn label(&mut self, a: Option<&Attr>) {
        self.label_attr = None;
        match a {
            Some(Attr::Named(n)) => {
                self.label.clear();
                self.label_attr = Some(n.clone());
                self.named(n);
            }
            _ => self.label = a.map(|a| a.value().to_owned()).unwrap_or_default(),
        }
    }
}
pub fn parse_int(s: &str) -> f64 {
    let s = s.trim_start();
    let (sign, s) = if let Some(s) = s.strip_prefix('-') {
        (-1., s)
    } else {
        (1., s.strip_prefix('+').unwrap_or(s))
    };
    let (radix, s) = if s.starts_with("0x") || s.starts_with("0X") {
        (16, &s[2..])
    } else {
        (10, s)
    };
    let mut value = 0.;
    let mut any = false;
    for c in s.chars() {
        let Some(d) = c.to_digit(radix) else { break };
        value = value * radix as f64 + d as f64;
        any = true;
    }
    if any { sign * value } else { f64::NAN }
}
#[derive(Clone, Debug)]
pub struct Model {
    pub c4_type: Option<String>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub shapes: Vec<Element>,
    pub boundaries: Vec<Element>,
    pub rels: Vec<Rel>,
    pub shape_in_row: f64,
    pub boundary_in_row: f64,
    shape_index: HashMap<String, usize>,
    boundary_index: HashMap<String, usize>,
    rel_index: HashMap<(String, String), usize>,
    current: usize,
    stack: Vec<usize>,
}
impl Default for Model {
    fn default() -> Self {
        let mut global = Element {
            alias: "global".into(),
            label: "global".into(),
            ..Default::default()
        };
        global.fields.insert("type".into(), "global".into());
        Self {
            c4_type: None,
            title: None,
            acc_title: None,
            acc_descr: None,
            shapes: vec![],
            boundaries: vec![global],
            rels: vec![],
            shape_in_row: 4.,
            boundary_in_row: 2.,
            shape_index: HashMap::new(),
            boundary_index: HashMap::new(),
            rel_index: HashMap::new(),
            current: 0,
            stack: vec![],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ref {
    Shape(usize),
    Boundary(usize),
}
fn element(list: &mut Vec<Element>, index: &mut HashMap<String, usize>, attrs: &[Attr]) -> usize {
    let alias = &attrs[0];
    let known = alias.text().and_then(|a| index.get(a)).copied();
    let i = known.unwrap_or_else(|| {
        let i = list.len();
        let mut el = Element::default();
        match alias {
            Attr::Text(a) => {
                el.alias = a.clone();
                index.insert(a.clone(), i);
            }
            Attr::Named(n) => {
                el.alias_attr = Some(n.clone());
                el.named(n);
            }
        }
        list.push(el);
        i
    });
    list[i].label(attrs.get(1));
    i
}
impl Model {
    pub fn get(&self, alias: &str) -> Option<Ref> {
        self.shape_index
            .get(alias)
            .copied()
            .map(Ref::Shape)
            .or_else(|| self.boundary_index.get(alias).copied().map(Ref::Boundary))
    }
    pub fn add_shape(&mut self, kind: &str, attrs: &[Attr], container: bool) {
        let i = element(&mut self.shapes, &mut self.shape_index, attrs);
        let shape = &mut self.shapes[i];
        let at = if container {
            shape.text("techn", attrs.get(2));
            shape.text("descr", attrs.get(3));
            4
        } else {
            shape.text("descr", attrs.get(2));
            3
        };
        shape.put("sprite", attrs.get(at));
        shape.put("tags", attrs.get(at + 1));
        shape.put("link", attrs.get(at + 2));
        shape.kind = kind.into();
        shape.parent = Some(self.current);
    }
    pub fn add_boundary(&mut self, attrs: &[Attr], fallback: &str, node_type: Option<&str>) {
        let i = element(&mut self.boundaries, &mut self.boundary_index, attrs);
        let b = &mut self.boundaries[i];
        if attrs.get(2).is_none() {
            b.fields.insert("type".into(), fallback.into());
        } else {
            b.put("type", attrs.get(2));
        }
        if let Some(node_type) = node_type {
            b.text("descr", attrs.get(3));
            b.put("sprite", attrs.get(4));
            b.put("tags", attrs.get(5));
            b.put("link", attrs.get(6));
            b.node_type = Some(node_type.into());
        } else {
            b.put("tags", attrs.get(3));
            b.put("link", attrs.get(4));
        }
        b.parent = Some(self.current);
        self.stack.push(self.current);
        self.current = i;
    }
    pub fn pop(&mut self) {
        self.current = self.stack.pop().unwrap_or(0);
    }
    pub fn add_rel(&mut self, kind: &str, attrs: &[Attr]) {
        if attrs.len() < 3 {
            return;
        }
        let (from, to) = (&attrs[0], &attrs[1]);
        let key = from.text().zip(to.text()).map(|(f, t)| (f.to_owned(), t.to_owned()));
        let known = key.as_ref().and_then(|k| self.rel_index.get(k)).copied();
        let i = known.unwrap_or_else(|| {
            let i = self.rels.len();
            let mut rel = Rel::default();
            if let Some(key) = key {
                self.rel_index.insert(key, i);
            }
            match from {
                Attr::Text(v) => rel.from = v.clone(),
                Attr::Named(n) => rel.from_attr = Some(n.clone()),
            }
            match to {
                Attr::Text(v) => rel.to = v.clone(),
                Attr::Named(n) => rel.to_attr = Some(n.clone()),
            }
            self.rels.push(rel);
            i
        });
        let r = &mut self.rels[i];
        r.kind = kind.into();
        r.label(attrs.get(2));
        r.text("techn", attrs.get(3));
        r.text("descr", attrs.get(4));
        r.put("sprite", attrs.get(5));
        r.put("tags", attrs.get(6));
        r.put("link", attrs.get(7));
    }
    pub fn update_element(&mut self, attrs: &[Attr]) {
        let Some(r) = attrs[0].text().and_then(|a| self.get(a)) else {
            return;
        };
        let e = match r {
            Ref::Shape(i) => &mut self.shapes[i],
            Ref::Boundary(i) => &mut self.boundaries[i],
        };
        for (i, field) in [
            "bgColor",
            "fontColor",
            "borderColor",
            "shadowing",
            "shape",
            "sprite",
            "techn",
            "legendText",
            "legendSprite",
        ]
        .iter()
        .enumerate()
        {
            e.put(field, attrs.get(i + 1));
        }
    }
    pub fn update_rel(&mut self, attrs: &[Attr]) {
        let Some((from, to)) = attrs.first().and_then(Attr::text).zip(attrs.get(1).and_then(Attr::text)) else {
            return;
        };
        let Some(&i) = self.rel_index.get(&(from.into(), to.into())) else {
            return;
        };
        let r = &mut self.rels[i];
        r.put("textColor", attrs.get(2));
        r.put("lineColor", attrs.get(3));
        for (i, x) in [(4, true), (5, false)] {
            match attrs.get(i) {
                Some(Attr::Text(v)) => {
                    if x {
                        r.offset_x = Some(parse_int(v))
                    } else {
                        r.offset_y = Some(parse_int(v))
                    }
                }
                Some(Attr::Named(n)) => r.named(n),
                None => {}
            }
        }
    }
    pub fn update_layout(&mut self, attrs: &[Attr]) {
        if let Some(a) = attrs.first() {
            let count = parse_int(a.value());
            if count >= 1. {
                self.shape_in_row = count;
            }
        }
        if let Some(a) = attrs.get(1) {
            let count = parse_int(a.value());
            if count >= 1. {
                self.boundary_in_row = count;
            }
        }
    }
}
