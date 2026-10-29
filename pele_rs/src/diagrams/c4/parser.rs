use super::{
    lexer::{Token, tokenize},
    model::{Attr, Model, Named},
    tokens as t,
};
use crate::{PeleError, errors::syntax_error};
const KINDS: &[&str] = &[
    "person",
    "external_person",
    "system",
    "system_db",
    "system_queue",
    "external_system",
    "external_system_db",
    "external_system_queue",
    "container",
    "container_db",
    "container_queue",
    "external_container",
    "external_container_db",
    "external_container_queue",
    "component",
    "component_db",
    "component_queue",
    "external_component",
    "external_component_db",
    "external_component_queue",
];
const REL_TYPES: &[&str] = &["rel", "birel", "rel_u", "rel_d", "rel_l", "rel_r", "rel_b", "rel"];
const NODE_TYPES: &[&str] = &["node", "nodeL", "nodeR"];
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
    fn text(&mut self) -> String {
        let t = self.tokens[self.i].text.clone();
        self.i += 1;
        t
    }
    fn fail(&self, expected: &str) -> PeleError {
        let t = &self.tokens[self.i];
        syntax_error(
            "c4",
            &self.src,
            t.start,
            &if t.kind == t::ERROR {
                String::new()
            } else {
                format!("Expecting {expected}, got '{}'", t::TOKEN_NAMES[t.kind])
            },
            t.kind == t::ERROR,
        )
    }
    fn expect(&mut self, k: usize) -> Result<(), PeleError> {
        if self.kind() != k {
            return Err(self.fail(&format!("'{}'", t::TOKEN_NAMES[k])));
        }
        self.i += 1;
        Ok(())
    }
    fn attributes(&mut self) -> Result<Vec<Attr>, PeleError> {
        let mut out = vec![];
        loop {
            match self.kind() {
                t::STR => out.push(Attr::Text(self.text().trim().into())),
                t::ATTRIBUTE_EMPTY => {
                    self.i += 1;
                    out.push(Attr::Text(String::new()));
                }
                t::STR_KEY => {
                    let key = self.text().trim().into();
                    if self.kind() != t::STR_VALUE {
                        return Err(self.fail("'STR_VALUE'"));
                    }
                    let value = self.text().trim().into();
                    out.push(Attr::Named(Named { key, value }));
                }
                _ => break,
            }
        }
        if out.is_empty() {
            return Err(self.fail("'STR', 'STR_KEY', 'ATTRIBUTE', 'ATTRIBUTE_EMPTY'"));
        }
        Ok(out)
    }
    fn other(&mut self, k: usize) -> Result<(), PeleError> {
        if k == t::title {
            self.db.title = Some(self.text().chars().skip(6).collect());
        } else if k == t::accDescription {
            let text = self.text().chars().skip(15).collect::<String>();
            self.descr(&text);
        } else if k == t::acc_descr_multiline_value {
            let text = self.text();
            self.descr(text.trim());
        } else {
            self.i += 1;
            if k == t::acc_title {
                if self.kind() != t::acc_title_value {
                    return Err(self.fail("'acc_title_value'"));
                }
                self.db.title = Some(self.text().trim().into());
            } else {
                if self.kind() != t::acc_descr_value {
                    return Err(self.fail("'acc_descr_value'"));
                }
                let text = self.text();
                self.descr(text.trim());
            }
        }
        Ok(())
    }
    fn descr(&mut self, s: &str) {
        let mut text = String::new();
        let mut after = false;
        for c in s.chars() {
            if after && c.is_whitespace() {
                continue;
            }
            text.push(c);
            after = c == '\n';
        }
        self.db.acc_descr = Some(text);
    }
    fn diagram(&mut self, k: usize) -> Result<bool, PeleError> {
        self.i += 1;
        let mut attrs = self.attributes()?;
        if k <= t::SYSTEM_EXT_QUEUE {
            self.db.add_shape(KINDS[k - t::PERSON], &attrs, false);
        } else if k <= t::COMPONENT_EXT_QUEUE {
            self.db.add_shape(KINDS[k - t::PERSON], &attrs, true);
        } else if k <= t::NODE_R {
            let typ = match k {
                t::ENTERPRISE_BOUNDARY => Some("ENTERPRISE"),
                t::SYSTEM_BOUNDARY => Some("SYSTEM"),
                t::CONTAINER_BOUNDARY => Some("CONTAINER"),
                _ => None,
            };
            if let Some(typ) = typ {
                attrs.insert(2.min(attrs.len()), Attr::Text(typ.into()));
            }
            if k >= t::NODE {
                self.db.add_boundary(&attrs, "node", Some(NODE_TYPES[k - t::NODE]));
            } else {
                self.db
                    .add_boundary(&attrs, if k == t::CONTAINER_BOUNDARY { "container" } else { "system" }, None);
            }
            if self.kind() == t::LBRACE {
                self.i += 1;
                self.expect(t::NEWLINE)?;
            } else if self.kind() == t::NEWLINE {
                self.i += 1;
                self.expect(t::LBRACE)?;
                if self.kind() == t::NEWLINE {
                    self.i += 1;
                }
            } else {
                return Err(self.fail("'NEWLINE', 'LBRACE'"));
            }
            return Ok(true);
        } else if k <= t::REL_INDEX {
            if k == t::REL_INDEX {
                attrs.remove(0);
            }
            self.db.add_rel(REL_TYPES[k - t::REL], &attrs);
        } else if k == t::UPDATE_EL_STYLE {
            self.db.update_element(&attrs);
        } else if k == t::UPDATE_REL_STYLE {
            self.db.update_rel(&attrs);
        } else {
            self.db.update_layout(&attrs);
        }
        Ok(false)
    }
}
fn is_other(k: usize) -> bool {
    matches!(
        k,
        t::title | t::accDescription | t::acc_title | t::acc_descr | t::acc_descr_multiline_value
    )
}
fn is_diagram(k: usize) -> bool {
    (t::PERSON..=t::UPDATE_LAYOUT_CONFIG).contains(&k)
}
pub fn parse(src: &str, db: &mut Model) -> Result<(), PeleError> {
    let mut p = Parser {
        src: src.encode_utf16().collect(),
        tokens: tokenize(src),
        i: 0,
        db,
    };
    if !(t::C4_CONTEXT..=t::C4_DEPLOYMENT).contains(&p.kind()) {
        return Err(p.fail("'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'C4_CONTEXT', 'C4_CONTAINER', 'C4_COMPONENT', 'C4_DYNAMIC', 'C4_DEPLOYMENT'"));
    }
    p.i += 1;
    p.expect(t::NEWLINE)?;
    let (mut depth, mut state) = (0usize, 0);
    loop {
        let k = p.kind();
        if state == 3 || state == 4 {
            if k == t::NEWLINE {
                p.i += 1;
                state = 2;
                continue;
            }
            if state == 3 && is_diagram(k) {
                state = 1;
                continue;
            }
        } else if is_diagram(k) {
            if p.diagram(k)? {
                depth += 1;
                state = 1;
            } else {
                state = 4;
            }
            continue;
        } else if is_other(k) && state != 1 {
            p.other(k)?;
            state = 3;
            continue;
        } else if state != 2 {
            return Err(p.fail("'title', 'accDescription', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'PERSON', 'REL'"));
        }
        if depth > 0 {
            if k != t::RBRACE {
                return Err(p.fail("'NEWLINE', 'RBRACE'"));
            }
            p.i += 1;
            p.db.pop();
            depth -= 1;
            state = 4;
        } else {
            if k != t::EOF {
                return Err(p.fail("'NEWLINE', 'EOF'"));
            }
            p.i += 1;
            p.expect(t::END)?;
            p.db.c4_type = Some(p.tokens[0].text.clone());
            return Ok(());
        }
    }
}
