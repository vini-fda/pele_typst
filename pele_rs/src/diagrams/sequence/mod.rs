mod actors;
mod colors;
pub mod db;
mod kinds;
mod lexer;
mod parser;
mod render;
mod tokens;
use crate::{Config, PeleError};
pub use db::Model;
pub use render::render;
pub fn parse(source: &str, config: &Config, title: Option<&str>, _: usize, _: usize) -> Result<Model, PeleError> {
    let mut db = Model::new(config, title);
    parser::parse_sequence(source, &mut db)?;
    Ok(db)
}
