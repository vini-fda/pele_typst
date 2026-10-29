mod graph;
mod lexer;
mod markers;
pub mod model;
pub mod parser;
mod render;
mod tokens;
use crate::{Config, PeleError};
pub use model::Model;
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let mut model = Model::default();
    model.title = title.map(str::to_owned);
    parser::parse(source, &mut model)?;
    Ok(model)
}
pub use render::render;
