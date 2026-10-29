//! Cynefin domains and transitions.
mod boundaries;
mod render;
use crate::diagrams::tokens::*;
use crate::errors::PeleError;
use crate::preprocess::Config;
use crate::util::{is_js_space, trim_start};
use indexmap::IndexMap;
pub use render::render;

#[derive(Debug, Clone)]
pub struct Transition {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub domains: IndexMap<String, Vec<String>>,
    pub transitions: Vec<Transition>,
}
fn mark(s: &'static str) -> TokenType {
    TokenType {
        name: s,
        pattern: Pattern::Literal(s),
        hidden: false,
        first: None,
    }
}
fn domain(s: &[u16], p: usize) -> Option<usize> {
    ["complex", "complicated", "clear", "chaotic", "confusion"]
        .into_iter()
        .find(|word| word.encode_utf16().enumerate().all(|(i, c)| s.get(p + i) == Some(&c)))
        .map(|v| p + v.len())
}
fn lines(r: &mut Reader) {
    while r.accept(9) {}
}
fn eol(r: &mut Reader) -> Result<(), PeleError> {
    if r.kind() != -1 && r.kind() != 9 {
        return Err(r.fail("a line break or the end of input"));
    }
    lines(r);
    Ok(())
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let types = vec![
        mark("cynefin-beta:"),
        keyword("cynefin-beta"),
        mark("-->"),
        mark(":"),
        TokenType {
            name: "DOMAIN_NAME",
            pattern: Pattern::Match(domain),
            hidden: false,
            first: Some("c"),
        },
        acc_descr_type(),
        acc_title_type(),
        title_type(),
        string_type(),
        newline_type(),
        whitespace_type(),
        yaml_type(),
        directive_type(),
        comment_type(),
    ];
    let tokens = tokenize(&source.encode_utf16().collect::<Vec<_>>(), &types, "cynefin")?;
    let mut r = Reader {
        i: 0,
        tokens,
        types: &types,
        diagram: "cynefin",
    };
    let mut model = Model::default();
    lines(&mut r);
    if !matches!(r.kind(), 0 | 1) {
        return Err(r.fail("'cynefin-beta'"));
    }
    r.i += 1;
    while r.kind() != -1 {
        match r.kind() {
            9 => r.i += 1,
            5 => {
                model.acc_descr = Some(acc_descr_value(&r.take()));
                eol(&mut r)?;
            }
            6 => {
                model.acc_title = Some(acc_title_value(&r.take()));
                eol(&mut r)?;
            }
            7 => {
                model.title = Some(title_value(&r.take()));
                eol(&mut r)?;
            }
            4 => {
                let name = r.take();
                if r.accept(2) {
                    let to = r.expect(4)?;
                    let label = if r.accept(3) {
                        Some(string_value(&r.expect(8)?)).filter(|v| !v.is_empty())
                    } else {
                        None
                    };
                    eol(&mut r)?;
                    if name != to {
                        model.transitions.push(Transition { from: name, to, label });
                    }
                } else {
                    lines(&mut r);
                    let mut items = vec![];
                    while r.kind() == 8 {
                        items.push(string_value(&r.take()));
                        lines(&mut r);
                    }
                    model.domains.insert(name, items);
                }
            }
            _ => return Err(r.fail("a domain, a transition, a title, or a line break")),
        }
    }
    model.title = model.title.filter(|v| !v.is_empty()).or_else(|| title.map(String::from));
    model.acc_title = model.acc_title.filter(|v| !v.is_empty()).map(|v| trim_start(&v).to_string());
    model.acc_descr = model.acc_descr.filter(|v| !v.is_empty()).map(|v| {
        let mut out = String::new();
        let mut chars = v.chars().peekable();
        while let Some(c) = chars.next() {
            out.push(c);
            if c == '\n' {
                while chars.peek().is_some_and(|&c| is_js_space(c)) {
                    chars.next();
                }
            }
        }
        out
    });
    Ok(model)
}
