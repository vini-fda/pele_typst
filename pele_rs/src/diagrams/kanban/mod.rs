use crate::diagrams::common::{Rendered, title_room};
use crate::diagrams::mindmap::{icon_svg, label_line, outline};
use crate::errors::PeleErrorCode;
use crate::preprocess::{Config, parse_yaml, truthy, value_string};
use crate::svg::builder::{esc, esc_text, label_svg, text_style};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{CRITICAL, ROUNDED, class_names};
use crate::text::label::{Label, Span, layout_label, line_middle};
use crate::text::measurer::{BOLD, SUB, SUP, TextMeasurer, UNDERLINE};
use crate::url::{LinkInfo, link_url, rel_attr, safe_url};
use crate::util::{js_num, js_round, num, trim};
use crate::{PeleError, RenderOptions};
#[derive(Clone, Debug, Default)]
pub struct Node {
    pub id: String,
    pub level: usize,
    pub label: String,
    pub icon: Option<String>,
    pub assigned: Option<String>,
    pub ticket: Option<String>,
    pub priority: Option<String>,
    pub css_classes: Option<String>,
    pub items: Vec<Node>,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: Option<String>,
    pub sections: Vec<Node>,
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let mut model = Model {
        title: title.map(str::to_string),
        sections: Vec::new(),
    };
    let mut section_level = 0;
    let mut last: Option<(usize, Option<usize>)> = None;
    let mut misplaced: Option<String> = None;
    outline::parse(source, true, |op| {
        match op {
            outline::Op::Node {
                level, id, descr, data, ..
            } => {
                let mut node = Node {
                    id,
                    level,
                    label: descr,
                    ..Default::default()
                };
                if let Some(data) = data {
                    let yaml = if data.contains('\n') {
                        data + "\n"
                    } else {
                        format!("{{\n{data}\n}}")
                    };
                    let doc = parse_yaml(&yaml).map_err(|e| {
                        PeleError::new(format!("Metadata of \"{}\" is not valid YAML. {e}", node.id), PeleErrorCode::Syntax).of("kanban")
                    })?;
                    if let Some(map) = doc.as_object() {
                        let shape = if truthy(map.get("shape")) {
                            value_string(&map["shape"])
                        } else {
                            String::new()
                        };
                        if shape != shape.to_lowercase() || shape.contains('_') {
                            return Err(PeleError::new(
                                format!("No such shape: {shape}. Shape names should be lowercase."),
                                PeleErrorCode::Semantic,
                            )
                            .of("kanban"));
                        }
                        for (key, target) in [
                            ("icon", &mut node.icon),
                            ("assigned", &mut node.assigned),
                            ("ticket", &mut node.ticket),
                            ("priority", &mut node.priority),
                        ] {
                            if truthy(map.get(key)) {
                                *target = Some(value_string(&map[key]))
                            }
                        }
                        if truthy(map.get("label")) {
                            node.label = value_string(&map["label"])
                        }
                    }
                }
                if let Some(label) = &misplaced {
                    return Err(PeleError::new(
                        format!("Items without section detected, found section (\"{label}\")"),
                        PeleErrorCode::Semantic,
                    )
                    .of("kanban"));
                }
                if model.sections.is_empty() {
                    section_level = level
                }
                if level == section_level {
                    last = Some((model.sections.len(), None));
                    model.sections.push(node)
                } else {
                    let section = model.sections.len() - 1;
                    last = Some((section, Some(model.sections[section].items.len())));
                    if level < section_level {
                        misplaced = Some(node.label.clone())
                    }
                    model.sections[section].items.push(node)
                }
            }
            outline::Op::Icon(v) => {
                if !v.is_empty()
                    && let Some((s, c)) = last
                {
                    let n = if let Some(c) = c {
                        &mut model.sections[s].items[c]
                    } else {
                        &mut model.sections[s]
                    };
                    n.icon = Some(v)
                }
            }
            outline::Op::Class(v) => {
                if !v.is_empty()
                    && let Some((s, c)) = last
                {
                    let n = if let Some(c) = c {
                        &mut model.sections[s].items[c]
                    } else {
                        &mut model.sections[s]
                    };
                    n.css_classes = Some(v)
                }
            }
        }
        Ok(())
    })?;
    Ok(model)
}
fn fit(raw: Option<&str>, markdown: bool, m: &dyn TextMeasurer, size: f64, max: f64, base: u32) -> Label {
    let mut label = layout_label(raw, markdown, m, size, max, base);
    if label.width <= max {
        return label;
    }
    let mut lines = Vec::new();
    let mut widths = Vec::new();
    let mut widest = 0.0_f64;
    for (i, spans) in label.lines.iter().enumerate() {
        if label.widths[i] <= max {
            lines.push(spans.clone());
            widths.push(label.widths[i]);
            widest = widest.max(label.widths[i]);
            continue;
        }
        let mut line: Vec<Span> = Vec::new();
        let mut width = 0.0;
        for span in spans {
            if !span.plain() {
                if width > 0.0 && width + span.width > max {
                    lines.push(std::mem::take(&mut line));
                    widths.push(width);
                    widest = widest.max(width);
                    width = 0.0
                }
                line.push(span.clone());
                width += span.width;
                continue;
            }
            let mut text = String::new();
            let mut from = width;
            for ch in span.text.chars() {
                let w = m.width(&ch.to_string(), size, span.style);
                if width > 0.0 && width + w > max {
                    if !text.is_empty() {
                        let mut piece = span.clone();
                        piece.text = std::mem::take(&mut text);
                        piece.width = width - from;
                        line.push(piece)
                    }
                    lines.push(std::mem::take(&mut line));
                    widths.push(width);
                    widest = widest.max(width);
                    width = 0.0;
                    from = 0.0
                }
                text.push(ch);
                width += w
            }
            if !text.is_empty() {
                let mut piece = span.clone();
                piece.text = text;
                piece.width = width - from;
                line.push(piece)
            }
        }
        lines.push(line);
        widths.push(width);
        widest = widest.max(width)
    }
    label.height = lines.len() as f64 * label.line_height;
    label.lines = lines;
    label.widths = widths;
    label.width = widest;
    crate::text::label::with_heights(label)
}
fn text_block(label: &Label, x: f64, y: f64, attrs: &str, end: bool) -> String {
    let mut out = String::new();
    for (i, line) in label.lines.iter().enumerate() {
        let cy = y + line_middle(label, i);
        if line.len() == 1 && line[0].plain() && (line[0].style & (SUB | SUP | UNDERLINE)) == 0 {
            out += &format!(
                "<text{} x=\"{}\" y=\"{}\"{}>{}</text>",
                text_style(attrs, line[0].style),
                num(x),
                num(cy + label.size * 0.35),
                if end { " text-anchor=\"end\"" } else { "" },
                esc(&line[0].text)
            )
        } else {
            let width = label.widths[i];
            out += &label_svg(&label_line(label, i), x + if end { -width } else { width } / 2.0, cy, attrs)
        }
    }
    out
}
pub fn render(model: &Model, config: &Config, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let line = js_round(size * 1.5);
    let small_line = js_round(small * 1.5);
    let pad = o.padding.unwrap_or(8.0);
    let conf = config.get("kanban");
    let glyph = small * 1.25;
    let has = |name: Option<&str>| name.is_some_and(|s| !s.is_empty() && !m.icon_markup(s).is_empty());
    let mut math_width = 0.0_f64;
    for section in &model.sections {
        let count = m.width(&section.items.len().to_string(), small, 0);
        let mut holds = |raw: Option<&str>, font_size: f64, reserved: f64| {
            if let Some(raw) = raw.filter(|s| s.contains("$$")) {
                for span in layout_label(Some(raw), true, m, font_size, f64::INFINITY, 0).lines.iter().flatten() {
                    if let Some(math) = &span.math {
                        math_width = math_width.max(math.width() + reserved)
                    }
                }
            }
        };
        holds(
            Some(&section.label),
            size,
            36.0 + count + if has(section.icon.as_deref()) { glyph + 6.0 } else { 0.0 },
        );
        for item in &section.items {
            holds(
                Some(&item.label),
                small,
                36.0 + if has(item.icon.as_deref()) { glyph + 6.0 } else { 0.0 },
            );
            holds(item.ticket.as_deref(), small, 36.0);
            holds(item.assigned.as_deref(), small, 36.0)
        }
    }
    let own_width = conf
        .and_then(|v| v.get("sectionWidth"))
        .and_then(|v| v.as_f64())
        .filter(|w| *w > 0.0)
        .map(|w| w.max(80.0).min(2000.0))
        .unwrap_or(200.0)
        .max(math_width.ceil());
    let room = o.max_width.filter(|w| *w > 0.0).map(|w| w - 2.0 * pad).unwrap_or(f64::INFINITY);
    let count = model.sections.len();
    let fits = count as f64 * (own_width + 12.0) - 12.0 <= room;
    let per_row = if fits {
        count.max(1)
    } else {
        ((room + 12.0) / (own_width + 12.0)).floor().max(1.0) as usize
    };
    let column_width = if fits {
        own_width
    } else {
        own_width
            .min(room)
            .max(((room + 12.0) / per_row as f64).floor() - 12.0)
            .min(own_width * 2.0)
            .max(math_width.ceil())
    };
    let base_url = conf.and_then(|v| v.get("ticketBaseUrl")).and_then(|v| v.as_str()).unwrap_or("");
    let card_width = column_width - 16.0;
    let text_width = card_width - 20.0;
    let mut links = Vec::new();
    struct Card<'a> {
        item: &'a Node,
        label: Label,
        ticket: Label,
        assigned: Label,
        meta: f64,
        height: f64,
    }
    let sections = &model.sections;
    let mut titles = Vec::new();
    let mut count_widths = Vec::new();
    let mut cards: Vec<Vec<Card>> = Vec::new();
    let mut head_height = line;
    let mut body_heights = Vec::new();
    for section in sections {
        let count_width = m.width(&section.items.len().to_string(), small, 0);
        let reserved = count_width + 12.0 + if has(section.icon.as_deref()) { glyph + 6.0 } else { 0.0 };
        let title = fit(Some(&section.label), true, m, size, card_width - 8.0 - reserved, BOLD);
        head_height = head_height.max(title.height);
        count_widths.push(count_width);
        titles.push(title);
        let mut list = Vec::new();
        let mut height = 0.0;
        for item in &section.items {
            let label = fit(
                Some(&item.label),
                true,
                m,
                small,
                text_width - if has(item.icon.as_deref()) { glyph + 6.0 } else { 0.0 },
                0,
            );
            let ticket = fit(item.ticket.as_deref(), false, m, small, text_width, 0);
            let assigned = fit(item.assigned.as_deref(), false, m, small, text_width, 0);
            let meta = if ticket.width + assigned.width + 8.0 > text_width {
                ticket.height + assigned.height
            } else {
                ticket.height.max(assigned.height)
            };
            let card_height = 20.0 + label.height.max(small_line) + if meta != 0.0 { 4.0 + meta } else { 0.0 };
            list.push(Card {
                item,
                label,
                ticket,
                assigned,
                meta,
                height: card_height,
            });
            height += card_height + 8.0
        }
        cards.push(list);
        body_heights.push(height)
    }
    head_height += 16.0;
    let mut row_heights = Vec::new();
    let mut row_tops = Vec::new();
    let mut first = 0;
    let mut y = 0.0;
    while first < sections.len() {
        let tallest = body_heights[first..(first + per_row).min(sections.len())]
            .iter()
            .copied()
            .fold(8.0, f64::max);
        row_tops.push(y);
        row_heights.push(head_height + tallest);
        y += head_height + tallest + 12.0;
        first += per_row
    }
    let board_height = row_heights.last().zip(row_tops.last()).map(|(h, y)| h + y).unwrap_or(0.0);
    let title = crate::svg::title::title_label(model.title.as_deref(), m, size, title_room(o));
    let title_height = if title.height > 0.0 { title.height + 16.0 } else { 0.0 };
    let board_width = (sections.len().min(per_row) as f64 * (column_width + 12.0) - 12.0).max(0.0);
    let width = board_width.max(title.width);
    let left = pad + (width - board_width) / 2.0;
    let edge = column_width - 12.0;
    let mut out = String::new();
    for (index, section) in sections.iter().enumerate() {
        let row = index / per_row;
        let mut body = format!(
            "<rect width=\"{}\" height=\"{}\" fill=\"var(--_s)\"{ROUNDED}/>",
            js_num(column_width),
            num(row_heights[row])
        ) + &text_block(
            &titles[index],
            12.0,
            8.0,
            " class=\"pele-cluster-label\" font-weight=\"var(--_hw)\"",
            false,
        ) + &format!(
            "<text class=\"pele-count\" x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
            js_num(edge),
            num(8.0 + line / 2.0 + small * 0.35),
            js_num(small),
            section.items.len()
        );
        if has(section.icon.as_deref()) {
            body += &icon_svg(
                section.icon.as_deref().unwrap(),
                edge - count_widths[index] - 6.0 - glyph,
                8.0 + (line - glyph) / 2.0,
                glyph,
                o,
            )
        }
        let mut y = head_height;
        for card in &cards[index] {
            let item = card.item;
            let priority = trim(item.priority.as_deref().unwrap_or("")).to_lowercase();
            let color = match priority.as_str() {
                "very high" => Some(format!("stroke=\"{CRITICAL}\"")),
                "high" => Some(format!("stroke=\"{CRITICAL}\" opacity=\"0.5\"")),
                "low" => Some("stroke=\"var(--_m)\" opacity=\"0.5\"".into()),
                "very low" => Some("stroke=\"var(--_m)\"".into()),
                _ => None,
            };
            let mut inner = format!(
                "<rect width=\"{}\" height=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{ROUNDED}/>",
                js_num(card_width),
                num(card.height)
            );
            if let Some(c) = &color {
                inner += &format!(
                    "<path class=\"pele-priority\" d=\"M2.5,5V{}\" fill=\"none\" {c} stroke-width=\"3\" stroke-linecap=\"round\"/>",
                    num(card.height - 5.0)
                )
            }
            inner += &text_block(&card.label, 10.0, 10.0, " class=\"pele-label\"", false);
            if has(item.icon.as_deref()) {
                inner += &icon_svg(
                    item.icon.as_deref().unwrap(),
                    card_width - 10.0 - glyph,
                    10.0 + (small_line - glyph) / 2.0,
                    glyph,
                    o,
                )
            }
            let bottom = card.height - 10.0;
            if let Some(ticket) = item.ticket.as_ref().filter(|s| !s.is_empty()) {
                let top = bottom - card.meta;
                let href = if !base_url.is_empty() {
                    link_url(&safe_url(&base_url.replacen("#TICKET#", ticket, 1)), o)
                } else {
                    None
                };
                if let Some(href) = href {
                    inner += &(format!("<a href=\"{}\" target=\"_blank\"{}>", esc(&href), rel_attr(o))
                        + &text_block(&card.ticket, 10.0, top, " class=\"pele-ticket\" fill=\"var(--_c)\"", false)
                        + "</a>");
                    links.push(LinkInfo {
                        id: item.id.clone(),
                        href,
                        internal: false,
                    })
                } else {
                    inner += &text_block(&card.ticket, 10.0, top, " class=\"pele-ticket\" fill=\"var(--_m)\"", false)
                }
            }
            if item.assigned.as_ref().is_some_and(|s| !s.is_empty()) {
                inner += &text_block(
                    &card.assigned,
                    card_width - 10.0,
                    bottom - card.assigned.height,
                    " class=\"pele-assigned\" fill=\"var(--_m)\"",
                    true,
                )
            }
            body += &format!(
                "<g class=\"pele-node pele-card{}{}\" data-id=\"{}\" transform=\"translate(8,{})\" font-size=\"{}\">{inner}</g>",
                if color.is_some() {
                    format!(" pele-priority-{}", priority.replacen(' ', "-", 1))
                } else {
                    String::new()
                },
                class_names(item.css_classes.as_deref().unwrap_or("")),
                esc_text(&item.id),
                num(y),
                js_num(small)
            );
            y += card.height + 8.0
        }
        out += &format!(
            "<g class=\"pele-cluster pele-column{}\" data-id=\"{}\" transform=\"translate({},{})\">{body}</g>",
            class_names(section.css_classes.as_deref().unwrap_or("")),
            esc_text(&section.id),
            num(left + (index % per_row) as f64 * (column_width + 12.0)),
            num(pad + title_height + row_tops[row])
        );
    }
    let tw = (width + 2.0 * pad).ceil();
    let th = (board_height + title_height + 2.0 * pad).ceil();
    let content = label_svg(
        &title,
        tw / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &out;
    Rendered {
        svg: svg_document(
            "kanban",
            tw,
            th,
            size,
            o,
            &Accessible {
                acc_title: None,
                acc_descr: None,
            },
            &content,
        ),
        width: tw,
        height: th,
        links,
    }
}
