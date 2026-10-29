use super::{
    lexer::{Token, tokenize},
    model::{Model, Node, Relation},
    tokens as t,
};
use crate::errors::{PeleError, syntax_error};
const VALUES: &[&str] = &[
    "Requirement",
    "Functional Requirement",
    "Interface Requirement",
    "Performance Requirement",
    "Physical Requirement",
    "Design Constraint",
    "Low",
    "Medium",
    "High",
    "Analysis",
    "Demonstration",
    "Inspection",
    "Test",
    "",
    "contains",
    "copies",
    "derives",
    "satisfies",
    "verifies",
    "refines",
    "traces",
];
struct Parser {
    src: Vec<u16>,
    tokens: Vec<Token>,
    i: usize,
}
fn style_part(k: usize) -> bool {
    matches!(k, t::ALPHA | t::COLON | t::BRKT | t::MINUS | t::SEMICOLON)
}
impl Parser {
    fn current(&self) -> &Token {
        &self.tokens[self.i.min(self.tokens.len() - 1)]
    }
    fn kind(&self) -> usize {
        self.current().kind
    }
    fn take(&mut self) -> String {
        let s = self.current().text.clone();
        self.i += 1;
        s
    }
    fn fail(&self, from: usize, to: usize, others: &[usize]) -> PeleError {
        let token = self.current();
        let expected = (from..=to)
            .chain(others.iter().copied())
            .map(|k| format!("'{}'", t::TOKEN_NAMES[k]))
            .collect::<Vec<_>>()
            .join(", ");
        syntax_error(
            "requirement",
            &self.src,
            token.start,
            &if token.kind == t::ERROR {
                String::new()
            } else {
                format!("Expecting {}, got '{}'", expected, t::TOKEN_NAMES[token.kind])
            },
            token.kind == t::ERROR,
        )
    }
    fn expect(&mut self, k: usize) -> Result<String, PeleError> {
        if self.kind() != k {
            return Err(self.fail(k, k, &[]));
        }
        Ok(self.take())
    }
    fn keyword(&mut self, from: usize, to: usize) -> Result<String, PeleError> {
        let k = self.kind();
        if !(from..=to).contains(&k) {
            return Err(self.fail(from, to, &[]));
        }
        self.i += 1;
        Ok(VALUES[k - t::REQUIREMENT].into())
    }
    fn string(&mut self) -> Result<String, PeleError> {
        if !matches!(self.kind(), t::qString | t::unqString) {
            return Err(self.fail(t::qString, t::unqString, &[]));
        }
        Ok(self.take())
    }
    fn directive(&mut self, db: &mut Model) -> Result<bool, PeleError> {
        let k = self.kind();
        let value = match k {
            t::acc_title => {
                self.i += 1;
                self.expect(t::acc_title_value)?
            }
            t::acc_descr => {
                self.i += 1;
                self.expect(t::acc_descr_value)?
            }
            t::acc_descr_multiline_value => self.take(),
            _ => return Ok(false),
        };
        let value = value.trim().to_owned();
        if k == t::acc_title {
            db.acc_title = Some(value.trim_start().into());
        } else {
            db.acc_descr = Some(regex::Regex::new(r"\n\s+").unwrap().replace_all(&value, "\n").into_owned());
        }
        Ok(true)
    }
    fn ids(&mut self) -> Result<Vec<String>, PeleError> {
        let mut out = vec![];
        loop {
            if !matches!(self.kind(), t::ALPHA | t::unqString | t::qString) {
                return Err(self.fail(t::qString, t::unqString, &[t::ALPHA]));
            }
            out.push(self.take());
            if self.kind() != t::COMMA {
                return Ok(out);
            }
            self.i += 1;
        }
    }
    fn styles(&mut self) -> Result<Vec<String>, PeleError> {
        let mut out = vec![];
        loop {
            if !style_part(self.kind()) {
                return Err(self.fail(t::ALPHA, t::SEMICOLON, &[t::MINUS, t::BRKT]));
            }
            let mut s = self.take();
            while style_part(self.kind()) {
                s += &self.take();
            }
            out.push(s);
            if self.kind() != t::COMMA {
                return Ok(out);
            }
            self.i += 1;
        }
    }
    fn classes(&mut self) -> Result<Vec<String>, PeleError> {
        let classes = if self.kind() == t::STYLE_SEPARATOR {
            self.i += 1;
            let c = self.ids()?;
            self.expect(t::STRUCT_START)?;
            c
        } else if self.kind() == t::STRUCT_START {
            self.i += 1;
            vec![]
        } else {
            return Err(self.fail(t::STRUCT_START, t::STRUCT_START, &[t::STYLE_SEPARATOR]));
        };
        self.expect(t::NEWLINE)?;
        Ok(classes)
    }
    fn body(&mut self, requirement: bool) -> Result<Node, PeleError> {
        let mut fields = vec![];
        loop {
            let k = self.kind();
            if k == t::STRUCT_STOP {
                self.i += 1;
                break;
            }
            if k == t::NEWLINE {
                self.i += 1;
                continue;
            }
            if if requirement {
                !(t::ID..=t::VERIFYMTHD).contains(&k)
            } else {
                !matches!(k, t::TYPE | t::DOCREF)
            } {
                return Err(if requirement {
                    self.fail(t::ID, t::VERIFYMTHD, &[t::NEWLINE, t::STRUCT_STOP])
                } else {
                    self.fail(t::TYPE, t::DOCREF, &[t::NEWLINE, t::STRUCT_STOP])
                });
            }
            self.i += 1;
            self.expect(t::COLONSEP)?;
            let value = if k == t::RISK {
                self.keyword(t::LOW_RISK, t::HIGH_RISK)?
            } else if k == t::VERIFYMTHD {
                self.keyword(t::VERIFY_ANALYSIS, t::VERIFY_TEST)?
            } else {
                self.string()?
            };
            fields.push((k, value));
            self.expect(t::NEWLINE)?;
        }
        let mut node = Node {
            classes: vec!["default".into()],
            ..Default::default()
        };
        for (k, v) in fields.into_iter().rev() {
            match k {
                t::ID => node.requirement_id = v,
                t::TEXT => node.text = v,
                t::RISK => node.risk = v,
                t::VERIFYMTHD => node.verify_method = v,
                t::TYPE => node.kind = v,
                _ => node.doc_ref = v,
            }
        }
        Ok(node)
    }
}
pub fn parse(source: &str, db: &mut Model) -> Result<(), PeleError> {
    let mut p = Parser {
        src: source.encode_utf16().collect(),
        tokens: tokenize(source),
        i: 0,
    };
    while p.directive(db)? {
        if p.kind() == t::NEWLINE {
            p.i += 1;
        }
    }
    if p.kind() != t::RD {
        return Err(p.fail(t::RD, t::RD, &[t::acc_title, t::acc_descr, t::acc_descr_multiline_value]));
    }
    p.i += 1;
    p.expect(t::NEWLINE)?;
    loop {
        let k = p.kind();
        if k == t::EOF {
            return Ok(());
        }
        if k == t::NEWLINE {
            p.i += 1;
            continue;
        }
        if p.directive(db)? {
            continue;
        }
        if (t::direction_tb..=t::direction_lr).contains(&k) {
            p.i += 1;
            db.direction = ["TB", "BT", "RL", "LR"][k - t::direction_tb].into();
        } else if (t::REQUIREMENT..=t::DESIGN_CONSTRAINT).contains(&k) {
            let kind = p.keyword(t::REQUIREMENT, t::DESIGN_CONSTRAINT)?;
            let name = p.string()?;
            let classes = p.classes()?;
            let mut node = p.body(true)?;
            node.name = name.clone();
            node.kind = kind;
            db.requirements.entry(name.clone()).or_insert(node);
            db.set_class(&[name], &classes)?;
        } else if k == t::ELEMENT {
            p.i += 1;
            let name = p.string()?;
            let classes = p.classes()?;
            let mut node = p.body(false)?;
            node.name = name.clone();
            db.elements.entry(name.clone()).or_insert(node);
            db.set_class(&[name], &classes)?;
        } else if matches!(k, t::STYLE | t::CLASSDEF | t::CLASS) {
            p.i += 1;
            let ids = p.ids()?;
            if k == t::CLASS {
                db.set_class(&ids, &p.ids()?)?;
            } else {
                let styles = p.styles()?;
                if k == t::STYLE {
                    db.style(&ids, &styles)?;
                } else {
                    db.define_class(&ids, &styles)?;
                }
            }
        } else if matches!(k, t::unqString | t::qString) {
            let id = p.take();
            let next = p.kind();
            if next == t::STYLE_SEPARATOR {
                p.i += 1;
                db.set_class(&[id], &p.ids()?)?;
            } else if next == t::END_ARROW_L || next == t::LINE {
                p.i += 1;
                let kind = p.keyword(t::CONTAINS, t::TRACES)?;
                p.expect(if next == t::LINE { t::END_ARROW_R } else { t::LINE })?;
                let other = p.string()?;
                let (src, dst) = if next == t::LINE { (id, other) } else { (other, id) };
                db.relations.push(Relation { kind, src, dst });
            } else {
                return Err(p.fail(t::STYLE_SEPARATOR, t::STYLE_SEPARATOR, &[t::END_ARROW_L, t::LINE]));
            }
        } else {
            return Err(p.fail(
                t::NEWLINE,
                t::EOF,
                &[
                    t::acc_title,
                    t::acc_descr,
                    t::acc_descr_multiline_value,
                    t::direction_tb,
                    t::direction_bt,
                    t::direction_rl,
                    t::direction_lr,
                    t::REQUIREMENT,
                    t::FUNCTIONAL_REQUIREMENT,
                    t::INTERFACE_REQUIREMENT,
                    t::PERFORMANCE_REQUIREMENT,
                    t::PHYSICAL_REQUIREMENT,
                    t::DESIGN_CONSTRAINT,
                    t::ELEMENT,
                    t::CLASSDEF,
                    t::CLASS,
                    t::STYLE,
                    t::unqString,
                    t::qString,
                ],
            ));
        }
    }
}
