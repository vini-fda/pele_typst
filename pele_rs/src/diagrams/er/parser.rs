use super::{
    lexer::{Token, tokenize},
    model::{Attribute, DocItem, Model},
    tokens as t,
};
use crate::errors::{PeleError, syntax_error};
const NAME: &str = "'UNICODE_TEXT', 'NUM', 'ENTITY_NAME', 'DECIMAL_NUM', 'ENTITY_ONE'";
const CARDS: &str = "'ZERO_OR_ONE', 'ZERO_OR_MORE', 'ONE_OR_MORE', 'ONLY_ONE', 'MD_PARENT'";
const LINE: &str = "'EOF', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'SUBGRAPH', 'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'CLASSDEF', 'CLASS', 'STYLE', 'UNICODE_TEXT', 'NUM', 'ENTITY_NAME', 'DECIMAL_NUM', 'ENTITY_ONE'";
fn is_name(k: usize) -> bool {
    matches!(k, t::UNICODE_TEXT | t::ENTITY_NAME | t::NUM | t::DECIMAL_NUM | t::ENTITY_ONE)
}
fn card(k: usize) -> Option<&'static str> {
    match k {
        t::ZERO_OR_ONE => Some("ZERO_OR_ONE"),
        t::ZERO_OR_MORE => Some("ZERO_OR_MORE"),
        t::ONE_OR_MORE => Some("ONE_OR_MORE"),
        t::ONLY_ONE => Some("ONLY_ONE"),
        t::MD_PARENT => Some("MD_PARENT"),
        _ => None,
    }
}
fn style_part(k: usize) -> bool {
    matches!(k, t::STYLE_TEXT | t::NUM | t::COLON | t::BRKT)
}
struct Parser {
    src: Vec<u16>,
    tokens: Vec<Token>,
    i: usize,
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
    fn fail(&self, expected: &str) -> PeleError {
        let t = self.current();
        syntax_error(
            "er",
            &self.src,
            t.start,
            &if t.kind == t::ERROR {
                String::new()
            } else {
                format!(
                    "Expecting {}, got '{}'",
                    expected,
                    if t.kind == t::CHAR {
                        t.text.as_str()
                    } else {
                        t::TOKEN_NAMES[t.kind]
                    }
                )
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
    fn separator(&mut self) -> Result<(), PeleError> {
        if !matches!(self.kind(), t::NEWLINE | t::SEMI | t::EOF) {
            return Err(self.fail("'EOF', 'NEWLINE', 'SEMI'"));
        }
        self.i += 1;
        Ok(())
    }
    fn name(&mut self) -> Result<String, PeleError> {
        let k = self.kind();
        if !is_name(k) {
            return Err(self.fail(NAME));
        }
        let s = self.take();
        Ok(if k == t::ENTITY_NAME { s[1..s.len() - 1].into() } else { s })
    }
    fn ids(&mut self) -> Result<Vec<String>, PeleError> {
        let mut out = vec![];
        loop {
            if !matches!(self.kind(), t::UNICODE_TEXT | t::STYLE_TEXT) {
                return Err(self.fail("'UNICODE_TEXT', 'STYLE_TEXT'"));
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
                return Err(self.fail("'STYLE_TEXT', 'COLON', 'NUM', 'BRKT'"));
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
    fn is_char(&self, c: &str) -> bool {
        self.kind() == t::CHAR && self.current().text == c
    }
    fn block(&mut self) -> Result<Vec<Attribute>, PeleError> {
        let mut out = vec![];
        if self.kind() != t::BLOCK_START {
            return Ok(out);
        }
        self.i += 1;
        if self.kind() == t::BLOCK_STOP {
            self.i += 1;
            return Ok(out);
        }
        loop {
            let mut kind = self.expect(t::ATTRIBUTE_WORD)?;
            if self.is_char("?") {
                kind += &self.take();
            }
            if self.kind() != t::ATTRIBUTE_WORD {
                return Err(self.fail(if kind.ends_with('?') {
                    "'ATTRIBUTE_WORD'"
                } else {
                    "'ATTRIBUTE_WORD', '?'"
                }));
            }
            let mut a = Attribute {
                kind,
                name: self.take(),
                ..Default::default()
            };
            if self.kind() == t::ATTRIBUTE_KEY {
                a.keys.push(self.take());
                while self.is_char(",") {
                    self.i += 1;
                    a.keys.push(self.expect(t::ATTRIBUTE_KEY)?);
                }
            }
            if self.kind() == t::COMMENT {
                let s = self.take();
                a.comment = s[1..s.len() - 1].into();
            }
            out.push(a);
            if self.kind() == t::BLOCK_STOP {
                self.i += 1;
                return Ok(out);
            }
        }
    }
    fn entity(&mut self, db: &mut Model) -> Result<Vec<String>, PeleError> {
        let a = self.name()?;
        if self.kind() == t::SQS {
            self.i += 1;
            let alias = self.name()?;
            self.expect(t::SQE)?;
            let classes = if self.kind() == t::STYLE_SEPARATOR {
                self.i += 1;
                self.ids()?
            } else {
                vec![]
            };
            let attrs = self.block()?;
            db.add_entity(&a, &alias).attributes.extend(attrs);
            db.set_class(std::slice::from_ref(&a), &classes);
            return Ok(vec![a]);
        }
        let classes = if self.kind() == t::STYLE_SEPARATOR {
            self.i += 1;
            self.ids()?
        } else {
            vec![]
        };
        if let Some(card_b) = card(self.kind()) {
            let card_b = card_b.to_owned();
            self.i += 1;
            if !matches!(self.kind(), t::IDENTIFYING | t::NON_IDENTIFYING) {
                return Err(self.fail("'NON_IDENTIFYING', 'IDENTIFYING'"));
            }
            let identifying = self.kind() == t::IDENTIFYING;
            self.i += 1;
            let Some(card_a) = card(self.kind()) else {
                return Err(self.fail(CARDS));
            };
            let card_a = card_a.to_owned();
            self.i += 1;
            let b = self.name()?;
            let classes_b = if self.kind() == t::STYLE_SEPARATOR {
                self.i += 1;
                self.ids()?
            } else {
                vec![]
            };
            self.expect(t::COLON)?;
            let k = self.kind();
            if !matches!(k, t::WORD | t::ENTITY_NAME | t::UNICODE_TEXT) {
                return Err(self.fail("'UNICODE_TEXT', 'ENTITY_NAME', 'WORD'"));
            }
            let label = self.take();
            let label = if k == t::UNICODE_TEXT {
                label
            } else {
                label[1..label.len() - 1].into()
            };
            db.add_entity(&a, "");
            db.add_entity(&b, "");
            db.add_relationship(&a, label, &b, card_a, card_b, identifying);
            db.set_class(std::slice::from_ref(&a), &classes);
            db.set_class(std::slice::from_ref(&b), &classes_b);
            Ok(vec![a, b])
        } else {
            let attrs = self.block()?;
            db.add_entity(&a, "").attributes.extend(attrs);
            db.set_class(std::slice::from_ref(&a), &classes);
            Ok(vec![a])
        }
    }
}
struct Frame {
    id: String,
    title: String,
    list: Vec<DocItem>,
}
pub fn parse(source: &str, db: &mut Model) -> Result<(), PeleError> {
    let mut p = Parser {
        src: source.encode_utf16().collect(),
        tokens: tokenize(source),
        i: 0,
    };
    p.expect(t::ER_DIAGRAM)?;
    let mut frames = vec![Frame {
        id: String::new(),
        title: String::new(),
        list: vec![],
    }];
    loop {
        let nested = frames.len() > 1;
        match p.kind() {
            t::NEWLINE => p.i += 1,
            t::EOF => {
                p.i += 1;
                if nested {
                    return Err(p.fail(&format!("'END', {LINE}")));
                }
                return Ok(());
            }
            t::END_KW => {
                if !nested {
                    return Err(p.fail(LINE));
                }
                p.i += 1;
                let f = frames.pop().unwrap();
                let id = db.add_subgraph(&f.id, f.list, &f.title);
                if frames.len() > 1 {
                    frames.last_mut().unwrap().list.push(DocItem::Name(id));
                }
            }
            t::SUBGRAPH => {
                p.i += 1;
                let id = p.name()?;
                let mut title = id.clone();
                if p.kind() == t::SQS {
                    p.i += 1;
                    title = p.name()?;
                    while is_name(p.kind()) {
                        title.push(' ');
                        title += &p.name()?;
                    }
                    p.expect(t::SQE)?;
                }
                p.separator()?;
                frames.push(Frame { id, title, list: vec![] });
            }
            t::acc_title | t::acc_descr | t::acc_descr_multiline_value => {
                let k = p.kind();
                let value = if k == t::acc_descr_multiline_value {
                    p.take()
                } else {
                    p.i += 1;
                    p.expect(if k == t::acc_title {
                        t::acc_title_value
                    } else {
                        t::acc_descr_value
                    })?
                };
                let value = value.trim().to_owned();
                if k == t::acc_title {
                    db.acc_title = Some(value.trim_start().into());
                } else {
                    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"\n\s+").unwrap());
                    let re = &*RE;
                    db.acc_descr = Some(re.replace_all(&value, "\n").into_owned());
                }
                if nested {
                    frames.last_mut().unwrap().list.push(DocItem::Name(value));
                }
            }
            t::direction_tb | t::direction_bt | t::direction_rl | t::direction_lr => {
                let value = ["TB", "BT", "RL", "LR"][p.kind() - t::direction_tb].to_owned();
                p.i += 1;
                if nested {
                    frames.last_mut().unwrap().list.push(DocItem::Dir(value));
                } else {
                    db.direction = value;
                }
            }
            t::CLASSDEF | t::CLASS | t::STYLE => {
                let k = p.kind();
                let keyword = p.take();
                let ids = p.ids()?;
                if k == t::CLASS {
                    db.set_class(&ids, &p.ids()?);
                } else {
                    let styles = p.styles()?;
                    p.separator()?;
                    if k == t::CLASSDEF {
                        db.add_class(&ids, &styles);
                    } else {
                        db.add_styles(&ids, &styles);
                    }
                }
                if nested {
                    frames.last_mut().unwrap().list.push(DocItem::Name(keyword));
                }
            }
            _ => {
                if !is_name(p.kind()) {
                    return Err(p.fail(&if nested { format!("'END', {LINE}") } else { LINE.into() }));
                }
                let names = p.entity(db)?;
                if nested {
                    frames.last_mut().unwrap().list.extend(names.into_iter().map(DocItem::Name));
                }
            }
        }
    }
}
