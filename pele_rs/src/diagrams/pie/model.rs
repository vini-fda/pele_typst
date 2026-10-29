use indexmap::IndexMap;

use super::parser::PieAst;
use crate::errors::{PeleError, PeleErrorCode};
use crate::util::{is_js_space, js_num, trim_start};

#[derive(Debug, Clone)]
pub struct PieModel {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub show_data: bool,
    pub sections: IndexMap<String, f64>,
}

// `text.replace(/\n\s+/g, '\n')`
fn squeeze_indents(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '\n' {
            while chars.peek().is_some_and(|&n| is_js_space(n)) {
                chars.next();
            }
        }
    }
    out
}

pub fn build_pie(ast: PieAst, title: Option<&str>) -> Result<PieModel, PeleError> {
    let mut sections = IndexMap::new();
    for (label, value) in ast.sections {
        if value < 0.0 {
            return Err(PeleError::new(
                format!(
                    "\"{}\" has invalid value: {}. Negative values are not allowed in pie charts. All slice values must be >= 0.",
                    label,
                    js_num(value)
                ),
                PeleErrorCode::Semantic,
            )
            .of("pie"));
        }
        sections.entry(label).or_insert(value);
    }
    Ok(PieModel {
        title: ast.title.filter(|t| !t.is_empty()).or_else(|| title.map(String::from)),
        acc_title: ast.acc_title.filter(|t| !t.is_empty()).map(|t| trim_start(&t).to_string()),
        acc_descr: ast.acc_descr.filter(|t| !t.is_empty()).map(|t| squeeze_indents(&t)),
        show_data: ast.show_data,
        sections,
    })
}
