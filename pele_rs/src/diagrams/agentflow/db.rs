pub use super::lexer::Loc;
use super::shapes;
pub use crate::diagrams::flowchart::db::{DocItem, FlowClassDef as ClassDef, FlowText, LabelType};
use crate::errors::{PeleError, PeleErrorCode, Result, locate};
use crate::preprocess::{parse_yaml, truthy, value_string};
use crate::url::safe_url;
use crate::util::{is_js_space, js_number, replace_first, trim, trim_start};
use indexmap::IndexMap;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub text: Option<String>,
    pub label_type: LabelType,
    pub kind: Option<String>,
    pub styles: Vec<String>,
    pub classes: Vec<String>,
    pub dir: Option<String>,
    pub props: Option<IndexMap<String, String>>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub metadata: Option<Map<String, Value>>,
    pub is_connector: bool,
}
impl Node {
    fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            text: None,
            label_type: LabelType::Text,
            kind: None,
            styles: vec![],
            classes: vec![],
            dir: None,
            props: None,
            link: None,
            link_target: None,
            metadata: None,
            is_connector: false,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Edge {
    pub start: String,
    pub end: String,
    pub kind: Option<String>,
    pub text: String,
    pub label_type: LabelType,
    pub stroke: Option<String>,
    pub edge_semantic: Option<String>,
    pub length: Option<f64>,
    pub style: Option<Vec<String>>,
    pub classes: Vec<String>,
    pub id: String,
    pub is_user_defined_id: bool,
    pub interpolate: Option<String>,
    pub animate: Option<Value>,
    pub animation: Option<Value>,
    pub metadata: Option<Map<String, Value>>,
}
#[derive(Debug, Clone)]
pub struct Subgraph {
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
    pub edge_semantic: Option<String>,
    pub text: Option<FlowText>,
    pub id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub start_index: usize,
    pub end_index: usize,
}
#[derive(Debug, Clone)]
pub struct Mapping {
    pub id: String,
    pub kind: String,
    pub position: Position,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub id: String,
    pub severity: String,
    pub message: String,
    pub node_id: Option<String>,
    pub edge_id: Option<String>,
    pub position: Option<Position>,
}
impl Position {
    pub fn to_json(&self) -> Value {
        json!({"startLine":self.start_line,"startColumn":self.start_column,"endLine":self.end_line,"endColumn":self.end_column,"startIndex":self.start_index,"endIndex":self.end_index})
    }
}
impl Diagnostic {
    pub fn to_json(&self) -> Value {
        let mut out = json!({"id":self.id,"severity":self.severity,"message":self.message});
        let fields = out.as_object_mut().unwrap();
        if let Some(id) = &self.node_id {
            fields.insert("nodeId".into(), json!(id));
        }
        if let Some(id) = &self.edge_id {
            fields.insert("edgeId".into(), json!(id));
        }
        if let Some(position) = &self.position {
            fields.insert("position".into(), position.to_json());
        }
        out
    }
}
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub direction: Option<String>,
    pub nodes: IndexMap<String, Node>,
    pub edges: Vec<Edge>,
    pub subgraphs: Vec<Subgraph>,
    pub connectors: IndexMap<String, Node>,
    pub classes: IndexMap<String, ClassDef>,
    pub tooltips: IndexMap<String, String>,
    pub default_edge_style: Option<Vec<String>>,
    pub default_interpolate: Option<String>,
    pub mappings: Vec<Mapping>,
    pub diagnostics: Vec<Diagnostic>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub line_offset: usize,
    subgraph_lookup: HashMap<String, usize>,
    global_nodes: HashSet<String>,
    members: HashSet<String>,
    edge_by_id: HashMap<String, usize>,
    pair_count: HashMap<(String, String), usize>,
    sub_count: usize,
    bare_mappings: HashSet<usize>,
    first_mapping: HashMap<String, usize>,
    pub inherit_dir: bool,
    pub configured_direction: Option<String>,
    pub source: String,
    pub max_edges: usize,
}
pub type FlowDb = Model;
#[derive(Default)]
pub struct Vertex<'a> {
    pub text: Option<&'a FlowText>,
    pub kind: Option<&'a str>,
    pub style: Option<Vec<String>>,
    pub classes: Option<Vec<String>>,
    pub dir: Option<&'a str>,
    pub props: Option<IndexMap<String, String>>,
    pub metadata: Option<&'a str>,
    pub metadata_loc: Option<Loc>,
}
fn label_type(kind: Option<&str>) -> LabelType {
    match kind {
        Some("text") => LabelType::Text,
        Some("string") => LabelType::String,
        _ => LabelType::Markdown,
    }
}
fn unquote(text: &str) -> String {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        text[1..text.len() - 1].into()
    } else {
        text.into()
    }
}
fn json_string(value: &Value) -> String {
    match value {
        Value::Number(n) => crate::util::js_num(n.as_f64().unwrap()),
        Value::Array(values) => format!("[{}]", values.iter().map(json_string).collect::<Vec<_>>().join(",")),
        Value::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), json_string(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        _ => value.to_string(),
    }
}
fn strip_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in ["__proto__", "constructor", "prototype"] {
                map.remove(key);
            }
            for v in map.values_mut() {
                strip_keys(v);
            }
        }
        Value::Array(list) => {
            for v in list {
                strip_keys(v);
            }
        }
        _ => {}
    }
}
fn merge(into: &mut Option<Map<String, Value>>, value: &Value) {
    let out = into.get_or_insert_with(Map::new);
    match value {
        Value::Object(map) => out.extend(map.clone()),
        Value::Array(list) => {
            for (i, v) in list.iter().enumerate() {
                out.insert(i.to_string(), v.clone());
            }
        }
        Value::String(s) => {
            for (i, c) in s.chars().enumerate() {
                out.insert(i.to_string(), Value::String(c.to_string()));
            }
        }
        _ => {}
    }
}
fn strip_commas(body: &str) -> String {
    let (mut single, mut double, mut depth, mut block) = (false, false, 0usize, None);
    let mut out = vec![];
    for line in body.split('\n') {
        let chars: Vec<_> = line.chars().collect();
        let indent = chars.iter().take_while(|&&c| is_js_space(c)).count();
        if let Some(level) = block {
            if trim(line).is_empty() || indent > level {
                out.push(line.to_string());
                continue;
            }
            block = None;
        }
        let mut comment = chars.len();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if double {
                if c == '\\' {
                    i += 1;
                } else if c == '"' {
                    double = false;
                }
            } else if single {
                if c == '\'' {
                    single = false;
                }
            } else if c == '"' {
                double = true;
            } else if c == '\'' {
                single = true;
            } else if c == '#' && (i == 0 || matches!(chars[i - 1], ' ' | '\t')) {
                comment = i;
                break;
            } else if matches!(c, '[' | '{') {
                depth += 1;
            } else if matches!(c, ']' | '}') {
                depth = depth.saturating_sub(1);
            }
            i += 1;
        }
        if single || double || depth > 0 {
            out.push(line.into());
            continue;
        }
        let code: String = chars[..comment].iter().collect();
        let trimmed = code.trim_end_matches(is_js_space);
        let scalar = trimmed.rsplit_once(':').is_some_and(|(_, s)| {
            let s = trim(s);
            s.starts_with(['>', '|']) && s.chars().skip(1).all(|c| c.is_ascii_digit() || matches!(c, '+' | '-'))
        });
        if scalar {
            block = Some(indent);
            out.push(line.into());
            continue;
        }
        let end = code.trim_end_matches([' ', '\t']).len();
        if code[..end].ends_with(',') {
            out.push(format!(
                "{}{}{}",
                &code[..end - 1],
                &code[end..],
                chars[comment..].iter().collect::<String>()
            ));
        } else {
            out.push(line.into());
        }
    }
    out.join("\n")
}
impl Model {
    /// Presentation controls are omitted from this machine-readable semantic view.
    pub fn semantic_model(&self) -> Value {
        let semantic_metadata = |metadata: &Option<Map<String, Value>>| -> Option<Value> {
            let mut out = metadata.clone()?;
            for key in [
                "shape",
                "view",
                "icon",
                "img",
                "form",
                "pos",
                "w",
                "h",
                "class",
                "style",
                "labelType",
            ] {
                out.remove(key);
            }
            (!out.is_empty()).then_some(Value::Object(out))
        };
        let mut vertices = vec![];
        let mut connectors = vec![];
        for (id, node) in &self.nodes {
            if self.subgraph_lookup.contains_key(id) {
                continue;
            }
            let mut value = json!({"id":id});
            let out = value.as_object_mut().unwrap();
            if node.is_connector {
                if let Some(text) = node.text.as_ref().filter(|s| *s != id) {
                    out.insert("title".into(), json!(text));
                }
                if let Some(metadata) = semantic_metadata(&node.metadata) {
                    out.insert("metadata".into(), metadata);
                }
                connectors.push(value);
                continue;
            }
            if let Some(text) = &node.text {
                out.insert("label".into(), json!(text));
            }
            if let Some(shape) = &node.kind {
                out.insert("shape".into(), json!(shapes::resolve(shape)));
            }
            out.insert("vertexKind".into(), json!(Self::vertex_kind(node)));
            if let Some(metadata) = semantic_metadata(&node.metadata) {
                out.insert("metadata".into(), metadata);
            }
            vertices.push(value);
        }
        let edges = self
            .edges
            .iter()
            .map(|e| {
                let mut value = json!({"start":e.start,"end":e.end,"id":e.id});
                let out = value.as_object_mut().unwrap();
                if !e.text.is_empty() {
                    out.insert("label".into(), json!(e.text));
                }
                if let Some(kind) = &e.kind {
                    out.insert("type".into(), json!(kind));
                }
                if let Some(stroke) = &e.stroke {
                    out.insert("stroke".into(), json!(stroke));
                }
                if let Some(semantic) = &e.edge_semantic {
                    out.insert("edgeSemantic".into(), json!(semantic));
                }
                if let Some(length) = e.length {
                    out.insert("length".into(), json!(length));
                }
                if let Some(metadata) = e.metadata.as_ref().filter(|m| !m.is_empty()) {
                    out.insert("metadata".into(), json!(metadata));
                }
                value
            })
            .collect::<Vec<_>>();
        let groups = self
            .subgraphs
            .iter()
            .map(|g| {
                let mut value = json!({"id":g.id,"nodes":g.nodes,"type":"flow","title":g.title});
                let out = value.as_object_mut().unwrap();
                if let Some(dir) = &g.dir {
                    out.insert("direction".into(), json!(dir));
                }
                if let Some(metadata) = semantic_metadata(&g.metadata) {
                    out.insert("metadata".into(), metadata);
                }
                value
            })
            .collect::<Vec<_>>();
        let mut out = json!({"vertices":vertices,"edges":edges,"subGraphs":groups,"connectors":connectors,"diagnostics":self.diagnostics.iter().map(Diagnostic::to_json).collect::<Vec<_>>()});
        if let Some(direction) = &self.direction {
            out.as_object_mut().unwrap().insert("direction".into(), json!(direction));
        }
        out
    }
    fn error_at(&self, loc: Option<Loc>) -> (usize, usize) {
        let Some(loc) = loc else {
            return (0, 0);
        };
        let src: Vec<u16> = self.source.encode_utf16().collect();
        let mut start = 0;
        for _ in 1..loc.first_line {
            start = src[start..].iter().position(|&c| c == 10).map_or(0, |i| start + i + 1);
        }
        let location = locate(&src, start + loc.first_column);
        (location.line, location.column)
    }
    fn shape_error(&self, message: impl Into<String>, loc: Option<Loc>) -> PeleError {
        let mut error = PeleError::new(message, PeleErrorCode::Semantic).of("agentflow");
        (error.line, error.column) = self.error_at(loc);
        error
    }
    fn metadata(&self, id: &str, source: &str, loc: Option<Loc>) -> Result<Value> {
        let multiline = source.contains('\n');
        let parsed = parse_yaml(&if multiline {
            format!("{source}\n")
        } else {
            format!("{{\n{source}\n}}")
        });
        let mut value = match parsed {
            Ok(v) => v,
            Err(error) => {
                let stripped = if multiline { strip_commas(source) } else { source.into() };
                if stripped != source
                    && let Ok(v) = parse_yaml(&(stripped + "\n"))
                {
                    let mut v = v;
                    strip_keys(&mut v);
                    return Ok(v);
                }
                let (line, column) = self.error_at(loc);
                let at = if loc.is_some() {
                    format!(" ({line}:{column})")
                } else {
                    String::new()
                };
                let mut out = PeleError::new(
                    format!("Metadata of \"{id}\" is not valid YAML. {error}{at}"),
                    PeleErrorCode::Syntax,
                )
                .of("agentflow");
                out.line = line;
                out.column = column;
                return Err(out);
            }
        };
        if value.is_null() {
            value = Value::Object(Map::new());
        }
        strip_keys(&mut value);
        Ok(value)
    }
    pub fn add_vertex(&mut self, id: &str, v: Vertex) -> Result<()> {
        if trim(id).is_empty() {
            return Ok(());
        }
        let mut doc = v.metadata.map(|s| self.metadata(id, s, v.metadata_loc)).transpose()?;
        let mut authored = None;
        if let Some(fields) = doc.as_mut().and_then(Value::as_object_mut)
            && let Some(shape) = fields.get("shape").and_then(Value::as_str)
        {
            authored = Some(shape.to_string());
            fields.insert("shape".into(), Value::String(shapes::resolve(shape).into()));
        }
        if let Some(&i) = self.subgraph_lookup.get(id)
            && let Some(doc) = &doc
        {
            merge(&mut self.subgraphs[i].metadata, doc);
            return Ok(());
        }
        if self.connectors.contains_key(id)
            && let Some(doc) = &doc
        {
            merge(&mut self.connectors.get_mut(id).unwrap().metadata, doc);
            merge(&mut self.nodes.get_mut(id).unwrap().metadata, doc);
            return Ok(());
        }
        if id == "connectors" {
            return Ok(());
        }
        if let Some(&i) = self.edge_by_id.get(id) {
            let edge = &mut self.edges[i];
            if let Some(fields) = doc.as_ref().and_then(Value::as_object) {
                edge.animate = fields.get("animate").cloned().or(edge.animate.clone());
                edge.animation = fields.get("animation").cloned().or(edge.animation.clone());
                if let Some(curve) = fields.get("curve") {
                    edge.interpolate = Some(value_string(curve));
                }
            }
            if let Some(doc) = &doc {
                merge(&mut edge.metadata, doc);
            }
            return Ok(());
        }
        let vertex = self.nodes.entry(id.into()).or_insert_with(|| Node::new(id));
        if let Some(text) = v.text {
            vertex.label_type = text.kind;
            vertex.text = Some(unquote(trim(&text.text)));
        } else if vertex.text.is_none() {
            vertex.text = Some(id.into());
        }
        if let Some(kind) = v.kind {
            vertex.kind = Some(kind.into());
        }
        if let Some(style) = v.style {
            vertex.styles.extend(style);
        }
        if let Some(classes) = v.classes {
            vertex.classes.extend(classes);
        }
        if let Some(dir) = v.dir {
            vertex.dir = Some(dir.into());
        }
        vertex.props.get_or_insert_with(IndexMap::new).extend(v.props.unwrap_or_default());
        let Some(doc) = doc else {
            return Ok(());
        };
        merge(&mut vertex.metadata, &doc);
        let Some(fields) = doc.as_object() else {
            return Ok(());
        };
        if let Some(shape) = fields.get("shape").filter(|v| truthy(Some(v))) {
            let original = authored.as_deref().map(Value::from).unwrap_or_else(|| shape.clone());
            let Some(authored) = original.as_str() else {
                return Err(self.shape_error(format!("No such shape: {}.", json_string(&original)), v.metadata_loc));
            };
            if authored != authored.to_lowercase() || authored.contains('_') {
                return Err(self.shape_error(
                    format!("No such shape: {authored}. Shape names should be lowercase."),
                    v.metadata_loc,
                ));
            }
            let shape = shape.as_str().unwrap_or("");
            if !shapes::valid(shape) {
                return Err(self.shape_error(format!("No such shape: {shape}."), v.metadata_loc));
            }
            self.nodes.get_mut(id).unwrap().kind = Some(shape.into());
        }
        if let Some(label) = fields.get("label").filter(|v| truthy(Some(v))) {
            let vertex = self.nodes.get_mut(id).unwrap();
            vertex.text = Some(value_string(label));
            vertex.label_type = label_type(fields.get("labelType").and_then(Value::as_str));
        }
        Ok(())
    }
    pub fn add_link(&mut self, starts: &[String], ends: &[String], info: Option<&LinkInfo>) -> Result<()> {
        let id = info.and_then(|i| i.id.as_ref()).map(|s| s.replacen('@', "", 1));
        for start in starts {
            for end in ends {
                if self.edges.len() >= self.max_edges {
                    return Err(PeleError::new(
                        format!("Edge limit exceeded. The limit is {}.", self.max_edges),
                        PeleErrorCode::Limit,
                    )
                    .of("agentflow"));
                }
                let pair = (start.clone(), end.clone());
                let count = *self.pair_count.get(&pair).unwrap_or(&0);
                let given = id.as_ref().filter(|s| {
                    !s.is_empty() && Some(start) == starts.last() && Some(end) == ends.first() && !self.edge_by_id.contains_key(*s)
                });
                let edge_id = given
                    .cloned()
                    .unwrap_or_else(|| format!("L_{start}_{end}_{}", if count == 0 { 0 } else { count + 1 }));
                let label = info.and_then(|i| i.text.as_ref());
                let edge = Edge {
                    start: start.clone(),
                    end: end.clone(),
                    kind: info.map(|i| i.kind.clone()),
                    text: label.map(|l| unquote(trim(&l.text))).unwrap_or_default(),
                    label_type: label.map(|l| l.kind).unwrap_or(LabelType::Text),
                    stroke: info.map(|i| i.stroke.clone()),
                    edge_semantic: info.and_then(|i| i.edge_semantic.clone()),
                    length: info.and_then(|i| i.length).map(|v| v.min(10.0)),
                    style: None,
                    classes: vec![],
                    id: edge_id.clone(),
                    is_user_defined_id: given.is_some(),
                    interpolate: self.default_interpolate.clone(),
                    animate: None,
                    animation: None,
                    metadata: None,
                };
                self.edge_by_id.entry(edge_id).or_insert(self.edges.len());
                self.edges.push(edge);
                self.pair_count.insert(pair, count + 1);
            }
        }
        Ok(())
    }
    fn edge_at(&self, pos: &str) -> Result<usize> {
        let i = js_number(pos);
        if i >= 0.0 && i.fract() == 0.0 && (i as usize) < self.edges.len() {
            Ok(i as usize)
        } else {
            Err(PeleError::new(format!("The index {pos} for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and {}. (Help: Ensure that the index is within the range of existing edges.)",self.edges.len()as i64-1),PeleErrorCode::Semantic).of("agentflow"))
        }
    }
    pub fn add_subgraph(&mut self, id_text: Option<&FlowText>, list: &[DocItem], title_text: Option<&FlowText>, _same: bool) -> String {
        let id = id_text
            .map(|s| trim(&s.text).to_string())
            .unwrap_or_else(|| format!("subGraph{}", self.sub_count));
        self.sub_count += 1;
        let title = title_text.map(|s| trim(&s.text)).unwrap_or("");
        let mut node_list = vec![];
        let mut seen = HashSet::new();
        let mut dir = None;
        for entry in list {
            let ids = match entry {
                DocItem::Str(s) => vec![s],
                DocItem::List(ids) => ids.iter().collect(),
                DocItem::Dir(value) => {
                    dir = Some(value.clone());
                    continue;
                }
            };
            for item in ids {
                if !trim(item).is_empty() && seen.insert(item.clone()) {
                    node_list.push(item.clone());
                }
            }
        }
        if dir.is_none() && self.inherit_dir {
            dir = self.direction.clone().or(self.configured_direction.clone());
        }
        let nodes: Vec<_> = node_list
            .into_iter()
            .filter(|n| n != &id && !self.global_nodes.contains(n) && !self.members.contains(n))
            .collect();
        self.members.extend(nodes.iter().cloned());
        if let Some(&i) = self.subgraph_lookup.get(&id) {
            let group = &mut self.subgraphs[i];
            group.nodes.extend(nodes);
            if !title.is_empty() {
                group.title = title.into();
            }
            if dir.is_some() {
                group.dir = dir;
            }
            return id;
        }
        let group = Subgraph {
            id: id.clone(),
            nodes,
            title: title.into(),
            classes: vec![],
            dir,
            label_type: title_text.map(|t| t.kind).unwrap_or(LabelType::Markdown),
            metadata: None,
        };
        self.subgraph_lookup.insert(id.clone(), self.subgraphs.len());
        self.subgraphs.push(group);
        id
    }
    pub fn add_global(&mut self, list: &[DocItem]) {
        let mut release = false;
        for item in list {
            let ids = match item {
                DocItem::Str(s) => vec![s],
                DocItem::List(ids) => ids.iter().collect(),
                DocItem::Dir(_) => continue,
            };
            for id in ids {
                let id = trim(id);
                if !id.is_empty() {
                    self.global_nodes.insert(id.into());
                    release |= self.members.remove(id);
                }
            }
        }
        if release {
            for group in &mut self.subgraphs {
                group.nodes.retain(|id| !self.global_nodes.contains(id));
            }
        }
    }
    pub fn add_connector(&mut self, text: Option<&FlowText>, title: Option<&FlowText>) -> String {
        let id = text.map(|s| trim(&s.text)).unwrap_or("");
        if id.is_empty() {
            return String::new();
        }
        let title = title.map(|s| trim(&s.text)).unwrap_or("");
        if !self.connectors.contains_key(id) {
            let mut node = Node::new(id);
            node.is_connector = true;
            node.text = Some(if title.is_empty() { id } else { title }.into());
            self.nodes.insert(id.into(), node.clone());
            self.connectors.insert(id.into(), node);
        } else if !title.is_empty() {
            self.connectors.get_mut(id).unwrap().text = Some(title.into());
            self.nodes.get_mut(id).unwrap().text = Some(title.into());
        }
        id.into()
    }
    pub fn destruct_link(&self, end: &str, start: Option<&str>) -> LinkInfo {
        let s = trim(end);
        let mut line = s.strip_suffix(|_: char| true).unwrap_or("");
        let kind = match s.chars().last() {
            Some('x') => "arrow_cross",
            Some('>') => "arrow_point",
            Some('-' | '.') => {
                line = s;
                "arrow_open"
            }
            _ => "arrow_open",
        };
        let dots = line.chars().filter(|&c| c == '.').count();
        let stroke = if dots > 0 { "dotted" } else { "normal" };
        if start.is_some_and(|s| if s.contains('.') { stroke != "dotted" } else { stroke != "normal" }) {
            return LinkInfo {
                kind: "INVALID".into(),
                stroke: "INVALID".into(),
                length: None,
                edge_semantic: None,
                text: None,
                id: None,
            };
        }
        let semantic = match (kind, stroke) {
            ("arrow_point", "normal") => Some("sequence"),
            ("arrow_cross", "normal") => Some("failure"),
            ("arrow_open", "dotted") => Some("reference"),
            _ => None,
        };
        LinkInfo {
            kind: kind.into(),
            stroke: stroke.into(),
            length: Some(if dots > 0 {
                dots as f64
            } else {
                line.encode_utf16().count() as f64 - 1.0
            }),
            edge_semantic: semantic.map(str::to_string),
            text: None,
            id: None,
        }
    }
    pub fn subgraph(&self, id: &str) -> Option<&Subgraph> {
        self.subgraph_lookup.get(id).map(|&i| &self.subgraphs[i])
    }
    pub fn vertex_kind(vertex: &Node) -> &'static str {
        let shape = vertex.kind.as_deref().unwrap_or("");
        if is_tool(vertex) {
            return "tool";
        }
        match shapes::resolve(shape) {
            "hexagon" | "hex" => "action",
            "lean-right" | "lean_right" => "input",
            "lin-doc" | "lined-document" => "refdoc",
            "diamond" => "decision",
            _ => "task",
        }
    }
    fn position(&self, loc: Loc) -> Position {
        Position {
            start_line: loc.first_line + self.line_offset,
            start_column: loc.first_column,
            end_line: loc.last_line + self.line_offset,
            end_column: loc.last_column,
            start_index: 0,
            end_index: 0,
        }
    }
    fn push_mapping(&mut self, id: &str, kind: &str, position: Position) -> usize {
        let i = self.mappings.len();
        self.first_mapping.entry(id.into()).or_insert(i);
        self.mappings.push(Mapping {
            id: id.into(),
            kind: kind.into(),
            position,
        });
        i
    }
    pub fn vertex_mapping(&mut self, id: &str, shape: Option<&str>, loc: Loc) {
        if id.is_empty() {
            return;
        }
        let i = self.push_mapping(id, "vertex", self.position(loc));
        if shape.is_none() {
            self.bare_mappings.insert(i);
        }
    }
    pub fn extend_mapping(&mut self, id: &str, loc: Loc) {
        if id.is_empty() {
            return;
        }
        let end = self.position(loc);
        if let Some(i) = self.mappings.iter().rposition(|m| m.kind == "vertex" && m.id == id) {
            if self.bare_mappings.remove(&i) {
                self.mappings[i].kind = "attachment".into();
            }
            let p = &mut self.mappings[i].position;
            if (end.end_line, end.end_column) > (p.end_line, p.end_column) {
                p.end_line = end.end_line;
                p.end_column = end.end_column;
            }
        } else {
            self.push_mapping(id, "attachment", end);
        }
    }
    pub fn edge_mapping(&mut self, to: &[String], loc: Loc) {
        let ids: Vec<_> = to.iter().filter(|s| !s.is_empty()).cloned().collect();
        self.push_mapping(
            &if ids.is_empty() { "edge".into() } else { ids.join(">") },
            "edge",
            self.position(loc),
        );
    }
    pub fn block_mapping(&mut self, id: &str, kind: &str, start: Loc, end: Loc) {
        if id.is_empty() {
            return;
        }
        let mut p = self.position(start);
        let end = self.position(end);
        p.end_line = end.end_line;
        p.end_column = end.end_column;
        self.push_mapping(id, kind, p);
    }
    pub fn diagnostic(&mut self, id: &str, severity: &str, message: String, node_id: Option<&str>, edge_id: Option<&str>) {
        let position = node_id
            .or(edge_id)
            .and_then(|id| self.first_mapping.get(id))
            .map(|&i| self.mappings[i].position.clone());
        let d = Diagnostic {
            id: id.into(),
            severity: severity.into(),
            message,
            node_id: node_id.map(str::to_string),
            edge_id: edge_id.map(str::to_string),
            position,
        };
        if !self.diagnostics.contains(&d) {
            self.diagnostics.push(d);
        }
    }
}
pub fn is_tool(node: &Node) -> bool {
    matches!(
        node.kind.as_deref(),
        Some("subroutine" | "subprocess" | "subproc" | "framed-rectangle" | "tool")
    )
}

impl Model {
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
            let class = self.classes.entry(id.to_string()).or_insert(ClassDef {
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
}
