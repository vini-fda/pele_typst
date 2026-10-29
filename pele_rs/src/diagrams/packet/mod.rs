//! Packet field diagrams, ported from Pele 1.12.0.
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, title_room, turn_to_fit};
use crate::diagrams::tokens::*;
use crate::errors::{PeleError, PeleErrorCode};
use crate::preprocess::Config;
use crate::svg::builder::{esc, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::ROUNDED;
use crate::text::entities::decode_entities;
use crate::text::label::{Label, Span, layout_label};
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{is_js_space, js_num, js_number, js_round, num, trim, trim_end, trim_start};

#[derive(Debug, Clone)]
pub struct Block {
    pub start: f64,
    pub end: f64,
    pub bits: f64,
    pub label: String,
}
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub bits_per_row: f64,
    pub rows: Vec<Vec<Block>>,
}

fn integer(s: &[u16], p: usize) -> Option<usize> {
    if s.get(p) == Some(&48) {
        return Some(p + 1);
    }
    if !s.get(p).is_some_and(|c| (49..=57).contains(c)) {
        return None;
    }
    let mut end = p + 1;
    while s.get(end).is_some_and(|c| (48..=57).contains(c)) {
        end += 1;
    }
    if s.get(end) == Some(&46) {
        end -= 1;
    }
    (end > p).then_some(end)
}
fn dash(s: &[u16], p: usize) -> Option<usize> {
    if yaml(s, p).is_some() {
        None
    } else {
        (s.get(p) == Some(&45)).then_some(p + 1)
    }
}
fn mark(s: &'static str) -> TokenType {
    TokenType {
        name: s,
        pattern: Pattern::Literal(s),
        hidden: false,
        first: None,
    }
}
fn eol(r: &mut Reader) -> Result<(), PeleError> {
    if r.kind() != -1 && r.kind() != 10 {
        return Err(r.fail("a line break or the end of input"));
    }
    while r.accept(10) {}
    Ok(())
}
fn invalid(message: String) -> PeleError {
    PeleError::new(message, PeleErrorCode::Semantic).of("packet")
}
fn option(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    config
        .get("packet")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_f64())
        .filter(|&v| v >= min)
        .map_or(fallback, |v| v.min(max))
}
pub fn parse(source: &str, config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let types = vec![
        mark("packet-beta"),
        keyword("packet"),
        TokenType {
            name: "-",
            pattern: Pattern::Match(dash),
            hidden: false,
            first: Some("-"),
        },
        mark("+"),
        mark(":"),
        acc_descr_type(),
        acc_title_type(),
        title_type(),
        TokenType {
            name: "INT",
            pattern: Pattern::Match(integer),
            hidden: false,
            first: Some("0123456789"),
        },
        string_type(),
        newline_type(),
        whitespace_type(),
        yaml_type(),
        directive_type(),
        comment_type(),
    ];
    let tokens = tokenize(&source.encode_utf16().collect::<Vec<_>>(), &types, "packet")?;
    let mut r = Reader {
        i: 0,
        tokens,
        types: &types,
        diagram: "packet",
    };
    let mut model = Model {
        bits_per_row: option(config, "bitsPerRow", 32.0, 1.0, 4096.0).floor(),
        ..Default::default()
    };
    let mut last_bit = -1.0;
    let mut word = vec![];
    let mut row = 1.0;
    while r.accept(10) {}
    if !r.accept(0) {
        r.expect(1)?;
    }
    while r.kind() != -1 {
        match r.kind() {
            10 => r.i += 1,
            5 => {
                model.acc_descr = Some(acc_descr_value(&r.take()));
                eol(&mut r)?;
            }
            6 => {
                model.acc_title = Some(acc_title_value(&r.take()));
                eol(&mut r)?;
            }
            7 => {
                model.title = Some(title_value(&r.take()));
                eol(&mut r)?;
            }
            8 | 3 => {
                let (given_start, given_end, given_bits) = if r.accept(3) {
                    (None, None, Some(js_number(&r.expect(8)?)))
                } else {
                    let start = js_number(&r.take());
                    let end = if r.accept(2) { Some(js_number(&r.expect(8)?)) } else { None };
                    (Some(start), end, None)
                };
                r.expect(4)?;
                let label = string_value(&r.expect(9)?);
                eol(&mut r)?;
                if let (Some(start), Some(end)) = (given_start, given_end)
                    && end < start
                {
                    return Err(invalid(format!(
                        "Packet block {} - {} is invalid. End must be greater than start.",
                        js_num(start),
                        js_num(end)
                    )));
                }
                let mut start = given_start.unwrap_or(last_bit + 1.0);
                if start != last_bit + 1.0 {
                    return Err(invalid(format!(
                        "Packet block {} - {} is not contiguous. It should start from {}.",
                        js_num(start),
                        js_num(given_end.unwrap_or(start)),
                        js_num(last_bit + 1.0)
                    )));
                }
                if given_bits == Some(0.0) {
                    return Err(invalid(format!(
                        "Packet block {} is invalid. Cannot have a zero bit field.",
                        js_num(start)
                    )));
                }
                let end = given_end.unwrap_or(start + given_bits.unwrap_or(1.0) - 1.0);
                let mut bits = given_bits.unwrap_or(end - start + 1.0);
                last_bit = end;
                while word.len() as f64 <= model.bits_per_row + 1.0 && model.rows.len() < 10000 {
                    let row_end = row * model.bits_per_row - 1.0;
                    if end <= row_end {
                        word.push(Block {
                            start,
                            end,
                            bits,
                            label: label.clone(),
                        });
                        if end == row_end {
                            model.rows.push(std::mem::take(&mut word));
                            row += 1.0;
                        }
                        break;
                    }
                    word.push(Block {
                        start,
                        end: row_end,
                        bits: row_end - start,
                        label: label.clone(),
                    });
                    model.rows.push(std::mem::take(&mut word));
                    row += 1.0;
                    start = row_end + 1.0;
                    bits = end - start;
                }
            }
            _ => return Err(r.fail("a field, a title, or a line break")),
        }
    }
    if !word.is_empty() {
        model.rows.push(word);
    }
    model.title = model.title.filter(|v| !v.is_empty()).or_else(|| title.map(String::from));
    model.acc_title = model.acc_title.filter(|v| !v.is_empty()).map(|v| trim_start(&v).to_string());
    model.acc_descr = model.acc_descr.filter(|v| !v.is_empty()).map(|v| {
        let mut out = String::new();
        let mut chars = v.chars().peekable();
        while let Some(c) = chars.next() {
            out.push(c);
            if c == '\n' {
                while chars.peek().is_some_and(|&c| is_js_space(c)) {
                    chars.next();
                }
            }
        }
        out
    });
    Ok(model)
}

