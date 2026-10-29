use super::{
    lexer::{Token, tokenize},
    model::{Model, Note, State, Stmt},
    tokens as t,
};
use crate::{
    PeleError,
    errors::{PeleErrorCode, locate, syntax_error},
};
const STATEMENT: &str = "'NL', 'ID', 'HIDE_EMPTY', 'scale', 'COMPOSIT_STATE', 'STATE_DESCR', 'FORK', 'JOIN', 'CHOICE', 'CONCURRENT', 'note', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'CLICK', 'classDef', 'style', 'class', 'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'EDGE_STATE'";
struct Parser {
    src: Vec<u16>,
    tokens: Vec<Token>,
    error: Option<String>,
    i: usize,
}
impl Parser {
    fn kind(&self) -> usize {
        self.tokens[self.i].kind
    }
    fn take(&mut self) -> String {
        let text = self.tokens[self.i].text.clone();
        self.i += 1;
        text
    }
    fn accept(&mut self, k: usize) -> bool {
        if self.kind() == k {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn fail(&self, expected: &str) -> PeleError {
        let token = &self.tokens[self.i];
        if token.kind == t::ERROR
            && let Some(message) = &self.error
        {
            let mut error = PeleError::new(message, PeleErrorCode::Syntax).of("state");
            let location = locate(&self.src, token.start);
            error.line = location.line;
            error.column = location.column;
            error.snippet = location.snippet;
            return error;
        }
        syntax_error(
            "state",
            &self.src,
            token.start,
            &if token.kind == t::ERROR {
                String::new()
            } else {
                format!("Expecting {expected}, got '{}'", t::TOKEN_NAMES[token.kind])
            },
            token.kind == t::ERROR,
        )
    }
    fn expect(&mut self, k: usize) -> Result<String, PeleError> {
        if self.kind() != k {
            return Err(self.fail(&format!("'{}'", t::TOKEN_NAMES[k])));
        }
        Ok(self.take())
    }
    fn id(&mut self) -> Result<State, PeleError> {
        if self.kind() != t::ID && self.kind() != t::EDGE_STATE {
            return Err(self.fail("'ID', 'EDGE_STATE'"));
        }
        let mut state = State {
            id: self.take().trim().into(),
            kind: Some("default".into()),
            ..Default::default()
        };
        if self.accept(t::STYLE_SEPARATOR) {
            state.classes.push(self.expect(t::ID)?.trim().into());
        }
        Ok(state)
    }
}
fn colon(s: String) -> String {
    s.strip_prefix(':').unwrap_or(&s).trim().into()
}
pub fn parse(src: &str, model: &mut Model) -> Result<(), PeleError> {
    let tokens = tokenize(src);
    let mut p = Parser {
        src: src.encode_utf16().collect(),
        tokens: tokens.out,
        error: tokens.error,
        i: 0,
    };
    while p.accept(t::NL) {}
    if !p.accept(t::SD) {
        return Err(p.fail("'NL', 'SD'"));
    }
    let mut doc = Vec::new();
    let mut outer: Vec<(Vec<Stmt>, State)> = Vec::new();
    loop {
        let kind = p.kind();
        let item = match kind {
            t::NL => {
                p.i += 1;
                None
            }
            t::ID | t::EDGE_STATE => {
                let mut first = p.id()?;
                if p.kind() == t::DESCR {
                    first.description = vec![colon(p.take())];
                    Some(Stmt::State(Box::new(first)))
                } else if p.accept(t::ARROW) {
                    let second = p.id()?;
                    let description = if p.kind() == t::DESCR { Some(colon(p.take())) } else { None };
                    Some(Stmt::Relation {
                        state1: Box::new(first),
                        state2: Box::new(second),
                        description,
                    })
                } else {
                    Some(Stmt::State(Box::new(first)))
                }
            }
            t::HIDE_EMPTY => Some(Stmt::Text(p.take())),
            t::scale => {
                let text = p.take();
                p.expect(t::WIDTH)?;
                Some(Stmt::Text(text))
            }
            t::COMPOSIT_STATE => {
                let id = p.take();
                if p.accept(t::STRUCT_START) {
                    outer.push((
                        std::mem::take(&mut doc),
                        State {
                            id,
                            kind: Some("default".into()),
                            ..Default::default()
                        },
                    ));
                    None
                } else {
                    Some(Stmt::Text(id))
                }
            }
            t::STATE_DESCR => {
                let description = p.take();
                p.expect(t::AS)?;
                let id = p.expect(t::ID)?;
                if p.accept(t::STRUCT_START) {
                    outer.push((
                        std::mem::take(&mut doc),
                        State {
                            id,
                            kind: Some("default".into()),
                            description: vec![description],
                            ..Default::default()
                        },
                    ));
                    None
                } else if id.contains(':') {
                    let parts: Vec<_> = id.split(':').collect();
                    Some(Stmt::State(Box::new(State {
                        id: parts[0].into(),
                        kind: Some("default".into()),
                        description: vec![description.trim().into(), parts[1].into()],
                        description_array: true,
                        ..Default::default()
                    })))
                } else {
                    Some(Stmt::State(Box::new(State {
                        id,
                        kind: Some("default".into()),
                        description: vec![description.trim().into()],
                        ..Default::default()
                    })))
                }
            }
            t::FORK | t::JOIN | t::CHOICE => Some(Stmt::State(Box::new(State {
                id: p.take(),
                kind: Some(
                    if kind == t::FORK {
                        "fork"
                    } else if kind == t::JOIN {
                        "join"
                    } else {
                        "choice"
                    }
                    .into(),
                ),
                ..Default::default()
            }))),
            t::CONCURRENT => {
                p.i += 1;
                Some(Stmt::State(Box::new(State {
                    id: model.divider(),
                    kind: Some("divider".into()),
                    ..Default::default()
                })))
            }
            t::note => {
                let keyword = p.take();
                if p.kind() == t::left_of || p.kind() == t::right_of {
                    let position = Some(p.take().trim().into());
                    let id = p.expect(t::ID)?.trim().into();
                    let text = p.expect(t::NOTE_TEXT)?.trim().into();
                    Some(Stmt::State(Box::new(State {
                        id,
                        note: Some(Note { position, text }),
                        ..Default::default()
                    })))
                } else {
                    if p.kind() != t::NOTE_TEXT {
                        return Err(p.fail("'NOTE_TEXT', 'left_of', 'right_of'"));
                    }
                    p.i += 1;
                    p.expect(t::AS)?;
                    p.expect(t::ID)?;
                    Some(Stmt::Text(keyword))
                }
            }
            t::direction_tb | t::direction_bt | t::direction_rl | t::direction_lr => {
                p.i += 1;
                Some(Stmt::Direction(["TB", "BT", "RL", "LR"][kind - t::direction_tb].into()))
            }
            t::acc_title => {
                p.i += 1;
                let value = p.expect(t::acc_title_value)?.trim().to_owned();
                model.acc_title = Some(value.trim_start().into());
                Some(Stmt::Text(value))
            }
            t::acc_descr => {
                p.i += 1;
                let value = p.expect(t::acc_descr_value)?.trim().to_owned();
                model.set_descr(value.clone());
                Some(Stmt::Text(value))
            }
            t::acc_descr_multiline_value => {
                let value = p.take().trim().to_owned();
                model.set_descr(value.clone());
                Some(Stmt::Text(value))
            }
            t::CLICK => {
                p.i += 1;
                let id = p.id()?.id;
                let (url, tooltip) = if p.accept(t::HREF) {
                    (p.expect(t::STRING)?, String::new())
                } else {
                    if p.kind() != t::STRING {
                        return Err(p.fail("'STRING', 'HREF'"));
                    }
                    (p.take(), p.expect(t::STRING)?)
                };
                p.expect(t::NL)?;
                Some(Stmt::Click { id, url, tooltip })
            }
            t::classDef => {
                p.i += 1;
                let id = p.expect(t::CLASSDEF_ID)?.trim().into();
                let classes = p.expect(t::CLASSDEF_STYLEOPTS)?.trim().into();
                Some(Stmt::ClassDef { id, classes })
            }
            t::style => {
                p.i += 1;
                let id = p.expect(t::STYLE_IDS)?.trim().into();
                let style_class = p.expect(t::STYLEDEF_STYLEOPTS)?.trim().into();
                Some(Stmt::Style { id, style_class })
            }
            t::class => {
                p.i += 1;
                let id = p.expect(t::CLASSENTITY_IDS)?.trim().into();
                let style_class = p.expect(t::STYLECLASS)?.trim().into();
                Some(Stmt::ApplyClass { id, style_class })
            }
            t::STRUCT_STOP => {
                let Some((parent, mut state)) = outer.pop() else {
                    return Err(p.fail(&format!("'$end', {STATEMENT}")));
                };
                p.i += 1;
                state.doc = Some(std::mem::replace(&mut doc, parent).into());
                Some(Stmt::State(Box::new(state)))
            }
            t::END => {
                if !outer.is_empty() {
                    return Err(p.fail(&format!("'STRUCT_STOP', {STATEMENT}")));
                }
                model.set_root(doc)?;
                return Ok(());
            }
            _ => {
                return Err(p.fail(&format!(
                    "{}, {STATEMENT}",
                    if outer.is_empty() { "'$end'" } else { "'STRUCT_STOP'" }
                )));
            }
        };
        if let Some(item) = item
            && !matches!(&item,Stmt::Text(s)if s=="nl")
        {
            doc.push(item);
        }
    }
}
