use super::{colors::is_css_color, kinds as lt};
use crate::preprocess::{parse_yaml, truthy, value_string};
use crate::util::trim;
use crate::{Config, PeleError, PeleErrorCode};
use indexmap::IndexMap;
use serde_json::Value;
#[derive(Debug, Clone, Default)]
pub struct Text {
    pub text: String,
    pub wrap: Option<bool>,
}
#[derive(Debug, Clone)]
pub struct BoxData {
    pub text: Option<String>,
    pub color: String,
    pub wrap: Option<bool>,
}
#[derive(Debug, Clone)]
pub struct SeqBox {
    pub name: Option<String>,
    pub wrap: bool,
    pub fill: String,
    pub actor_keys: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct Actor {
    pub box_id: Option<usize>,
    pub name: String,
    pub description: String,
    pub wrap: bool,
    pub prev_actor: Option<String>,
    pub next_actor: Option<String>,
    pub links: IndexMap<String, Value>,
    pub properties: IndexMap<String, Value>,
    pub details: Option<String>,
    pub kind: String,
}
#[derive(Debug, Clone, Default)]
pub struct Numbering {
    pub start: Option<f64>,
    pub step: Option<f64>,
    pub visible: bool,
}
#[derive(Debug, Clone, Default)]
pub struct Message {
    pub id: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub message: String,
    pub numbering: Option<Numbering>,
    pub wrap: bool,
    pub kind: Option<i32>,
    pub activate: bool,
    pub placement: Option<i32>,
    pub central_connection: i32,
}
#[derive(Debug, Clone)]
pub struct Note {
    pub actor: Vec<String>,
    pub placement: i32,
    pub message: String,
    pub wrap: bool,
}
#[derive(Debug, Clone, Default)]
pub struct Statement {
    pub kind: &'static str,
    pub actor: Vec<String>,
    pub draw: Option<String>,
    pub description: Option<Text>,
    pub config: Option<String>,
    pub signal_type: Option<i32>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub msg: Option<Text>,
    pub activate: bool,
    pub central_connection: i32,
    pub placement: i32,
    pub text: Option<Text>,
    pub box_data: Option<BoxData>,
    pub sequence_index: Option<f64>,
    pub sequence_step: Option<f64>,
    pub sequence_visible: bool,
}
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub actors: IndexMap<String, Actor>,
    pub created_actors: IndexMap<String, usize>,
    pub destroyed_actors: IndexMap<String, usize>,
    pub boxes: Vec<SeqBox>,
    pub messages: Vec<Message>,
    pub notes: Vec<Note>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    prev_actor: Option<String>,
    current_box: Option<usize>,
    last_created: Option<String>,
    last_destroyed: Option<String>,
    wrap_enabled: Option<bool>,
    sequence_wrap: bool,
    active: IndexMap<String, usize>,
}
fn fail(message: impl Into<String>) -> PeleError {
    PeleError::new(message, PeleErrorCode::Semantic).of("sequence")
}
pub fn parse_message(raw: &str) -> Text {
    let text = trim(raw);
    let tail = text.strip_prefix(':').unwrap_or(text);
    let (text, wrap) = if let Some(t) = tail.strip_prefix("wrap:") {
        (t, Some(true))
    } else if let Some(t) = tail.strip_prefix("nowrap:") {
        (t, Some(false))
    } else {
        (text, None)
    };
    Text {
        text: trim(text).into(),
        wrap,
    }
}
pub fn box_data(raw: &str) -> BoxData {
    let terminator = |s: &str| s.contains(['\r', '\u{2028}', '\u{2029}']);
    let parts = (|| {
        let open = raw.find('(');
        if let Some(open) = open.filter(|&i| matches!(raw[..i].trim_end(), "rgb" | "rgba" | "hsl" | "hsla"))
            && let Some(close) = raw.rfind(')').filter(|&c| c > open)
        {
            if terminator(&raw[open + 1..]) {
                return None;
            }
            return Some((&raw[..=close], &raw[close + 1..]));
        }
        if terminator(raw) {
            return None;
        }
        let end = raw
            .char_indices()
            .find(|(_, c)| !c.is_ascii_alphanumeric() && *c != '_')
            .map_or(raw.len(), |(i, _)| i);
        Some((&raw[..end], &raw[end..]))
    })();
    let mut color = parts
        .filter(|(c, _)| !c.is_empty())
        .map(|(c, _)| trim(c))
        .unwrap_or("transparent")
        .to_string();
    let mut title = parts.filter(|(_, t)| !t.is_empty()).map(|(_, t)| trim(t).to_string());
    if !is_css_color(&color) {
        color = "transparent".into();
        title = Some(trim(raw).into());
    }
    let text = title.map(|s| parse_message(&s));
    BoxData {
        text: text.as_ref().map(|s| s.text.clone()).filter(|s| !s.is_empty()),
        color,
        wrap: text.and_then(|s| s.wrap),
    }
}
fn entries(v: Value) -> Vec<(String, Value)> {
    match v {
        Value::String(s) => s
            .chars()
            .enumerate()
            .map(|(i, c)| (i.to_string(), Value::String(c.to_string())))
            .collect(),
        Value::Object(o) => o.into_iter().collect(),
        Value::Array(a) => a.into_iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect(),
        _ => Vec::new(),
    }
}
impl Model {
    pub fn new(config: &Config, title: Option<&str>) -> Self {
        Self {
            wrap_enabled: config.get("wrap").and_then(Value::as_bool),
            sequence_wrap: config.get("sequence").and_then(|v| v.get("wrap")) == Some(&Value::Bool(true)),
            title: title.map(String::from),
            ..Default::default()
        }
    }
    pub fn auto_wrap(&self) -> bool {
        self.wrap_enabled.unwrap_or(self.sequence_wrap)
    }
    pub fn set_acc_descr(&mut self, text: &str) {
        let chars: Vec<_> = text.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            out.push(c);
            i += 1;
            if c == '\n' {
                while i < chars.len() && crate::util::is_js_space(chars[i]) {
                    i += 1;
                }
            }
        }
        self.acc_descr = Some(out);
    }
    fn add_actor(
        &mut self,
        id: &str,
        mut description: Option<Text>,
        mut kind: Option<String>,
        metadata: Option<&str>,
    ) -> Result<(), PeleError> {
        let mut assigned = self.current_box;
        if let Some(metadata) = metadata {
            let yaml = if metadata.contains('\n') {
                format!("{metadata}\n")
            } else {
                format!("{{\n{metadata}\n}}")
            };
            let doc = parse_yaml(&yaml).map_err(|e| {
                PeleError::new(
                    format!("Config of participant \"{id}\" is not valid YAML. {e}"),
                    PeleErrorCode::Syntax,
                )
                .of("sequence")
            })?;
            if let Some(doc) = doc.as_object() {
                if let Some(t) = doc.get("type").filter(|v| !v.is_null()) {
                    kind = Some(value_string(t));
                }
                if truthy(doc.get("alias")) && description.as_ref().is_none_or(|d| d.text == id) {
                    description = Some(Text {
                        text: value_string(&doc["alias"]),
                        wrap: description.as_ref().and_then(|d| d.wrap),
                    });
                }
            }
        }
        if let Some(old) = self.actors.get_mut(id) {
            if let (Some(current), Some(old_box)) = (self.current_box, old.box_id)
                && current != old_box
            {
                return Err(fail(format!(
                    "A same participant should only be defined in one Box: {} can't be in '{}' and in '{}' at the same time.",
                    old.name,
                    self.boxes[old_box].name.as_deref().unwrap_or("undefined"),
                    self.boxes[current].name.as_deref().unwrap_or("undefined")
                )));
            }
            assigned = old.box_id.or(self.current_box);
            old.box_id = assigned;
            if old.name == id && description.is_none() {
                return Ok(());
            }
        }
        if description.is_none() || kind.is_none() {
            description = Some(Text {
                text: id.into(),
                wrap: None,
            });
        }
        let description = description.unwrap();
        self.actors.insert(
            id.into(),
            Actor {
                box_id: assigned,
                name: id.into(),
                description: description.text,
                wrap: description.wrap.unwrap_or(self.auto_wrap()),
                prev_actor: self.prev_actor.clone(),
                next_actor: None,
                links: IndexMap::new(),
                properties: IndexMap::new(),
                details: None,
                kind: kind.unwrap_or_else(|| "participant".into()),
            },
        );
        if let Some(prev) = self.prev_actor.as_ref().and_then(|id| self.actors.get_mut(id)) {
            prev.next_actor = Some(id.into());
        }
        if let Some(current) = self.current_box {
            self.boxes[current].actor_keys.push(id.into());
        }
        self.prev_actor = Some(id.into());
        Ok(())
    }
    fn add_signal(
        &mut self,
        from: Option<String>,
        to: Option<String>,
        message: Option<Text>,
        kind: Option<i32>,
        activate: bool,
        central: i32,
    ) -> Result<(), PeleError> {
        if kind == Some(lt::ACTIVE_END) {
            let count = from.as_ref().and_then(|id| self.active.get(id)).copied().unwrap_or(0);
            if count < 1 {
                return Err(fail(format!(
                    "Trying to inactivate an inactive participant ({})",
                    from.as_deref().unwrap_or("undefined")
                )));
            }
            self.active.insert(from.clone().unwrap(), count - 1);
        } else if kind == Some(lt::ACTIVE_START)
            && let Some(from) = &from
        {
            *self.active.entry(from.clone()).or_default() += 1;
        }
        let message = message.unwrap_or_default();
        self.messages.push(Message {
            id: self.messages.len().to_string(),
            from,
            to,
            message: message.text,
            wrap: message.wrap.unwrap_or(self.auto_wrap()),
            kind,
            activate,
            central_connection: central,
            ..Default::default()
        });
        Ok(())
    }
    pub fn apply(&mut self, params: Vec<Statement>) -> Result<(), PeleError> {
        for p in params {
            let actor = p.actor.first().cloned().unwrap_or_default();
            match p.kind {
                "sequenceIndex" => self.messages.push(Message {
                    id: self.messages.len().to_string(),
                    numbering: Some(Numbering {
                        start: p.sequence_index,
                        step: p.sequence_step,
                        visible: p.sequence_visible,
                    }),
                    kind: p.signal_type,
                    ..Default::default()
                }),
                "addParticipant" => self.add_actor(&actor, p.description, p.draw, p.config.as_deref())?,
                "createParticipant" => {
                    if self.actors.contains_key(&actor) {
                        return Err(fail(
                            "It is not possible to have actors with the same id, even if one is destroyed before the next is created. Use 'AS' aliases to simulate the behavior",
                        ));
                    }
                    self.last_created = Some(actor.clone());
                    self.add_actor(&actor, p.description, p.draw, p.config.as_deref())?;
                    self.created_actors.insert(actor, self.messages.len());
                }
                "destroyParticipant" => {
                    self.last_destroyed = Some(actor.clone());
                    self.destroyed_actors.insert(actor, self.messages.len());
                }
                "activeStart" | "activeEnd" | "centralConnection" | "centralConnectionReverse" => {
                    self.add_signal(Some(actor), None, None, p.signal_type, false, 0)?
                }
                "addNote" => {
                    let text = p.text.unwrap();
                    let wrap = text.wrap.unwrap_or(self.auto_wrap());
                    let from = p.actor.first().cloned();
                    let to = p.actor.get(1).cloned().or_else(|| from.clone());
                    self.notes.push(Note {
                        actor: p.actor,
                        placement: p.placement,
                        message: text.text.clone(),
                        wrap,
                    });
                    self.messages.push(Message {
                        id: self.messages.len().to_string(),
                        from,
                        to,
                        message: text.text,
                        wrap,
                        kind: Some(lt::NOTE),
                        placement: Some(p.placement),
                        ..Default::default()
                    });
                }
                "addLinks" | "addProperties" => {
                    if let Some(actor) = self.actors.get_mut(&actor)
                        && let Ok(value) = serde_json::from_str::<Value>(&p.text.unwrap().text)
                    {
                        let target = if p.kind == "addLinks" {
                            &mut actor.links
                        } else {
                            &mut actor.properties
                        };
                        for (k, v) in entries(value) {
                            target.insert(k, v);
                        }
                    }
                }
                "addALink" => {
                    if let Some(actor) = self.actors.get_mut(&actor) {
                        let text = p.text.unwrap().text;
                        let chars: Vec<_> = text.chars().collect();
                        let sep = chars.iter().position(|&c| c == '@').map(|i| i as isize).unwrap_or(-1);
                        let end = if sep - 1 < 0 {
                            (chars.len() as isize + sep - 1).max(0) as usize
                        } else {
                            (sep - 1) as usize
                        };
                        let key = chars[..end].iter().collect::<String>();
                        let value = chars[(sep + 1) as usize..].iter().collect::<String>();
                        actor.links.insert(trim(&key).into(), Value::String(trim(&value).into()));
                    }
                }
                "addDetails" => {
                    if let Some(actor) = self.actors.get_mut(&actor) {
                        actor.details = Some(p.text.unwrap().text);
                    }
                }
                "addMessage" => {
                    if let Some(created) = &self.last_created {
                        if p.to.as_ref() != Some(created) {
                            return Err(fail(format!(
                                "The created participant {created} does not have an associated creating message after its declaration. Please check the sequence diagram."
                            )));
                        }
                        self.last_created = None;
                    } else if let Some(destroyed) = &self.last_destroyed {
                        if p.to.as_ref() != Some(destroyed) && p.from.as_ref() != Some(destroyed) {
                            return Err(fail(format!(
                                "The destroyed participant {destroyed} does not have an associated destroying message after its declaration. Please check the sequence diagram."
                            )));
                        }
                        self.last_destroyed = None;
                    }
                    self.add_signal(p.from, p.to, p.msg, p.signal_type, p.activate, p.central_connection)?;
                }
                "boxStart" => {
                    let data = p.box_data.unwrap();
                    let wrap = data.wrap.unwrap_or(self.auto_wrap());
                    self.current_box = Some(self.boxes.len());
                    self.boxes.push(SeqBox {
                        name: data.text,
                        wrap,
                        fill: data.color,
                        actor_keys: Vec::new(),
                    });
                }
                "boxEnd" => self.current_box = None,
                "loopStart" | "rectStart" | "optStart" | "altStart" | "else" | "parStart" | "and" | "criticalStart" | "option"
                | "breakStart" => self.add_signal(None, None, p.text, p.signal_type, false, 0)?,
                "loopEnd" | "rectEnd" | "optEnd" | "altEnd" | "parEnd" | "criticalEnd" | "breakEnd" => {
                    self.add_signal(None, None, None, p.signal_type, false, 0)?
                }
                _ => {}
            }
        }
        Ok(())
    }
}
