mod lexer;
mod markers;
pub mod model;
pub mod parser;
pub mod render;
mod tokens;
use crate::diagrams::common::Rendered;
use crate::{Config, PeleError, RenderOptions, TextMeasurer};
pub use model::Model;
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let mut db = Model::default();
    db.title = title.map(str::to_owned);
    parser::parse(source, &mut db)?;
    Ok(db)
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    render::render_er(model, config, options, measurer)
}
