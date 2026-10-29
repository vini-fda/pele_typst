// The agentflow parser, after pele's diagrams/agentflow/parser.ts.

use super::db::{DocItem, FlowDb, FlowText, LabelType, LinkInfo, Vertex};
use super::lexer::{Loc, T, TOKEN_NAMES, tokenize};
use crate::errors::{PeleError, Result, syntax_error};
use crate::util::trim;

fn is_id(t: T) -> bool {
    matches!(
        t,
        T::NUM | T::NODE_STRING | T::DOWN | T::MINUS | T::DEFAULT | T::COMMA | T::COLON | T::AMP | T::BRKT | T::MULT | T::UNICODE_TEXT
    )
}

fn is_alphanum(t: T) -> bool {
    matches!(
        t,
        T::NUM | T::UNICODE_TEXT | T::NODE_STRING | T::DIR | T::DOWN | T::MINUS | T::COMMA | T::COLON | T::AMP | T::BRKT | T::MULT
    )
}

fn is_no_tags(t: T) -> bool {
    matches!(
        t,
        T::NUM
            | T::NODE_STRING
            | T::SPACE
            | T::MINUS
            | T::AMP
            | T::UNICODE_TEXT
            | T::COLON
            | T::MULT
            | T::BRKT
            | T::STYLE
            | T::LINKSTYLE
            | T::CLASSDEF
            | T::CLASS
            | T::CLICK
            | T::GRAPH
            | T::DIR
            | T::flow
            | T::connector
            | T::global
            | T::end
            | T::DOWN
            | T::UP
            | T::START_LINK
    )
}

fn is_style_part(t: T) -> bool {
    matches!(t, T::NUM | T::NODE_STRING | T::COLON | T::SPACE | T::BRKT | T::STYLE)
}

const TEXT_TOKENS: &str = "'UNICODE_TEXT', 'TEXT', 'TAGSTART'";
const STATEMENT_START: &str = "'COMMENT', 'SEMI', 'NEWLINE', 'SPACE', 'EOF', 'flow', 'global', 'connector', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'STYLE', 'LINKSTYLE', 'CLASSDEF', 'CLASS', 'CLICK', 'NODE_STRING'";
const SEPARATORS: &str = "'COMMENT', 'SEMI', 'NEWLINE', 'EOF'";
fn is_separator(t: T) -> bool {
    matches!(t, T::COMMENT | T::SEMI | T::NEWLINE | T::EOF)
}

const DIRECTIONS: [&str; 5] = ["TB", "BT", "RL", "LR", "TD"];

struct Frame {
    id: Option<FlowText>,
    title: Option<FlowText>,
    global: bool,
    at: usize,
    data: Option<String>,
    data_from: usize,
    data_to: usize,
    list: Vec<DocItem>,
    collect: bool,
}

struct Parser<'a> {
    src: Vec<u16>,
    types: Vec<T>,
    texts: Vec<String>,
    starts: Vec<usize>,
    locs: Vec<Loc>,
    original: Vec<u16>,
    offsets: Vec<usize>,
    data_from: usize,
    data_to: usize,
    i: usize,
    pending_data: Option<String>,
    db: &'a mut FlowDb,
}

