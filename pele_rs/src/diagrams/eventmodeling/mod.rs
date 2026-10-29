mod render;
use crate::PeleError;
use crate::diagrams::timeline::story;
use crate::diagrams::tokens::{self, Pattern, Reader, TokenType};
use crate::errors::{PeleErrorCode, locate};
use crate::preprocess::Config;
use crate::util::{is_space16, is_word16, trim, trim_end, trim_start};
use indexmap::IndexMap;
pub use render::render;
#[derive(Clone, Debug)]
pub struct Lane {
    pub index: usize,
    pub label: String,
    pub group: String,
    pub namespace: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Box {
    pub id: String,
    pub kind: String,
    pub entity: String,
    pub name: String,
    pub namespace: Option<String>,
    pub reset: bool,
    pub lane: usize,
    pub data: Option<String>,
    pub data_type: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Note {
    pub frame: String,
    pub text: String,
    pub data_type: Option<String>,
}
#[derive(Clone, Debug)]
pub struct Specification {
    pub frame: String,
    pub given: Vec<String>,
    pub when: Vec<String>,
    pub then: Vec<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub boxes: Vec<Box>,
    pub lanes: Vec<Lane>,
    pub relations: Vec<(usize, usize)>,
    pub notes: Vec<Note>,
    pub specifications: Vec<Specification>,
    pub entities: Vec<String>,
}
#[derive(Default)]
struct Frame {
    reset: bool,
    id: String,
    kind: String,
    entity: String,
    sources: Vec<String>,
    reference: Option<String>,
    data_type: Option<String>,
    inline: Option<String>,
    offset: usize,
}
macro_rules! word {
    ($name:ident,$word:expr) => {
        fn $name(s: &[u16], p: usize) -> Option<usize> {
            let end = p + $word.len();
            if $word.encode_utf16().enumerate().all(|(i, c)| s.get(p + i) == Some(&c)) && !s.get(end).is_some_and(|c| is_word16(*c as u32))
            {
                Some(end)
            } else {
                None
            }
        }
    };
}
// Direct UTF-16 scanners keep tokenization linear and use JavaScript's whitespace set.
fn id(s: &[u16], p: usize) -> Option<usize> {
    if !s.get(p).is_some_and(|c| matches!(c, 65..=90 | 97..=122 | 95)) {
        return None;
    }
    let mut end = p + 1;
    while s.get(end).is_some_and(|c| is_word16(*c as u32)) {
        end += 1;
    }
    Some(end)
}
fn fid(s: &[u16], p: usize) -> Option<usize> {
    let mut end = p;
    while end < p + 3 && s.get(end).is_some_and(|c| matches!(c, 48..=57)) {
        end += 1;
    }
    (end > p).then_some(end)
}
fn inline(s: &[u16], p: usize) -> Option<usize> {
    let close = match s.get(p) {
        Some(123) => 125,
        Some(34) => 34,
        Some(39) => 39,
        _ => return None,
    };
    let mut end = None;
    for i in p + 1..s.len() {
        if matches!(s[i], 10 | 13 | 0x2028 | 0x2029) {
            break;
        }
        if s[i] == close {
            end = Some(i + 1);
        }
    }
    end
}
fn space(s: &[u16], p: usize) -> Option<usize> {
    let mut end = p;
    while s.get(end).is_some_and(|c| is_space16(*c as u32)) {
        end += 1;
    }
    (end > p).then_some(end)
}
fn mlcomment(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) != Some(&47) || s.get(p + 1) != Some(&42) {
        return None;
    }
    (p + 2..s.len()).find(|&i| s[i] == 42 && s.get(i + 1) == Some(&47)).map(|i| i + 2)
}
fn slcomment(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) != Some(&47) || s.get(p + 1) != Some(&47) {
        return None;
    }
    let mut end = p + 2;
    while s.get(end).is_some_and(|c| !matches!(c, 10 | 13)) {
        end += 1
    }
    Some(end)
}
fn block(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) != Some(&123) {
        return None;
    }
    let mut q = p + 1;
    while s.get(q).is_some_and(|c| matches!(c, 9 | 32)) {
        q += 1
    }
    q = tokens::newline(s, q)?;
    for at in q..s.len() {
        if s[at] == 125 && (at == q || s[at - 1] == 10) {
            let end = at + 1;
            if let Some(n) = tokens::newline(s, end) {
                return Some(n);
            }
            if end == s.len() || is_space16(s[end] as u32) {
                return Some(end);
            }
        }
    }
    None
}
fn title(s: &[u16], p: usize) -> Option<usize> {
    let mut at = p;
    while s.get(at).is_some_and(|c| matches!(c, 9 | 32)) {
        at += 1
    }
    if s.get(at + 5).is_some_and(|c| is_word16(*c as u32)) {
        return None;
    }
    tokens::title(s, p)
}
fn token(name: &'static str, m: tokens::Matcher, hidden: bool) -> TokenType {
    TokenType {
        name,
        pattern: Pattern::Match(m),
        hidden,
        first: None,
    }
}
fn sign(text: &'static str) -> TokenType {
    TokenType {
        name: text,
        pattern: Pattern::Literal(text),
        hidden: false,
        first: None,
    }
}
word!(w_resetframe, "resetframe");
word!(w_readmodel, "readmodel");
word!(w_processor, "processor");
word!(w_timeframe, "timeframe");
word!(w_command, "command");
word!(w_entity, "entity");
word!(w_event, "event");
word!(w_jsobj, "jsobj");
word!(w_figma, "figma");
word!(w_given, "given");
word!(w_json, "json");
word!(w_salt, "salt");
word!(w_html, "html");
word!(w_text, "text");
word!(w_data, "data");
word!(w_note, "note");
word!(w_when, "when");
word!(w_then, "then");
word!(w_rmo, "rmo");
word!(w_cmd, "cmd");
word!(w_evt, "evt");
word!(w_pcr, "pcr");
word!(w_uri, "uri");
word!(w_gwt, "gwt");
word!(w_ui, "ui");
word!(w_md, "md");
word!(w_tf, "tf");
word!(w_rf, "rf");
fn types() -> Vec<TokenType> {
    vec![
        tokens::keyword("eventmodeling"),
        token("resetframe", w_resetframe, false),
        token("readmodel", w_readmodel, false),
        token("processor", w_processor, false),
        token("timeframe", w_timeframe, false),
        token("command", w_command, false),
        token("entity", w_entity, false),
        token("event", w_event, false),
        token("jsobj", w_jsobj, false),
        token("figma", w_figma, false),
        token("given", w_given, false),
        token("json", w_json, false),
        token("salt", w_salt, false),
        token("html", w_html, false),
        token("text", w_text, false),
        token("data", w_data, false),
        token("note", w_note, false),
        token("when", w_when, false),
        token("then", w_then, false),
        token("rmo", w_rmo, false),
        token("cmd", w_cmd, false),
        token("evt", w_evt, false),
        token("pcr", w_pcr, false),
        token("uri", w_uri, false),
        sign("->>"),
        token("gwt", w_gwt, false),
        token("ui", w_ui, false),
        token("md", w_md, false),
        token("tf", w_tf, false),
        sign("[["),
        sign("]]"),
        token("rf", w_rf, false),
        sign("`"),
        sign("."),
        token("EM_ID", id, false),
        token("EM_FID", fid, false),
        token("EM_DATA_INLINE", inline, false),
        token("EM_DATA_BLOCK", block, false),
        token("EM_ACC_DESCR", tokens::acc_descr, false),
        token("EM_ACC_TITLE", tokens::acc_title, false),
        token("EM_TITLE", title, false),
        token("EM_WS", space, true),
        token("EM_YAML", tokens::yaml, true),
        token("EM_DIRECTIVE", tokens::directive, true),
        token("EM_SINGLE_LINE_COMMENT", tokens::comment, true),
        token("EM_ML_COMMENT", mlcomment, true),
        token("EM_SL_COMMENT", slcomment, true),
    ]
}
fn kind(r: &Reader) -> &'static str {
    if r.kind() == -1 { "" } else { r.types[r.kind() as usize].name }
}
fn is_entity(k: &str) -> bool {
    matches!(
        k,
        "rmo" | "readmodel" | "ui" | "cmd" | "command" | "evt" | "event" | "pcr" | "processor"
    )
}
fn expect(r: &mut Reader, name: &str) -> Result<String, PeleError> {
    let id = r.types.iter().position(|t| t.name == name).unwrap();
    r.expect(id as i32)
}
fn accept(r: &mut Reader, name: &str) -> bool {
    if kind(r) == name {
        r.i += 1;
        true
    } else {
        false
    }
}
fn qualified(r: &mut Reader) -> Result<String, PeleError> {
    let mut name = expect(r, "EM_ID")?;
    while accept(r, ".") {
        name.push('.');
        name += &expect(r, "EM_ID")?
    }
    Ok(name)
}
fn data_type(r: &mut Reader) -> Result<Option<String>, PeleError> {
    if !accept(r, "`") {
        return Ok(None);
    }
    if !matches!(kind(r), "json" | "jsobj" | "figma" | "salt" | "uri" | "md" | "html" | "text") {
        return Err(r.fail("a data type"));
    }
    let name = r.take();
    expect(r, "`")?;
    Ok(Some(name))
}
fn entity_type(r: &mut Reader) -> Result<String, PeleError> {
    if is_entity(kind(r)) {
        Ok(r.take())
    } else {
        Err(r.fail("an entity type"))
    }
}
fn statements(r: &mut Reader) -> Result<Vec<String>, PeleError> {
    let mut out = Vec::new();
    loop {
        entity_type(r)?;
        out.push(expect(r, "EM_ID")?);
        if !is_entity(kind(r)) {
            return Ok(out);
        }
    }
}
fn block_text(raw: &str) -> String {
    let open = raw.find('{');
    let close = raw.rfind('}');
    let inner = match (open, close) {
        (Some(a), Some(b)) if b > a => &raw[a + 1..b],
        (None, _) if raw.len() >= 2 => &raw[1..raw.len() - 1],
        _ => "",
    };
    let cleaned = inner.replace('\r', "");
    let mut lines: Vec<&str> = cleaned.split('\n').collect();
    while lines.first().is_some_and(|l| trim(l).is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| trim(l).is_empty()) {
        lines.pop();
    }
    let indent = lines
        .iter()
        .filter(|l| !trim(l).is_empty())
        .map(|l| l.encode_utf16().count() - trim_start(l).encode_utf16().count())
        .min()
        .unwrap_or(usize::MAX);
    lines
        .iter()
        .map(|l| trim_end(&story::substring(l, indent.min(l.encode_utf16().count()))).to_string())
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let types = types();
    let src: Vec<u16> = source.encode_utf16().collect();
    let mut r = Reader {
        i: 0,
        tokens: tokens::tokenize(&src, &types, "eventmodeling")?,
        types: &types,
        diagram: "eventmodeling",
    };
    let mut model = Model {
        title: title.map(str::to_string),
        ..Default::default()
    };
    let mut frames = Vec::new();
    let mut data: IndexMap<String, String> = IndexMap::new();
    expect(&mut r, "eventmodeling")?;
    while r.kind() != -1 {
        match kind(&r) {
            "EM_ACC_DESCR" => model.acc_descr = Some(story::squeeze(&tokens::acc_descr_value(&r.take()))),
            "EM_ACC_TITLE" => model.acc_title = Some(story::acc(&tokens::acc_title_value(&r.take()))),
            "EM_TITLE" => {
                let v = tokens::title_value(&r.take());
                model.title = if v.is_empty() { title.map(str::to_string) } else { Some(v) };
            }
            "entity" => {
                r.i += 1;
                model.entities.push(qualified(&mut r)?)
            }
            "tf" | "timeframe" | "rf" | "resetframe" => {
                let mut f = Frame {
                    reset: matches!(kind(&r), "rf" | "resetframe"),
                    offset: r.tokens.starts[r.i],
                    ..Default::default()
                };
                r.i += 1;
                f.id = expect(&mut r, "EM_FID")?;
                f.kind = entity_type(&mut r)?;
                f.entity = qualified(&mut r)?;
                while accept(&mut r, "->>") {
                    f.sources.push(expect(&mut r, "EM_FID")?)
                }
                if accept(&mut r, "[[") {
                    f.reference = Some(expect(&mut r, "EM_ID")?);
                    expect(&mut r, "]]")?;
                }
                if matches!(kind(&r), "`" | "EM_DATA_INLINE") {
                    f.data_type = data_type(&mut r)?;
                    f.inline = Some(expect(&mut r, "EM_DATA_INLINE")?)
                }
                frames.push(f)
            }
            "data" => {
                r.i += 1;
                let name = expect(&mut r, "EM_ID")?;
                data_type(&mut r)?;
                let text = expect(&mut r, "EM_DATA_BLOCK")?;
                data.entry(name).or_insert(text);
            }
            "note" => {
                r.i += 1;
                let frame = expect(&mut r, "EM_FID")?;
                let data_type = data_type(&mut r)?;
                let text = block_text(&expect(&mut r, "EM_DATA_BLOCK")?);
                model.notes.push(Note { frame, text, data_type })
            }
            "gwt" => {
                r.i += 1;
                let frame = expect(&mut r, "EM_FID")?;
                expect(&mut r, "given")?;
                let given = statements(&mut r)?;
                let when = if accept(&mut r, "when") { statements(&mut r)? } else { Vec::new() };
                expect(&mut r, "then")?;
                let then = statements(&mut r)?;
                model.specifications.push(Specification { frame, given, when, then })
            }
            _ => return Err(r.fail("a frame, an entity, a data block, a note, or a specification")),
        }
    }
    let mut ids = std::collections::HashSet::new();
    for f in &frames {
        if !ids.insert(&f.id) {
            let line = locate(&src, f.offset).line;
            let mut e = PeleError::new(
                format!("Duplicate event modeling frame ID \"{}\" on line {line}", f.id),
                PeleErrorCode::Semantic,
            )
            .of("eventmodeling");
            e.line = line;
            return Err(e);
        }
    }
    let mut lane_of: IndexMap<String, usize> = IndexMap::new();
    let mut next = [1, 101, 201];
    for (index, f) in frames.iter().enumerate() {
        let parts: Vec<&str> = f.entity.split('.').collect();
        let namespace = if parts.len() == 2 { Some(parts[0].to_string()) } else { None };
        let kind = match f.kind.as_str() {
            "ui" => "ui",
            "pcr" | "processor" => "processor",
            "rmo" | "readmodel" => "readmodel",
            "cmd" | "command" => "command",
            _ => "event",
        };
        let group = match kind {
            "ui" | "processor" => 0,
            "event" => 2,
            _ => 1,
        };
        let group_name = ["trigger", "state", "stream"][group];
        let key = namespace
            .as_ref()
            .map(|ns| format!("{group_name} {ns}"))
            .unwrap_or_else(|| group_name.to_string());
        let lane = if let Some(&i) = lane_of.get(&key) {
            i
        } else {
            let i = model.lanes.len();
            let lane_index = if namespace.is_some() {
                let n = next[group];
                next[group] += 1;
                n
            } else {
                group * 100
            };
            let label = namespace
                .as_ref()
                .map(|ns| format!("{}{ns}", ["UI/A: ", "C/RM: ", "Stream: "][group]))
                .unwrap_or_else(|| ["UI/Automation", "Command/Read Model", "Events"][group].to_string());
            model.lanes.push(Lane {
                index: lane_index,
                label,
                group: group_name.into(),
                namespace: namespace.clone(),
            });
            lane_of.insert(key, i);
            i
        };
        let mut text = f.inline.as_deref().map(block_text);
        if let Some(reference) = &f.reference
            && let Some(block) = data.get(reference)
        {
            text = Some(block_text(block))
        }
        model.boxes.push(Box {
            id: f.id.clone(),
            kind: kind.into(),
            entity: f.entity.clone(),
            name: if parts.len() == 2 { parts[1].into() } else { f.entity.clone() },
            namespace,
            reset: f.reset,
            lane,
            data: text,
            data_type: f.data_type.clone(),
        });
        if !f.sources.is_empty() {
            for (i, b) in model.boxes.iter().enumerate() {
                if f.sources.contains(&b.id) {
                    model.relations.push((i, index))
                }
            }
        } else if index > 0
            && !f.reset
            && let Some(i) = (0..index).rev().find(|&i| model.boxes[i].lane != lane)
        {
            model.relations.push((i, index))
        }
    }
    let mut order: Vec<usize> = (0..model.lanes.len()).collect();
    order.sort_by_key(|i| model.lanes[*i].index);
    for b in &mut model.boxes {
        b.lane = order.iter().position(|i| *i == b.lane).unwrap()
    }
    model.lanes = order.iter().map(|i| model.lanes[*i].clone()).collect();
    Ok(model)
}
