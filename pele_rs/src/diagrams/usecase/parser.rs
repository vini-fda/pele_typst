use super::lexer::{Tokens, location, location_text, tokenize};
use super::tokens as t;
use super::*;
use crate::errors::PeleErrorCode;
use crate::util::{is_word16, trim};
const ARROWS: &str = "DEPENDENCY_ARROW|GENERALIZATION|FORWARD_SOLID|BACKWARD_SOLID|MARKERLESS_SOLID|FORWARD_CIRCLE|BACKWARD_CIRCLE|FORWARD_CROSS|BACKWARD_CROSS";
const NAMES: &str = "IDENTIFIER|PLAIN_STRING|MARKDOWN_STRING";
const LABELS: &str = "PLAIN_STRING|MARKDOWN_STRING|LABEL_TEXT";
const STATEMENTS: &str =
    "ACC_TITLE_LINE|ACC_DESCR_LINE|ACC_DESCR_BLOCK|DIRECTION|ACTOR|SYSTEM_BOUNDARY|NOTE|JSON_DECLARATION_START|CLASS_DEF|CLASS|STYLE";
const STYLE_PARTS: &str = "WORD|PLAIN_STRING|NUMBER|HASH_COLOR|CSS_IDENTIFIER|CSS_ESCAPED_COMMA|DASH|DOT|PERCENT|CSS_PUNCTUATION|COLON|LPAREN|RPAREN|LBRACKET|RBRACKET|LBRACE|RBRACE|AT|MARKERLESS_SOLID";
fn flags() -> Vec<u8> {
    let mut f = vec![0; t::RPAREN + 1];
    for (s, b) in [
        (format!("NEWLINE|COMMENT|{STATEMENTS}|{NAMES}"), 1),
        (format!("IDENTIFIER|{ARROWS}"), 2),
        (STYLE_PARTS.into(), 4),
        (format!("STEREOTYPE_START|CLASS_SEPARATOR|IDENTIFIER|{ARROWS}|NEWLINE|EOF"), 8),
        (format!("NEWLINE|COMMENT|ACTOR|{NAMES}"), 16),
        ("FORWARD_SOLID|MARKERLESS_SOLID|FORWARD_CIRCLE|FORWARD_CROSS".into(), 32),
        ("NEWLINE|IDENTIFIER|PLAIN_STRING|RBRACE".into(), 64),
    ] {
        for n in s.split('|') {
            if n == "WORD" {
                for k in t::USECASE..=t::IDENTIFIER {
                    f[k] |= b
                }
            } else {
                f[t::NAMES.iter().position(|name| *name == n).unwrap()] |= b
            }
        }
    }
    f
}
fn statement_sequences() -> String {
    let names: Vec<&str> = NAMES.split('|').collect();
    let metadata: Vec<String> = names
        .iter()
        .flat_map(|n| ["NEWLINE", "IDENTIFIER", "PLAIN_STRING", "RBRACE"].map(|next| format!("{n}, METADATA_START, {next}")))
        .collect();
    let follows = format!("STEREOTYPE_START|CLASS_SEPARATOR|IDENTIFIER|{ARROWS}|NEWLINE|EOF");
    let mut entities = Vec::new();
    for (i, n) in names.iter().enumerate() {
        if i == 0 {
            entities.push(format!("{n}, LPAREN"));
            entities.push(format!("{n}, LBRACKET"))
        }
        for next in follows.split('|') {
            entities.push(format!("{n}, {next}"))
        }
    }
    let mut all = vec![STATEMENTS.into()];
    all.extend(metadata.clone());
    all.extend(entities);
    all.extend(metadata);
    all.join("|")
}
struct Tail {
    id: Option<String>,
    id_span: Option<Span>,
    kind: String,
    arrow: usize,
    label: Option<Label>,
    minlen: f64,
    target: Item,
    span: Span,
}
struct Parser {
    ts: Tokens,
    p: usize,
    flags: Vec<u8>,
    drafts: Drafts,
    parent: Option<Item>,
    pending: Vec<(usize, usize)>,
}
impl Parser {
    fn k(&self) -> usize {
        self.ts.types[self.p]
    }
    fn slice(&self, a: usize, b: usize) -> String {
        String::from_utf16_lossy(&self.ts.src[a..b])
    }
    fn image(&self, k: usize) -> String {
        self.slice(self.ts.starts[k], self.ts.ends[k])
    }
    fn span(&self, k: usize) -> Span {
        (self.ts.starts[k], self.ts.ends[k])
    }
    fn fail(&self, detail: String) -> PeleError {
        let (line, column, _) = location(&self.ts.src, self.ts.starts[self.p]);
        let mut e = PeleError::new(
            format!(
                "Error parsing usecase diagram: {detail} at {}",
                location_text(&self.ts.src, self.ts.starts[self.p], self.ts.ends[self.p])
            ),
            PeleErrorCode::Syntax,
        )
        .of("usecase");
        e.line = line;
        e.column = column;
        e
    }
    fn mismatch(&self, name: &str) -> PeleError {
        self.fail(format!(
            "Expecting token of type --> {name} <-- but found --> '{}' <--",
            self.image(self.p)
        ))
    }
    fn expect(&mut self, k: usize) -> Result<usize, PeleError> {
        if self.k() != k {
            return Err(self.mismatch(t::NAMES[k]));
        }
        let p = self.p;
        self.p += 1;
        Ok(p)
    }
    fn no_alt(&self, expected: &str) -> PeleError {
        let list = expected
            .split('|')
            .enumerate()
            .map(|(i, s)| format!("  {}. [{s}]", i + 1))
            .collect::<Vec<_>>()
            .join("\n");
        self.fail(format!(
            "Expecting: one of these possible Token sequences:\n{list}\nbut found: '{}'",
            self.image(self.p)
        ))
    }
    fn line_end(&mut self) -> Result<(), PeleError> {
        if self.k() == t::NEWLINE {
            self.p += 1
        } else if self.k() != t::EOF {
            return Err(self.no_alt("NEWLINE|EOF"));
        }
        Ok(())
    }
    fn content(&self, k: usize) -> Span {
        let trim = if self.ts.types[k] == t::MARKDOWN_STRING {
            2
        } else if self.ts.types[k] == t::PLAIN_STRING {
            1
        } else {
            0
        };
        (self.ts.starts[k] + trim, self.ts.ends[k] - trim)
    }
    fn token_label(&self, k: usize) -> Label {
        let span = self.content(k);
        Label {
            text: self.slice(span.0, span.1),
            markdown: self.ts.types[k] == t::MARKDOWN_STRING,
            span,
        }
    }
    fn label(&mut self) -> Result<Label, PeleError> {
        let k = self.k();
        if k == t::PLAIN_STRING || k == t::MARKDOWN_STRING {
            let p = self.p;
            self.p += 1;
            return Ok(self.token_label(p));
        }
        if !(t::USECASE..=t::LABEL_SYMBOL).contains(&k) {
            return Err(self.no_alt(LABELS));
        }
        let first = self.p;
        while (t::USECASE..=t::LABEL_SYMBOL).contains(&self.k()) {
            self.p += 1
        }
        let span = (self.ts.starts[first], self.ts.ends[self.p - 1]);
        Ok(Label {
            text: self.slice(span.0, span.1),
            markdown: false,
            span,
        })
    }
    fn property(&mut self) -> Result<Property, PeleError> {
        let key = self.p;
        if !is_key(self.k()) {
            return Err(self.no_alt("IDENTIFIER|PLAIN_STRING"));
        }
        self.p += 1;
        self.expect(t::COLON)?;
        let value = self.p;
        let k = self.k();
        if !is_key(k) && k != t::TRUE && k != t::FALSE {
            return Err(self.no_alt("IDENTIFIER|PLAIN_STRING|TRUE|FALSE"));
        }
        self.p += 1;
        let key_span = self.content(key);
        let value_span = self.content(value);
        Ok(Property {
            key: self.slice(key_span.0, key_span.1),
            value: if k == t::TRUE {
                true.into()
            } else if k == t::FALSE {
                false.into()
            } else {
                self.slice(value_span.0, value_span.1).into()
            },
            span: (self.ts.starts[key], self.ts.ends[value]),
            key_span,
            value_span,
        })
    }
    fn separator(&self) -> bool {
        let a = self.k();
        let b = self.ts.types[self.p + 1];
        let c = self.ts.types[self.p + 2];
        if a == t::COMMA {
            return is_key(b) || b == t::NEWLINE && (c == t::NEWLINE || is_key(c));
        }
        a == t::NEWLINE && (b == t::COMMA || is_key(b) || b == t::NEWLINE && (c == t::NEWLINE || c == t::COMMA || is_key(c)))
    }
    fn metadata(&mut self) -> Result<Vec<Property>, PeleError> {
        self.expect(t::METADATA_START)?;
        let mut props = Vec::new();
        while self.k() == t::NEWLINE {
            self.p += 1
        }
        if is_key(self.k()) {
            props.push(self.property()?);
            while self.separator() {
                if self.k() == t::COMMA {
                    self.p += 1
                } else {
                    while self.k() == t::NEWLINE {
                        self.p += 1
                    }
                    if self.k() == t::COMMA {
                        self.p += 1
                    }
                }
                while self.k() == t::NEWLINE {
                    self.p += 1
                }
                props.push(self.property()?)
            }
            if self.k() == t::COMMA {
                self.p += 1
            }
            while self.k() == t::NEWLINE {
                self.p += 1
            }
        }
        self.expect(t::RBRACE)?;
        Ok(props)
    }
    fn classes(&mut self) -> Result<Vec<String>, PeleError> {
        let mut out = Vec::new();
        loop {
            self.p += 1;
            let id = self.expect(t::IDENTIFIER)?;
            out.push(self.image(id));
            if self.k() != t::COMMA {
                break;
            }
        }
        Ok(out)
    }
    fn name(&mut self, brackets: bool) -> Result<Item, PeleError> {
        let p = self.p;
        let k = self.k();
        let mut item = Item {
            span: self.content(p),
            generated: k != t::IDENTIFIER,
            ..Default::default()
        };
        if k == t::IDENTIFIER {
            self.p += 1;
            item.id = self.image(p);
            let open = self.k();
            if open == t::LPAREN || brackets && open == t::LBRACKET {
                self.p += 1;
                item.label = self.label()?;
                self.expect(if open == t::LPAREN { t::RPAREN } else { t::RBRACKET })?;
                item.shape = Some(if open == t::LPAREN { "ellipse" } else { "rect" }.into())
            } else {
                item.label = self.token_label(p)
            }
        } else if k == t::PLAIN_STRING || k == t::MARKDOWN_STRING {
            self.p += 1;
            item.label = self.token_label(p);
            item.id = generated(&item.label.text)
        } else {
            return Err(self.no_alt(NAMES));
        }
        Ok(item)
    }
    fn element(&mut self, actor: bool) -> Result<Item, PeleError> {
        let first = self.p;
        let mut item = self.name(!actor)?;
        if actor {
            item.shape = None
        }
        if self.k() == t::METADATA_START {
            item.metadata = Some(self.metadata()?)
        }
        if self.k() == t::STEREOTYPE_START {
            self.p += 1;
            let text = self.expect(t::STEREOTYPE_TEXT)?;
            self.expect(t::STEREOTYPE_END)?;
            item.stereotype = Some(trim(&self.image(text)).into())
        }
        if self.k() == t::CLASS_SEPARATOR {
            item.classes = self.classes()?
        }
        item.explicit = item.shape.is_some() || item.metadata.is_some() || item.stereotype.as_ref().is_some_and(|s| !s.is_empty());
        item.node_span = (self.ts.starts[first], self.ts.ends[self.p - 1]);
        Ok(item)
    }
    fn draft(&mut self, item: Item, kind: &str) {
        self.drafts.elements.push(DraftElement {
            item,
            kind: kind.into(),
            parent: self.parent.clone(),
        })
    }
    fn labeled_right(&self, closers: u8) -> bool {
        if !(t::USECASE..=t::MARKDOWN_STRING).contains(&self.k()) {
            return false;
        }
        for k in self.p + 1..self.ts.types.len() {
            let t = self.ts.types[k];
            if t == t::MARKERLESS_SOLID || self.flags[t] & closers != 0 {
                return true;
            }
            if !(t::USECASE..=t::MARKDOWN_STRING).contains(&t) {
                return false;
            }
        }
        false
    }
    fn minlen(&self, k: usize) -> f64 {
        let dashes = self.ts.ends[k] - self.ts.starts[k] - if self.ts.types[k] == t::MARKERLESS_SOLID { 0 } else { 1 };
        dashes.saturating_sub(1).max(1) as f64
    }
    fn tail(&mut self) -> Result<Tail, PeleError> {
        let first = self.p;
        let mut tail = Tail {
            id: None,
            id_span: None,
            kind: "association".into(),
            arrow: 0,
            label: None,
            minlen: 1.0,
            target: Default::default(),
            span: (0, 0),
        };
        if self.k() == t::IDENTIFIER && self.ts.types[self.p + 1] == t::AT {
            tail.id = Some(self.image(self.p));
            tail.id_span = Some(self.span(self.p));
            self.p += 2
        }
        let k = self.p;
        let kind = self.k();
        self.p += 1;
        match kind {
            t::DEPENDENCY_ARROW => {
                self.expect(t::COLON)?;
                if self.k() != t::INCLUDE && self.k() != t::EXTEND {
                    return Err(self.no_alt("INCLUDE|EXTEND"));
                }
                tail.kind = if self.k() == t::INCLUDE { "include" } else { "extend" }.into();
                tail.label = Some(Label {
                    text: tail.kind.clone(),
                    markdown: false,
                    span: self.span(self.p),
                });
                self.p += 1
            }
            t::GENERALIZATION => tail.kind = "generalization".into(),
            t::FORWARD_SOLID => tail.minlen = self.minlen(k),
            t::BACKWARD_SOLID => {
                tail.arrow = 1;
                if self.ts.ends[k] - self.ts.starts[k] == 3 && self.labeled_right(0) {
                    tail.label = Some(self.label()?);
                    let p = self.expect(t::MARKERLESS_SOLID)?;
                    tail.minlen = self.minlen(p)
                } else {
                    tail.minlen = self.minlen(k)
                }
            }
            t::MARKERLESS_SOLID => {
                tail.arrow = 2;
                if self.ts.ends[k] - self.ts.starts[k] == 2 && self.labeled_right(32) {
                    tail.label = Some(self.label()?);
                    tail.arrow = match self.k() {
                        t::FORWARD_SOLID => 0,
                        t::FORWARD_CIRCLE => 3,
                        t::FORWARD_CROSS => 4,
                        t::MARKERLESS_SOLID => 2,
                        _ => return Err(self.no_alt("FORWARD_SOLID|MARKERLESS_SOLID|FORWARD_CIRCLE|FORWARD_CROSS")),
                    };
                    if tail.arrow == 0 || tail.arrow == 2 {
                        tail.minlen = self.minlen(self.p)
                    }
                    self.p += 1
                } else {
                    tail.minlen = self.minlen(k)
                }
            }
            t::FORWARD_CIRCLE => tail.arrow = 3,
            t::FORWARD_CROSS => tail.arrow = 4,
            t::BACKWARD_CIRCLE | t::BACKWARD_CROSS => {
                tail.arrow = if kind == t::BACKWARD_CIRCLE { 5 } else { 6 };
                if self.labeled_right(0) {
                    tail.label = Some(self.label()?);
                    self.expect(t::MARKERLESS_SOLID)?;
                }
            }
            _ => {
                self.p -= 1;
                return Err(self.no_alt(ARROWS));
            }
        }
        tail.target = self.element(false)?;
        tail.span = (self.ts.starts[first], self.ts.ends[self.p - 1]);
        Ok(tail)
    }
    fn relationship(&mut self, source: Item, declaration: bool, tail: Tail) {
        let target = tail.target;
        if target.explicit {
            self.draft(target.clone(), "usecase")
        }
        self.drafts.relations.push(DraftRelation {
            source: Endpoint {
                classed: !source.classes.is_empty(),
                item: source,
                declares: declaration,
            },
            target: Endpoint {
                classed: !target.classes.is_empty(),
                declares: target.explicit,
                item: target,
            },
            span: tail.span,
            id: tail.id,
            id_span: tail.id_span,
            kind: tail.kind,
            arrow: tail.arrow,
            label: tail.label,
            minlen: tail.minlen,
        })
    }
    fn actors(&mut self, top: bool) -> Result<(), PeleError> {
        self.p += 1;
        let mut items = vec![self.element(true)?];
        let mut tail = None;
        if self.k() == t::COMMA {
            while self.k() == t::COMMA {
                self.p += 1;
                items.push(self.element(true)?);
            }
        } else if top && self.flags[self.k()] & 2 != 0 {
            tail = Some(self.tail()?)
        }
        self.line_end()?;
        for item in &items {
            self.draft(item.clone(), "actor")
        }
        if let Some(tail) = tail {
            self.relationship(items.remove(0), true, tail)
        }
        Ok(())
    }
    fn entity(&mut self, top: bool) -> Result<(), PeleError> {
        let mut source = self.element(false)?;
        let tail = if top && self.flags[self.k()] & 2 != 0 {
            Some(self.tail()?)
        } else {
            None
        };
        self.line_end()?;
        if tail.is_none() {
            source.explicit = true;
            source.shape.get_or_insert_with(|| "ellipse".into());
        }
        if source.explicit {
            self.draft(source.clone(), "usecase")
        }
        if let Some(tail) = tail {
            let explicit = source.explicit;
            self.relationship(source, explicit, tail)
        }
        Ok(())
    }
    fn boundary(&mut self) -> Result<(), PeleError> {
        self.p += 1;
        let mut group = self.name(true)?;
        let inline = if self.k() == t::METADATA_START {
            Some(self.metadata()?)
        } else {
            None
        };
        if self.k() == t::CLASS_SEPARATOR {
            group.classes = self.classes()?
        }
        self.line_end()?;
        self.drafts.boundaries.push(group.clone());
        self.parent = Some(group.clone());
        while self.flags[self.k()] & 16 != 0 {
            self.line(false)?
        }
        self.parent = None;
        self.expect(t::END)?;
        self.line_end()?;
        if let Some(properties) = inline {
            self.drafts.metadata.push((group.id, group.span, properties))
        }
        Ok(())
    }
    fn style(&mut self) -> Result<String, PeleError> {
        let first = self.p;
        let k = self.k();
        if (t::USECASE..=t::IDENTIFIER).contains(&k) || k == t::CSS_IDENTIFIER {
            self.p += 1
        } else if k == t::MARKERLESS_SOLID {
            self.p += 1;
            if !(t::USECASE..=t::IDENTIFIER).contains(&self.k()) {
                return Err(self.mismatch("WORD"));
            }
            self.p += 1
        } else {
            return Err(self.no_alt("WORD|CSS_IDENTIFIER|MARKERLESS_SOLID"));
        }
        self.expect(t::COLON)?;
        if self.flags[self.k()] & 4 == 0 {
            return Err(self.fail(format!("Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[{}]>\nbut found: '{}'",STYLE_PARTS.replace('|',"] ,["),self.image(self.p))));
        }
        while self.flags[self.k()] & 4 != 0 {
            self.p += 1
        }
        Ok(self.slice(self.ts.starts[first], self.ts.ends[self.p - 1]).replace("\\,", ","))
    }
    fn styles(&mut self) -> Result<Vec<String>, PeleError> {
        let mut out = vec![self.style()?];
        while self.k() == t::COMMA {
            self.p += 1;
            out.push(self.style()?)
        }
        Ok(out)
    }
    fn identifiers(&mut self) -> Result<Vec<usize>, PeleError> {
        let mut out = vec![self.expect(t::IDENTIFIER)?];
        while self.k() == t::COMMA {
            self.p += 1;
            out.push(self.expect(t::IDENTIFIER)?)
        }
        Ok(out)
    }
    fn top(&mut self) -> Result<(), PeleError> {
        let k = self.p;
        match self.k() {
            t::ACC_TITLE_LINE | t::ACC_DESCR_LINE | t::ACC_DESCR_BLOCK => {
                let kind = self.k();
                self.p += 1;
                let raw = self.image(k);
                let value = if kind == t::ACC_DESCR_BLOCK {
                    trim(&raw[raw.find('{').unwrap() + 1..raw.rfind('}').unwrap()]).to_string()
                } else {
                    trim(&raw[raw.find(':').unwrap() + 1..]).to_string()
                };
                if kind == t::ACC_TITLE_LINE {
                    self.drafts.acc_title = value
                } else {
                    self.drafts.acc_descr = value
                }
            }
            t::DIRECTION => {
                self.p += 1;
                if !(t::TD..=t::RL).contains(&self.k()) {
                    return Err(self.no_alt("TD|TB|BT|LR|RL"));
                }
                self.drafts.direction = Some(if self.k() == t::TD { "TB".into() } else { self.image(self.p) });
                self.p += 1
            }
            t::ACTOR => return self.actors(true),
            t::SYSTEM_BOUNDARY => return self.boundary(),
            t::NOTE => {
                self.p += 1;
                self.expect(t::FOR)?;
                let target = self.expect(t::IDENTIFIER)?;
                let label = self.label()?;
                self.drafts.notes.push((self.image(target), self.span(target), label))
            }
            t::JSON_DECLARATION_START => {
                self.p += 1;
                let literal = self.expect(t::JSON_OBJECT_LITERAL)?;
                let classes = if self.k() == t::CLASS_SEPARATOR {
                    self.classes()?
                } else {
                    Vec::new()
                };
                let mut from = self.ts.starts[k] + 4;
                while self.ts.src.get(from).is_some_and(|c| matches!(c, 9 | 32)) {
                    from += 1
                }
                let mut to = from;
                while self.ts.src.get(to).is_some_and(|c| is_word16(*c as u32)) {
                    to += 1
                }
                let json = Json {
                    id: self.slice(from, to),
                    value: super::json::Data::default(),
                    classes,
                    styles: Vec::new(),
                };
                self.pending.push((self.drafts.jsons.len(), literal));
                self.drafts.jsons.push((json, (from, to)))
            }
            t::CLASS_DEF => {
                self.p += 1;
                let ids = self.identifiers()?;
                let styles = self.styles()?;
                self.drafts
                    .class_defs
                    .push((ids.into_iter().map(|i| self.image(i)).collect(), styles))
            }
            t::CLASS => {
                self.p += 1;
                let targets = self.identifiers()?;
                let classes = self.identifiers()?;
                self.drafts.classes.push((
                    targets.into_iter().map(|i| (self.image(i), self.span(i))).collect(),
                    classes.into_iter().map(|i| self.image(i)).collect(),
                ))
            }
            t::STYLE => {
                self.p += 1;
                let target = self.expect(t::IDENTIFIER)?;
                let styles = self.styles()?;
                self.drafts.styles.push((self.image(target), self.span(target), styles))
            }
            _ => {
                let next = self.ts.types[self.p + 1];
                if next == t::METADATA_START {
                    if self.flags[self.ts.types[self.p + 2]] & 64 == 0 {
                        return Err(self.no_alt(&statement_sequences()));
                    }
                    self.p += 1;
                    let span = self.content(k);
                    let target = if self.ts.types[k] == t::IDENTIFIER {
                        self.image(k)
                    } else {
                        generated(&self.slice(span.0, span.1))
                    };
                    let assigned = self.metadata()?;
                    self.drafts.metadata.push((target, span, assigned))
                } else {
                    let follows =
                        self.flags[next] & 8 != 0 || self.ts.types[k] == t::IDENTIFIER && (next == t::LPAREN || next == t::LBRACKET);
                    if !follows
                        || self.ts.types[k] == t::IDENTIFIER
                            && matches!(
                                self.image(k).to_ascii_lowercase().as_str(),
                                "allowmixing" | "newpage" | "package" | "rectangle" | "skinparam"
                            )
                    {
                        return Err(self.no_alt(&statement_sequences()));
                    }
                    return self.entity(true);
                }
            }
        }
        self.line_end()
    }
    fn line(&mut self, top: bool) -> Result<(), PeleError> {
        if self.k() == t::NEWLINE {
            self.p += 1;
            return Ok(());
        }
        if self.k() == t::COMMENT {
            self.p += 1;
            return self.line_end();
        }
        if top {
            self.top()
        } else if self.k() == t::ACTOR {
            self.actors(false)
        } else {
            self.entity(false)
        }
    }
}
fn is_key(k: usize) -> bool {
    k == t::IDENTIFIER || k == t::PLAIN_STRING
}
fn generated(s: &str) -> String {
    s.encode_utf16()
        .map(|c| {
            if is_word16(c as u32) {
                char::from_u32(c as u32).unwrap()
            } else {
                '_'
            }
        })
        .collect()
}
pub fn parse(source: &str) -> Result<Drafts, PeleError> {
    let mut p = Parser {
        ts: tokenize(source)?,
        p: 0,
        flags: flags(),
        drafts: Default::default(),
        parent: None,
        pending: Vec::new(),
    };
    p.expect(t::USECASE)?;
    p.line_end()?;
    while p.flags[p.k()] & 1 != 0 {
        p.line(true)?
    }
    if p.k() != t::EOF {
        return Err(p.fail(format!("Redundant input, expecting EOF but found: {}", p.image(p.p))));
    }
    for &(index, k) in &p.pending {
        let raw = p.image(k);
        let value = super::json::parse(&raw, &p.ts.src, p.ts.starts[k])?;
        p.drafts.jsons[index].0.value = value;
    }
    Ok(p.drafts)
}
