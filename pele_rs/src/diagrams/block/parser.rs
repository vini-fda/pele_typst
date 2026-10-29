use super::{
    lexer::{Token, tokenize},
    model::{Block, Model, shape},
    tokens as t,
};
use crate::{PeleError, errors::syntax_error};
const STATEMENT_START: &str = "'NODE_ID', 'COLUMNS', 'SPACE_BLOCK', 'BLOCK_DIAGRAM_KEY', 'id-block', 'classDef', 'class', 'style'";
struct Parser<'a> {
    src: Vec<u16>,
    tokens: Vec<Token>,
    i: usize,
    db: &'a mut Model,
}
impl Parser<'_> {
    fn kind(&self) -> usize {
        self.tokens[self.i].kind
    }
    fn take(&mut self) -> String {
        let s = self.tokens[self.i].text.clone();
        self.i += 1;
        s
    }
    fn fail(&self, w: &str) -> PeleError {
        let t = &self.tokens[self.i];
        syntax_error(
            "block",
            &self.src,
            t.start,
            &if t.kind == t::ERROR {
                String::new()
            } else {
                format!("Expecting {w}, got '{}'", t::TOKEN_NAMES[t.kind])
            },
            t.kind == t::ERROR,
        )
    }
    fn expect(&mut self, k: usize) -> Result<String, PeleError> {
        if self.kind() != k {
            return Err(self.fail(&format!("'{}'", t::TOKEN_NAMES[k])));
        }
        Ok(self.take())
    }
    fn node(&mut self) -> Result<Block, PeleError> {
        let id = self.expect(t::NODE_ID)?;
        let mut out = Block {
            id,
            kind: "na".into(),
            ..Default::default()
        };
        if self.kind() == t::NODE_DSTART {
            let open = self.take();
            out.label = Some(self.expect(t::STR)?);
            let close = self.expect(t::NODE_DEND)?;
            out.kind = shape(Some(&(open + &close))).into();
        } else if self.kind() == t::BLOCK_ARROW_START {
            let open = self.take();
            out.label = Some(self.expect(t::STR)?);
            out.directions.push(self.expect(t::DIR)?);
            while self.kind() == t::DIR {
                out.directions.push(self.take());
            }
            let close = self.expect(t::BLOCK_ARROW_END)?;
            out.kind = shape(Some(&(open + &close))).into();
        }
        Ok(out)
    }
    fn node_statement(&mut self) -> Result<Vec<usize>, PeleError> {
        let mut from = self.node()?;
        from.span = Some(if self.kind() == t::SIZE {
            self.take().parse::<f64>().unwrap_or(f64::NAN)
        } else {
            1.
        });
        let mut chain = Vec::new();
        while matches!(self.kind(), t::LINK | t::START_LINK) {
            let start = self.kind() == t::START_LINK;
            self.i += 1;
            let mut label = String::new();
            if start {
                self.expect(t::LINK_LABEL)?;
                label = self.expect(t::STR)?;
                self.expect(t::LINK)?;
            }
            let link = self.tokens[self.i - 1].text.clone();
            let to = self.node()?;
            if chain.is_empty() {
                from.span = None;
                chain.push(self.db.push(from.clone()));
            }
            let arrow_end = match link.trim().chars().last() {
                Some('x') => "arrow_cross",
                Some('o') => "arrow_circle",
                Some('>') => "arrow_point",
                _ => "",
            };
            let arrow_start = match link.trim().chars().next() {
                Some('x') => "arrow_cross",
                Some('o') => "arrow_circle",
                Some('<') => "arrow_point",
                _ => "arrow_open",
            };
            let edge = Block {
                id: format!("{}-{}", from.id, to.id),
                start: Some(from.id.clone()),
                end: Some(to.id.clone()),
                label: Some(label),
                kind: "edge".into(),
                thickness: if link.contains("==") { "thick" } else { "normal" }.into(),
                pattern: if link.contains(".-") { "dotted" } else { "solid" }.into(),
                directions: to.directions.clone(),
                arrow_start: arrow_start.into(),
                arrow_end: arrow_end.into(),
                ..Default::default()
            };
            chain.push(self.db.push(edge));
            from = to;
            chain.push(self.db.push(from.clone()));
        }
        if chain.is_empty() {
            chain.push(self.db.push(from));
        }
        Ok(chain)
    }
}
pub fn parse(src: &str, db: &mut Model) -> Result<(), PeleError> {
    let mut p = Parser {
        src: src.encode_utf16().collect(),
        tokens: tokenize(src),
        i: 0,
        db,
    };
    p.expect(t::KEY)?;
    let mut open: Vec<(Vec<usize>, Option<Vec<usize>>)> = Vec::new();
    let mut list = Vec::new();
    loop {
        match p.kind() {
            t::NODE_ID => list.extend(p.node_statement()?),
            t::COLUMNS => {
                let columns = p.take().parse::<f64>().unwrap_or(f64::NAN);
                list.push(p.db.push(Block {
                    kind: "column-setting".into(),
                    columns: Some(columns),
                    ..Default::default()
                }));
            }
            t::SPACE_BLOCK => {
                let width = p.take().parse::<f64>().unwrap_or(f64::NAN);
                let id = p.db.id();
                list.push(p.db.push(Block {
                    id,
                    kind: "space".into(),
                    label: Some(String::new()),
                    width: Some(width),
                    ..Default::default()
                }));
            }
            t::KEY => {
                p.i += 1;
                open.push((std::mem::take(&mut list), None));
            }
            t::ID_BLOCK => {
                p.i += 1;
                if p.kind() != t::NODE_ID {
                    return Err(p.fail("'NODE_ID'"));
                }
                let header = p.node_statement()?;
                open.push((std::mem::take(&mut list), Some(header)));
            }
            t::end => {
                let Some((parent, header)) = open.pop().filter(|_| !list.is_empty()) else {
                    return Err(p.fail(STATEMENT_START));
                };
                p.i += 1;
                let id = p.db.id();
                let children = std::mem::replace(&mut list, parent);
                let mut block = match header {
                    None => Block {
                        id,
                        label: Some(String::new()),
                        ..Default::default()
                    },
                    Some(v) if v.len() == 1 => p.db.arena[v[0]].clone(),
                    _ => Block::default(),
                };
                block.kind = "composite".into();
                block.children = children;
                list.push(p.db.push(block));
            }
            t::CLASSDEF => {
                p.i += 1;
                let id = p.expect(t::CLASSDEF_ID)?.trim().into();
                let css = Some(p.expect(t::CLASSDEF_STYLEOPTS)?.trim().into());
                list.push(p.db.push(Block {
                    id,
                    kind: "classDef".into(),
                    css,
                    ..Default::default()
                }));
            }
            t::CLASS => {
                p.i += 1;
                let id = p.expect(t::CLASSENTITY_IDS)?.trim().into();
                let style_class = Some(p.expect(t::STYLECLASS)?.trim().into());
                list.push(p.db.push(Block {
                    id,
                    kind: "applyClass".into(),
                    style_class,
                    ..Default::default()
                }));
            }
            t::STYLE => {
                p.i += 1;
                let id = p.expect(t::STYLE_ENTITY_IDS)?.trim().into();
                let styles_str = Some(p.expect(t::STYLE_DEFINITION_DATA)?.trim().into());
                list.push(p.db.push(Block {
                    id,
                    kind: "applyStyles".into(),
                    styles_str,
                    ..Default::default()
                }));
            }
            t::EOF => {
                if !open.is_empty() {
                    return Err(p.fail(&(STATEMENT_START.to_owned() + ", 'end'")));
                }
                if list.is_empty() {
                    return Err(p.fail(STATEMENT_START));
                }
                return p.db.hierarchy(list);
            }
            _ => {
                return Err(p.fail(
                    &(STATEMENT_START.to_owned()
                        + if list.is_empty() {
                            ""
                        } else if !open.is_empty() {
                            ", 'end'"
                        } else {
                            ", 'EOF'"
                        }),
                ));
            }
        }
    }
}
