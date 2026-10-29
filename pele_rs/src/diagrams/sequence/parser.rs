use super::{kinds as lt, tokens as t};
fn signal(t: usize) -> Option<i32> {
    match t {
        t::SOLID_OPEN_ARROW => Some(lt::SOLID_OPEN),
        t::DOTTED_OPEN_ARROW => Some(lt::DOTTED_OPEN),
        t::SOLID_ARROW => Some(lt::SOLID),
        t::DOTTED_ARROW => Some(lt::DOTTED),
        t::BIDIRECTIONAL_SOLID_ARROW => Some(lt::BIDIRECTIONAL_SOLID),
        t::BIDIRECTIONAL_DOTTED_ARROW => Some(lt::BIDIRECTIONAL_DOTTED),
        t::SOLID_CROSS => Some(lt::SOLID_CROSS),
        t::DOTTED_CROSS => Some(lt::DOTTED_CROSS),
        t::SOLID_POINT => Some(lt::SOLID_POINT),
        t::DOTTED_POINT => Some(lt::DOTTED_POINT),
        t::SOLID_ARROW_TOP => Some(lt::SOLID_TOP),
        t::SOLID_ARROW_BOTTOM => Some(lt::SOLID_BOTTOM),
        t::STICK_ARROW_TOP => Some(lt::STICK_TOP),
        t::STICK_ARROW_BOTTOM => Some(lt::STICK_BOTTOM),
        t::SOLID_ARROW_TOP_DOTTED => Some(lt::SOLID_TOP_DOTTED),
        t::SOLID_ARROW_BOTTOM_DOTTED => Some(lt::SOLID_BOTTOM_DOTTED),
        t::STICK_ARROW_TOP_DOTTED => Some(lt::STICK_TOP_DOTTED),
        t::STICK_ARROW_BOTTOM_DOTTED => Some(lt::STICK_BOTTOM_DOTTED),
        t::SOLID_ARROW_TOP_REVERSE => Some(lt::SOLID_ARROW_TOP_REVERSE),
        t::SOLID_ARROW_BOTTOM_REVERSE => Some(lt::SOLID_ARROW_BOTTOM_REVERSE),
        t::STICK_ARROW_TOP_REVERSE => Some(lt::STICK_ARROW_TOP_REVERSE),
        t::STICK_ARROW_BOTTOM_REVERSE => Some(lt::STICK_ARROW_BOTTOM_REVERSE),
        t::SOLID_ARROW_TOP_REVERSE_DOTTED => Some(lt::SOLID_ARROW_TOP_REVERSE_DOTTED),
        t::SOLID_ARROW_BOTTOM_REVERSE_DOTTED => Some(lt::SOLID_ARROW_BOTTOM_REVERSE_DOTTED),
        t::STICK_ARROW_TOP_REVERSE_DOTTED => Some(lt::STICK_ARROW_TOP_REVERSE_DOTTED),
        t::STICK_ARROW_BOTTOM_REVERSE_DOTTED => Some(lt::STICK_ARROW_BOTTOM_REVERSE_DOTTED),
        _ => None,
    }
}
use super::db::{Model, Statement, Text, box_data, parse_message};
use super::lexer::{Tokens, tokenize};
use crate::errors::{PeleError, syntax_error};
use crate::util::trim;
const SIGNAL_TOKENS: &str =
    "'SOLID_OPEN_ARROW', 'DOTTED_OPEN_ARROW', 'SOLID_ARROW', 'DOTTED_ARROW', 'SOLID_CROSS', 'DOTTED_CROSS', 'SOLID_POINT', 'DOTTED_POINT'";
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Top,
    Box,
    Loop,
    Rect,
    Opt,
    Break,
    Alt,
    Par,
    Critical,
}
struct Frame {
    kind: Kind,
    text: String,
    start_type: i32,
    list: Vec<Statement>,
    sections: Vec<(Vec<Statement>, String)>,
}
impl Frame {
    fn new(kind: Kind, text: String, start_type: i32) -> Self {
        Self {
            kind,
            text,
            start_type,
            list: if kind == Kind::Top {
                Vec::new()
            } else {
                vec![Statement::default()]
            },
            sections: Vec::new(),
        }
    }
}
struct Parser<'a> {
    src: Vec<u16>,
    tokens: Tokens,
    i: usize,
    db: &'a mut Model,
}
impl Parser<'_> {
    fn t(&self) -> usize {
        self.tokens.types.get(self.i).copied().unwrap_or(t::END)
    }
    fn fail(&self, expected: &str) -> PeleError {
        let token = self.t();
        let at = self.tokens.starts.get(self.i).copied().unwrap_or(self.src.len());
        if token == t::ERROR {
            syntax_error("sequence", &self.src, at, "", true)
        } else {
            syntax_error(
                "sequence",
                &self.src,
                at,
                &format!("Expecting {expected}, got '{}'", t::NAMES[token]),
                false,
            )
        }
    }
    fn expect(&mut self, token: usize) -> Result<String, PeleError> {
        if self.t() != token {
            return Err(self.fail(&format!("'{}'", t::NAMES[token])));
        }
        let text = self.tokens.texts[self.i].clone();
        self.i += 1;
        Ok(text)
    }
    fn actor(&mut self) -> Result<Statement, PeleError> {
        if !matches!(self.t(), t::ACTOR | t::KW_LINK) {
            return Err(self.fail("'ACTOR'"));
        }
        let id = self.tokens.texts[self.i].clone();
        self.i += 1;
        Ok(Statement {
            kind: "addParticipant",
            actor: vec![id],
            ..Default::default()
        })
    }
    fn text2(&mut self) -> Result<Text, PeleError> {
        let text = self.expect(t::TXT)?;
        let text = trim(&text);
        Ok(parse_message(text.get(1..).unwrap_or("")))
    }
    fn participant(&mut self) -> Result<Statement, PeleError> {
        let token = self.t();
        self.i += 1;
        if token == t::KW_DESTROY {
            let mut node = self.actor()?;
            self.expect(t::NEWLINE)?;
            node.kind = "destroyParticipant";
            return Ok(node);
        }
        let mut node = self.actor()?;
        if self.t() == t::CONFIG_START && self.tokens.types[self.i - 1] == t::ACTOR {
            self.i += 1;
            let content = self.expect(t::CONFIG_CONTENT)?;
            self.expect(t::CONFIG_END)?;
            node.config = Some(trim(&content).into());
        }
        node.draw = Some(if token == t::KW_PARTICIPANT { "participant" } else { "actor" }.into());
        if self.t() == t::AS {
            self.i += 1;
            let alias = self.expect(t::KW_RESTOFLINE)?;
            self.expect(t::NEWLINE)?;
            node.description = Some(parse_message(&alias));
        } else if self.t() == t::NEWLINE {
            self.i += 1;
        } else {
            return Err(self.fail("'AS', 'NEWLINE'"));
        }
        Ok(node)
    }
    fn signal(&mut self, from: Statement) -> Result<Vec<Statement>, PeleError> {
        let central = self.t() == t::CENTRAL;
        if central {
            self.i += 1;
        }
        let signal_type = signal(self.t()).ok_or_else(|| self.fail(SIGNAL_TOKENS))?;
        self.i += 1;
        let mark = self.t();
        if mark == t::CENTRAL || (!central && matches!(mark, t::PLUS | t::MINUS)) {
            self.i += 1;
        }
        let to = self.actor()?;
        let msg = self.text2()?;
        let mut message = Statement {
            kind: "addMessage",
            from: from.actor.first().cloned(),
            to: to.actor.first().cloned(),
            signal_type: Some(signal_type),
            msg: Some(msg),
            ..Default::default()
        };
        let mut extra = Vec::new();
        if central {
            let dual = mark == t::CENTRAL;
            message.activate = dual;
            message.central_connection = if dual {
                lt::CENTRAL_CONNECTION_DUAL
            } else {
                lt::CENTRAL_CONNECTION_REVERSE
            };
            if dual {
                extra.push(Statement {
                    kind: "centralConnection",
                    signal_type: Some(lt::CENTRAL_CONNECTION),
                    actor: to.actor.clone(),
                    ..Default::default()
                });
            }
            extra.push(Statement {
                kind: "centralConnectionReverse",
                signal_type: Some(lt::CENTRAL_CONNECTION_REVERSE),
                actor: from.actor.clone(),
                ..Default::default()
            });
        } else if mark == t::PLUS {
            message.activate = true;
            extra.push(Statement {
                kind: "activeStart",
                signal_type: Some(lt::ACTIVE_START),
                actor: to.actor.clone(),
                ..Default::default()
            });
        } else if mark == t::MINUS {
            extra.push(Statement {
                kind: "activeEnd",
                signal_type: Some(lt::ACTIVE_END),
                actor: from.actor.clone(),
                ..Default::default()
            });
        } else if mark == t::CENTRAL {
            message.activate = true;
            message.central_connection = lt::CENTRAL_CONNECTION;
            extra.push(Statement {
                kind: "centralConnection",
                signal_type: Some(lt::CENTRAL_CONNECTION),
                actor: to.actor.clone(),
                ..Default::default()
            });
        }
        let mut out = vec![from, to, message];
        out.extend(extra);
        Ok(out)
    }
    fn menu(&mut self, kind: &'static str) -> Result<Vec<Statement>, PeleError> {
        self.i += 1;
        let who = self.actor()?;
        let text = self.text2()?;
        let node = Statement {
            kind,
            actor: who.actor.clone(),
            text: Some(text),
            ..Default::default()
        };
        Ok(vec![who, node])
    }
    fn note(&mut self) -> Result<Vec<Statement>, PeleError> {
        self.i += 1;
        let token = self.t();
        if token == t::KW_OVER {
            self.i += 1;
            let first = self.actor()?;
            let mut ids = vec![first.actor[0].clone(), first.actor[0].clone()];
            let mut out = vec![first];
            if self.t() == t::COMMA {
                self.i += 1;
                let second = self.actor()?;
                ids[1] = second.actor[0].clone();
                out.push(second);
            }
            out.push(Statement {
                kind: "addNote",
                placement: 2,
                actor: ids,
                text: Some(self.text2()?),
                ..Default::default()
            });
            return Ok(out);
        }
        if !matches!(token, t::KW_LEFT_OF | t::KW_RIGHT_OF) {
            return Err(self.fail("'over', 'left_of', 'right_of'"));
        }
        self.i += 1;
        let who = self.actor()?;
        let node = Statement {
            kind: "addNote",
            placement: if token == t::KW_LEFT_OF { 0 } else { 1 },
            actor: who.actor.clone(),
            text: Some(self.text2()?),
            ..Default::default()
        };
        Ok(vec![who, node])
    }
    fn autonumber(&mut self) -> Result<Statement, PeleError> {
        self.i += 1;
        let mut node = Statement {
            kind: "sequenceIndex",
            sequence_visible: true,
            signal_type: Some(lt::AUTONUMBER),
            ..Default::default()
        };
        if self.t() == t::NUM {
            node.sequence_index = Some(self.expect(t::NUM)?.parse().unwrap());
            node.sequence_step = Some(if self.t() == t::NUM {
                self.expect(t::NUM)?.parse().unwrap()
            } else {
                1.0
            });
        } else if self.t() == t::KW_OFF {
            self.i += 1;
            node.sequence_visible = false;
        }
        if self.t() != t::NEWLINE {
            return Err(self.fail("'NEWLINE', 'NUM', 'off'"));
        }
        self.i += 1;
        Ok(node)
    }
    fn close(frame: Frame) -> Vec<Statement> {
        let text = frame.text;
        let mut list = frame.list;
        let (start, end, start_type, end_type) = match frame.kind {
            Kind::Box => ("boxStart", "boxEnd", 0, 0),
            Kind::Loop => ("loopStart", "loopEnd", lt::LOOP_START, lt::LOOP_END),
            Kind::Rect => ("rectStart", "rectEnd", lt::RECT_START, lt::RECT_END),
            Kind::Opt => ("optStart", "optEnd", lt::OPT_START, lt::OPT_END),
            Kind::Break => ("breakStart", "breakEnd", lt::BREAK_START, lt::BREAK_END),
            Kind::Alt => ("altStart", "altEnd", lt::ALT_START, lt::ALT_END),
            Kind::Par => ("parStart", "parEnd", frame.start_type, lt::PAR_END),
            _ => ("criticalStart", "criticalEnd", lt::CRITICAL_START, lt::CRITICAL_END),
        };
        if matches!(frame.kind, Kind::Alt | Kind::Par | Kind::Critical) {
            let (divider, signal_type) = match frame.kind {
                Kind::Alt => ("else", lt::ALT_ELSE),
                Kind::Par => ("and", lt::PAR_AND),
                _ => ("option", lt::CRITICAL_OPTION),
            };
            for (mut prefix, text) in frame.sections.into_iter().rev() {
                prefix.push(Statement {
                    kind: divider,
                    signal_type: Some(signal_type),
                    text: Some(parse_message(&text)),
                    ..Default::default()
                });
                prefix.extend(list);
                list = prefix;
            }
        }
        list[0] = if frame.kind == Kind::Box {
            Statement {
                kind: start,
                box_data: Some(box_data(&text)),
                ..Default::default()
            }
        } else {
            Statement {
                kind: start,
                signal_type: Some(start_type),
                text: Some(parse_message(&text)),
                ..Default::default()
            }
        };
        list.push(Statement {
            kind: end,
            signal_type: Some(end_type),
            ..Default::default()
        });
        list
    }
    fn run(&mut self) -> Result<(), PeleError> {
        while self.t() == t::NEWLINE {
            self.i += 1;
        }
        self.expect(t::SD)?;
        let mut stack = Vec::new();
        let mut frame = Frame::new(Kind::Top, String::new(), 0);
        loop {
            let token = self.t();
            if frame.kind == Kind::Box {
                match token {
                    t::KW_PARTICIPANT | t::KW_PARTICIPANT_ACTOR | t::KW_DESTROY => frame.list.push(self.participant()?),
                    t::NEWLINE => self.i += 1,
                    t::KW_END => {
                        self.i += 1;
                        let done = Self::close(frame);
                        frame = stack.pop().unwrap();
                        frame.list.extend(done);
                    }
                    _ => return Err(self.fail("'end', 'NEWLINE', 'participant', 'participant_actor', 'destroy'")),
                }
                continue;
            }
            match token {
                t::NEWLINE | t::INVALID => self.i += 1,
                t::KW_PARTICIPANT | t::KW_PARTICIPANT_ACTOR | t::KW_DESTROY => frame.list.push(self.participant()?),
                t::KW_CREATE => {
                    self.i += 1;
                    if !matches!(self.t(), t::KW_PARTICIPANT | t::KW_PARTICIPANT_ACTOR | t::KW_DESTROY) {
                        return Err(self.fail("'participant', 'participant_actor', 'destroy'"));
                    }
                    let mut node = self.participant()?;
                    node.kind = "createParticipant";
                    frame.list.push(node);
                }
                t::ACTOR => {
                    let from = self.actor()?;
                    frame.list.extend(self.signal(from)?);
                    self.expect(t::NEWLINE)?;
                }
                t::KW_LINK => {
                    let next = self.tokens.types.get(self.i + 1).copied().unwrap_or(t::END);
                    if matches!(next, t::ACTOR | t::KW_LINK) {
                        frame.list.extend(self.menu("addALink")?);
                    } else {
                        let from = self.actor()?;
                        frame.list.extend(self.signal(from)?);
                    }
                    self.expect(t::NEWLINE)?;
                }
                t::KW_LINKS | t::KW_PROPERTIES | t::KW_DETAILS => {
                    frame.list.extend(self.menu(match token {
                        t::KW_LINKS => "addLinks",
                        t::KW_PROPERTIES => "addProperties",
                        _ => "addDetails",
                    })?);
                    self.expect(t::NEWLINE)?;
                }
                t::KW_NOTE => {
                    frame.list.extend(self.note()?);
                    self.expect(t::NEWLINE)?;
                }
                t::KW_AUTONUMBER => frame.list.push(self.autonumber()?),
                t::KW_ACTIVATE | t::KW_DEACTIVATE => {
                    self.i += 1;
                    let who = self.actor()?;
                    self.expect(t::NEWLINE)?;
                    frame.list.push(Statement {
                        kind: if token == t::KW_ACTIVATE { "activeStart" } else { "activeEnd" },
                        signal_type: Some(if token == t::KW_ACTIVATE {
                            lt::ACTIVE_START
                        } else {
                            lt::ACTIVE_END
                        }),
                        actor: who.actor,
                        ..Default::default()
                    });
                }
                t::KW_TITLE | t::KW_LEGACY_TITLE => {
                    let text = self.tokens.texts[self.i].clone();
                    self.i += 1;
                    self.db.title = Some(String::from_utf16_lossy(
                        &text
                            .encode_utf16()
                            .skip(if token == t::KW_TITLE { 6 } else { 7 })
                            .collect::<Vec<_>>(),
                    ));
                }
                t::KW_ACC_TITLE => {
                    self.i += 1;
                    self.db.acc_title = Some(trim(&self.expect(t::KW_ACC_TITLE_VALUE)?).into());
                }
                t::KW_ACC_DESCR => {
                    self.i += 1;
                    let value = self.expect(t::KW_ACC_DESCR_VALUE)?;
                    self.db.set_acc_descr(trim(&value));
                }
                t::KW_ACC_DESCR_MULTILINE_VALUE => {
                    let value = self.tokens.texts[self.i].clone();
                    self.i += 1;
                    self.db.set_acc_descr(trim(&value));
                }
                t::KW_LOOP | t::KW_RECT | t::KW_OPT | t::KW_BREAK | t::KW_ALT | t::KW_PAR | t::KW_PAR_OVER | t::KW_CRITICAL | t::KW_BOX => {
                    self.i += 1;
                    let text = self.expect(t::KW_RESTOFLINE)?;
                    let kind = match token {
                        t::KW_LOOP => Kind::Loop,
                        t::KW_RECT => Kind::Rect,
                        t::KW_OPT => Kind::Opt,
                        t::KW_BREAK => Kind::Break,
                        t::KW_ALT => Kind::Alt,
                        t::KW_PAR | t::KW_PAR_OVER => Kind::Par,
                        t::KW_BOX => Kind::Box,
                        _ => Kind::Critical,
                    };
                    stack.push(frame);
                    frame = Frame::new(
                        kind,
                        text,
                        if token == t::KW_PAR_OVER {
                            lt::PAR_OVER_START
                        } else {
                            lt::PAR_START
                        },
                    );
                }
                t::KW_ELSE | t::KW_AND | t::KW_OPTION => {
                    if frame.kind
                        != match token {
                            t::KW_ELSE => Kind::Alt,
                            t::KW_AND => Kind::Par,
                            _ => Kind::Critical,
                        }
                    {
                        return Err(self.fail("'end'"));
                    }
                    self.i += 1;
                    let text = self.expect(t::KW_RESTOFLINE)?;
                    frame.sections.push((std::mem::take(&mut frame.list), text));
                }
                t::KW_END => {
                    if frame.kind == Kind::Top {
                        return Err(self.fail("'NEWLINE', 'EOF'"));
                    }
                    self.i += 1;
                    let done = Self::close(frame);
                    frame = stack.pop().unwrap();
                    frame.list.extend(done);
                }
                t::END => {
                    if frame.kind != Kind::Top {
                        return Err(self.fail("'end'"));
                    }
                    return self.db.apply(frame.list);
                }
                _ => return Err(self.fail("'NEWLINE', 'ACTOR', 'participant', 'note', 'end'")),
            }
        }
    }
}
pub fn parse_sequence(src: &str, db: &mut Model) -> Result<(), PeleError> {
    Parser {
        src: src.encode_utf16().collect(),
        tokens: tokenize(src),
        i: 0,
        db,
    }
    .run()
}
