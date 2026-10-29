mod arrows;
mod layout;
mod lexer;
pub mod model;
pub mod parser;
mod render;
mod tokens;
use crate::{Config, PeleError, errors::PeleErrorCode};
pub use model::Model;
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let mut model = Model::default();
    model.title = title.map(str::to_owned);
    parser::parse(source, &mut model)?;
    Ok(model)
}
pub fn validate(model: &Model) -> Result<(), PeleError> {
    let mut pending = vec![model.root];
    while let Some(i) = pending.pop() {
        let block = &model.arena[i];
        pending.extend_from_slice(&block.children);
        if !block.children.is_empty() {
            let c = block.columns.unwrap_or(-1.);
            if !c.is_finite() || c == 0. || c.fract() != 0. {
                return Err(PeleError::new("Columns must be an integer !== 0.", PeleErrorCode::Semantic).of("block"));
            }
        }
    }
    Ok(())
}
pub use render::render;
