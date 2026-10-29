mod db;
mod graph;
mod json;
mod lexer;
mod parser;
mod render;
mod tokens;
use crate::{PeleError, preprocess::Config};
use indexmap::IndexMap;
pub use render::render;
pub type Span = (usize, usize);
#[derive(Clone, Debug, Default)]
pub struct Label {
    pub text: String,
    pub markdown: bool,
    pub span: Span,
}
#[derive(Clone, Debug, Default)]
pub struct Element {
    pub id: String,
    pub label: String,
    pub markdown: bool,
    pub kind: String,
    pub actor_type: String,
    pub shape: String,
    pub icon: Option<String>,
    pub business: bool,
    pub stereotype: Option<String>,
    pub parent: Option<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Boundary {
    pub id: String,
    pub label: String,
    pub markdown: bool,
    pub kind: String,
    pub members: Vec<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Relationship {
    pub id: String,
    pub explicit_id: bool,
    pub source: String,
    pub target: String,
    pub kind: String,
    pub arrow: usize,
    pub label: Option<Label>,
    pub minlen: f64,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub animate: bool,
    pub animation: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Note {
    pub id: String,
    pub target: String,
    pub label: Label,
}
#[derive(Clone, Debug)]
pub struct Json {
    pub id: String,
    pub value: json::Data,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub actors: IndexMap<String, Element>,
    pub usecases: IndexMap<String, Element>,
    pub boundaries: IndexMap<String, Boundary>,
    pub relationships: Vec<Relationship>,
    pub notes: Vec<Note>,
    pub jsons: IndexMap<String, Json>,
    pub class_defs: IndexMap<String, Vec<String>>,
    pub direction: String,
    pub title: Option<String>,
    pub acc_title: String,
    pub acc_descr: String,
    pub source: String,
}
#[derive(Clone, Debug)]
pub struct Property {
    pub key: String,
    pub value: serde_json::Value,
    pub span: Span,
    pub key_span: Span,
    pub value_span: Span,
}
#[derive(Clone, Debug, Default)]
pub struct Item {
    pub id: String,
    pub label: Label,
    pub span: Span,
    pub generated: bool,
    pub shape: Option<String>,
    pub metadata: Option<Vec<Property>>,
    pub stereotype: Option<String>,
    pub classes: Vec<String>,
    pub explicit: bool,
    pub node_span: Span,
}
#[derive(Clone, Debug)]
pub struct DraftElement {
    pub item: Item,
    pub kind: String,
    pub parent: Option<Item>,
}
#[derive(Clone, Debug)]
pub struct Endpoint {
    pub item: Item,
    pub declares: bool,
    pub classed: bool,
}
#[derive(Clone, Debug)]
pub struct DraftRelation {
    pub source: Endpoint,
    pub target: Endpoint,
    pub span: Span,
    pub id: Option<String>,
    pub id_span: Option<Span>,
    pub kind: String,
    pub arrow: usize,
    pub label: Option<Label>,
    pub minlen: f64,
}
pub type ClassAssignment = (Vec<(String, Span)>, Vec<String>);
#[derive(Clone, Debug, Default)]
pub struct Drafts {
    pub elements: Vec<DraftElement>,
    pub boundaries: Vec<Item>,
    pub jsons: Vec<(Json, Span)>,
    pub relations: Vec<DraftRelation>,
    pub notes: Vec<(String, Span, Label)>,
    pub metadata: Vec<(String, Span, Vec<Property>)>,
    pub classes: Vec<ClassAssignment>,
    pub styles: Vec<(String, Span, Vec<String>)>,
    pub class_defs: Vec<(Vec<String>, Vec<String>)>,
    pub direction: Option<String>,
    pub acc_title: String,
    pub acc_descr: String,
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let drafts = parser::parse(source)?;
    db::resolve(source, drafts, title)
}
