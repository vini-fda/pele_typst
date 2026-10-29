use super::{
    lexer::{Token, tokenize},
    model::{Model, Relation},
    tokens as t,
};
use crate::{PeleError, errors::syntax_error};
const NAME_TOKENS: &str = "'UNICODE_TEXT', 'NUM', 'ALPHA', 'MINUS', 'BQUOTE_STR'";
fn alpha(t: usize) -> bool {
    matches!(t, t::UNICODE_TEXT | t::ALPHA | t::MINUS)
}
fn style_part(t: usize) -> bool {
    matches!(t, t::ALPHA | t::COLON | t::BRKT | t::STYLE | t::PCT | t::LABEL)
}
fn end_type(t: usize) -> i32 {
    match t {
        t::AGGREGATION => 0,
        t::EXTENSION => 1,
        t::COMPOSITION => 2,
        t::DEPENDENCY => 3,
        t::LOLLIPOP => 4,
        _ => -1,
    }
}
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
        syntax_error(
            "class",
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
    fn alpha_num(&mut self) -> Result<String, PeleError> {
        if !alpha(self.kind()) {
            return Err(self.fail("'UNICODE_TEXT', 'NUM', 'ALPHA', 'MINUS'"));
        }
        Ok(self.take())
    }
    fn name(&mut self, generic: bool) -> Result<String, PeleError> {
        let mut out = String::new();
        loop {
            if self.kind() == t::BQUOTE_STR {
                out += &self.take();
                break;
            }
            if !alpha(self.kind()) {
                return Err(self.fail(NAME_TOKENS));
            }
            out += &self.take();
            if self.accept(t::DOT) {
                out.push('.');
            } else if !alpha(self.kind()) && self.kind() != t::BQUOTE_STR {
                break;
            }
        }
        if generic && self.kind() == t::GENERICTYPE {
            out += &format!("~{}~", self.take());
        }
        Ok(out)
    }
    fn class_label(&mut self) -> Result<String, PeleError> {
        self.i += 1;
        let label = self.expect(t::STR)?;
        self.expect(t::SQE)?;
        Ok(label)
    }
    fn members(&mut self, id: &str) -> Result<(), PeleError> {
        if self.kind() != t::MEMBER {
            return Err(self.fail("'STRUCT_STOP', 'MEMBER'"));
        }
        while self.kind() == t::MEMBER {
            let text = self.take();
            self.db.add_member(id, &text);
        }
        self.expect(t::STRUCT_STOP)?;
        Ok(())
    }
    fn class_statement(&mut self) -> Result<String, PeleError> {
        self.i += 1;
        let id = self.name(true)?;
        self.db.add_class(&id);
        if self.kind() == t::SQS {
            let label = self.class_label()?;
            self.db.set_label(&id, label);
        }
        match self.kind() {
            t::STYLE_SEPARATOR => {
                self.i += 1;
                let css = self.alpha_num()?;
                self.db.set_class(&id, &css);
                if self.accept(t::STRUCT_START) {
                    self.members(&id)?;
                }
            }
            t::STRUCT_START => {
                self.i += 1;
                if !self.accept(t::STRUCT_STOP) {
                    self.members(&id)?;
                }
            }
            t::ANNOTATION_START => {
                self.i += 1;
                let annotation = self.alpha_num()?;
                self.expect(t::ANNOTATION_END)?;
                self.db.add_annotation(&id, annotation);
                if self.accept(t::STRUCT_START) && !self.accept(t::STRUCT_STOP) {
                    self.members(&id)?;
                }
            }
            _ => {}
        }
        Ok(id)
    }
    fn note(&mut self) -> Result<String, PeleError> {
        let simple = self.kind() == t::NOTE;
        self.i += 1;
        let id = if simple { None } else { Some(self.name(true)?) };
        let text = self.expect(t::STR)?;
        Ok(self.db.add_note(text, id))
    }
    fn namespace(&mut self) -> Result<(), PeleError> {
        struct Frame {
            id: String,
            classes: Vec<String>,
            notes: Vec<String>,
        }
        let mut open: Vec<Frame> = vec![];
        loop {
            self.i += 1;
            let ns = self.name(false)?;
            let label = if self.kind() == t::SQS { Some(self.class_label()?) } else { None };
            let id = self.db.add_namespace(&ns, label);
            self.expect(t::STRUCT_START)?;
            self.accept(t::NEWLINE);
            open.push(Frame {
                id,
                classes: vec![],
                notes: vec![],
            });
            let mut item = true;
            while item {
                match self.kind() {
                    t::NAMESPACE => break,
                    t::CLASS => {
                        let id = self.class_statement()?;
                        open.last_mut().unwrap().classes.push(id);
                    }
                    t::NOTE | t::NOTE_FOR => {
                        let id = self.note()?;
                        open.last_mut().unwrap().notes.push(id);
                    }
                    _ => return Err(self.fail("'NAMESPACE', 'CLASS', 'NOTE_FOR', 'NOTE'")),
                }
                loop {
                    if self.kind() == t::NEWLINE {
                        let next = self.tokens[self.i + 1].kind;
                        self.i += 1;
                        if matches!(next, t::NAMESPACE | t::CLASS | t::NOTE | t::NOTE_FOR) {
                            break;
                        }
                    }
                    self.expect(t::STRUCT_STOP)?;
                    let done = open.pop().unwrap();
                    self.db.add_namespace_members(&done.id, done.classes, done.notes);
                    self.db.pop_namespace();
                    if open.is_empty() {
                        item = false;
                        break;
                    }
                }
            }
            if open.is_empty() {
                return Ok(());
            }
        }
    }
    fn relation_end(&mut self) -> i32 {
        let kind = end_type(self.kind());
        if kind >= 0 {
            self.i += 1;
        }
        kind
    }
    fn class_or_relation(&mut self) -> Result<(), PeleError> {
        let id1 = self.name(true)?;
        let mut kind = self.kind();
        if kind == t::LABEL {
            let label = self.take();
            self.db.add_member(&id1, label.strip_prefix(':').unwrap_or(&label).trim());
            return Ok(());
        }
        let title1 = if kind == t::STR {
            self.take()
        } else {
            if end_type(kind) < 0 && kind != t::LINE && kind != t::DOTTED_LINE {
                return Ok(());
            }
            "none".into()
        };
        let type1 = self.relation_end();
        kind = self.kind();
        if kind != t::LINE && kind != t::DOTTED_LINE {
            return Err(self.fail("'AGGREGATION', 'EXTENSION', 'COMPOSITION', 'DEPENDENCY', 'LOLLIPOP', 'LINE', 'DOTTED_LINE'"));
        }
        self.i += 1;
        let type2 = self.relation_end();
        let title2 = if self.kind() == t::STR { self.take() } else { "none".into() };
        let id2 = self.name(true)?;
        let title = if self.kind() == t::LABEL {
            let text = self.take();
            Some(text.strip_prefix(':').unwrap_or(&text).trim().into())
        } else {
            None
        };
        self.db.add_relation(Relation {
            id1,
            id2,
            type1,
            type2,
            title1,
            title2,
            title,
            line_type: if kind == t::LINE { 0 } else { 1 },
        });
        Ok(())
    }
    fn styles(&mut self) -> Result<Vec<String>, PeleError> {
        let mut out = Vec::new();
        loop {
            if !style_part(self.kind()) {
                return Err(self.fail("'ALPHA', 'NUM', 'COLON', 'UNIT', 'SPACE', 'BRKT', 'STYLE', 'PCT', 'LABEL'"));
            }
            let mut s = self.take();
            while style_part(self.kind()) {
                s += &self.take();
            }
            out.push(s);
            if !self.accept(t::COMMA) {
                return Ok(out);
            }
        }
    }
    fn tooltip(&mut self, id: &str) {
        if self.kind() == t::STR {
            let tip = self.take();
            self.db.set_tooltip(id, tip);
        }
    }
    fn link(&mut self, id: String) -> Result<(), PeleError> {
        let href = self.expect(t::STR)?;
        let tip = if self.kind() == t::STR { Some(self.take()) } else { None };
        let target = if self.kind() == t::LINK_TARGET { Some(self.take()) } else { None };
        self.db.set_link(&id, &href, target);
        if let Some(tip) = tip {
            self.db.set_tooltip(&id, tip);
        }
        Ok(())
    }
    fn statement(&mut self) -> Result<(), PeleError> {
        let kind = self.kind();
        match kind{
 t::UNICODE_TEXT|t::ALPHA|t::MINUS|t::BQUOTE_STR=>self.class_or_relation()?,t::NAMESPACE=>self.namespace()?,t::CLASS=>{self.class_statement()?;},t::MEMBER=>self.i+=1,
 t::ANNOTATION_START=>{self.i+=1;let a=self.alpha_num()?;self.expect(t::ANNOTATION_END)?;let name=self.name(true)?;self.db.add_annotation(&name,a);},
 t::CALLBACK=>{self.i+=1;let id=self.name(true)?;self.expect(t::STR)?;self.db.set_click(&id);self.tooltip(&id);},t::LINK=>{self.i+=1;let id=self.name(true)?;self.link(id)?;},t::CLICK=>{self.i+=1;let id=self.name(true)?;if self.accept(t::HREF){self.link(id)?;}else{self.expect(t::CALLBACK_NAME)?;if self.kind()==t::CALLBACK_ARGS{self.i+=1;}self.db.set_click(&id);self.tooltip(&id);}},
 t::STYLE=>{self.i+=1;let id=self.expect(t::ALPHA)?;let styles=self.styles()?;self.db.set_style(&id,styles);},t::CSSCLASS=>{self.i+=1;let ids=self.expect(t::STR)?;let class=self.expect(t::ALPHA)?;self.db.set_class(&ids,&class);},t::NOTE|t::NOTE_FOR=>{self.note()?;},t::CLASSDEF=>{self.i+=1;let mut ids=vec![self.expect(t::ALPHA)?];while self.accept(t::COMMA){ids.push(self.expect(t::ALPHA)?);}let styles=self.styles()?;self.db.define_class(ids,styles);},
 t::direction_tb|t::direction_bt|t::direction_rl|t::direction_lr=>{self.i+=1;self.db.direction=["TB","BT","RL","LR"][kind-t::direction_tb].into();},t::acc_title=>{self.i+=1;self.db.acc_title=Some(self.expect(t::acc_title_value)?.trim().into());},t::acc_descr=>{self.i+=1;let text=self.expect(t::acc_descr_value)?;self.db.set_descr(text.trim().into());},t::acc_descr_multiline_value=>{let text=self.take();self.db.set_descr(text.trim().into());},_=>return Err(self.fail("'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'NAMESPACE', 'ANNOTATION_START', 'CLASS', 'MEMBER', 'NOTE_FOR', 'NOTE', 'CLASSDEF', 'direction_tb', 'CALLBACK', 'LINK', 'CLICK', 'STYLE', 'CSSCLASS', 'ALPHA', 'MINUS', 'BQUOTE_STR'"))}
        Ok(())
    }
}
pub fn parse(src: &str, db: &mut Model) -> Result<(), PeleError> {
    let mut p = Parser {
        src: src.encode_utf16().collect(),
        tokens: tokenize(src),
        i: 0,
        db,
    };
    let header = p.accept(t::CLASS_DIAGRAM);
    if header {
        p.expect(t::NEWLINE)?;
    }
    loop {
        p.statement()?;
        if !p.accept(t::NEWLINE) {
            break;
        }
        if matches!(p.kind(), t::EOF | t::END) {
            break;
        }
    }
    if header {
        p.expect(t::EOF)?;
    }
    p.expect(t::END)?;
    Ok(())
}
