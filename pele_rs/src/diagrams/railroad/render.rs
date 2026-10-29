use super::{Model, Node};
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, tighten, title_room};
use crate::preprocess::Config;
use crate::svg::builder::{esc, esc_text, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{FONT_MONO, ROUNDED};
use crate::text::entities::decode_entities;
use crate::text::measurer::{BOLD, ITALIC, MONO, TextMeasurer};
use crate::util::{js_num, js_round, num};
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Terminal,
    Nonterminal,
    Special,
    Sequence,
    Choice,
    Bypass,
    Loop,
    Folded,
}
#[derive(Clone)]
struct Box {
    kind: Kind,
    width: f64,
    up: f64,
    down: f64,
    kids: Vec<usize>,
    text: String,
    id: String,
    reach: f64,
}
impl Box {
    fn new(kind: Kind, width: f64, up: f64, down: f64, kids: Vec<usize>) -> Self {
        Self {
            kind,
            width,
            up,
            down,
            kids,
            text: String::new(),
            id: String::new(),
            reach: 0.0,
        }
    }
}
struct Metrics<'a> {
    measurer: &'a dyn TextMeasurer,
    size: f64,
    padding: f64,
    height: f64,
    gap: f64,
    sep: f64,
    arc: f64,
}
fn option(c: &Config, key: &str, f: f64, max: f64) -> f64 {
    c.get("railroad")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_f64())
        .filter(|v| *v >= 0.0)
        .map(|v| v.min(max))
        .unwrap_or(f)
}
fn shown(s: &str) -> String {
    let s = if s.contains("¶ß") {
        decode_entities(&s.replace('&', "&amp;"))
    } else {
        s.to_string()
    };
    s.replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t")
}
fn even(n: f64) -> f64 {
    (n / 2.0).ceil() * 2.0
}
fn leaf(kind: Kind, text: String, id: String, style: u32, m: &Metrics) -> Box {
    let mut b = Box::new(
        kind,
        even(m.measurer.width(&text, m.size, style) + 2.0 * m.padding).max(m.height),
        m.height / 2.0,
        m.height / 2.0,
        Vec::new(),
    );
    b.text = text;
    b.id = id;
    b
}
fn count(min: f64, max: f64) -> String {
    if max == f64::INFINITY && min <= 1.0 {
        return String::new();
    }
    let text = |n: f64| if n.is_finite() { js_num(n) } else { "∞".into() };
    if min == max {
        format!("{}×", text(min))
    } else if max == f64::INFINITY {
        format!("≥{}×", text(min))
    } else if min == 0.0 {
        format!("≤{}×", text(max))
    } else {
        format!("{}–{}×", text(min), text(max))
    }
}
fn below(offset: f64, above: &Box, branch: &Box, m: &Metrics) -> f64 {
    (offset + above.down + m.sep + branch.up).max(2.0 * m.arc)
}
fn bypass(kid: usize, boxes: &[Box], m: &Metrics) -> Box {
    let k = &boxes[kid];
    let reach = (k.up + m.sep).max(2.0 * m.arc);
    let mut b = Box::new(Kind::Bypass, k.width + 4.0 * m.arc, reach, k.down, vec![kid]);
    b.reach = reach;
    b
}
fn seq(kids: Vec<usize>, boxes: &[Box], m: &Metrics) -> Box {
    let mut width = kids.len().saturating_sub(1) as f64 * m.gap;
    let mut up = 0.0_f64;
    let mut down = 0.0_f64;
    for &i in &kids {
        width += boxes[i].width;
        up = up.max(boxes[i].up);
        down = down.max(boxes[i].down)
    }
    Box::new(Kind::Sequence, width, up, down, kids)
}
fn add(boxes: &mut Vec<Box>, b: Box) -> usize {
    let i = boxes.len();
    boxes.push(b);
    i
}
fn measure(root: usize, model: &Model, m: &Metrics, room: f64) -> (Vec<Box>, usize) {
    let mut pending = vec![(root, room)];
    let mut cursor = 0;
    while cursor < pending.len() {
        let (i, width) = pending[cursor];
        let node = &model.nodes[i];
        let around = match node {
            Node::Choice(_) | Node::Optional(_) => 4.0 * m.arc,
            Node::Repetition { min, .. } => (if *min == 0.0 { 6.0 } else { 2.0 }) * m.arc,
            Node::Sequence(_) => 6.0 * m.arc,
            _ => 0.0,
        };
        for &child in node.parts() {
            pending.push((
                child,
                if room.is_finite() {
                    40.0_f64.max(width - around)
                } else {
                    f64::INFINITY
                },
            ))
        }
        cursor += 1
    }
    let mut boxes = Vec::new();
    let mut built = vec![0; model.nodes.len()];
    for &(i, room) in pending.iter().rev() {
        let kids: Vec<usize> = model.nodes[i].parts().iter().map(|&n| built[n]).collect();
        let b = match &model.nodes[i] {
            Node::Terminal(v) => {
                let text = shown(v);
                let text = if !text.is_empty() && text.bytes().all(|c| c == b' ') {
                    "␣".repeat(text.len())
                } else {
                    text
                };
                leaf(Kind::Terminal, text, String::new(), MONO, m)
            }
            Node::Nonterminal(v) => leaf(Kind::Nonterminal, shown(v), v.clone(), 0, m),
            Node::Special(v) => leaf(Kind::Special, format!("? {} ?", shown(v)), String::new(), ITALIC, m),
            Node::Sequence(_) => {
                let s = seq(kids.clone(), &boxes, m);
                if s.width <= room || kids.len() < 2 {
                    s
                } else {
                    let mut rows = Vec::new();
                    let mut row = Vec::new();
                    let mut rw = 0.0;
                    for k in kids {
                        if !row.is_empty() && rw + m.gap + boxes[k].width > 40.0_f64.max(room - 6.0 * m.arc) {
                            let row_box = seq(std::mem::take(&mut row), &boxes, m);
                            rows.push(add(&mut boxes, row_box));
                            rw = 0.0
                        }
                        rw += if row.is_empty() { 0.0 } else { m.gap } + boxes[k].width;
                        row.push(k)
                    }
                    let row_box = seq(row, &boxes, m);
                    rows.push(add(&mut boxes, row_box));
                    if rows.len() < 2 {
                        s
                    } else {
                        let mut fw = 0.0_f64;
                        let mut offset = 0.0;
                        for (j, &r) in rows.iter().enumerate() {
                            fw = fw.max(boxes[r].width);
                            if j > 0 {
                                offset += boxes[rows[j - 1]].down + m.sep.max(2.0 * m.arc) + boxes[r].up
                            }
                        }
                        Box::new(
                            Kind::Folded,
                            fw + 6.0 * m.arc,
                            boxes[rows[0]].up,
                            offset + boxes[*rows.last().unwrap()].down,
                            rows,
                        )
                    }
                }
            }
            Node::Choice(_) => {
                let mut inner = 0.0_f64;
                let mut offset = 0.0;
                for (j, &k) in kids.iter().enumerate() {
                    inner = inner.max(boxes[k].width);
                    if j > 0 {
                        offset = below(offset, &boxes[kids[j - 1]], &boxes[k], m)
                    }
                }
                Box::new(
                    Kind::Choice,
                    inner + 4.0 * m.arc,
                    kids.first().map(|&i| boxes[i].up).unwrap_or(0.0),
                    kids.last().map(|&i| offset + boxes[i].down).unwrap_or(0.0),
                    kids,
                )
            }
            Node::Optional(_) => bypass(kids[0], &boxes, m),
            Node::Repetition { min, max, .. } => {
                let kid = kids[0];
                if *min == 1.0 && *max == 1.0 {
                    built[i] = kid;
                    continue;
                }
                let text = count(*min, *max);
                let reach = (boxes[kid].down + m.sep).max(2.0 * m.arc);
                let label = if text.is_empty() {
                    0.0
                } else {
                    even(m.measurer.width(&text, 12.0, 0))
                };
                let mut loop_box = Box::new(
                    Kind::Loop,
                    boxes[kid].width.max(label) + 2.0 * m.arc,
                    boxes[kid].up,
                    reach + if text.is_empty() { 4.5 } else { 18.0 },
                    vec![kid],
                );
                loop_box.text = text;
                loop_box.reach = reach;
                if *min == 0.0 {
                    let k = add(&mut boxes, loop_box);
                    bypass(k, &boxes, m)
                } else {
                    loop_box
                }
            }
        };
        built[i] = add(&mut boxes, b)
    }
    let root = built[root];
    (boxes, root)
}
fn line(track: &mut String, x1: f64, y: f64, x2: f64) {
    if x2 > x1 {
        *track += &format!("M{},{}H{}", num(x1), num(y), num(x2))
    }
}
fn place(root: usize, boxes: &[Box], left: f64, baseline: f64, m: &Metrics, stroke: &str) -> (String, String, String) {
    let arc = m.arc;
    let a = num(arc);
    let turn = |sweep: u8, dx: &str, dy: &str| format!("a{a},{a} 0 0 {sweep} {dx}{a},{dy}{a}");
    let es = turn(1, "", "");
    let se = turn(0, "", "");
    let en = turn(0, "", "-");
    let ne = turn(1, "", "-");
    let ld = turn(0, "-", "");
    let lu = turn(0, "-", "-");
    let mut track = String::new();
    let mut arrows = String::new();
    let mut nodes = String::new();
    let mut pending = vec![(root, left, baseline)];
    while let Some((i, x, y)) = pending.pop() {
        let b = &boxes[i];
        let kids = &b.kids;
        let right = x + b.width;
        match b.kind {
            Kind::Sequence => {
                let mut kx = right;
                for (j, &k) in kids.iter().enumerate().rev() {
                    kx -= boxes[k].width;
                    pending.push((k, kx, y));
                    if j > 0 {
                        line(&mut track, kx - m.gap, y, kx)
                    }
                    kx -= m.gap
                }
            }
            Kind::Folded => {
                let start = x + 2.0 * arc;
                let from = pending.len();
                let mut offset = 0.0;
                line(&mut track, x, y, start);
                for (j, &k) in kids.iter().enumerate() {
                    let kid = &boxes[k];
                    let ky = y + offset;
                    pending.push((k, start, ky));
                    if j + 1 < kids.len() {
                        let next = &boxes[kids[j + 1]];
                        let next_offset = offset + kid.down + m.sep.max(2.0 * arc) + next.up;
                        let mid = y + offset + kid.down + m.sep.max(2.0 * arc) / 2.0;
                        track += &format!(
                            "M{},{}H{}V{}H{}V{}H{}",
                            num(start + kid.width),
                            num(ky),
                            num(right - 2.0 * arc),
                            num(mid),
                            num(x + arc),
                            num(y + next_offset),
                            num(start)
                        );
                        arrows += &format!("M{},{}l7,3.5l-7,3.5z", num(start - 7.0), num(y + next_offset - 3.5));
                        offset = next_offset
                    } else {
                        track += &format!(
                            "M{},{}H{}V{}H{}",
                            num(start + kid.width),
                            num(ky),
                            num(right - arc),
                            num(y),
                            num(right)
                        )
                    }
                }
                pending[from..].reverse()
            }
            Kind::Choice => {
                let inner = b.width - 4.0 * arc;
                let rail_in = x + arc;
                let rail_out = right - arc;
                let from = pending.len();
                let mut offset = 0.0;
                for (j, &k) in kids.iter().enumerate() {
                    let kid = &boxes[k];
                    if j > 0 {
                        offset = below(offset, &boxes[kids[j - 1]], kid, m)
                    }
                    let ky = y + offset;
                    let kx = x + 2.0 * arc + (inner - kid.width) / 2.0;
                    let last = j == kids.len() - 1;
                    if j == 0 {
                        line(&mut track, x, y, kx);
                        line(&mut track, kx + kid.width, y, right)
                    } else {
                        track += &format!(
                            "{}{se}{}M{},{}{}{en}{}",
                            if last {
                                format!("M{},{}{es}V{}", num(x), num(y), num(ky - arc))
                            } else {
                                format!("M{},{}", num(rail_in), num(ky - arc))
                            },
                            if kx > rail_in + arc {
                                format!("H{}", num(kx))
                            } else {
                                String::new()
                            },
                            num(kx + kid.width),
                            num(ky),
                            if kx > rail_in + arc {
                                format!("H{}", num(rail_out - arc))
                            } else {
                                String::new()
                            },
                            if last { format!("V{}{ne}", num(y + arc)) } else { String::new() }
                        )
                    }
                    pending.push((k, kx, ky));
                }
                pending[from..].reverse()
            }
            Kind::Bypass => {
                let kid = kids[0];
                let kx = x + 2.0 * arc;
                line(&mut track, x, y, kx);
                line(&mut track, kx + boxes[kid].width, y, right);
                track += &format!(
                    "M{},{}{en}V{}{ne}H{}{es}V{}{se}",
                    num(x),
                    num(y),
                    num(y - b.reach + arc),
                    num(right - 2.0 * arc),
                    num(y - arc)
                );
                pending.push((kid, kx, y))
            }
            Kind::Loop => {
                let kid = kids[0];
                let kx = x + (b.width - boxes[kid].width) / 2.0;
                let ly = y + b.reach;
                let middle = x + b.width / 2.0;
                line(&mut track, x, y, kx);
                line(&mut track, kx + boxes[kid].width, y, right);
                track += &format!(
                    "M{},{}{ld}V{}{se}H{}{en}V{}{lu}",
                    num(x + arc),
                    num(y),
                    num(ly - arc),
                    num(right - arc),
                    num(y + arc)
                );
                arrows += &format!("M{},{}l7,-3.5v7z", num(middle - 3.5), num(ly));
                if !b.text.is_empty() {
                    nodes += &format!(
                        "<text class=\"pele-edge-label pele-count\" x=\"{}\" y=\"{}\" font-size=\"12\" fill=\"var(--_m)\">{}</text>",
                        num(middle),
                        num(ly + 15.0),
                        esc(&b.text)
                    )
                }
                pending.push((kid, kx, y))
            }
            _ => {
                let text = &b.text;
                let space = if text.starts_with(' ') || text.ends_with(' ') || text.contains("  ") {
                    " xml:space=\"preserve\""
                } else {
                    ""
                };
                let shape = format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                    num(x),
                    num(y - m.height / 2.0),
                    num(b.width),
                    num(m.height)
                );
                let label = format!("<text x=\"{}\" y=\"{}\"{space}", num(x + b.width / 2.0), num(y + m.size * 0.35));
                nodes += &match b.kind {
                    Kind::Terminal => format!(
                        "<g class=\"pele-node pele-terminal\">{shape} rx=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{stroke}/>{label} font-family=\"{FONT_MONO}\">{}</text></g>",
                        num(m.height / 2.0),
                        esc(text)
                    ),
                    Kind::Nonterminal => format!(
                        "<g class=\"pele-node pele-nonterminal\" data-id=\"{}\">{shape} fill=\"var(--_bg)\" stroke=\"var(--_b)\"{stroke}{ROUNDED}/>{label}>{}</text></g>",
                        esc_text(&b.id),
                        esc(text)
                    ),
                    _ => format!(
                        "<g class=\"pele-node pele-special\">{shape} fill=\"var(--_bg)\" stroke=\"var(--_b)\" stroke-dasharray=\"4 3\"{stroke}{ROUNDED}/>{label} font-style=\"italic\"><tspan fill=\"var(--_m)\">? </tspan>{}<tspan fill=\"var(--_m)\"> ?</tspan></text></g>",
                        esc(&text[2..text.len() - 2])
                    ),
                }
            }
        }
    }
    (track, arrows, nodes)
}
pub fn render(model: &Model, c: &Config, o: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    tighten(o, |tight| draw(model, c, o, measurer, tight))
}
fn draw(model: &Model, c: &Config, o: &RenderOptions, measurer: &dyn TextMeasurer, tight: bool) -> Rendered {
    let base = o.font_size.unwrap_or(16.0);
    let fs = option(c, "fontSize", 0.0, 200.0);
    let size = if fs != 0.0 { fs } else { js_round(base * 0.875) };
    let head_size = if fs != 0.0 { fs } else { base };
    let padding = option(c, "padding", -1.0, 200.0);
    let lh = js_round(size * 1.5);
    let m = Metrics {
        measurer,
        size,
        padding: if padding < 0.0 { 12.0 } else { padding },
        height: even(lh + if padding < 0.0 { 9.0 } else { 2.0 * padding }),
        gap: option(c, "horizontalSeparation", 16.0, 500.0).min(if tight { 12.0 } else { f64::INFINITY }),
        sep: option(c, "verticalSeparation", 10.0, 500.0),
        arc: option(c, "arcRadius", 10.0, 200.0).min(if tight { 6.0 } else { f64::INFINITY }),
    };
    let radius = if c.get("railroad").and_then(|v| v.get("showMarkers")).and_then(|v| v.as_bool()) == Some(false) {
        0.0
    } else {
        option(c, "markerRadius", 4.0, 50.0)
    };
    let stroke_w = option(c, "strokeWidth", 0.0, 20.0);
    let stroke = if stroke_w > 0.0 {
        format!(" stroke-width=\"{}\"", num(stroke_w))
    } else {
        String::new()
    };
    let pad = o.padding.unwrap_or(8.0);
    let title = crate::text::label::layout_label(
        model.title.as_deref(),
        false,
        measurer,
        base,
        if tight { title_room(o) } else { 4000.0 },
        BOLD,
    );
    let title_h = if title.height > 0.0 { title.height + 16.0 } else { 0.0 };
    let head_h = js_round(head_size * 1.5);
    let mut rules = String::new();
    let mut width = title.width;
    let mut top = pad + title_h;
    let mut bottom = pad + title.height;
    for rule in &model.rules {
        let (boxes, root) = measure(
            rule.definition,
            model,
            &m,
            if tight {
                80.0_f64.max(o.max_width.unwrap_or(320.0) - 2.0 * pad - 4.0 * radius - 24.0 - 1.0)
            } else {
                f64::INFINITY
            },
        );
        let b = &boxes[root];
        let name = shown(&rule.name);
        let baseline = top + head_h + 6.0 + b.up;
        let start = pad + 2.0 * radius;
        let left = start + 12.0;
        let end = left + b.width + 12.0;
        let mut track = String::new();
        line(&mut track, start, baseline, left);
        line(&mut track, left + b.width, baseline, end);
        let (more, arrows, nodes) = place(root, &boxes, left, baseline, &m, &stroke);
        track += &more;
        let markers = if radius > 0.0 {
            format!(
                "<circle class=\"pele-marker pele-start\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_l)\"/><circle class=\"pele-marker pele-end\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_l)\"/>",
                num(pad + radius),
                num(baseline),
                num(radius),
                num(end + radius),
                num(baseline),
                num(radius)
            )
        } else {
            String::new()
        };
        rules += &format!(
            "<g class=\"pele-rule\" data-id=\"{}\"><text class=\"pele-rule-name\" x=\"{}\" y=\"{}\" font-size=\"{}\" font-weight=\"var(--_hw)\" text-anchor=\"start\">{}</text><path class=\"pele-edge pele-track\" d=\"{track}\" fill=\"none\" stroke=\"var(--_l)\"{stroke}/>{}{markers}{nodes}</g>",
            esc_text(&rule.name),
            num(pad),
            num(top + head_h / 2.0 + head_size * 0.35),
            num(head_size),
            esc(&name),
            if arrows.is_empty() {
                arrows
            } else {
                format!("<path class=\"pele-marker\" d=\"{arrows}\" fill=\"var(--_l)\"/>")
            }
        );
        width = width.max(end + 2.0 * radius - pad).max(measurer.width(&name, head_size, BOLD));
        bottom = baseline + b.down;
        top = bottom + m.sep + 18.0
    }
    let tw = (width + 2.0 * pad).ceil();
    let th = (bottom + pad).ceil();
    let content = label_svg(
        &title,
        tw / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &if rules.is_empty() {
        rules
    } else {
        format!(
            "<g class=\"pele-rules\" transform=\"translate(0.5,0.5)\" font-size=\"{}\" text-anchor=\"middle\">{rules}</g>",
            num(size)
        )
    };
    Rendered {
        svg: svg_document(
            "railroad",
            tw,
            th,
            base,
            o,
            &Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            &content,
        ),
        width: tw,
        height: th,
        links: Vec::new(),
    }
}
