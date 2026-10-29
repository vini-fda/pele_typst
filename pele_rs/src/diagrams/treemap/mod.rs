mod layout;
mod render;
use crate::PeleError;
use crate::diagrams::timeline::story;
use crate::diagrams::tokens::{self, Pattern, Reader, TokenType};
use crate::preprocess::Config;
use crate::util::{is_word16, parse_float};
use indexmap::IndexMap;
pub use render::render;
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub children: Option<Vec<usize>>,
    pub value: Option<f64>,
    pub class_selector: Option<String>,
    pub css_compiled_styles: Vec<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub roots: Vec<usize>,
    pub nodes: Vec<Node>,
    pub classes: IndexMap<String, Vec<String>>,
}
macro_rules! matcher {
    ($name:ident,$pattern:expr) => {
        fn $name(src: &[u16], p: usize) -> Option<usize> {
            static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
            let text = String::from_utf16_lossy(&src[p..]);
            RE.get_or_init(|| regex::Regex::new($pattern).unwrap())
                .find(&text)
                .map(|m| p + m.as_str().encode_utf16().count())
        }
    };
}
matcher!(head, r"^(?:treemap-beta|treemap)");
matcher!(classdef, r"^classDef\s+([a-zA-Z_][a-zA-Z0-9_]+)(?:\s+([^;\r\n]*))?;?");
matcher!(id, r"^[a-zA-Z_][a-zA-Z0-9_]*");
matcher!(number, r"^[0-9_.,]+");
matcher!(string, r#"^(?:"[^"]*"|'[^']*')"#);
matcher!(comment, r"^%%[^\n]*");
fn title(src: &[u16], p: usize) -> Option<usize> {
    let mut at = p;
    while src.get(at).is_some_and(|c| matches!(c, 9 | 32)) {
        at += 1
    }
    if src.get(at + 5).is_some_and(|c| is_word16(*c as u32)) {
        return None;
    }
    tokens::title(src, p)
}
fn token(name: &'static str, matcher: tokens::Matcher, hidden: bool) -> TokenType {
    TokenType {
        name,
        pattern: Pattern::Match(matcher),
        hidden,
        first: None,
    }
}
pub fn parse(
    source: &str,
    _config: &Config,
    front_title: Option<&str>,
    _line_offset: usize,
    _max_edges: usize,
) -> Result<Model, PeleError> {
    let types = vec![
        tokens::acc_descr_type(),
        tokens::acc_title_type(),
        token("TITLE", title, false),
        token("TREEMAP_KEYWORD", head, false),
        token("CLASS_DEF", classdef, false),
        TokenType {
            name: "STYLE_SEPARATOR",
            pattern: Pattern::Literal(":::"),
            hidden: false,
            first: None,
        },
        TokenType {
            name: "SEPARATOR",
            pattern: Pattern::Literal(":"),
            hidden: false,
            first: None,
        },
        TokenType {
            name: "COMMA",
            pattern: Pattern::Literal(","),
            hidden: false,
            first: None,
        },
        token("INDENTATION", tokens::whitespace, false),
        token("WS", tokens::whitespace, true),
        token("ML_COMMENT", comment, true),
        token("NL", tokens::newline, true),
        token("ID2", id, false),
        token("NUMBER2", number, false),
        token("STRING2", string, false),
    ];
    let src: Vec<u16> = source.encode_utf16().collect();
    let mut r = Reader {
        i: 0,
        tokens: tokens::tokenize(&src, &types, "treemap")?,
        types: &types,
        diagram: "treemap",
    };
    let mut model = Model::default();
    let mut rows: Vec<(usize, Node)> = Vec::new();
    static CLASS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = CLASS.get_or_init(|| regex::Regex::new(r"classDef\s+([A-Z_a-z][A-Za-z0-9_]+)(?:\s+([^\n\r;]*))?;?").unwrap());
    r.expect(3)?;
    while r.kind() != -1 {
        match r.kind() {
            0 => {
                model.acc_descr = Some(story::squeeze(&tokens::acc_descr_value(&r.take())));
                continue;
            }
            1 => {
                model.acc_title = Some(story::acc(&tokens::acc_title_value(&r.take())));
                continue;
            }
            2 => {
                model.title = Some(tokens::title_value(&r.take()));
                continue;
            }
            8 | 14 | 4 => {}
            _ => return Err(r.fail("a node, a class definition, or a title")),
        }
        let indent = if r.kind() == 8 { r.take().encode_utf16().count() } else { 0 };
        if r.kind() == 4 {
            let text = r.take();
            let c = re.captures(&text).unwrap();
            let styles = model.classes.entry(c[1].into()).or_default();
            let text = c.get(2).map(|m| m.as_str()).unwrap_or("").replace("\\,", "§§§");
            let text = text.replace(',', ";").replace("§§§", ",");
            styles.extend(text.split(';').map(str::to_string));
            continue;
        }
        let text = r.expect(14)?;
        let name = text[1..text.len() - 1].to_string();
        let after = if r.tokens.kinds[r.i] == 8 { r.i + 1 } else { r.i };
        let value = if matches!(r.tokens.kinds[after], 6 | 7) {
            r.i = after + 1;
            r.accept(8);
            Some(parse_float(&r.expect(13)?.replace(',', "")))
        } else {
            None
        };
        let class_selector = if r.accept(5) { Some(r.expect(12)?) } else { None };
        rows.push((
            indent,
            Node {
                name,
                children: if value.is_some() { None } else { Some(Vec::new()) },
                value,
                class_selector,
                css_compiled_styles: Vec::new(),
            },
        ));
    }
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for (level, mut node) in rows {
        if let Some(class) = &node.class_selector {
            node.css_compiled_styles = model.classes.get(class).cloned().unwrap_or_default()
        }
        while stack.last().is_some_and(|(_, l)| *l >= level) {
            stack.pop();
        }
        let index = model.nodes.len();
        if let Some((p, _)) = stack.last() {
            model.nodes[*p].children.as_mut().unwrap().push(index)
        } else {
            model.roots.push(index)
        }
        if node.children.is_some() {
            stack.push((index, level))
        }
        model.nodes.push(node)
    }
    if model.title.as_ref().is_none_or(|s| s.is_empty()) {
        model.title = front_title.map(str::to_string)
    }
    model.acc_title = model.acc_title.filter(|s| !s.is_empty());
    model.acc_descr = model.acc_descr.filter(|s| !s.is_empty());
    Ok(model)
}
