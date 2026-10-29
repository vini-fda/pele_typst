use crate::{PeleError, errors::PeleErrorCode};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
#[derive(Clone, Debug, Default)]
pub struct Block {
    pub id: String,
    pub kind: String,
    pub label: Option<String>,
    pub children: Vec<usize>,
    pub columns: Option<f64>,
    pub span: Option<f64>,
    pub width: Option<f64>,
    pub directions: Vec<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub color_index: Option<usize>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub arrow_start: String,
    pub arrow_end: String,
    pub thickness: String,
    pub pattern: String,
    pub css: Option<String>,
    pub style_class: Option<String>,
    pub styles_str: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Class {
    pub id: String,
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Model {
    pub root: usize,
    pub blocks: Vec<usize>,
    pub edges: Vec<usize>,
    pub arena: Vec<Block>,
    pub classes: IndexMap<String, Class>,
    pub by_id: IndexMap<String, usize>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    ids: usize,
    colors: usize,
    spaces: f64,
    pending: HashSet<String>,
    edge_count: HashMap<String, usize>,
}
impl Default for Model {
    fn default() -> Self {
        Self {
            root: 0,
            blocks: vec![],
            edges: vec![],
            arena: vec![Block {
                id: "root".into(),
                kind: "composite".into(),
                columns: Some(-1.),
                ..Default::default()
            }],
            classes: IndexMap::new(),
            by_id: IndexMap::new(),
            title: None,
            acc_title: None,
            acc_descr: None,
            ids: 0,
            colors: 0,
            spaces: 0.,
            pending: HashSet::new(),
            edge_count: HashMap::new(),
        }
    }
}
impl Model {
    pub fn id(&mut self) -> String {
        self.ids += 1;
        format!("id-{}", self.ids)
    }
    pub fn push(&mut self, b: Block) -> usize {
        let i = self.arena.len();
        self.arena.push(b);
        i
    }
    fn named(&mut self, id: &str) -> usize {
        if let Some(&i) = self.by_id.get(id) {
            return i;
        }
        let i = self.push(Block {
            id: id.into(),
            kind: "na".into(),
            ..Default::default()
        });
        self.by_id.insert(id.into(), i);
        self.pending.insert(id.into());
        i
    }
    fn class(&mut self, id: String, attrs: String) {
        let class = self.classes.entry(id.clone()).or_insert_with(|| Class { id, ..Default::default() });
        for attr in attrs.split(',') {
            let fixed = attr.replacen(';', "", 1).trim().to_owned();
            if attr.contains("color") {
                class
                    .text_styles
                    .push(fixed.replacen("fill", "bgFill", 1).replacen("color", "fill", 1));
            }
            class.styles.push(fixed);
        }
    }
    pub fn hierarchy(&mut self, list: Vec<usize>) -> Result<(), PeleError> {
        struct Frame {
            list: Vec<usize>,
            at: usize,
            parent: usize,
            children: Vec<usize>,
        }
        let mut stack = vec![Frame {
            list,
            at: 0,
            parent: self.root,
            children: vec![],
        }];
        while let Some(f) = stack.last_mut() {
            if f.at == f.list.len() {
                self.arena[f.parent].children = std::mem::take(&mut f.children);
                stack.pop();
                continue;
            }
            let i = f.list[f.at];
            f.at += 1;
            let id = self.arena[i].id.clone();
            match self.arena[i].kind.as_str() {
                "classDef" => {
                    self.class(id, self.arena[i].css.clone().unwrap_or_default());
                    continue;
                }
                "applyClass" => {
                    for id in id.split(',') {
                        let n = self.named(id.trim());
                        let css = self.arena[i].style_class.clone().unwrap_or_default();
                        self.arena[n].classes.push(css);
                    }
                    continue;
                }
                "applyStyles" => {
                    if let Some(style) = self.arena[i].styles_str.clone().filter(|s| !s.is_empty()) {
                        let n = self.named(&id);
                        self.arena[n].styles = style.split(',').map(str::to_owned).collect();
                    }
                    continue;
                }
                "column-setting" => {
                    self.arena[f.parent].columns = Some(self.arena[i].columns.unwrap_or(-1.));
                    continue;
                }
                "edge" => {
                    let count = self.edge_count.entry(id.clone()).or_default();
                    *count += 1;
                    self.arena[i].id = format!("{count}-{id}");
                    self.edges.push(i);
                    continue;
                }
                _ => {}
            }
            if self.arena[i].id.is_empty() {
                self.arena[i].id = self.id();
            }
            let id = self.arena[i].id.clone();
            if self.arena[i].label.as_ref().is_none_or(|s| s.is_empty()) {
                self.arena[i].label = Some(if self.arena[i].kind == "composite" {
                    String::new()
                } else {
                    id.clone()
                });
            }
            let known = self.by_id.get(&id).copied();
            let fresh = known.is_none() || self.pending.remove(&id);
            if !fresh {
                let k = known.unwrap();
                if self.arena[i].kind != "na" {
                    self.arena[k].kind = self.arena[i].kind.clone();
                }
                if self.arena[i].label.as_deref() != Some(&id) {
                    self.arena[k].label = self.arena[i].label.clone();
                }
            } else {
                if let Some(k) = known {
                    self.arena[i].classes = self.arena[k].classes.clone();
                    self.arena[i].styles = self.arena[k].styles.clone();
                }
                if self.arena[i].kind == "composite" {
                    self.arena[i].color_index = Some(self.colors);
                    self.colors += 1;
                }
                self.by_id.insert(id.clone(), i);
            }
            let inner = std::mem::take(&mut self.arena[i].children);
            if self.arena[i].kind == "space" {
                let width = self.arena[i].width.unwrap_or(1.);
                if !(width <= 100000. - self.spaces) {
                    return Err(PeleError::new("A block diagram can hold at most 100000 space blocks.", PeleErrorCode::Limit).of("block"));
                }
                self.spaces += width.max(0.);
                let mut j = 0;
                while (j as f64) < width {
                    let mut copy = self.arena[i].clone();
                    copy.id = format!("{id}-{j}");
                    let id = copy.id.clone();
                    let at = self.push(copy);
                    self.by_id.insert(id, at);
                    f.children.push(at);
                    j += 1;
                }
            } else if fresh {
                f.children.push(i);
            }
            if !inner.is_empty() {
                stack.push(Frame {
                    list: inner,
                    at: 0,
                    parent: i,
                    children: vec![],
                });
            }
        }
        self.blocks = self.arena[self.root].children.clone();
        Ok(())
    }
}
pub fn shape(raw: Option<&str>) -> &'static str {
    match raw {
        Some("[]") => "square",
        Some("()") => "round",
        Some("(())") => "circle",
        Some(">]") => "rect_left_inv_arrow",
        Some("{}") => "diamond",
        Some("{{}}") => "hexagon",
        Some("([])") => "stadium",
        Some("[[]]") => "subroutine",
        Some("[()]") => "cylinder",
        Some("((()))") => "doublecircle",
        Some("[//]") => "lean_right",
        Some("[\\\\]") => "lean_left",
        Some("[/\\]") => "trapezoid",
        Some("[\\/]") => "inv_trapezoid",
        Some("<[]>") => "block_arrow",
        _ => "na",
    }
}
