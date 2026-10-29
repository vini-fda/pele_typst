use super::{
    actors::{ActorSize, actor_kind, actor_size, draw_actor, is_figure},
    db::{Actor, Message, Model},
    kinds as lt,
};
use crate::diagrams::common::{Rendered, tighten, title_room};
use crate::diagrams::treeview::literal::break_label;
use crate::svg::builder::{esc, esc_text, label_svg};
use crate::svg::edges::{ARROW, marker};
use crate::svg::ink::ink;
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{FONT_MONO, ROUNDED, class_names, resolve_style, rounded};
use crate::svg::title::title_label;
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, MONO};
use crate::util::{js_num, js_round, num};
use crate::{Config, MonospacePart, RenderOptions, TextMeasurer};
use indexmap::IndexMap;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
const SELF_W: f64 = 28.0;
const SELF_H: f64 = 22.0;
const ACT_W: f64 = 10.0;
const ACT_STEP: f64 = 4.0;
const NOTE_GAP: f64 = 10.0;
const TAB_H: f64 = 20.0;
const BADGE_H: f64 = 16.0;
const CENTRAL_R: f64 = 4.0;
const DASH: &str = " stroke-dasharray=\"4 4\"";
#[derive(Clone, Copy, PartialEq, Eq)]
enum Head {
    None,
    Arrow,
    Cross,
    Open,
    HalfSolid,
    HalfStick,
}
#[derive(Clone, Copy)]
struct LineStyle {
    dotted: bool,
    head: Head,
    at_start: bool,
    at_end: bool,
    bottom: bool,
}
fn style(t: i32) -> Option<LineStyle> {
    let (head, dotted, ends, bottom) = match t {
        lt::SOLID => (Head::Arrow, false, 1, false),
        lt::DOTTED => (Head::Arrow, true, 1, false),
        lt::SOLID_CROSS => (Head::Cross, false, 1, false),
        lt::DOTTED_CROSS => (Head::Cross, true, 1, false),
        lt::SOLID_OPEN => (Head::None, false, 1, false),
        lt::DOTTED_OPEN => (Head::None, true, 1, false),
        lt::SOLID_POINT => (Head::Open, false, 1, false),
        lt::DOTTED_POINT => (Head::Open, true, 1, false),
        lt::BIDIRECTIONAL_SOLID => (Head::Arrow, false, 2, false),
        lt::BIDIRECTIONAL_DOTTED => (Head::Arrow, true, 2, false),
        41..=48 | 51..=58 => {
            let n = if t >= 51 { t - 10 } else { t };
            (
                if matches!(n, 43 | 44 | 47 | 48) {
                    Head::HalfStick
                } else {
                    Head::HalfSolid
                },
                t >= 51,
                if n >= 45 { 0 } else { 1 },
                n % 2 == 0,
            )
        }
        _ => return None,
    };
    Some(LineStyle {
        dotted,
        head,
        at_start: ends != 1,
        at_end: ends != 0,
        bottom,
    })
}
fn frame_kind(t: i32) -> Option<&'static str> {
    Some(match t {
        lt::LOOP_START => "loop",
        lt::ALT_START => "alt",
        lt::OPT_START => "opt",
        lt::PAR_START | lt::PAR_OVER_START => "par",
        lt::CRITICAL_START => "critical",
        lt::BREAK_START => "break",
        lt::RECT_START => "rect",
        _ => return None,
    })
}
fn frame_end(t: i32) -> bool {
    matches!(
        t,
        lt::LOOP_END | lt::ALT_END | lt::OPT_END | lt::PAR_END | lt::CRITICAL_END | lt::BREAK_END | lt::RECT_END
    )
}
fn divider(t: i32) -> bool {
    matches!(t, lt::ALT_ELSE | lt::PAR_AND | lt::CRITICAL_OPTION)
}
fn is_line(m: &Message) -> bool {
    m.kind.and_then(style).is_some()
}
fn numbering(messages: &[Message], always: bool) -> (Vec<f64>, Vec<bool>) {
    let mut index = vec![f64::NAN; messages.len()];
    let mut shown = vec![false; messages.len()];
    let (mut current, mut step, mut visible) = (1.0, 1.0, false);
    for (i, m) in messages.iter().enumerate() {
        if m.kind == Some(lt::AUTONUMBER) {
            let spec = m.numbering.as_ref().unwrap();
            if let Some(n) = spec.start.filter(|n| *n != 0.0 && !n.is_nan()) {
                current = n;
            }
            if let Some(n) = spec.step.filter(|n| *n != 0.0 && !n.is_nan()) {
                step = n;
            }
            visible = spec.visible;
        } else if is_line(m) {
            index[i] = current;
            shown[i] = visible || always;
            current = js_round((current + step) * 100.0) / 100.0;
        }
    }
    (index, shown)
}
fn option(conf: &Config, key: &str, fallback: f64) -> f64 {
    conf.get(key)
        .and_then(Value::as_f64)
        .filter(|v| *v >= 0.0 && *v <= 2000.0)
        .unwrap_or(fallback)
}
fn head(kind: Head, x: f64, y: f64, dir: f64, up: f64, color: &str) -> String {
    match kind {
        Head::Arrow => marker("arrow_point", x, y, dir, 0.0, color, "none"),
        Head::Cross => marker("arrow_cross", x, y, dir, 0.0, color, "none"),
        Head::Open => format!(
            "<path class=\"pele-marker\" d=\"M{},{}L{},{}L{},{}\" stroke-linejoin=\"round\"/>",
            num(x - dir * ARROW),
            num(y - 4.5),
            num(x),
            num(y),
            num(x - dir * ARROW),
            num(y + 4.5)
        ),
        Head::HalfStick => format!(
            "<path class=\"pele-marker\" d=\"M{},{}L{},{}\"/>",
            num(x),
            num(y),
            num(x - dir * ARROW),
            num(y + up * 5.0)
        ),
        Head::HalfSolid => format!(
            "<path class=\"pele-marker\" d=\"M{},{}L{},{}V{}Z\" fill=\"{}\" stroke=\"none\"/>",
            num(x),
            num(y),
            num(x - dir * ARROW),
            num(y + up * 4.5),
            num(y),
            color
        ),
        Head::None => String::new(),
    }
}
fn head_trim(kind: Head) -> f64 {
    match kind {
        Head::Arrow => ARROW - 1.0,
        Head::Cross => 4.0,
        _ => 0.0,
    }
}
#[derive(Clone)]
struct Column {
    id: String,
    actor: Actor,
    kind: String,
    label: Label,
    size: ActorSize,
    x: f64,
    create_at: isize,
    destroy_at: isize,
    top: f64,
    life_start: f64,
    life_end: f64,
    activations: Vec<f64>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewKind {
    Line,
    SelfLoop,
    Note,
}
#[derive(Clone)]
struct View {
    kind: ViewKind,
    a: usize,
    b: usize,
    label: Label,
    x: f64,
    w: f64,
}
#[derive(Clone)]
struct Frame {
    kind: &'static str,
    x0: f64,
    x1: f64,
    top: f64,
    title: Label,
    tab_w: f64,
    fill: String,
    dividers: Vec<usize>,
    divider_y: Vec<f64>,
    divider_labels: Vec<Label>,
}
struct BoxRun {
    box_id: usize,
    first: usize,
    last: usize,
    title: Label,
    grow: f64,
}
struct Mono<'a>(&'a dyn TextMeasurer);
impl TextMeasurer for Mono<'_> {
    fn width(&self, t: &str, s: f64, style: u32) -> f64 {
        self.0.width(t, s, style | MONO)
    }
    fn icon_markup(&self, n: &str) -> String {
        self.0.icon_markup(n)
    }
    fn icon_resolver(&self) -> Option<crate::IconResolver> {
        self.0.icon_resolver()
    }
    fn math_active(&self) -> bool {
        self.0.math_active()
    }
    fn typeset(&self, t: &str, s: f64) -> Option<crate::MathMarkup> {
        self.0.typeset(t, s)
    }
}
struct Drawing<'a> {
    db: &'a Model,
    measurer: &'a dyn TextMeasurer,
    mono: Mono<'a>,
    monospaced: bool,
    tight: bool,
    small: f64,
    tiny: f64,
    wrap_w: f64,
    note_pad: f64,
    box_pad: f64,
    msg_pad: f64,
    note_over: f64,
    frame_pad: f64,
    frame_nest: f64,
    cols: Vec<Column>,
    col_of: HashMap<String, usize>,
    views: Vec<Option<View>>,
    frames: Vec<Option<usize>>,
    arena: Vec<Frame>,
    runs: Vec<BoxRun>,
    gaps: Vec<f64>,
    index: Vec<f64>,
    shown: Vec<bool>,
    min_x: f64,
    max_x: f64,
}
impl Drawing<'_> {
    fn text(&self, raw: Option<&str>, size: f64, wrap: bool, width: f64, message: bool) -> Label {
        let by: &dyn TextMeasurer = if message && self.monospaced { &self.mono } else { self.measurer };
        let label = layout_label(raw, false, by, size, if self.tight || wrap { width } else { f64::INFINITY }, 0);
        if self.tight {
            break_label(label, by, 120.0f64.max(width))
        } else {
            label
        }
    }
    fn message_text(&self, raw: &str, wrap: bool, width: f64) -> Label {
        self.text(Some(raw), self.small, wrap, width, true)
    }
    fn badge_width(&self, i: usize) -> f64 {
        if self.shown[i] {
            BADGE_H.max(self.measurer.width(&js_num(self.index[i]), self.tiny, 0) + 8.0)
        } else {
            0.0
        }
    }
    fn extend(&mut self, l: f64, r: f64) {
        if l < self.min_x {
            self.min_x = l;
        }
        if r > self.max_x {
            self.max_x = r;
        }
    }
    fn box_edges(&self, run: &BoxRun) -> (f64, f64) {
        (
            self.cols[run.first].x - self.cols[run.first].size.w / 2.0 - self.box_pad - run.grow,
            self.cols[run.last].x + self.cols[run.last].size.w / 2.0 + self.box_pad + run.grow,
        )
    }
    fn place(&mut self) {
        for i in 1..self.cols.len() {
            self.cols[i].x = self.cols[i - 1].x + self.gaps[i - 1];
        }
    }
    fn layout(&mut self) {
        self.place();
        self.min_x = f64::INFINITY;
        self.max_x = f64::NEG_INFINITY;
        for i in 0..self.cols.len() {
            let c = &self.cols[i];
            let (l, r) = (c.x - c.size.w / 2.0, c.x + c.size.w / 2.0);
            self.extend(l, r);
        }
        for i in 0..self.runs.len() {
            let (l, r) = self.box_edges(&self.runs[i]);
            self.extend(l, r);
        }
        self.arena.clear();
        self.frames.fill(None);
        let mut open = Vec::<usize>::new();
        for i in 0..self.db.messages.len() {
            let m = &self.db.messages[i];
            let kind = m.kind.unwrap_or(-1);
            if let Some(frame_kind) = frame_kind(kind) {
                let id = self.arena.len();
                self.arena.push(Frame {
                    kind: frame_kind,
                    x0: f64::INFINITY,
                    x1: f64::NEG_INFINITY,
                    top: 0.0,
                    title: Label::default(),
                    tab_w: 0.0,
                    fill: if frame_kind == "rect" { m.message.clone() } else { String::new() },
                    dividers: vec![i],
                    divider_y: Vec::new(),
                    divider_labels: Vec::new(),
                });
                self.frames[i] = Some(id);
                open.push(id);
            } else if divider(kind) {
                if let Some(&id) = open.last() {
                    self.arena[id].dividers.push(i);
                }
            } else if frame_end(kind) {
                let Some(id) = open.pop() else {
                    continue;
                };
                self.frames[i] = Some(id);
                let mut frame = self.arena[id].clone();
                if frame.x0 > frame.x1 {
                    (frame.x0, frame.x1) = if self.cols.is_empty() {
                        (0.0, 120.0)
                    } else {
                        (self.cols[0].x - self.frame_pad, self.cols.last().unwrap().x + self.frame_pad)
                    };
                }
                if frame.kind != "rect" {
                    frame.tab_w = self.measurer.width(frame.kind, self.tiny, BOLD) + 14.0;
                    for (k, &mi) in frame.dividers.iter().enumerate() {
                        let raw = &self.db.messages[mi].message;
                        let tab = if k == 0 { frame.tab_w + 8.0 } else { 0.0 };
                        let room = frame.x1 - frame.x0 - 16.0 - tab;
                        let label = if !raw.is_empty() {
                            self.text(
                                Some(&format!("[{raw}]")),
                                self.small,
                                true,
                                room.max(if self.tight { 240.0f64.min(self.wrap_w) } else { 240.0 }),
                                false,
                            )
                        } else {
                            Label::default()
                        };
                        if label.width > room {
                            frame.x1 += label.width - room;
                        }
                        if k == 0 {
                            frame.title = label;
                        } else {
                            frame.divider_labels.push(label);
                        }
                    }
                }
                self.extend(frame.x0, frame.x1);
                if let Some(&parent) = open.last() {
                    self.arena[parent].x0 = self.arena[parent].x0.min(frame.x0 - self.frame_nest);
                    self.arena[parent].x1 = self.arena[parent].x1.max(frame.x1 + self.frame_nest);
                }
                self.arena[id] = frame;
            } else if let Some(mut view) = self.views[i].clone() {
                let a = &self.cols[view.a];
                let b = &self.cols[view.b];
                let (x0, x1) = if view.kind == ViewKind::Note {
                    if m.placement == Some(0) {
                        view.x = a.x - NOTE_GAP - view.w;
                    } else if m.placement == Some(1) {
                        view.x = a.x + NOTE_GAP;
                    } else if view.a == view.b {
                        view.x = a.x - view.w / 2.0;
                    } else {
                        let lo = a.x.min(b.x) - self.note_over;
                        let hi = a.x.max(b.x) + self.note_over;
                        if self.tight || m.wrap {
                            view.label = self.text(
                                Some(&m.message),
                                self.small,
                                true,
                                self.wrap_w.max(hi - lo - 2.0 * self.note_pad),
                                false,
                            );
                        }
                        view.w = (hi - lo).max(view.label.width + 2.0 * self.note_pad);
                        view.x = (lo + hi - view.w) / 2.0;
                    }
                    (view.x, view.x + view.w)
                } else if view.kind == ViewKind::SelfLoop {
                    (a.x, a.x + ACT_W + SELF_W + 8.0 + view.label.width + self.badge_width(i))
                } else {
                    let (mut x0, mut x1) = (a.x.min(b.x), a.x.max(b.x));
                    if self.tight || m.wrap {
                        view.label = self.message_text(&m.message, true, self.wrap_w.max(x1 - x0 - 2.0 * self.msg_pad - ACT_W));
                    }
                    if b.create_at == i as isize {
                        if b.x > a.x {
                            x1 += b.size.w / 2.0;
                        } else {
                            x0 -= b.size.w / 2.0;
                        }
                    }
                    (x0, x1)
                };
                self.extend(x0, x1);
                if let Some(&id) = open.last() {
                    self.arena[id].x0 = self.arena[id].x0.min(x0 - self.frame_pad);
                    self.arena[id].x1 = self.arena[id].x1.max(x1 + self.frame_pad);
                }
                self.views[i] = Some(view);
            }
        }
    }
    fn spread(&mut self, before: &[f64], delta: &[f64], wrapped: &[usize]) -> Vec<usize> {
        for g in 0..self.gaps.len() {
            self.gaps[g] = before[g] + delta[g];
        }
        self.layout();
        wrapped.iter().map(|&i| self.views[i].as_ref().unwrap().label.lines.len()).collect()
    }
    fn fill_under<'a>(&'a self, stack: &[usize], l: f64, r: f64) -> Option<&'a str> {
        for &i in stack.iter().rev() {
            if self.arena[i].kind == "rect" {
                return Some(&self.arena[i].fill);
            }
        }
        for run in &self.runs {
            let (x0, x1) = self.box_edges(run);
            if l >= x0 && r <= x1 {
                return Some(&self.db.boxes[run.box_id].fill);
            }
        }
        None
    }
    fn under(&self, stack: &[usize], l: f64, r: f64) -> String {
        ink_on(self.fill_under(stack, l, r))
    }
    fn backed(&self, label: &Label, cx: f64, cy: f64, frame: bool, stack: &[usize]) -> String {
        if label.width == 0.0 {
            return String::new();
        }
        let l = cx - label.width / 2.0;
        let plate = self.cols.iter().any(|c| c.x > l && c.x < l + label.width);
        let back = if plate {
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\" stroke=\"none\"/>",
                num(l - 3.0),
                num(cy - label.height / 2.0 + 2.0),
                num(label.width + 6.0),
                num(label.height - 4.0)
            )
        } else {
            String::new()
        };
        let fill = if plate {
            String::new()
        } else {
            self.under(stack, l, l + label.width)
        };
        let attrs = if frame {
            format!(
                " class=\"pele-frame-label\"{} stroke=\"none\"",
                if fill.is_empty() { " fill=\"var(--_m)\"" } else { &fill }
            )
        } else {
            fill
        };
        back + &label_svg(label, cx, cy, &attrs)
    }
    fn actor_svg(&self, col: &Column, top: f64, footer: bool) -> String {
        let cls = col
            .actor
            .properties
            .get("class")
            .and_then(Value::as_str)
            .map(class_names)
            .unwrap_or_default();
        let color = if is_figure(&col.kind) {
            ink_on(col.actor.box_id.map(|i| self.db.boxes[i].fill.as_str()))
        } else {
            String::new()
        };
        format!(
            "<g class=\"pele-node pele-actor pele-actor-{}{}{}\" data-id=\"{}\">{}</g>",
            col.kind,
            if footer { " pele-actor-footer" } else { "" },
            cls,
            esc_text(&col.id),
            draw_actor(&col.kind, col.x, top, col.size, &col.label, &color)
        )
    }
    fn badge(&self, i: usize, cx: f64, cy: f64, w: f64) -> String {
        format!(
            "<g class=\"pele-sequence-number\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"16\" rx=\"8\" fill=\"var(--_l)\" stroke=\"none\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" fill=\"var(--_bg)\" stroke=\"none\">{}</text></g>",
            num(cx - w / 2.0),
            num(cy - 8.0),
            num(w),
            num(cx),
            num(cy + self.tiny * 0.35),
            js_num(self.tiny),
            esc(&js_num(self.index[i]))
        )
    }
    fn end_activation(&mut self, i: usize, end: f64, out: &mut Vec<String>) -> f64 {
        let col = &mut self.cols[i];
        let start = col.activations.pop().unwrap();
        let bottom = end.max(start + 14.0);
        let x = col.x - ACT_W / 2.0 + col.activations.len() as f64 * ACT_STEP;
        out.push(format!(
            "<rect class=\"pele-activation\" data-id=\"{}\" x=\"{}\" y=\"{}\" width=\"10\" height=\"{}\" rx=\"2\"/>",
            esc_text(&col.id),
            num(x),
            num(start),
            num(bottom - start)
        ));
        self.extend(x, x + ACT_W);
        bottom
    }
    fn destroy(&mut self, i: usize, line_y: f64, out: &mut String) -> f64 {
        let col = &mut self.cols[i];
        let (cx, cy) = (col.x, line_y + 16.0);
        col.life_end = cy;
        *out += &format!(
            "<path class=\"pele-destroy\" data-id=\"{}\" d=\"M{},{}L{},{}M{},{}L{},{}\"/>",
            esc_text(&col.id),
            num(cx - 6.0),
            num(cy - 6.0),
            num(cx + 6.0),
            num(cy + 6.0),
            num(cx + 6.0),
            num(cy - 6.0),
            num(cx - 6.0),
            num(cy + 6.0)
        );
        cy + 6.0
    }
}
fn ink_of(fill: Option<&str>) -> Option<&'static str> {
    fill.filter(|s| !s.is_empty())
        .and_then(|s| ink(&resolve_style(&[format!("fill:{s}")]).shape, ""))
}
fn ink_on(fill: Option<&str>) -> String {
    ink_of(fill).map(|c| format!(" fill=\"{c}\"")).unwrap_or_default()
}
fn left_edge(col: &Column, extra: usize) -> f64 {
    if col.activations.len() + extra > 0 {
        col.x - ACT_W / 2.0
    } else {
        col.x
    }
}
fn right_edge(col: &Column, extra: usize) -> f64 {
    let depth = col.activations.len() + extra;
    if depth > 0 {
        col.x + ACT_W / 2.0 + (depth - 1) as f64 * ACT_STEP
    } else {
        col.x
    }
}
fn need(needs: &mut IndexMap<usize, f64>, n: usize, lo: isize, hi: isize, width: f64) {
    if lo < 0 || hi >= n as isize {
        return;
    }
    let key = lo as usize * n + hi as usize;
    if needs.get(&key).copied().unwrap_or(0.0) < width {
        needs.insert(key, width);
    }
}
pub fn render(db: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    tighten(options, |tight| draw(db, config, options, measurer, tight, 0.0, true))
}
fn draw(
    db: &Model,
    config: &Config,
    options: &RenderOptions,
    measurer: &dyn TextMeasurer,
    tight: bool,
    hold: f64,
    by_need: bool,
) -> Rendered {
    let empty = Config::new();
    let conf = config.get("sequence").and_then(Value::as_object).unwrap_or(&empty);
    let size = options.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let tiny = js_round(size * 0.75);
    let monospaced = options.monospace.as_ref().is_some_and(|p| p.contains(&MonospacePart::Messages));
    let pad = options.padding.unwrap_or(8.0);
    let mirror = conf.get("mirrorActors") != Some(&Value::Bool(false));
    let right_angles = conf.get("rightAngles") == Some(&Value::Bool(true));
    let actor_margin = option(conf, "actorMargin", 40.0).min(if tight { 8.0 } else { f64::INFINITY });
    let min_w = option(conf, "width", 64.0).min(if tight { 48.0 } else { f64::INFINITY });
    let min_h = option(conf, "height", js_round(size * 1.5) + 20.0);
    let wrap_w = option(conf, "width", 160.0).min(if tight {
        32.0f64.max((options.max_width.unwrap_or(320.0) - 2.0 * pad) / db.actors.len().max(1) as f64 - actor_margin - 24.0)
    } else {
        f64::INFINITY
    });
    let msg_gap = 4.0f64.max(14.0 + option(conf, "messageMargin", 35.0) - 35.0);
    let note_pad = option(conf, "noteMargin", 10.0).min(if tight { 6.0 } else { f64::INFINITY });
    let box_pad = option(conf, "boxMargin", 10.0).min(if tight { 6.0 } else { f64::INFINITY });
    let align = match conf.get("messageAlign").and_then(Value::as_str) {
        Some("left") => -1,
        Some("right") => 1,
        _ => 0,
    };
    let count = db.messages.len();
    let (index, shown) = numbering(&db.messages, conf.get("showSequenceNumbers") == Some(&Value::Bool(true)));
    let mut d = Drawing {
        db,
        measurer,
        mono: Mono(measurer),
        monospaced,
        tight,
        small,
        tiny,
        wrap_w,
        note_pad,
        box_pad,
        msg_pad: if tight { 6.0 } else { 12.0 },
        note_over: if tight { 8.0 } else { 16.0 },
        frame_pad: if tight { 12.0 } else { 20.0 },
        frame_nest: if tight { 6.0 } else { 10.0 },
        cols: Vec::new(),
        col_of: HashMap::new(),
        views: vec![None; count],
        frames: vec![None; count],
        arena: Vec::new(),
        runs: Vec::new(),
        gaps: Vec::new(),
        index,
        shown,
        min_x: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
    };
    let used = if conf.get("hideUnusedParticipants") == Some(&Value::Bool(true)) {
        Some(
            db.messages
                .iter()
                .flat_map(|m| [m.from.as_deref(), m.to.as_deref()])
                .flatten()
                .collect::<HashSet<_>>(),
        )
    } else {
        None
    };
    let mut next_line = vec![-1isize; count + 1];
    for i in (0..count).rev() {
        next_line[i] = if is_line(&db.messages[i]) { i as isize } else { next_line[i + 1] };
    }
    for (id, actor) in &db.actors {
        if used.as_ref().is_some_and(|u| !u.contains(id.as_str())) {
            continue;
        }
        let kind = actor_kind(&actor.kind).to_string();
        let label = d.text(Some(&actor.description), size, actor.wrap, wrap_w, false);
        let mut create_at = db.created_actors.get(id).map(|&i| next_line[i.min(count)]).unwrap_or(-1);
        let mut destroy_at = db.destroyed_actors.get(id).map(|&i| next_line[i.min(count)]).unwrap_or(-1);
        if create_at >= 0 && db.messages[create_at as usize].to.as_ref() != Some(id) {
            create_at = -1;
        }
        if destroy_at >= 0
            && db.messages[destroy_at as usize].to.as_ref() != Some(id)
            && db.messages[destroy_at as usize].from.as_ref() != Some(id)
        {
            destroy_at = -1;
        }
        let size = actor_size(&kind, &label, min_w, min_h, if tight { 8.0 } else { 16.0 });
        d.col_of.insert(id.clone(), d.cols.len());
        d.cols.push(Column {
            id: id.clone(),
            actor: actor.clone(),
            kind,
            label,
            size,
            x: 0.0,
            create_at,
            destroy_at,
            top: 0.0,
            life_start: 0.0,
            life_end: -1.0,
            activations: Vec::new(),
        });
    }
    let n = d.cols.len();
    let mut needs = IndexMap::new();
    for (i, m) in db.messages.iter().enumerate() {
        let (Some(&a), Some(&b)) = (
            m.from.as_ref().and_then(|id| d.col_of.get(id)),
            m.to.as_ref().and_then(|id| d.col_of.get(id)),
        ) else {
            continue;
        };
        if m.numbering.is_some() {
            continue;
        }
        if m.kind == Some(lt::NOTE) {
            let label = d.text(Some(&m.message), small, m.wrap, wrap_w, false);
            let w = label.width + 2.0 * note_pad;
            match m.placement {
                Some(0) => need(&mut needs, n, a as isize - 1, a as isize, w + 2.0 * NOTE_GAP + ACT_W),
                Some(1) => need(&mut needs, n, a as isize, a as isize + 1, w + 2.0 * NOTE_GAP + ACT_W),
                _ if a == b => {
                    need(&mut needs, n, a as isize - 1, a as isize, w / 2.0 + NOTE_GAP);
                    need(&mut needs, n, a as isize, a as isize + 1, w / 2.0 + NOTE_GAP);
                }
                _ => need(&mut needs, n, a.min(b) as isize, a.max(b) as isize, w - 2.0 * d.note_over),
            }
            d.views[i] = Some(View {
                kind: ViewKind::Note,
                a,
                b,
                label,
                x: 0.0,
                w,
            });
        } else if is_line(m) {
            let label = d.message_text(&m.message, m.wrap, wrap_w);
            let badge = d.badge_width(i);
            let kind = if a == b {
                need(
                    &mut needs,
                    n,
                    a as isize,
                    a as isize + 1,
                    SELF_W + 8.0 + label.width + 16.0 + ACT_W + badge,
                );
                ViewKind::SelfLoop
            } else {
                let created = if d.cols[b].create_at == i as isize {
                    d.cols[b].size.w / 2.0
                } else {
                    0.0
                };
                let central = if m.central_connection != 0 { 4.0 * CENTRAL_R } else { 0.0 };
                need(
                    &mut needs,
                    n,
                    a.min(b) as isize,
                    a.max(b) as isize,
                    (if tight {
                        label.width.min(16.0f64.max(wrap_w - 16.0))
                    } else {
                        label.width
                    }) + 2.0 * d.msg_pad
                        + ACT_W
                        + badge
                        + created
                        + central,
                );
                ViewKind::Line
            };
            d.views[i] = Some(View {
                kind,
                a,
                b,
                label,
                x: 0.0,
                w: 0.0,
            });
        }
    }
    for i in 0..n {
        if let Some(box_id) = d.cols[i].actor.box_id {
            if let Some(run) = d.runs.last_mut().filter(|r| r.box_id == box_id && r.last + 1 == i) {
                run.last = i;
            } else {
                let b = &db.boxes[box_id];
                let title = d.text(b.name.as_deref(), small, b.wrap, wrap_w, false);
                d.runs.push(BoxRun {
                    box_id,
                    first: i,
                    last: i,
                    title,
                    grow: 0.0,
                });
            }
        }
    }
    d.gaps = vec![0.0; n.saturating_sub(1)];
    for i in 0..d.gaps.len() {
        let left = &d.cols[i];
        let right = &d.cols[i + 1];
        let boxed = if left.actor.box_id != right.actor.box_id {
            (if left.actor.box_id.is_some() { box_pad } else { 0.0 }) + (if right.actor.box_id.is_some() { box_pad } else { 0.0 })
        } else {
            0.0
        };
        d.gaps[i] =
            (left.size.w / 2.0 + right.size.w / 2.0 + actor_margin + boxed).max(needs.get(&(i * n + i + 1)).copied().unwrap_or(0.0));
    }
    let mut spans: Vec<_> = needs
        .iter()
        .filter_map(|(&key, &width)| {
            let (lo, hi) = (key / n, key % n);
            (hi - lo > 1).then_some((lo, hi, width))
        })
        .collect();
    spans.sort_by_key(|(lo, hi, _)| (hi - lo, *lo));
    for (lo, hi, width) in spans {
        let total: f64 = d.gaps[lo..hi].iter().sum();
        if total < width {
            let share = (width - total) / (hi - lo) as f64;
            for g in &mut d.gaps[lo..hi] {
                *g += share;
            }
        }
    }
    d.place();
    for i in 0..d.runs.len() {
        let run = &mut d.runs[i];
        let inner = d.cols[run.last].x + d.cols[run.last].size.w / 2.0 - (d.cols[run.first].x - d.cols[run.first].size.w / 2.0);
        run.grow = 0.0f64.max((run.title.width - inner) / 2.0);
        if run.grow == 0.0 {
            continue;
        }
        if run.first > 0 {
            d.gaps[run.first - 1] += run.grow;
        }
        if run.last + 1 < n {
            d.gaps[run.last] += run.grow;
        }
    }
    d.layout();
    let mut grown = false;
    let mut uneven = false;
    if tight && let Some(max_width) = options.max_width {
        let spare = max_width.floor() - (d.max_x - d.min_x + 2.0 * pad).ceil() - hold;
        let mut stretch = vec![0i32; n];
        let mut want = vec![0.0f64; d.gaps.len()];
        let mut wrapped = Vec::new();
        let mut lines = 0;
        for i in 0..d.views.len() {
            if spare <= 0.0 {
                break;
            }
            let Some(view) = d.views[i].as_ref().filter(|v| v.kind == ViewKind::Line) else {
                continue;
            };
            let natural = d.message_text(&db.messages[i].message, false, f64::INFINITY);
            if view.label.lines.len() <= natural.lines.len() {
                continue;
            }
            wrapped.push(i);
            lines += view.label.lines.len();
            let (lo, hi) = (view.a.min(view.b), view.a.max(view.b));
            stretch[lo] += 1;
            stretch[hi] -= 1;
            let short = (natural.width - (d.cols[hi].x - d.cols[lo].x - 2.0 * d.msg_pad - ACT_W)) / (hi - lo) as f64;
            for g in lo..hi {
                want[g] = want[g].max(short);
            }
        }
        let (mut active, mut crossed) = (0, 0);
        for value in stretch.iter_mut().take(d.gaps.len()) {
            active += *value;
            *value = if active > 0 { 1 } else { 0 };
            crossed += *value;
        }
        if crossed > 0 {
            let before = d.gaps.clone();
            let even: Vec<_> = (0..d.gaps.len()).map(|g| stretch[g] as f64 * spare / crossed as f64).collect();
            let mut counts = d.spread(&before, &even, &wrapped);
            let missing: f64 = want.iter().sum();
            let needs: Vec<_> = want.iter().map(|w| w * (spare - 0.01) / missing).collect();
            if by_need && crossed > 1 && missing > 0.0 && needs.iter().zip(&even).any(|(n, e)| n > e) {
                let first = counts;
                counts = d.spread(&before, &needs, &wrapped);
                uneven = counts.iter().sum::<usize>() < first.iter().sum::<usize>() && counts.iter().zip(&first).all(|(n, f)| n <= f);
                if !uneven {
                    counts = d.spread(&before, &even, &wrapped);
                }
            }
            grown = counts.iter().sum::<usize>() < lines;
            if !grown {
                d.gaps = before;
                d.layout();
            }
        }
    }
    let box_title_h = d.runs.iter().map(|r| r.title.height).fold(0.0f64, f64::max);
    let box_top = if !d.runs.is_empty() {
        box_pad + if box_title_h > 0.0 { box_title_h + 4.0 } else { 0.0 }
    } else {
        0.0
    };
    let row_h = d.cols.iter().filter(|c| c.create_at < 0).map(|c| c.size.h).fold(0.0f64, f64::max);
    let head_bottom = box_top + row_h;
    for c in &mut d.cols {
        c.top = head_bottom - c.size.h;
        c.life_start = head_bottom;
    }
    let mut stack = Vec::<usize>::new();
    let mut rects_out = Vec::<String>::new();
    let mut activations_out = Vec::<String>::new();
    let (mut frames_out, mut edges_out, mut labels_out, mut notes_out, mut actors_out, mut crosses_out) = (
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    );
    let mut y = head_bottom + 6.0;
    for (i, m) in db.messages.iter().enumerate() {
        let kind = m.kind.unwrap_or(-1);
        let view = d.views[i].clone();
        let frame = d.frames[i];
        if kind == lt::ACTIVE_START {
            if let Some(&col) = m.from.as_ref().and_then(|id| d.col_of.get(id)) {
                d.cols[col].activations.push(y);
            }
        } else if kind == lt::ACTIVE_END {
            if let Some(&col) = m.from.as_ref().and_then(|id| d.col_of.get(id))
                && !d.cols[col].activations.is_empty()
            {
                y = y.max(d.end_activation(col, y, &mut activations_out));
            }
        } else if let Some(id) = frame.filter(|&id| d.arena[id].dividers[0] == i) {
            stack.push(id);
            let f = &mut d.arena[id];
            if f.kind == "rect" {
                f.top = y + 8.0;
                y = f.top;
            } else {
                f.top = y + 12.0;
                y = f.top + TAB_H.max(f.title.height + 2.0);
            }
        } else if let Some(id) = frame {
            stack.pop();
            let f = &d.arena[id];
            let bottom = y + 12.0;
            let x = num(f.x0);
            let w = num(f.x1 - f.x0);
            if f.kind == "rect" {
                let fill = if !f.fill.is_empty() {
                    resolve_style(&[format!("fill:{}", f.fill)]).shape
                } else {
                    String::new()
                };
                rects_out.push(format!(
                    "<rect class=\"pele-rect\" x=\"{x}\" y=\"{}\" width=\"{w}\" height=\"{}\"{}/>",
                    num(f.top),
                    num(bottom - f.top),
                    rounded(&(if fill.is_empty() { " fill-opacity=\"0.5\"" } else { "" }.to_string() + &fill))
                ));
            } else {
                let mut inner = format!(
                    "<rect x=\"{x}\" y=\"{}\" width=\"{w}\" height=\"{}\" fill=\"none\"{ROUNDED}/>",
                    num(f.top),
                    num(bottom - f.top)
                );
                for (k, &dy) in f.divider_y.iter().enumerate() {
                    let label = &f.divider_labels[k];
                    inner += &format!("<path d=\"M{x},{}H{}\"{DASH}/>", num(dy), num(f.x1));
                    inner += &d.backed(label, f.x0 + 8.0 + label.width / 2.0, dy + 2.0 + label.height / 2.0, true, &stack);
                }
                inner += &format!(
                    "<rect x=\"{x}\" y=\"{}\" width=\"{}\" height=\"20\" fill=\"var(--_s)\"{ROUNDED}/><text class=\"pele-frame-kind\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" font-weight=\"var(--_hw)\" fill=\"var(--_m)\" stroke=\"none\">{}</text>",
                    num(f.top),
                    num(f.tab_w),
                    num(f.x0 + f.tab_w / 2.0),
                    num(f.top + TAB_H / 2.0 + tiny * 0.35),
                    js_num(tiny),
                    f.kind
                );
                inner += &d.backed(
                    &f.title,
                    f.x0 + f.tab_w + 8.0 + f.title.width / 2.0,
                    f.top + TAB_H.max(f.title.height) / 2.0,
                    true,
                    &stack,
                );
                frames_out += &format!("<g class=\"pele-frame pele-frame-{}\">{inner}</g>", f.kind);
            }
            y = bottom;
        } else if divider(kind) {
            if let Some(&id) = stack.last() {
                let f = &mut d.arena[id];
                let label = &f.divider_labels[f.divider_y.len()];
                f.divider_y.push(y + 12.0);
                y += 12.0 + if label.height > 0.0 { label.height } else { 0.0 };
            }
        } else if let Some(view) = view.filter(|v| v.kind == ViewKind::Note) {
            let a = &d.cols[view.a];
            let h = view.label.height + 2.0 * 4.0f64.max(note_pad - 4.0);
            let top = y + 10.0;
            let mut x = view.x;
            if m.placement == Some(1) {
                x += right_edge(a, 0) - a.x;
            } else if m.placement == Some(0) {
                x += left_edge(a, 0) - a.x;
            }
            d.extend(x, x + view.w);
            notes_out += &format!(
                "<g class=\"pele-note\" data-id=\"i{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{ROUNDED}/>{}</g>",
                esc_text(&m.id),
                num(x),
                num(top),
                num(view.w),
                num(h),
                label_svg(
                    &view.label,
                    x + view.w / 2.0,
                    top + h / 2.0,
                    " class=\"pele-label\" stroke=\"none\" fill=\"var(--_fg)\""
                )
            );
            y = top + h;
        } else if let Some(view) = d.views[i].clone() {
            let st = style(kind).unwrap();
            let a = d.cols[view.a].clone();
            let b = d.cols[view.b].clone();
            let label = &view.label;
            let bw = d.badge_width(i);
            let mut starting = 0;
            for next in &db.messages[i + 1..] {
                let kind = next.kind.unwrap_or(-1);
                if kind == lt::ACTIVE_START {
                    if next.from == m.to {
                        starting += 1;
                    }
                } else if !matches!(kind, lt::ACTIVE_END | lt::CENTRAL_CONNECTION | lt::CENTRAL_CONNECTION_REVERSE) {
                    break;
                }
            }
            let up = if st.bottom { 1.0 } else { -1.0 };
            let attrs = if st.dotted { DASH } else { "" };
            let cc = m.central_connection;
            let circle_to = matches!(cc, lt::CENTRAL_CONNECTION | lt::CENTRAL_CONNECTION_DUAL);
            let circle_from = matches!(cc, lt::CENTRAL_CONNECTION_REVERSE | lt::CENTRAL_CONNECTION_DUAL);
            let tail_at_end = st.at_start && !st.at_end;
            let trim = head_trim(st.head);
            let mut inner = String::new();
            let line;
            let line_y;
            if view.kind == ViewKind::SelfLoop {
                let top = y + msg_gap + 0.0f64.max((label.height - SELF_H) / 2.0);
                let bottom = top + SELF_H;
                let mut x0 = if circle_from { a.x + CENTRAL_R } else { right_edge(&a, 0) };
                let mut x1 = if circle_to { a.x + CENTRAL_R } else { right_edge(&a, starting) };
                let out = x0.max(x1) + SELF_W + bw / 2.0;
                line = ink_of(d.fill_under(&stack, a.x, out));
                let color = line.unwrap_or("var(--_l)");
                if circle_from {
                    inner += &circle(a.x, top, color);
                }
                if circle_to {
                    inner += &circle(a.x, bottom, color);
                }
                if bw > 0.0 {
                    inner += &d.badge(i, if tail_at_end { x1 } else { x0 }, if tail_at_end { bottom } else { top }, bw);
                    if tail_at_end {
                        x1 += bw / 2.0;
                    } else {
                        x0 += bw / 2.0;
                    }
                }
                let heads = (if st.at_start {
                    head(st.head, x0, top, -1.0, up, color)
                } else {
                    String::new()
                }) + &if st.at_end {
                    head(st.head, x1, bottom, -1.0, -up, color)
                } else {
                    String::new()
                };
                let from = num(x0 + if st.at_start { trim } else { 0.0 });
                let to = num(x1 + if st.at_end { trim } else { 0.0 });
                let path = if right_angles {
                    format!("M{from},{}H{}V{}H{to}", num(top), num(out), num(bottom))
                } else {
                    format!(
                        "M{from},{}H{}A6,6 0 0 1 {},{}V{}A6,6 0 0 1 {},{}H{to}",
                        num(top),
                        num(out - 6.0),
                        num(out),
                        num(top + 6.0),
                        num(bottom - 6.0),
                        num(out - 6.0),
                        num(bottom)
                    )
                };
                inner = format!("<path d=\"{path}\"{attrs}/>{heads}{inner}");
                d.extend(a.x, out + 8.0 + label.width);
                if label.width > 0.0 {
                    labels_out += &format!(
                        "<g class=\"pele-edge-label\" data-id=\"i{}\">{}</g>",
                        esc_text(&m.id),
                        label_svg(
                            label,
                            out + 8.0 + label.width / 2.0,
                            (top + bottom) / 2.0,
                            &d.under(&stack, out + 8.0, out + 8.0 + label.width)
                        )
                    );
                }
                line_y = bottom.max((top + bottom) / 2.0 + label.height / 2.0);
            } else {
                let dir = if b.x > a.x { 1.0 } else { -1.0 };
                let creating = b.create_at == i as isize;
                line = ink_of(d.fill_under(&stack, a.x.min(b.x), a.x.max(b.x)));
                let color = line.unwrap_or("var(--_l)");
                line_y = y
                    + msg_gap
                    + (if label.height > 0.0 { label.height + 2.0 } else { 6.0 }).max(if creating {
                        b.size.h / 2.0 - msg_gap + 6.0
                    } else {
                        0.0
                    });
                let mut x0 = if dir > 0.0 { right_edge(&a, 0) } else { left_edge(&a, 0) };
                let mut x1 = if dir > 0.0 {
                    left_edge(&b, starting)
                } else {
                    right_edge(&b, starting)
                };
                if creating {
                    d.cols[view.b].top = line_y - b.size.h / 2.0;
                    d.cols[view.b].life_start = d.cols[view.b].top + b.size.h;
                    x1 = b.x - dir * if is_figure(&b.kind) { 18.0 } else { b.size.w / 2.0 + 1.0 };
                }
                if circle_from {
                    inner += &circle(a.x, line_y, color);
                    x0 = a.x + dir * CENTRAL_R;
                }
                if circle_to {
                    inner += &circle(b.x, line_y, color);
                    x1 = b.x - dir * CENTRAL_R;
                }
                if bw > 0.0 {
                    if tail_at_end {
                        let cx = if circle_to { x1 - dir * (bw / 2.0 + 2.0) } else { x1 };
                        inner += &d.badge(i, cx, line_y, bw);
                        x1 = cx - dir * bw / 2.0;
                    } else {
                        let cx = if circle_from { x0 + dir * (bw / 2.0 + 2.0) } else { x0 };
                        inner += &d.badge(i, cx, line_y, bw);
                        x0 = cx + dir * bw / 2.0;
                    }
                }
                let heads = (if st.at_start {
                    head(st.head, x0, line_y, -dir, up * dir, color)
                } else {
                    String::new()
                }) + &if st.at_end {
                    head(st.head, x1, line_y, dir, up * dir, color)
                } else {
                    String::new()
                };
                let from = x0 + if st.at_start { dir * trim } else { 0.0 };
                let to = x1 - if st.at_end { dir * trim } else { 0.0 };
                inner = format!("<path d=\"M{},{}H{}\"{attrs}/>{heads}{inner}", num(from), num(line_y), num(to));
                if label.width > 0.0 {
                    let (l, r) = (x0.min(x1), x0.max(x1));
                    let cx = if align < 0 {
                        l + d.msg_pad + label.width / 2.0
                    } else if align > 0 {
                        r - d.msg_pad - label.width / 2.0
                    } else {
                        (l + r) / 2.0
                    };
                    d.extend(cx - label.width / 2.0, cx + label.width / 2.0);
                    labels_out += &format!(
                        "<g class=\"pele-edge-label\" data-id=\"i{}\">{}</g>",
                        esc_text(&m.id),
                        d.backed(label, cx, line_y - 4.0 - label.height / 2.0, false, &stack)
                    );
                }
                if creating {
                    y = line_y + b.size.h / 2.0 - 6.0;
                }
            }
            edges_out += &format!(
                "<g class=\"pele-edge pele-message\" data-id=\"i{}\"{}>{inner}</g>",
                esc_text(&m.id),
                line.map(|l| format!(" stroke=\"{l}\"")).unwrap_or_default()
            );
            y = y.max(line_y);
            if a.destroy_at == i as isize {
                y = y.max(d.destroy(view.a, line_y, &mut crosses_out));
            }
            if b.destroy_at == i as isize && view.b != view.a {
                y = y.max(d.destroy(view.b, line_y, &mut crosses_out));
            }
        }
    }
    y += 20.0;
    for i in 0..d.cols.len() {
        while !d.cols[i].activations.is_empty() {
            d.end_activation(i, y, &mut activations_out);
        }
    }
    let mut bottom = y;
    if mirror {
        let mut foot_h = 0.0f64;
        for col in &d.cols {
            if col.life_end >= 0.0 {
                continue;
            }
            foot_h = foot_h.max(col.size.h);
            actors_out += &d.actor_svg(col, y, true);
        }
        bottom = y + foot_h;
    }
    let mut lifelines = String::new();
    for col in &d.cols {
        actors_out += &d.actor_svg(col, col.top, false);
        let end = if col.life_end >= 0.0 { col.life_end } else { y };
        if end > col.life_start {
            lifelines += &format!(
                "<path class=\"pele-lifeline\" data-id=\"{}\" d=\"M{},{}V{}\"/>",
                esc_text(&col.id),
                num(col.x),
                num(col.life_start),
                num(end)
            );
        }
    }
    let mut boxes_out = String::new();
    for run in &d.runs {
        let (x0, x1) = d.box_edges(run);
        let b = &db.boxes[run.box_id];
        let fill = if b.fill == "transparent" {
            String::new()
        } else {
            resolve_style(&[format!("fill:{}", b.fill)]).shape
        };
        let color = ink_on(Some(&b.fill));
        boxes_out += &format!(
            "<g class=\"pele-cluster pele-box\"{}><rect x=\"{}\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\" stroke=\"var(--_b)\"{}/>{}</g>",
            b.name
                .as_ref()
                .filter(|s| !s.is_empty())
                .map(|s| format!(" data-id=\"{}\"", esc_text(s)))
                .unwrap_or_default(),
            num(x0),
            num(x1 - x0),
            num(bottom + box_pad),
            rounded(&(if fill.is_empty() { " fill-opacity=\"0.5\"" } else { "" }.to_string() + &fill)),
            label_svg(
                &run.title,
                (x0 + x1) / 2.0,
                box_pad / 2.0 + 2.0 + run.title.height / 2.0,
                &format!(
                    " class=\"pele-cluster-label\"{}",
                    if color.is_empty() { " fill=\"var(--_m)\"" } else { &color }
                )
            )
        );
    }
    if !d.runs.is_empty() {
        bottom += box_pad;
    }
    let mut min_y = 0.0;
    let mut title_out = String::new();
    let room = title_room(options);
    let title = title_label(db.title.as_deref(), measurer, size, if tight { room } else { f64::INFINITY });
    let title = if tight {
        break_label(title, measurer, 120.0f64.max(room))
    } else {
        title
    };
    if d.min_x > d.max_x {
        d.extend(0.0, title.width.max(40.0));
    }
    if title.width > 0.0 {
        let cx = (d.min_x + d.max_x) / 2.0;
        d.extend(cx - title.width / 2.0, cx + title.width / 2.0);
        min_y = -title.height - 12.0;
        title_out = label_svg(
            &title,
            cx,
            min_y + title.height / 2.0,
            " class=\"pele-title\" font-weight=\"var(--_tw)\"",
        );
    }
    let width = (d.max_x - d.min_x + 2.0 * pad).ceil();
    if grown && width > options.max_width.unwrap() {
        return draw(
            db,
            config,
            options,
            measurer,
            tight,
            if uneven {
                hold
            } else if hold > 0.0 {
                f64::INFINITY
            } else {
                width - options.max_width.unwrap().floor()
            },
            false,
        );
    }
    let height = (bottom - min_y + 2.0 * pad).ceil();
    let mut content = format!(
        "<g transform=\"translate({},{})\">{title_out}",
        num(pad - d.min_x),
        num(pad - min_y)
    );
    if !boxes_out.is_empty() {
        content += &format!("<g class=\"pele-boxes\" font-size=\"{}\">{boxes_out}</g>", js_num(small));
    }
    if !rects_out.is_empty() {
        rects_out.reverse();
        content += &format!("<g class=\"pele-rects\" fill=\"var(--_a)\">{}</g>", rects_out.join(""));
    }
    if !lifelines.is_empty() {
        content += &format!("<g class=\"pele-lifelines\" fill=\"none\" stroke=\"var(--_b)\">{lifelines}</g>");
    }
    if !activations_out.is_empty() {
        activations_out.reverse();
        content += &format!(
            "<g class=\"pele-activations\" fill=\"var(--_s)\" stroke=\"var(--_b)\">{}</g>",
            activations_out.join("")
        );
    }
    if !frames_out.is_empty() {
        content += &format!(
            "<g class=\"pele-frames\" font-size=\"{}\" stroke=\"var(--_b)\">{frames_out}</g>",
            js_num(small)
        );
    }
    if !edges_out.is_empty() {
        content += &format!("<g class=\"pele-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{edges_out}</g>");
    }
    if !crosses_out.is_empty() {
        content += &format!(
            "<g class=\"pele-destroys\" fill=\"none\" stroke=\"var(--_l)\" stroke-width=\"1.5\" stroke-linecap=\"round\">{crosses_out}</g>"
        );
    }
    if !labels_out.is_empty() {
        content += &format!(
            "<g class=\"pele-edge-labels\" font-size=\"{}\"{}>{labels_out}</g>",
            js_num(small),
            if monospaced {
                format!(" font-family=\"{FONT_MONO}\"")
            } else {
                String::new()
            }
        );
    }
    if !notes_out.is_empty() {
        content += &format!(
            "<g class=\"pele-notes\" font-size=\"{}\" fill=\"var(--_a)\" stroke=\"var(--_b)\">{notes_out}</g>",
            js_num(small)
        );
    }
    if !actors_out.is_empty() {
        content += &format!("<g class=\"pele-nodes\">{actors_out}</g>");
    }
    content += "</g>";
    Rendered {
        svg: svg_document(
            "sequence",
            width,
            height,
            size,
            options,
            &Accessible {
                acc_title: db.acc_title.as_deref(),
                acc_descr: db.acc_descr.as_deref(),
            },
            &content,
        ),
        width,
        height,
        links: Vec::new(),
    }
}
fn circle(cx: f64, cy: f64, color: &str) -> String {
    format!(
        "<circle class=\"pele-central\" cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"{color}\" stroke=\"none\"/>",
        num(cx),
        num(cy)
    )
}
