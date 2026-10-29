mod layout;
mod lexer;
pub mod model;
pub mod parser;
mod render;
mod route;
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
    use std::collections::HashSet;
    let mut children = vec![vec![]; model.boundaries.len()];
    for (i, b) in model.boundaries.iter().enumerate() {
        if let Some(p) = b.parent
            && p < children.len()
        {
            children[p].push(i);
        }
    }
    let mut visited = HashSet::from([0]);
    let mut stack = vec![0];
    while let Some(i) = stack.pop() {
        for &child in &children[i] {
            if visited.insert(child) {
                stack.push(child);
            }
        }
    }
    let exists = |alias: &str| match model.get(alias) {
        Some(model::Ref::Shape(i)) => model.shapes[i].parent.is_some_and(|p| visited.contains(&p)),
        Some(model::Ref::Boundary(i)) => i != 0 && visited.contains(&i),
        None => false,
    };
    for rel in &model.rels {
        if rel.from_attr.is_some() || rel.to_attr.is_some() || !exists(&rel.from) || !exists(&rel.to) {
            return Err(PeleError::new(
                format!("C4 rel \"{}\" -> \"{}\" references an unknown shape or boundary", rel.from, rel.to),
                PeleErrorCode::Semantic,
            )
            .of("c4"));
        }
    }
    Ok(())
}
pub use render::render;
