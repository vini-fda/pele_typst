use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use serde_json::{Map, Value};

use super::shapes::is_valid_shape;
use crate::errors::{PeleError, PeleErrorCode, Result};
use crate::preprocess::{parse_yaml, truthy, value_string};
use crate::url::safe_url;
use crate::util::{is_js_space, js_number, replace_first, trim, trim_start};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelType {
    Text,
    String,
    Markdown,
}

fn label_type(kind: Option<&str>) -> LabelType {
    match kind {
        Some("text") => LabelType::Text,
        Some("string") => LabelType::String,
        _ => LabelType::Markdown,
    }
}

#[derive(Debug, Clone)]
pub struct FlowText {
    pub text: String,
    pub kind: LabelType,
}

#[derive(Debug, Clone)]
pub struct FlowNode {
    pub id: String,
    pub text: Option<String>,
    pub label_type: LabelType,
    pub kind: Option<String>,
    pub styles: Vec<String>,
    pub classes: Vec<String>,
    pub dir: Option<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub icon: Option<String>,
    pub form: Option<String>,
    pub pos: Option<String>,
    pub img: Option<String>,
    pub constraint: Option<String>,
    pub asset_width: Option<f64>,
    pub asset_height: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct FlowEdge {
    pub start: String,
    pub end: String,
    pub kind: Option<String>,
    pub text: String,
    pub label_type: LabelType,
    pub stroke: Option<String>,
    pub length: Option<f64>,
    pub style: Option<Vec<String>>,
    pub classes: Vec<String>,
    pub id: String,
    pub is_user_defined_id: bool,
    pub interpolate: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FlowClassDef {
    pub styles: Vec<String>,
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FlowSubgraph {
    pub id: String,
    pub nodes: Vec<String>,
    pub title: String,
    pub classes: Vec<String>,
    pub dir: Option<String>,
    pub label_type: LabelType,
    pub metadata: Option<Map<String, Value>>,
}

#[derive(Debug, Clone)]
pub struct LinkInfo {
    pub kind: String,
    pub stroke: String,
    pub length: Option<f64>,
    pub text: Option<FlowText>,
    pub id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DocItem {
    Str(String),
    List(Vec<String>),
    Dir(String),
}

fn unquote(text: &str) -> String {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        text[1..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}

fn destruct_start_link(raw: &str) -> (String, String) {
    let mut s = trim(raw);
    let mut kind = "arrow_open";
    match s.chars().next() {
        Some('<') => {
            kind = "arrow_point";
            s = &s[1..];
        }
        Some('x') => {
            kind = "arrow_cross";
            s = &s[1..];
        }
        Some('o') => {
            kind = "arrow_circle";
            s = &s[1..];
        }
        _ => {}
    }
    let mut stroke = "normal";
    if s.contains('=') {
        stroke = "thick";
    }
    if s.contains('.') {
        stroke = "dotted";
    }
    (kind.to_string(), stroke.to_string())
}

fn destruct_end_link(raw: &str) -> (String, String, f64) {
    let s = trim(raw);
    let last = s.chars().last();
    let mut line: &str = match last {
        Some(c) => &s[..s.len() - c.len_utf8()],
        None => "",
    };
    let mut kind = "arrow_open".to_string();
    match last {
        Some('x') => {
            kind = "arrow_cross".into();
            if s.starts_with('x') {
                kind = format!("double_{}", kind);
                line = &line[1.min(line.len())..];
            }
        }
        Some('>') => {
            kind = "arrow_point".into();
            if s.starts_with('<') {
                kind = format!("double_{}", kind);
                line = &line[1.min(line.len())..];
            }
        }
        Some('o') => {
            kind = "arrow_circle".into();
            if s.starts_with('o') {
                kind = format!("double_{}", kind);
                line = &line[1.min(line.len())..];
            }
        }
        _ => {}
    }
    let mut stroke = "normal";
    let mut length = line.encode_utf16().count() as f64 - 1.0;
    if line.starts_with('=') {
        stroke = "thick";
    }
    if line.starts_with('~') {
        stroke = "invisible";
    }
    let dots = line.chars().filter(|&c| c == '.').count();
    if dots > 0 {
        stroke = "dotted";
        length = dots as f64;
    }
    (kind, stroke.to_string(), length)
}

#[derive(Default)]
pub struct FlowDb {
    pub direction: Option<String>,
    pub nodes: IndexMap<String, FlowNode>,
    pub edges: Vec<FlowEdge>,
    pub subgraphs: Vec<FlowSubgraph>,
    pub classes: IndexMap<String, FlowClassDef>,
    pub tooltips: HashMap<String, String>,
    pub default_edge_style: Option<Vec<String>>,
    pub default_interpolate: Option<String>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    subgraph_lookup: HashMap<String, usize>,
    members: HashSet<String>,
    edge_by_id: HashMap<String, usize>,
    pair_count: HashMap<(String, String), usize>,
    sub_count: usize,
    pub max_edges: Option<usize>,
    pub inherit_dir: bool,
}

#[derive(Default)]
pub struct Vertex<'a> {
    pub text: Option<&'a FlowText>,
    pub kind: Option<&'a str>,
    pub style: Option<Vec<String>>,
    pub props: Option<(String, String)>,
    pub metadata: Option<&'a str>,
}

impl FlowDb {
    pub fn new(inherit_dir: bool, max_edges: Option<usize>) -> Self {
        FlowDb {
            inherit_dir,
            max_edges,
            ..Default::default()
        }
    }

    pub fn add_vertex(&mut self, id: &str, v: Vertex) -> Result<()> {
        if trim(id).is_empty() {
            return Ok(());
        }
        let mut doc: Option<Map<String, Value>> = None;
        if let Some(metadata) = v.metadata {
            let yaml = if metadata.contains('\n') {
                format!("{}\n", metadata)
            } else {
                format!("{{\n{}\n}}", metadata)
            };
            let parsed = parse_yaml(&yaml).map_err(|e| {
                PeleError::new(format!("Metadata of \"{}\" is not valid YAML. {}", id, e), PeleErrorCode::Syntax).of("flowchart")
            })?;
            if let Value::Object(o) = parsed {
                doc = Some(o);
            }
        }

        if let (Some(&si), Some(doc)) = (self.subgraph_lookup.get(id), &doc) {
            let sg = &mut self.subgraphs[si];
            let mut merged = sg.metadata.clone().unwrap_or_default();
            for (k, val) in doc {
                merged.insert(k.clone(), val.clone());
            }
            sg.metadata = Some(merged);
            return Ok(());
        }

        if let Some(&ei) = self.edge_by_id.get(id) {
            if let Some(doc) = &doc
                && let Some(curve) = doc.get("curve")
            {
                self.edges[ei].interpolate = Some(value_string(curve));
            }
            return Ok(());
        }

        if !self.nodes.contains_key(id) {
            self.nodes.insert(
                id.to_string(),
                FlowNode {
                    id: id.to_string(),
                    text: None,
                    label_type: LabelType::Text,
                    kind: None,
                    styles: Vec::new(),
                    classes: Vec::new(),
                    dir: None,
                    link: None,
                    link_target: None,
                    icon: None,
                    form: None,
                    pos: None,
                    img: None,
                    constraint: None,
                    asset_width: None,
                    asset_height: None,
                },
            );
        }
        let vertex = self.nodes.get_mut(id).unwrap();
        if let Some(t) = v.text {
            vertex.label_type = t.kind;
            vertex.text = Some(unquote(trim(&t.text)));
        } else if vertex.text.is_none() {
            vertex.text = Some(id.to_string());
        }
        if let Some(k) = v.kind {
            vertex.kind = Some(k.to_string());
        }
        if let Some(style) = v.style {
            vertex.styles.extend(style);
        }
        let _ = v.props;

        let Some(doc) = doc else { return Ok(()) };

        if truthy(doc.get("shape")) {
            let shape = value_string(&doc["shape"]);
            if shape != shape.to_lowercase() || shape.contains('_') {
                return Err(PeleError::new(
                    format!("No such shape: {}. Shape names should be lowercase.", shape),
                    PeleErrorCode::Semantic,
                )
                .of("flowchart"));
            }
            if !is_valid_shape(&shape) {
                return Err(PeleError::new(format!("No such shape: {}.", shape), PeleErrorCode::Semantic).of("flowchart"));
            }
            vertex.kind = Some(shape);
        }
        let label = match doc.get("label") {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        };
        let label_blank = label.as_deref().is_none_or(|l| trim(l).is_empty());
        if truthy(doc.get("label")) {
            vertex.text = Some(value_string(&doc["label"]));
            vertex.label_type = label_type(doc.get("labelType").and_then(|v| v.as_str()));
        }
        if truthy(doc.get("icon")) {
            vertex.icon = Some(value_string(&doc["icon"]));
            if label_blank && vertex.text.as_deref() == Some(id) {
                vertex.text = Some(String::new());
            }
        }
        if truthy(doc.get("form")) {
            vertex.form = Some(value_string(&doc["form"]));
        }
        if truthy(doc.get("pos")) {
            vertex.pos = Some(value_string(&doc["pos"]));
        }
        if truthy(doc.get("img")) {
            vertex.img = Some(value_string(&doc["img"]));
            if label_blank && vertex.text.as_deref() == Some(id) {
                vertex.text = Some(String::new());
            }
        }
        if truthy(doc.get("constraint")) {
            vertex.constraint = Some(value_string(&doc["constraint"]));
        }
        let number = |v: &Value| match v {
            Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
            other => js_number(&value_string(other)),
        };
        if truthy(doc.get("w")) {
            vertex.asset_width = Some(number(&doc["w"]));
        }
        if truthy(doc.get("h")) {
            vertex.asset_height = Some(number(&doc["h"]));
        }
        Ok(())
    }

    fn add_single_link(&mut self, start: &str, end: &str, info: Option<&LinkInfo>, id: Option<&str>) -> Result<()> {
        let mut edge = FlowEdge {
            start: start.to_string(),
            end: end.to_string(),
            kind: None,
            text: String::new(),
            label_type: LabelType::Text,
            stroke: None,
            length: None,
            style: None,
            classes: Vec::new(),
            id: String::new(),
            is_user_defined_id: false,
            interpolate: self.default_interpolate.clone(),
        };
        if let Some(info) = info {
            if let Some(t) = &info.text {
                edge.text = unquote(trim(&t.text));
                edge.label_type = t.kind;
            }
            edge.kind = Some(info.kind.clone());
            edge.stroke = Some(info.stroke.clone());
            edge.length = info.length.map(|l| if l > 10.0 { 10.0 } else { l });
        }
        let pair = (start.to_string(), end.to_string());
        let count = self.pair_count.get(&pair).copied().unwrap_or(0);
        match id {
            Some(id) if !id.is_empty() && !self.edge_by_id.contains_key(id) => {
                edge.id = id.to_string();
                edge.is_user_defined_id = true;
            }
            _ => {
                edge.id = format!("L_{}_{}_{}", start, end, if count == 0 { 0 } else { count + 1 });
            }
        }
        if let Some(max) = self.max_edges
            && self.edges.len() >= max
        {
            return Err(PeleError::new(format!("Edge limit exceeded. The limit is {}.", max), PeleErrorCode::Limit).of("flowchart"));
        }
        let eid = edge.id.clone();
        self.edges.push(edge);
        self.pair_count.insert(pair, count + 1);
        self.edge_by_id.entry(eid).or_insert(self.edges.len() - 1);
        Ok(())
    }

    pub fn add_link(&mut self, starts: &[String], ends: &[String], info: Option<&LinkInfo>) -> Result<()> {
        let id = info.and_then(|i| i.id.as_ref()).map(|s| replace_first(s, "@", ""));
        let last_start = starts.last().cloned().unwrap_or_default();
        let first_end = ends.first().cloned().unwrap_or_default();
        for start in starts {
            for end in ends {
                let use_id = if *start == last_start && *end == first_end {
                    id.as_deref()
                } else {
                    None
                };
                self.add_single_link(start, end, info, use_id)?;
            }
        }
        Ok(())
    }

    fn edge_at(&mut self, pos: &str) -> Result<usize> {
        let n = js_number(pos);
        if n >= 0.0 && n.fract() == 0.0 && (n as usize) < self.edges.len() {
            return Ok(n as usize);
        }
        Err(PeleError::new(
            format!(
                "The index {} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",
                pos,
                self.edges.len() as i64 - 1
            ),
            PeleErrorCode::Semantic,
        )
        .of("flowchart"))
    }

    pub fn update_link_interpolate(&mut self, positions: &[String], interpolate: &str) -> Result<()> {
        for pos in positions {
            if pos == "default" {
                self.default_interpolate = Some(interpolate.to_string());
            } else {
                let e = self.edge_at(pos)?;
                self.edges[e].interpolate = Some(interpolate.to_string());
            }
        }
        Ok(())
    }

    pub fn update_link(&mut self, positions: &[String], mut style: Vec<String>) -> Result<()> {
        // Every position shares one list, which gains a `fill:none` when an edge takes it.
        let needs_fill = !style.is_empty() && !style.iter().any(|s| s.starts_with("fill"));
        let mut targets = Vec::new();
        for pos in positions {
            if pos == "default" {
                self.default_edge_style = Some(style.clone());
            } else {
                targets.push(self.edge_at(pos)?);
            }
        }
        if !targets.is_empty() && needs_fill {
            style.push("fill:none".into());
        }
        for e in targets {
            self.edges[e].style = Some(style.clone());
        }
        Ok(())
    }

    pub fn add_class(&mut self, ids: &str, styles: &[String]) {
        let joined = styles.join(",").replace("\\,", "§§§").replace(',', ";").replace("§§§", ",");
        let style: Vec<&str> = joined.split(';').collect();
        for id in ids.split(',') {
            let class = self.classes.entry(id.to_string()).or_insert(FlowClassDef {
                styles: Vec::new(),
                text_styles: Vec::new(),
            });
            for s in &style {
                if s.contains("color") {
                    class.text_styles.push(replace_first(s, "fill", "bgFill"));
                }
                class.styles.push(s.to_string());
            }
        }
    }

    pub fn set_direction(&mut self, dir: &str) {
        let mut d = trim(dir).to_string();
        if d.contains('<') {
            d = "RL".into();
        }
        if d.contains('^') {
            d = "BT".into();
        }
        if d.contains('>') {
            d = "LR".into();
        }
        if d.contains('v') {
            d = "TB".into();
        }
        if d == "TD" {
            d = "TB".into();
        }
        self.direction = Some(d);
    }

    pub fn set_class(&mut self, ids: &str, class: &str) {
        for id in ids.split(',') {
            if let Some(node) = self.nodes.get_mut(id) {
                node.classes.push(class.to_string());
            }
            if let Some(&e) = self.edge_by_id.get(id) {
                self.edges[e].classes.push(class.to_string());
            }
            if let Some(&s) = self.subgraph_lookup.get(id) {
                self.subgraphs[s].classes.push(class.to_string());
            }
        }
    }

    pub fn set_tooltip(&mut self, ids: &str, tooltip: &str) {
        for id in ids.split(',') {
            self.tooltips.insert(id.to_string(), tooltip.to_string());
        }
    }

    pub fn set_link(&mut self, ids: &str, link: &str, target: Option<&str>) {
        let url = trim(link);
        for id in ids.split(',') {
            if let Some(vertex) = self.nodes.get_mut(id) {
                vertex.link = if url.is_empty() { None } else { Some(safe_url(url)) };
                vertex.link_target = target.map(String::from);
            }
        }
        self.set_class(ids, "clickable");
    }

    // Callbacks are never executed; the node is only marked clickable, as in Mermaid's strict mode.
    pub fn set_click_event(&mut self, ids: &str) {
        self.set_class(ids, "clickable");
    }

    pub fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(trim_start(text).to_string());
    }

    pub fn set_acc_description(&mut self, text: &str) {
        let mut out = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\n' && chars.peek().is_some_and(|&n| is_js_space(n)) {
                while chars.peek().is_some_and(|&n| is_js_space(n)) {
                    chars.next();
                }
                out.push('\n');
            } else {
                out.push(c);
            }
        }
        self.acc_descr = Some(out);
    }

    // `same` is whether the title is the id itself, as written without brackets.
    pub fn add_subgraph(&mut self, id_text: Option<&FlowText>, list: &[DocItem], title_text: Option<&FlowText>, same: bool) -> String {
        let mut id: Option<String> = id_text.map(|t| trim(&t.text).to_string());
        let title = title_text.map(|t| t.text.clone()).unwrap_or_default();
        if id_text.is_some() && same && title.chars().any(is_js_space) {
            id = None;
        }
        let mut seen: HashSet<String> = HashSet::new();
        let mut node_list: Vec<String> = Vec::new();
        let mut dir: Option<String> = None;
        let add = |item: &str, seen: &mut HashSet<String>, node_list: &mut Vec<String>| {
            if !trim(item).is_empty() && !seen.contains(item) {
                seen.insert(item.to_string());
                node_list.push(item.to_string());
            }
        };
        for entry in list {
            match entry {
                DocItem::Str(s) => add(s, &mut seen, &mut node_list),
                DocItem::List(items) => {
                    for item in items {
                        add(item, &mut seen, &mut node_list);
                    }
                }
                DocItem::Dir(d) => dir = Some(d.clone()),
            }
        }
        if dir.is_none() && self.inherit_dir {
            dir = self.direction.clone();
        }
        let id = id.unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        self.sub_count += 1;
        // A node belongs to the first subgraph that lists it, and a subgraph never contains itself.
        let nodes: Vec<String> = node_list.into_iter().filter(|n| !self.members.contains(n) && *n != id).collect();
        for n in &nodes {
            self.members.insert(n.clone());
        }
        if let Some(&existing) = self.subgraph_lookup.get(&id) {
            self.subgraphs[existing].nodes.extend(nodes);
        } else {
            self.subgraphs.push(FlowSubgraph {
                id: id.clone(),
                nodes,
                title: trim(&title).to_string(),
                classes: Vec::new(),
                dir,
                label_type: match title_text {
                    Some(t) => t.kind,
                    None => LabelType::Markdown,
                },
                metadata: None,
            });
            self.subgraph_lookup.insert(id.clone(), self.subgraphs.len() - 1);
        }
        id
    }

    pub fn destruct_link(&self, end: &str, start: Option<&str>) -> LinkInfo {
        let (kind, stroke, length) = destruct_end_link(end);
        let Some(start) = start.filter(|s| !s.is_empty()) else {
            return LinkInfo {
                kind,
                stroke,
                length: Some(length),
                text: None,
                id: None,
            };
        };
        let (mut skind, sstroke) = destruct_start_link(start);
        if sstroke != stroke {
            return LinkInfo {
                kind: "INVALID".into(),
                stroke: "INVALID".into(),
                length: None,
                text: None,
                id: None,
            };
        }
        if skind == "arrow_open" {
            skind = kind;
        } else {
            if skind != kind {
                return LinkInfo {
                    kind: "INVALID".into(),
                    stroke: "INVALID".into(),
                    length: None,
                    text: None,
                    id: None,
                };
            }
            skind = format!("double_{}", skind);
        }
        if skind == "double_arrow" {
            skind = "double_arrow_point".into();
        }
        LinkInfo {
            kind: skind,
            stroke: sstroke,
            length: Some(length),
            text: None,
            id: None,
        }
    }

    pub fn subgraph(&self, id: &str) -> Option<&FlowSubgraph> {
        self.subgraph_lookup.get(id).map(|&i| &self.subgraphs[i])
    }
}
