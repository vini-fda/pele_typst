//! Agent flows, their semantic model, diagnostics, and source mappings.
pub mod db;
pub mod graph;
pub mod lexer;
mod parser;
mod render;
pub mod shapes;
#[cfg(test)]
mod tests;
use crate::errors::PeleError;
use crate::preprocess::Config;
pub use db::Model;
pub use render::render;
pub fn parse(source: &str, config: &Config, title: Option<&str>, line_offset: usize, max_edges: usize) -> Result<Model, PeleError> {
    let mut db = Model::default();
    db.source = source.into();
    db.line_offset = line_offset;
    db.max_edges = max_edges;
    db.inherit_dir = config.get("flowchart").and_then(|v| v.get("inheritDir")).and_then(|v| v.as_bool()) == Some(true);
    db.configured_direction = config.get("direction").and_then(|v| v.as_str()).map(str::to_string);
    db.title = title.filter(|s| !s.is_empty()).map(str::to_string);
    parser::parse_agentflow(source, &mut db)?;
    for (id, node) in &mut db.connectors {
        *node = db.nodes[id].clone();
    }
    graph::build(
        &mut db,
        8,
        config.get("flowchart").and_then(|v| v.get("curve")).and_then(|v| v.as_str()),
    );
    Ok(db)
}