fn trim_brace_trail(source: &str) -> (Vec<u16>, Vec<usize>) {
    let source: Vec<u16> = source.encode_utf16().collect();
    let mut out = vec![];
    let mut offsets = vec![];
    let mut i = 0;
    while i < source.len() {
        out.push(source[i]);
        offsets.push(i);
        if source[i] == 125 {
            let mut end = i + 1;
            while end < source.len() && source[end] != 10 && crate::util::is_space16(source[end] as u32) {
                end += 1;
            }
            if source.get(end) == Some(&10) {
                out.push(10);
                offsets.push(i + 1);
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    offsets.push(source.len());
    (out, offsets)
}
impl Parser<'_> {
    fn t(&self) -> T {
        self.types.get(self.i).copied().unwrap_or(T::END)
    }

    fn at(&self, k: usize) -> T {
        self.types.get(k).copied().unwrap_or(T::END)
    }

    fn fail<X>(&self, expected: &str) -> Result<X> {
        let t = self.t();
        let mut at = self.starts.get(self.i).copied().unwrap_or(self.src.len());
        if matches!(t, T::EOF | T::END) {
            let mut k = self.i;
            while k > 0 && matches!(self.at(k - 1), T::COMMENT | T::NEWLINE | T::SPACE | T::EOF) {
                k -= 1;
            }
            at = self.starts.get(k).copied().unwrap_or(self.src.len());
            while at > 0 && crate::util::is_space16(self.src[at - 1] as u32) {
                at -= 1;
            }
        }
        let at = self.offsets.get(at).copied().unwrap_or(self.original.len());
        if t == T::ERROR {
            return Err(syntax_error("agentflow", &self.original, at, "", true));
        }
        Err(syntax_error(
            "agentflow",
            &self.original,
            at,
            &format!("Expecting {}, got '{}'", expected, TOKEN_NAMES[t as usize]),
            false,
        ))
    }

    fn take(&mut self) -> String {
        let s = self.texts[self.i].clone();
        self.i += 1;
        s
    }

    fn expect(&mut self, kind: T, also: &str) -> Result<String> {
        if self.t() != kind {
            let extra = if also.is_empty() { String::new() } else { format!(", {}", also) };
            return self.fail(&format!("'{}'{}", TOKEN_NAMES[kind as usize], extra));
        }
        Ok(self.take())
    }

    fn separator(&mut self) -> Result<()> {
        let t = self.t();
        if !is_separator(t) {
            return self.fail(SEPARATORS);
        }
        self.i += 1;
        Ok(())
    }

    fn run_of(&mut self, first: fn(T) -> bool, expected: &str) -> Result<String> {
        if !first(self.t()) {
            return self.fail(expected);
        }
        let mut s = self.take();
        while first(self.t()) {
            s += &self.take();
        }
        Ok(s)
    }

    fn id_string(&mut self) -> Result<String> {
        self.run_of(is_id, "'NODE_STRING'")
    }

    fn alpha_num(&mut self) -> Result<String> {
        self.run_of(is_alphanum, "'NODE_STRING'")
    }

    fn text(&mut self) -> Result<FlowText> {
        let t = self.t();
        let kind = match t {
            T::STR => LabelType::String,
            T::MD_STR => LabelType::Markdown,
            T::TEXT | T::TAGSTART | T::TAGEND | T::UNICODE_TEXT => LabelType::Text,
            _ => return self.fail(&format!("{}, 'TAGEND', 'STR', 'MD_STR'", TEXT_TOKENS)),
        };
        let mut s = self.take();
        while matches!(self.t(), T::TEXT | T::TAGSTART | T::TAGEND | T::UNICODE_TEXT) {
            s += &self.take();
        }
        Ok(FlowText { text: s, kind })
    }

    fn closed(&mut self, kind: T) -> Result<()> {
        self.expect(kind, TEXT_TOKENS).map(|_| ())
    }

    fn edge_text(&mut self) -> Result<FlowText> {
        let kind = match self.t() {
            T::STR => LabelType::String,
            T::MD_STR => LabelType::Markdown,
            T::EDGE_TEXT | T::UNICODE_TEXT => LabelType::Text,
            _ => return self.fail("'EDGE_TEXT', 'STR', 'MD_STR'"),
        };
        let mut s = self.take();
        while matches!(self.t(), T::EDGE_TEXT | T::UNICODE_TEXT) {
            s += &self.take();
        }
        Ok(FlowText { text: s, kind })
    }

    fn text_no_tags(&mut self) -> Result<FlowText> {
        let t = self.t();
        let kind = if t == T::STR {
            LabelType::Text
        } else if t == T::MD_STR {
            LabelType::Markdown
        } else if is_no_tags(t) {
            LabelType::Text
        } else {
            return self.fail("'NODE_STRING', 'STR', 'MD_STR'");
        };
        let mut s = self.take();
        while is_no_tags(self.t()) {
            s += &self.take();
        }
        Ok(FlowText { text: s, kind })
    }

    fn span(&self, a: usize, b: usize) -> Loc {
        let first = self.locs[a];
        let last = self.locs[b];
        Loc {
            first_line: first.first_line,
            first_column: first.first_column,
            last_line: last.last_line,
            last_column: last.last_column,
        }
    }
    fn shape_data(&mut self) -> String {
        self.data_from = self.i;
        let mut s = self.take();
        while self.t() == T::SHAPE_DATA {
            s += &self.take();
        }
        self.data_to = self.i - 1;
        s
    }

    fn add(&mut self, id: &str, label: &FlowText, kind: &str) -> Result<()> {
        self.db.add_vertex(
            id,
            Vertex {
                text: Some(label),
                kind: Some(kind),
                ..Default::default()
            },
        )
    }

    fn vertex(&mut self) -> Result<String> {
        let from = self.i;
        let id = self.id_string()?;
        let shaped = matches!(
            self.t(),
            T::SQS
                | T::DOUBLECIRCLESTART
                | T::PS
                | T::ELLIPSE_START
                | T::STADIUMSTART
                | T::SUBROUTINESTART
                | T::VERTEX_WITH_PROPS_START
                | T::CYLINDERSTART
                | T::DIAMOND_START
                | T::TAGEND
                | T::TRAPSTART
                | T::INVTRAPSTART
        );
        match self.t() {
            T::SQS => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::SQE)?;
                self.add(&id, &label, "square")?;
            }
            T::DOUBLECIRCLESTART => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::DOUBLECIRCLEEND)?;
                self.add(&id, &label, "doublecircle")?;
            }
            T::PS => {
                self.i += 1;
                if self.t() == T::PS {
                    self.i += 1;
                    let label = self.text()?;
                    self.closed(T::PE)?;
                    self.expect(T::PE, "")?;
                    self.add(&id, &label, "circle")?;
                } else {
                    let label = self.text()?;
                    self.closed(T::PE)?;
                    self.add(&id, &label, "round")?;
                }
            }
            T::ELLIPSE_START => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::ELLIPSE_END)?;
                self.add(&id, &label, "ellipse")?;
            }
            T::STADIUMSTART => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::STADIUMEND)?;
                self.add(&id, &label, "stadium")?;
            }
            T::SUBROUTINESTART => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::SUBROUTINEEND)?;
                self.add(&id, &label, "subroutine")?;
            }
            T::VERTEX_WITH_PROPS_START => {
                self.i += 1;
                let field = self.expect(T::NODE_STRING, "")?;
                self.expect(T::COLON, "")?;
                let value = self.expect(T::NODE_STRING, "")?;
                self.expect(T::PIPE, "")?;
                let label = self.text()?;
                self.closed(T::SQE)?;
                self.db.add_vertex(
                    &id,
                    Vertex {
                        text: Some(&label),
                        kind: Some("rect"),
                        props: Some([(field, value)].into_iter().collect()),
                        ..Default::default()
                    },
                )?;
            }
            T::CYLINDERSTART => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::CYLINDEREND)?;
                self.add(&id, &label, "cylinder")?;
            }
            T::DIAMOND_START => {
                self.i += 1;
                if self.t() == T::DIAMOND_START {
                    self.i += 1;
                    let label = self.text()?;
                    self.closed(T::DIAMOND_STOP)?;
                    self.expect(T::DIAMOND_STOP, "")?;
                    self.add(&id, &label, "hexagon")?;
                } else {
                    let label = self.text()?;
                    self.closed(T::DIAMOND_STOP)?;
                    self.add(&id, &label, "diamond")?;
                }
            }
            T::TAGEND => {
                self.i += 1;
                let label = self.text()?;
                self.closed(T::SQE)?;
                self.add(&id, &label, "odd")?;
            }
            T::TRAPSTART => {
                self.i += 1;
                let label = self.text()?;
                if self.t() == T::INVTRAPEND {
                    self.i += 1;
                    self.add(&id, &label, "lean_right")?;
                } else {
                    self.closed(T::TRAPEND)?;
                    self.add(&id, &label, "trapezoid")?;
                }
            }
            T::INVTRAPSTART => {
                self.i += 1;
                let label = self.text()?;
                if self.t() == T::TRAPEND {
                    self.i += 1;
                    self.add(&id, &label, "lean_left")?;
                } else {
                    self.closed(T::INVTRAPEND)?;
                    self.add(&id, &label, "inv_trapezoid")?;
                }
            }
            _ => self.db.add_vertex(&id, Vertex::default())?,
        }
        self.db.vertex_mapping(&id, shaped.then_some(""), self.span(from, self.i - 1));
        Ok(id)
    }

    fn styled_vertex(&mut self) -> Result<String> {
        let id = self.vertex()?;
        if self.t() == T::STYLE_SEPARATOR {
            self.i += 1;
            let class = self.id_string()?;
            self.db.set_class(&id, &class);
        }
        Ok(id)
    }

    fn metadata(&mut self, id: &str, data: &str, from: usize, to: usize) -> Result<()> {
        let loc = self.span(from, to);
        self.db.add_vertex(
            id,
            Vertex {
                metadata: Some(data),
                metadata_loc: Some(loc),
                ..Default::default()
            },
        )?;
        self.db.extend_mapping(id, loc);
        Ok(())
    }
    fn attach_data(&mut self, id: &str, frame: &Frame) -> Result<()> {
        if let Some(data) = &frame.data {
            let loc = self.span(frame.data_from, frame.data_to);
            self.db.add_vertex(
                id,
                Vertex {
                    metadata: Some(data),
                    metadata_loc: Some(loc),
                    ..Default::default()
                },
            )?;
        }
        Ok(())
    }
    fn node(&mut self) -> Result<Vec<String>> {
        let mut ids = vec![self.styled_vertex()?];
        self.pending_data = None;
        loop {
            let data = if self.t() == T::SHAPE_DATA { Some(self.shape_data()) } else { None };
            let from = self.data_from;
            let to = self.data_to;
            if self.t() != T::SPACE {
                self.pending_data = data;
                return Ok(ids);
            }
            while self.t() == T::SPACE {
                self.i += 1;
            }
            if self.t() != T::AMP {
                if data.is_some() {
                    return self.fail("'AMP'");
                }
                return Ok(ids);
            }
            self.i += 1;
            if self.t() != T::SPACE {
                return self.fail("'SPACE'");
            }
            while self.t() == T::SPACE {
                self.i += 1;
            }
            let next = self.styled_vertex()?;
            if let Some(data) = data {
                let last = ids.last().unwrap().clone();
                self.metadata(&last, &data, from, to)?;
            }
            ids.push(next);
        }
    }

    fn link(&mut self) -> Result<LinkInfo> {
        let mut id = None;
        if self.t() == T::LINK_ID {
            id = Some(self.take());
        }
        if self.t() == T::LINK {
            let raw = self.take();
            let mut info = self.db.destruct_link(&raw, None);
            if id.is_some() {
                info.id = id;
            }
            if self.t() == T::PIPE {
                self.i += 1;
                info.text = Some(self.text()?);
                self.closed(T::PIPE)?;
                if self.t() == T::SPACE {
                    self.i += 1;
                }
            }
            return Ok(info);
        }
        if self.t() == T::START_LINK {
            let start = self.take();
            let label = self.edge_text()?;
            let end = self.expect(T::LINK, "'EDGE_TEXT'")?;
            let mut info = self.db.destruct_link(&end, Some(&start));
            info.text = Some(label);
            if id.is_some() {
                info.id = id;
            }
            return Ok(info);
        }
        self.fail("'LINK', 'START_LINK'")
    }

    fn vertex_statement(&mut self, collect: bool) -> Result<Vec<String>> {
        let from = self.i;
        let mut stmt = self.node()?;
        if let Some(data) = self.pending_data.take() {
            let last = stmt.last().unwrap().clone();
            self.metadata(&last, &data, self.data_from, self.data_to)?;
        }
        let mut nodes = stmt.clone();
        loop {
            let t = self.t();
            if t != T::LINK && t != T::START_LINK && t != T::LINK_ID {
                return Ok(nodes);
            }
            let info = self.link()?;
            let next = self.node()?;
            if let Some(data) = self.pending_data.take() {
                let last = next.last().unwrap().clone();
                self.metadata(&last, &data, self.data_from, self.data_to)?;
            }
            self.db.add_link(&stmt, &next, Some(&info))?;
            self.db.edge_mapping(&next, self.span(from, self.i - 1));
            if collect {
                let mut joined = next.clone();
                joined.extend(nodes);
                nodes = joined;
            }
            stmt = next;
        }
    }

    fn style(&mut self) -> Result<String> {
        self.run_of(is_style_part, "'NODE_STRING', 'NUM', 'COLON', 'BRKT'")
    }

    fn styles_opt(&mut self) -> Result<Vec<String>> {
        let mut out = vec![self.style()?];
        while self.t() == T::COMMA {
            self.i += 1;
            out.push(self.style()?);
        }
        Ok(out)
    }

    fn link_style_statement(&mut self) -> Result<()> {
        self.i += 1;
        self.expect(T::SPACE, "")?;
        let mut positions: Vec<String>;
        if self.t() == T::DEFAULT {
            positions = vec![self.take()];
        } else {
            positions = vec![self.expect(T::NUM, "'DEFAULT'")?];
            while self.t() == T::COMMA {
                self.i += 1;
                positions.push(self.expect(T::NUM, "")?);
            }
        }
        self.expect(T::SPACE, "")?;
        if self.t() != T::INTERPOLATE {
            let styles = self.styles_opt()?;
            return self.db.update_link(&positions, styles);
        }
        self.i += 1;
        self.expect(T::SPACE, "")?;
        let interpolate = self.alpha_num()?;
        if self.t() == T::SPACE {
            self.i += 1;
            let styles = self.styles_opt()?;
            self.db.update_link_interpolate(&positions, &interpolate)?;
            self.db.update_link(&positions, styles)?;
        } else {
            self.db.update_link_interpolate(&positions, &interpolate)?;
        }
        Ok(())
    }

    fn link_tail(&mut self, id: &str, url: &str) -> Result<()> {
        if self.t() != T::SPACE {
            self.db.set_link(id, url, None);
            return Ok(());
        }
        self.i += 1;
        if self.t() == T::LINK_TARGET {
            let target = self.take();
            self.db.set_link(id, url, Some(&target));
            return Ok(());
        }
        let tooltip = self.expect(T::STR, "'LINK_TARGET'")?;
        if self.t() == T::SPACE {
            self.i += 1;
            let target = self.expect(T::LINK_TARGET, "")?;
            self.db.set_link(id, url, Some(&target));
        } else {
            self.db.set_link(id, url, None);
        }
        self.db.set_tooltip(id, &tooltip);
        Ok(())
    }

    fn click_statement(&mut self) -> Result<()> {
        let id = self.take();
        match self.t() {
            T::CALLBACKNAME => {
                self.i += 1;
                if self.t() == T::CALLBACKARGS {
                    self.i += 1;
                }
                let mut tooltip = None;
                if self.t() == T::SPACE {
                    self.i += 1;
                    tooltip = Some(self.expect(T::STR, "")?);
                }
                self.db.set_click_event(&id);
                if let Some(t) = tooltip {
                    self.db.set_tooltip(&id, &t);
                }
            }
            T::HREF => {
                self.i += 1;
                let url = self.expect(T::STR, "")?;
                self.link_tail(&id, &url)?;
            }
            T::STR => {
                let url = self.take();
                self.link_tail(&id, &url)?;
            }
            _ => {
                self.alpha_num()?;
                if self.t() == T::SPACE {
                    self.i += 1;
                    let tooltip = self.expect(T::STR, "")?;
                    self.db.set_click_event(&id);
                    self.db.set_tooltip(&id, &tooltip);
                } else {
                    self.db.set_click_event(&id);
                }
            }
        }
        Ok(())
    }

    fn header(&mut self, at: usize) -> Result<Frame> {
        self.expect(T::SPACE, SEPARATORS)?;
        let id = self.text_no_tags()?;
        let mut title = FlowText {
            text: String::new(),
            kind: LabelType::Text,
        };
        if self.t() == T::SQS {
            self.i += 1;
            title = self.text()?;
            self.closed(T::SQE)?;
        }
        let data = if self.t() == T::SHAPE_DATA { Some(self.shape_data()) } else { None };
        if !is_separator(self.t()) {
            return self.fail(&(SEPARATORS.to_string() + ", 'SHAPE_DATA'"));
        }
        Ok(Frame {
            global: false,
            at,
            id: Some(id),
            title: Some(title),
            data,
            data_from: self.data_from,
            data_to: self.data_to,
            list: vec![],
            collect: false,
        })
    }
    fn statement(&mut self, collect: bool) -> Result<DocItem> {
        let t = self.t();
        match t {
            T::STYLE => {
                self.i += 1;
                self.expect(T::SPACE, "")?;
                let id = self.id_string()?;
                self.expect(T::SPACE, "")?;
                let styles = self.styles_opt()?;
                self.db.add_vertex(
                    &id,
                    Vertex {
                        style: Some(styles),
                        ..Default::default()
                    },
                )?;
                self.separator()?;
                Ok(DocItem::List(Vec::new()))
            }
            T::LINKSTYLE => {
                self.link_style_statement()?;
                self.separator()?;
                Ok(DocItem::List(Vec::new()))
            }
            T::CLASSDEF => {
                self.i += 1;
                self.expect(T::SPACE, "")?;
                let id = self.id_string()?;
                self.expect(T::SPACE, "")?;
                let styles = self.styles_opt()?;
                self.db.add_class(&id, &styles);
                self.separator()?;
                Ok(DocItem::List(Vec::new()))
            }
            T::CLASS => {
                self.i += 1;
                self.expect(T::SPACE, "")?;
                let ids = self.id_string()?;
                self.expect(T::SPACE, "")?;
                let class = self.id_string()?;
                self.db.set_class(&ids, &class);
                self.separator()?;
                Ok(DocItem::List(Vec::new()))
            }
            T::CLICK => {
                self.click_statement()?;
                self.separator()?;
                Ok(DocItem::List(Vec::new()))
            }
            T::connector => {
                let at = self.i;
                self.i += 1;
                let frame = self.header(at)?;
                let sep = self.i;
                self.i += 1;
                let id = self.db.add_connector(frame.id.as_ref(), frame.title.as_ref());
                self.attach_data(&id, &frame)?;
                let text = frame.id.as_ref().map(|s| trim(&s.text)).unwrap_or("");
                self.db
                    .block_mapping(text, "connector", self.span(frame.at, frame.at), self.span(sep, sep));
                Ok(DocItem::Str(id))
            }
            T::direction_tb | T::direction_bt | T::direction_rl | T::direction_lr | T::direction_td => {
                self.i += 1;
                Ok(DocItem::Dir(DIRECTIONS[t as usize - T::direction_tb as usize].to_string()))
            }
            T::acc_title => {
                self.i += 1;
                let value = trim(&self.expect(T::acc_title_value, "")?).to_string();
                self.db.set_acc_title(&value);
                Ok(DocItem::Str(value))
            }
            T::acc_descr => {
                self.i += 1;
                let value = trim(&self.expect(T::acc_descr_value, "")?).to_string();
                self.db.set_acc_description(&value);
                Ok(DocItem::Str(value))
            }
            T::acc_descr_multiline_value => {
                let value = trim(&self.take()).to_string();
                self.db.set_acc_description(&value);
                Ok(DocItem::Str(value))
            }
            _ => {
                if !is_id(t) {
                    return self.fail(STATEMENT_START);
                }
                let nodes = self.vertex_statement(collect)?;
                self.separator()?;
                Ok(DocItem::List(nodes))
            }
        }
    }

    fn document(&mut self) -> Result<()> {
        let mut open: Vec<Frame> = vec![];
        let mut list = vec![];
        let mut collect = false;
        loop {
            let t = self.t();
            if t == T::END {
                break;
            }
            if t == T::end {
                let Some(frame) = open.pop() else {
                    break;
                };
                let at = self.i;
                self.i += 1;
                if frame.global {
                    self.db.add_global(&list);
                    list = frame.list;
                    collect = frame.collect;
                    continue;
                }
                let id = self.db.add_subgraph(frame.id.as_ref(), &list, frame.title.as_ref(), false);
                self.attach_data(&id, &frame)?;
                self.db.block_mapping(
                    frame.id.as_ref().map(|s| s.text.as_str()).unwrap_or(""),
                    "subgraph",
                    self.span(frame.at, frame.at),
                    self.span(at, at),
                );
                list = frame.list;
                collect = frame.collect;
                if collect {
                    list.push(DocItem::Str(id));
                }
            } else if matches!(t, T::SEMI | T::NEWLINE | T::SPACE | T::EOF) {
                if collect {
                    list.push(DocItem::Str(self.texts[self.i].clone()));
                }
                self.i += 1;
            } else if t == T::COMMENT {
                self.i += 1;
            } else if matches!(t, T::flow | T::global) {
                let at = self.i;
                self.i += 1;
                let mut frame = if t == T::global || is_separator(self.t()) {
                    self.separator()?;
                    Frame {
                        global: t == T::global,
                        at,
                        id: None,
                        title: None,
                        data: None,
                        data_from: 0,
                        data_to: 0,
                        list: vec![],
                        collect: false,
                    }
                } else {
                    let frame = self.header(at)?;
                    self.i += 1;
                    frame
                };
                frame.list = std::mem::take(&mut list);
                frame.collect = collect;
                open.push(frame);
                collect = true;
            } else {
                let item = self.statement(collect)?;
                if collect && !matches!(&item,DocItem::List(ids)if ids.is_empty()) {
                    list.push(item);
                }
            }
        }
        if !open.is_empty() {
            return self.fail("'end'");
        }
        Ok(())
    }
    fn parse(&mut self) -> Result<()> {
        while matches!(self.t(), T::SPACE | T::NEWLINE | T::COMMENT) {
            self.i += 1;
        }
        self.expect(T::GRAPH, "'SPACE', 'NEWLINE', 'COMMENT'")?;
        if self.t() == T::NODIR {
            self.i += 1;
            self.db.set_direction("TB");
        } else {
            let dir = self.expect(T::DIR, "'NODIR'")?;
            let t = self.t();
            if t == T::SPACE {
                while self.t() == T::SPACE {
                    self.i += 1;
                }
                if self.t() != T::NEWLINE {
                    return self.fail("'NEWLINE', 'SPACE'");
                }
            } else if t != T::SEMI && t != T::NEWLINE && t != T::COMMENT {
                return self.fail("'COMMENT', 'SEMI', 'NEWLINE', 'SPACE'");
            }
            self.i += 1;
            self.db.set_direction(&dir);
        }
        self.document()?;
        if self.t() != T::END {
            return self.fail(STATEMENT_START);
        }
        let _ = self.at(0);
        Ok(())
    }
}

pub fn parse_agentflow(source: &str, db: &mut FlowDb) -> std::result::Result<(), PeleError> {
    let (src, offsets) = trim_brace_trail(source);
    let tokens = tokenize(&src);
    let mut p = Parser {
        src,
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        locs: tokens.locs,
        original: source.encode_utf16().collect(),
        offsets,
        data_from: 0,
        data_to: 0,
        i: 0,
        pending_data: None,
        db,
    };
    p.parse()
}
