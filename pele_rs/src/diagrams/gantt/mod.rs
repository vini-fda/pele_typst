//! Gantt grammar, task dependencies, calendar exclusions, and SVG rendering.
mod axis;
pub mod dates;
mod lexer;
mod render;
#[cfg(test)]
mod tests;
mod tokens;
use crate::errors::{PeleError, PeleErrorCode, syntax_error};
use crate::preprocess::Config;
use crate::util::{is_js_space, js_number};
use dates::{WEEKDAYS, Zone};
use indexmap::IndexMap;
pub use render::render;
use std::collections::{HashMap, HashSet};
use tokens::{T, TOKEN_NAMES};
#[derive(Debug, Clone)]
pub struct RawTask {
    pub data: String,
    pub start: Option<String>,
    pub end: String,
}
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub task: String,
    pub section: String,
    pub start_time: Option<f64>,
    pub end_time: Option<f64>,
    pub render_end_time: Option<f64>,
    pub manual_end_time: bool,
    pub processed: bool,
    pub active: bool,
    pub done: bool,
    pub crit: bool,
    pub milestone: bool,
    pub vert: bool,
    pub order: i64,
    pub classes: Vec<String>,
    pub prev_task_id: Option<String>,
    pub raw: RawTask,
}
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub date_format: String,
    pub axis_format: String,
    pub tick_interval: Option<String>,
    pub today_marker: String,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub inclusive_end_dates: bool,
    pub top_axis: bool,
    pub display_mode: String,
    pub weekday: Option<String>,
    pub weekend: String,
    pub sections: Vec<String>,
    pub links: IndexMap<String, String>,
    pub tasks: Vec<Task>,
    by_id: HashMap<String, usize>,
    current_section: String,
    task_count: usize,
    last_order: i64,
    last_task_id: Option<String>,
}
fn invalid(message: impl Into<String>) -> PeleError {
    PeleError::new(message, PeleErrorCode::Semantic).of("gantt")
}
impl Model {
    fn add_task(&mut self, description: String, data_str: String) -> Result<(), PeleError> {
        let mut data: Vec<_> = data_str.strip_prefix(':').unwrap_or(&data_str).split(',').collect();
        let mut task = Task {
            id: String::new(),
            task: description,
            section: self.current_section.clone(),
            start_time: None,
            end_time: None,
            render_end_time: None,
            manual_end_time: false,
            processed: false,
            active: false,
            done: false,
            crit: false,
            milestone: false,
            vert: false,
            order: -1,
            classes: vec![],
            prev_task_id: self.last_task_id.clone(),
            raw: RawTask {
                data: data_str.clone(),
                start: None,
                end: String::new(),
            },
        };
        while let Some(tag) = data.first() {
            match tag.trim_matches(is_js_space) {
                "active" => task.active = true,
                "done" => task.done = true,
                "crit" => task.crit = true,
                "milestone" => task.milestone = true,
                "vert" => task.vert = true,
                _ => break,
            }
            data.remove(0);
        }
        if data.is_empty() {
            return Err(invalid(format!(
                "Task \"{}\" needs a duration or an end date.",
                task.task.trim_matches(is_js_space)
            )));
        }
        if data.len() > 3 {
            return Err(invalid(format!(
                "Task \"{}\" has too many fields. A task takes an id, a start, and an end or duration.",
                task.task.trim_matches(is_js_space)
            )));
        }
        let data: Vec<_> = data.iter().map(|s| s.trim_matches(is_js_space)).collect();
        task.id = if data.len() == 3 {
            data[0].to_string()
        } else {
            self.task_count += 1;
            format!("task{}", self.task_count)
        };
        if data.len() > 1 {
            task.raw.start = Some(data[data.len() - 2].to_string());
        }
        task.raw.end = data[data.len() - 1].to_string();
        if !task.vert {
            task.order = self.last_order;
            self.last_order += 1;
        }
        self.last_task_id = Some(task.id.clone());
        self.by_id.insert(task.id.clone(), self.tasks.len());
        self.tasks.push(task);
        Ok(())
    }
    fn class(&mut self, ids: &str) {
        for id in ids.split(',') {
            if let Some(&i) = self.by_id.get(id) {
                self.tasks[i].classes.push("clickable".into());
            }
        }
    }
    fn link(&mut self, ids: &str, link: &str) {
        let href = crate::url::safe_url(link);
        for id in ids.split(',') {
            if self.by_id.contains_key(id) {
                self.links.insert(id.to_string(), href.clone());
            }
        }
        self.class(ids);
    }
}
fn merge(existing: &mut Vec<String>, text: &str) {
    for token in text.to_lowercase().split(|c| is_js_space(c) || c == ',') {
        if !token.is_empty() && !existing.iter().any(|v| v == token) {
            existing.push(token.to_string());
        }
    }
}
const LINE: &str = "'EOF', 'SPACE', 'NL', 'weekday_monday', 'weekday_tuesday', 'weekday_wednesday', 'weekday_thursday', 'weekday_friday', 'weekday_saturday', 'weekday_sunday', 'weekend_friday', 'weekend_saturday', 'dateFormat', 'inclusiveEndDates', 'topAxis', 'axisFormat', 'tickInterval', 'excludes', 'includes', 'todayMarker', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'taskTxt', 'click'";
fn starts(t: T) -> bool {
    matches!(
        t,
        T::EOF
            | T::NL
            | T::dateFormat
            | T::inclusiveEndDates
            | T::topAxis
            | T::axisFormat
            | T::tickInterval
            | T::excludes
            | T::includes
            | T::todayMarker
            | T::title
            | T::acc_title
            | T::acc_descr
            | T::acc_descr_multiline_value
            | T::section
            | T::taskTxt
            | T::click
    ) || (T::weekday_monday as usize..=T::weekend_saturday as usize).contains(&(t as usize))
}
struct Reader {
    tokens: lexer::Tokens,
    i: usize,
}
impl Reader {
    fn kind(&self) -> T {
        self.tokens.types.get(self.i).copied().unwrap_or(T::END)
    }
    fn take(&mut self) -> String {
        let text = self.tokens.texts.get(self.i).cloned().unwrap_or_default();
        self.i += 1;
        text
    }
    fn fail(&self, expected: &str) -> PeleError {
        syntax_error(
            "gantt",
            &self.tokens.src,
            self.tokens.starts.get(self.i).copied().unwrap_or(self.tokens.src.len()),
            &format!(
                "Expecting {expected}, got '{}'",
                if self.kind() == T::END {
                    "1"
                } else {
                    TOKEN_NAMES[self.kind() as usize]
                }
            ),
            false,
        )
    }
    fn end(&self, also: &str) -> Result<(), PeleError> {
        if starts(self.kind()) {
            Ok(())
        } else {
            Err(self.fail(&(LINE.to_string() + also)))
        }
    }
    fn expect(&mut self, t: T, name: &str) -> Result<String, PeleError> {
        if self.kind() != t {
            return Err(self.fail(name));
        }
        Ok(self.take())
    }
}
// Grammar actions slice JavaScript strings by UTF-16 code units. The delimiter
// after a keyword may be a non-ASCII whitespace character.
fn keyword_value(text: &str, start: usize) -> String {
    String::from_utf16_lossy(&text.encode_utf16().skip(start).collect::<Vec<_>>())
}
pub fn parse(source: &str, config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let mut db = Model {
        title: title.filter(|s| !s.is_empty()).map(str::to_string),
        display_mode: config
            .get("gantt")
            .and_then(|c| c.get("displayMode"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .into(),
        weekend: "saturday".into(),
        ..Model::default()
    };
    let mut r = Reader {
        tokens: lexer::tokenize(source),
        i: 0,
    };
    r.expect(T::gantt, "'gantt'")?;
    loop {
        let t = r.kind();
        if !starts(t) {
            return Err(r.fail(LINE));
        }
        let text = r.take();
        match t {
            T::EOF => {
                if r.kind() == T::END {
                    return Ok(db);
                }
                r.end("")?;
            }
            T::NL => {}
            T::acc_title | T::acc_descr => {
                let value = if t == T::acc_title {
                    r.expect(T::acc_title_value, "'acc_title_value'")?
                } else {
                    r.expect(T::acc_descr_value, "'acc_descr_value'")?
                };
                r.end("")?;
                let value = value.trim_matches(is_js_space).to_string();
                if t == T::acc_title {
                    db.acc_title = Some(value);
                } else {
                    db.acc_descr = Some(clean_description(&value));
                }
            }
            T::taskTxt => {
                let data = r.expect(T::taskData, "'taskData'")?;
                r.end("")?;
                db.add_task(text, data)?;
            }
            T::click => {
                let (mut name, mut args, mut href) = (None, None, None);
                if r.kind() == T::callbackname {
                    name = Some(r.take());
                    if r.kind() == T::callbackargs {
                        args = Some(r.take());
                    }
                    if r.kind() == T::href {
                        href = Some(r.take());
                        r.end("")?;
                    } else {
                        r.end(if args.is_none() { ", 'callbackargs', 'href'" } else { ", 'href'" })?;
                    }
                } else if r.kind() == T::href {
                    href = Some(r.take());
                    if r.kind() == T::callbackname {
                        name = Some(r.take());
                        if r.kind() == T::callbackargs {
                            r.take();
                            r.end("")?;
                        } else {
                            r.end(", 'callbackargs'")?;
                        }
                    } else {
                        r.end(", 'callbackname'")?;
                    }
                } else {
                    return Err(r.fail("'callbackname', 'href'"));
                }
                if name.is_some() {
                    db.class(&text);
                }
                if let Some(href) = href {
                    db.link(&text, &href);
                }
            }
            _ => {
                r.end("")?;
                match t {
                    T::dateFormat => db.date_format = keyword_value(&text, 11),
                    T::inclusiveEndDates => db.inclusive_end_dates = true,
                    T::topAxis => db.top_axis = true,
                    T::axisFormat => db.axis_format = keyword_value(&text, 11),
                    T::tickInterval => db.tick_interval = Some(keyword_value(&text, 13)),
                    T::excludes => merge(&mut db.excludes, &keyword_value(&text, 9)),
                    T::includes => merge(&mut db.includes, &keyword_value(&text, 9)),
                    T::todayMarker => db.today_marker = keyword_value(&text, 12),
                    T::weekend_friday => db.weekend = "friday".into(),
                    T::weekend_saturday => db.weekend = "saturday".into(),
                    T::title => db.title = Some(keyword_value(&text, 6)),
                    T::acc_descr_multiline_value => db.acc_descr = Some(clean_description(text.trim_matches(is_js_space))),
                    T::section => {
                        db.current_section = keyword_value(&text, 8);
                        db.sections.push(db.current_section.clone());
                    }
                    _ => {
                        db.weekday = Some(
                            ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]
                                [t as usize - T::weekday_monday as usize]
                                .into(),
                        )
                    }
                }
            }
        }
    }
}
fn clean_description(text: &str) -> String {
    let mut out = String::new();
    let mut after_newline = false;
    for c in text.chars() {
        if after_newline && is_js_space(c) {
            continue;
        }
        out.push(c);
        after_newline = c == '\n';
    }
    out
}
pub struct Schedule {
    pub model: Model,
    pub now: f64,
    pub zone: Zone,
    work: f64,
    includes: HashSet<String>,
    excludes: HashSet<String>,
    excluded_days: [bool; 7],
    dated_tokens: bool,
}
impl Schedule {
    pub fn new(model: &Model, now: f64, zone: Zone) -> Result<Self, PeleError> {
        let includes: HashSet<_> = model.includes.iter().cloned().collect();
        let excludes: HashSet<_> = model.excludes.iter().cloned().collect();
        let weekend = if model.weekend == "friday" {
            5
        } else if model.weekend == "saturday" {
            6
        } else {
            99
        };
        let excluded_days = std::array::from_fn(|i| {
            let day = if i == 0 { 7 } else { i };
            (excludes.contains("weekends") && (day == weekend || day == weekend + 1)) || excludes.contains(&WEEKDAYS[i].to_lowercase())
        });
        let numeric = "YMDHhmsSZXx-/.:, T";
        let dated_tokens = !includes.is_empty()
            || !model.date_format.trim_matches(is_js_space).chars().all(|c| numeric.contains(c))
            || excludes
                .iter()
                .any(|s| s != "weekends" && !WEEKDAYS.iter().any(|d| d.to_lowercase() == *s));
        let mut out = Self {
            model: model.clone(),
            now,
            zone,
            work: 0.0,
            includes,
            excludes,
            excluded_days,
            dated_tokens,
        };
        for task in &mut out.model.tasks {
            task.start_time = None;
            task.end_time = None;
            task.render_end_time = None;
            task.processed = false;
            task.manual_end_time = false;
        }
        for _ in 0..11 {
            let mut all = true;
            for i in 0..out.model.tasks.len() {
                out.compile(i)?;
                all &= out.model.tasks[i].processed;
            }
            if all {
                break;
            }
        }
        Ok(out)
    }
    fn spend(&mut self, cost: f64) -> Result<(), PeleError> {
        self.work += cost;
        if self.work > 4e8 {
            return Err(PeleError::new("The gantt chart takes too much work to schedule.", PeleErrorCode::Limit).of("gantt"));
        }
        Ok(())
    }
    fn parse_date(&mut self, text: &str, template: &str) -> Result<Option<f64>, PeleError> {
        self.spend((template.encode_utf16().count() + 1) as f64 * (text.encode_utf16().count() + 1) as f64)?;
        Ok(dates::parse_strict(text, template, self.now, &self.zone))
    }
    pub fn is_invalid(&mut self, time: f64) -> Result<bool, PeleError> {
        let weekday = self.zone.parts(time).is_some_and(|p| self.excluded_days[p.weekday as usize]);
        if !self.dated_tokens {
            return Ok(weekday);
        }
        let template = self.model.date_format.trim_matches(is_js_space).to_string();
        let date_only = dates::iso_date(time, &self.zone);
        self.spend(200.0)?;
        let formatted = if template == "YYYY-MM-DD" {
            Some(date_only.clone())
        } else {
            self.spend(400.0 + template.encode_utf16().count() as f64 * 8.0)?;
            dates::format(time, &template, &self.zone)
        };
        let listed = |set: &HashSet<String>| set.contains(&date_only) || formatted.as_ref().is_some_and(|s| set.contains(s));
        if listed(&self.includes) {
            return Ok(false);
        }
        Ok(weekday || listed(&self.excludes))
    }
    fn today(&self) -> f64 {
        dates::start_day(self.now, &self.zone)
    }
    fn reference(&self, text: &str, keyword: &str) -> Option<Option<f64>> {
        let rest = text.strip_prefix(keyword)?;
        let first = rest.chars().next()?;
        if !is_js_space(first) {
            return None;
        }
        let rest = rest.trim_start_matches(is_js_space);
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ')))
            .unwrap_or(rest.len());
        if end == 0 {
            return None;
        }
        let mut found = false;
        let mut selected = None;
        for id in rest[..end].split(' ') {
            if let Some(&i) = self.model.by_id.get(id) {
                let task = &self.model.tasks[i];
                let value = if keyword == "after" { task.end_time } else { task.start_time };
                if !found || matches!((value,selected),(Some(a),Some(b)) if if keyword=="after"{a>b}else{a<b}) {
                    selected = value;
                    found = true;
                }
            }
        }
        Some(if found { selected } else { Some(self.today()) })
    }
    fn start(&mut self, raw: &str) -> Result<Option<f64>, PeleError> {
        let text = raw.trim_matches(is_js_space);
        let template = self.model.date_format.trim_matches(is_js_space).to_string();
        if (template == "x" || template == "X") && !text.is_empty() && text.bytes().all(|c| c.is_ascii_digit()) {
            let date = js_number(text).trunc();
            return if dates::valid(date) {
                Ok(Some(date))
            } else {
                Err(invalid(format!("Invalid date:{text}")))
            };
        }
        if let Some(date) = self.reference(text, "after") {
            return Ok(date);
        }
        if let Some(date) = self.parse_date(text, &template)? {
            return Ok(Some(date));
        }
        let date = dates::fallback(text, &self.zone)
            .filter(|&d| dates::valid(d) && self.zone.parts(d).is_some_and(|p| (-10000..=10000).contains(&p.year)))
            .ok_or_else(|| invalid(format!("Invalid date:{text}")))?;
        Ok(Some(date))
    }
    fn end(&mut self, start: f64, raw: &str) -> Result<Option<f64>, PeleError> {
        let text = raw.trim_matches(is_js_space);
        if let Some(date) = self.reference(text, "until") {
            return Ok(date);
        }
        let template = self.model.date_format.trim_matches(is_js_space).to_string();
        if let Some(date) = self.parse_date(text, &template)? {
            return Ok(Some(if self.model.inclusive_end_dates {
                dates::add_days(date, 1.0, &self.zone)
            } else {
                date
            }));
        }
        let end = if let Some((value, unit)) = duration(text) {
            dates::add(start, value, unit, &self.zone)
        } else {
            start
        };
        Ok(Some(if dates::valid(end) { end } else { start }))
    }
    fn compile(&mut self, i: usize) -> Result<(), PeleError> {
        let raw = self.model.tasks[i].raw.clone();
        if let Some(start) = raw.start {
            if let Some(date) = self.start(&start)? {
                self.model.tasks[i].start_time = Some(date);
            }
        } else {
            let previous = self.model.tasks[i]
                .prev_task_id
                .as_ref()
                .and_then(|id| self.model.by_id.get(id))
                .copied();
            self.model.tasks[i].start_time = previous.map(|p| self.model.tasks[p].end_time).unwrap_or_else(|| Some(self.today()));
        }
        let Some(start) = self.model.tasks[i].start_time else {
            return Ok(());
        };
        self.model.tasks[i].end_time = self.end(start, &raw.end)?;
        let Some(mut end) = self.model.tasks[i].end_time else {
            return Ok(());
        };
        self.model.tasks[i].processed = true;
        let manual = self.parse_date(&raw.end, "YYYY-MM-DD")?.is_some();
        self.model.tasks[i].manual_end_time = manual;
        if self.model.excludes.is_empty() || manual {
            return Ok(());
        }
        let mut cursor = dates::add_days(start, 1.0, &self.zone);
        let max_end = dates::add_days(end, 10000.0, &self.zone);
        let mut invalid_day = false;
        let mut render_end = None;
        while cursor <= end {
            self.spend(400.0)?;
            if !invalid_day {
                render_end = Some(end);
            }
            invalid_day = self.is_invalid(cursor)?;
            if invalid_day {
                end = dates::add_days(end, 1.0, &self.zone);
                if end > max_end {
                    return Err(invalid(
                        "Failed to find a valid date that was not excluded by `excludes` after 10,000 iterations.",
                    ));
                }
            }
            cursor = dates::add_days(cursor, 1.0, &self.zone);
        }
        self.model.tasks[i].end_time = Some(end);
        self.model.tasks[i].render_end_time = render_end;
        Ok(())
    }
}
fn duration(text: &str) -> Option<(f64, &str)> {
    let end = text.find(|c: char| !c.is_ascii_digit() && c != '.')?;
    let value = &text[..end];
    let mut dot = false;
    for (i, c) in value.bytes().enumerate() {
        if c == b'.' {
            if dot || i == 0 || i + 1 == value.len() {
                return None;
            }
            dot = true;
        }
    }
    let unit = &text[end..];
    if !matches!(unit, "M" | "d" | "h" | "m" | "s" | "w" | "y" | "ms") {
        return None;
    }
    Some((js_number(value), unit))
}