fn literal(text: &str, measurer: &dyn TextMeasurer, size: f64, room: f64) -> Label {
    let mut lines = vec![];
    for raw in text.split('\n') {
        let chars: Vec<char> = raw.chars().collect();
        if chars.is_empty() {
            lines.push(String::new());
        }
        let mut from = 0;
        while from < chars.len() {
            let fits = |length| measurer.width(&chars[from..from + length].iter().collect::<String>(), size, 0) <= room;
            let mut hi = 1;
            while hi < chars.len() - from && fits(hi) {
                hi = (chars.len() - from).min(hi * 2);
            }
            let mut lo = 1;
            while lo < hi {
                let mid = (lo + hi).div_ceil(2);
                if fits(mid) {
                    lo = mid;
                } else {
                    hi = mid - 1;
                }
            }
            let mut take = lo;
            let wraps = from + take < chars.len();
            if wraps && !is_js_space(chars[from + take]) {
                for i in (1..take).rev() {
                    if is_js_space(chars[from + i]) {
                        take = i + 1;
                        break;
                    }
                }
            }
            let line: String = chars[from..from + take].iter().collect();
            lines.push(if wraps && !trim_end(&line).is_empty() {
                trim_end(&line).to_string()
            } else {
                line
            });
            from += take;
            while wraps && from < chars.len() && is_js_space(chars[from]) {
                from += 1;
            }
        }
    }
    if text.is_empty() {
        lines.clear();
    }
    let widths: Vec<_> = lines.iter().map(|v| measurer.width(v, size, 0)).collect();
    let width = widths.iter().copied().fold(0.0, f64::max);
    let line_height = js_round(size * 1.5);
    Label {
        height: lines.len() as f64 * line_height,
        lines: lines
            .into_iter()
            .zip(&widths)
            .map(|(text, &width)| {
                vec![Span {
                    text,
                    style: 0,
                    width,
                    ..Default::default()
                }]
            })
            .collect(),
        widths,
        width,
        size,
        line_height,
    }
}
fn fit(text: &str, max: f64, size: f64, min: f64, measurer: &dyn TextMeasurer) -> Option<(String, f64)> {
    if text.is_empty() || max <= 0.0 {
        return None;
    }
    let width = measurer.width(text, size, 0);
    if width <= max {
        return Some((text.to_string(), size));
    }
    let mut fitted = min.max((size * max / width).floor());
    while fitted >= min {
        if measurer.width(text, fitted, 0) <= max {
            return Some((text.to_string(), fitted));
        }
        fitted -= 1.0;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut low = 0;
    let mut high = chars.len() - 1;
    while low < high {
        let mid = (low + high + 1) >> 1;
        let s = trim_end(&chars[..mid].iter().collect::<String>()).to_string() + "…";
        if measurer.width(&s, min, 0) <= max {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    if low == 0 {
        return None;
    }
    let cut = trim_end(&chars[..low].iter().collect::<String>()).to_string() + "…";
    (measurer.width(&cut, min, 0) <= max).then_some((cut, min))
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    turn_to_fit(options, |down| draw(model, config, options, measurer, down))
}
fn draw(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer, down: bool) -> Rendered {
    let size = options.font_size.unwrap_or(16.0);
    let label_size = js_round(size * 0.875);
    let bit_size = js_round(size * 0.625);
    let pad = options.padding.unwrap_or(8.0);
    let row_height = option(config, "rowHeight", 32.0, 1.0, 400.0);
    let mut bit_width = option(config, "bitWidth", 32.0, 1.0, 400.0);
    let padding_x = option(config, "paddingX", 5.0, 0.0, 400.0);
    let padding_y = option(config, "paddingY", 5.0, 0.0, 400.0);
    let section = config.get("packet");
    let show_bits = section.and_then(|v| v.get("showBits")).and_then(|v| v.as_bool()) != Some(false);
    let descending = section.and_then(|v| v.get("bitOrder")).and_then(|v| v.as_str()) == Some("descending");
    let mut bits_per_row = model.bits_per_row;
    let mut rows = model.rows.clone();
    if down {
        let count = 1.0_f64.max(((options.max_width.unwrap_or(320.0) - 2.0 * pad) / bit_width.min(18.0)).floor());
        bits_per_row = bits_per_row.min(8.0_f64.max(2.0_f64.powf(count.log2().floor())));
        let last = model.rows.last().and_then(|r| r.last()).map_or(0.0, |b| b.end);
        if ((last + 1.0) / bits_per_row).ceil() > 10000.0 {
            bits_per_row = model.bits_per_row;
        }
        bit_width = bit_width.min(18.0_f64.max((options.max_width.unwrap_or(320.0) - 2.0 * pad) / bits_per_row));
        let mut whole: Vec<Block> = vec![];
        for block in model.rows.iter().flatten() {
            if let Some(prev) = whole.last_mut()
                && prev.bits == prev.end - prev.start
                && block.bits == block.end - block.start
                && prev.label == block.label
                && block.start == prev.end + 1.0
                && block.start % model.bits_per_row == 0.0
            {
                prev.end = block.end;
                prev.bits = prev.end - prev.start;
                continue;
            }
            whole.push(block.clone());
        }
        rows.clear();
        for block in whole {
            let mut start = block.start;
            while start <= block.end {
                let end = block.end.min(((start / bits_per_row).floor() + 1.0) * bits_per_row - 1.0);
                let index = (start / bits_per_row).floor() as usize;
                if rows.len() <= index {
                    rows.resize_with(index + 1, Vec::new);
                }
                rows[index].push(Block {
                    start,
                    end,
                    ..block.clone()
                });
                start = end + 1.0;
            }
        }
    }
    let labels: Vec<Vec<Label>> = if down {
        rows.iter()
            .map(|row| {
                row.iter()
                    .map(|b| {
                        literal(
                            trim(&decode_entities(&b.label)),
                            measurer,
                            label_size,
                            1.0_f64.max((b.end - b.start + 1.0) * bit_width - padding_x.min(bit_width - 1.0) - 6.0),
                        )
                    })
                    .collect()
            })
            .collect()
    } else {
        vec![]
    };
    let numbers = if show_bits { bit_size + 4.0 } else { 0.0 };
    let mut tops = vec![];
    let mut row_heights = vec![];
    let mut rows_height = 0.0;
    for (i, _) in rows.iter().enumerate() {
        let mut height = row_height;
        if down {
            for label in &labels[i] {
                height = height.max(label.height + 10.0);
            }
        }
        tops.push(rows_height);
        row_heights.push(height);
        rows_height += numbers + height + padding_y;
    }
    if !rows.is_empty() {
        rows_height -= padding_y;
    }
    let gap = padding_x.min(bit_width - 1.0);
    let width = if rows.is_empty() { 0.0 } else { bits_per_row * bit_width - gap };
    let title = layout_label(
        model.title.as_deref(),
        false,
        measurer,
        size,
        if down { title_room(options) } else { 4000.0 },
        BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + if rows_height > 0.0 { 12.0 } else { 0.0 }
    } else {
        0.0
    };
    let left = pad + 0.0_f64.max((title.width - width) / 2.0);
    let mut fields = String::new();
    let mut bits = String::new();
    for (index, row) in rows.iter().enumerate() {
        let y = pad + tops[index] + numbers;
        let h = row_heights[index];
        for (j, block) in row.iter().enumerate() {
            let count = block.end - block.start + 1.0;
            let first = block.start % bits_per_row;
            let x = left + if descending { bits_per_row - first - count } else { first } * bit_width;
            let w = count * bit_width - gap;
            fields += &format!(
                "<g class=\"pele-node\" data-id=\"{}-{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{ROUNDED}/>",
                js_num(block.start),
                js_num(block.end),
                num(x),
                num(y),
                num(w),
                num(h)
            );
            if down {
                fields += &label_svg(&labels[index][j], x + w / 2.0, y + h / 2.0, "");
            } else if let Some((text, fs)) = fit(
                trim(&decode_entities(&block.label)),
                w - 6.0,
                label_size,
                js_round(size * 0.5),
                measurer,
            ) && fs <= h
            {
                fields += &format!(
                    "<text x=\"{}\" y=\"{}\"{}>{}</text>",
                    num(x + w / 2.0),
                    num(y + h / 2.0 + fs * 0.35),
                    if fs == label_size {
                        String::new()
                    } else {
                        format!(" font-size=\"{}\"", num(fs))
                    },
                    esc(&text)
                );
            }
            fields += "</g>";
            if !show_bits {
                continue;
            }
            let leading = if descending { block.end } else { block.start };
            let trailing = if descending { block.start } else { block.end };
            if count == 1.0 {
                bits += &format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
                    num(x + w / 2.0),
                    num(y - 4.0),
                    js_num(leading)
                );
            } else {
                bits += &format!(
                    "<text x=\"{}\" y=\"{}\">{}</text><text x=\"{}\" y=\"{}\" text-anchor=\"end\">{}</text>",
                    num(x),
                    num(y - 4.0),
                    js_num(leading),
                    num(x + w),
                    num(y - 4.0),
                    js_num(trailing)
                );
            }
        }
    }
    let total_width = (width.max(title.width) + 2.0 * pad).ceil();
    let total_height = (rows_height + title_height + 2.0 * pad).ceil();
    let mut content = String::new();
    if !fields.is_empty() {
        content += &format!(
            "<g class=\"pele-packet-fields\" font-size=\"{}\" text-anchor=\"middle\">{fields}</g>",
            js_num(label_size)
        );
    }
    if !bits.is_empty() {
        content += &format!(
            "<g class=\"pele-packet-bits\" font-size=\"{}\" fill=\"var(--_m)\">{bits}</g>",
            js_num(bit_size)
        );
    }
    content += &label_svg(
        &title,
        total_width / 2.0,
        total_height - pad - title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    let acc = Accessible {
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    };
    Rendered {
        svg: svg_document("packet", total_width, total_height, size, options, &acc, &content),
        width: total_width,
        height: total_height,
        links: vec![],
    }
}
