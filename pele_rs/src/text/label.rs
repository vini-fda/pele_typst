use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;

use super::entities::decode_entities;
use super::measurer::{BOLD, ITALIC, MONO, SUB, SUP, TextMeasurer, UNDERLINE};
use crate::util::{JS_WS, is_js_space, js_round, trim, trim_end, trim_start};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Span {
    pub text: String,
    pub style: u32,
    pub width: f64,
    pub icon: Option<String>,
    pub icon_markup: Option<String>,
    pub icon_resolver: Option<crate::IconResolver>,
    pub math: Option<crate::MathMarkup>,
}

impl Span {
    pub fn plain(&self) -> bool {
        self.icon.is_none() && self.math.is_none()
    }
    pub fn draw_icon(&self) -> String {
        if let Some(resolve) = &self.icon_resolver {
            self.icon.as_deref().and_then(|name| resolve.resolve(name)).unwrap_or_default()
        } else {
            self.icon_markup.clone().unwrap_or_default()
        }
    }
}

struct Marks {
    tex: Vec<String>,
    math: Vec<Option<crate::MathMarkup>>,
}

#[derive(Debug, Clone, Default)]
pub struct Label {
    pub lines: Vec<Vec<Span>>,
    pub widths: Vec<f64>,
    pub width: f64,
    pub height: f64,
    pub size: f64,
    pub line_height: f64,
}

pub const LINE_HEIGHT: f64 = 1.5;
// Lines of one wrapped label sit closer, so the label reads as one block.
const WRAPPED_LINE_HEIGHT: f64 = 1.3;

const CJK: &str = r"\u{2e80}-\u{9fff}\u{f900}-\u{faff}\u{ff00}-\u{ff60}";
const CLOSE: &str = "、。，．：；！？）］｝」』】〕〉》〙〗ー…‥・ゝゞぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮ,.:;!?)]}";
const OPEN: &str = "（［｛「『【〔〈《〘〖([{";
const HINT: &str = r"\u{200b}\u{ad}";

fn class_chars(chars: &str) -> String {
    let mut out = String::new();
    for c in chars.chars() {
        if c.is_ascii_punctuation() {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn re_break() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(&format!(r"(?i)<br[{}]*/?>|\\n|\n", JS_WS)).unwrap())
}

fn re_tag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(&format!(r"^</?([a-zA-Z][0-9A-Za-z_-]*)(?:[{}][^<>]*)?/?>", JS_WS)).unwrap())
}

fn re_icon() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(fa[bklrs]?):fa-([0-9A-Za-z_-]+)").unwrap())
}

fn re_markup() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[<\\\n*_]|fa[bklrs]?:fa-").unwrap())
}

fn re_cjk() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(&format!("[{}]", CJK)).unwrap())
}

fn re_hint() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(&format!("[{}]", HINT)).unwrap())
}

fn re_piece() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let ws = JS_WS;
        let open = class_chars(OPEN);
        let close = class_chars(CLOSE);
        Regex::new(&format!(
            "[{ws}]*(?:[{open}]*[{CJK}][{close}]*|[^{ws}{CJK}{HINT}]+[{HINT}]*[{close}]*|[{HINT}]+)[{ws}]*|[{ws}]+"
        ))
        .unwrap()
    })
}

fn tag_style(tag: &str) -> u32 {
    match tag {
        "b" | "strong" => BOLD,
        "i" | "em" | "cite" | "dfn" | "var" => ITALIC,
        "code" | "kbd" | "samp" | "tt" => MONO,
        "sub" => SUB,
        "sup" => SUP,
        "u" | "ins" => UNDERLINE,
        _ => 0,
    }
}

const SHIFT: u32 = SUB | SUP;

fn html_tags() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        concat!(
            "a abbr address article aside audio b bdi bdo big blockquote body br button caption center cite code col ",
            "colgroup data dd del details dfn dialog div dl dt em embed fieldset figcaption figure font footer form h1 h2 ",
            "h3 h4 h5 h6 head header hr html i iframe img input ins kbd label legend li link main map mark meta meter nav ",
            "noscript object ol optgroup option output p picture pre progress q rp rt ruby s samp script section select ",
            "small source span strike strong style sub summary sup table tbody td template textarea tfoot th thead time ",
            "title tr track tt u ul var video wbr"
        )
        .split(' ')
        .collect()
    })
}

fn leading_space(line: &str) -> usize {
    line.chars().take_while(|&c| is_js_space(c)).count()
}

fn dedent(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut min = usize::MAX;
    for line in &lines {
        if trim(line).is_empty() {
            continue;
        }
        let indent = leading_space(line);
        if indent < min {
            min = indent;
        }
    }
    if min == usize::MAX || min == 0 {
        return text.to_string();
    }
    lines
        .iter()
        .map(|line| {
            let skip = min.min(leading_space(line));
            let at = line.char_indices().nth(skip).map(|(i, _)| i).unwrap_or(line.len());
            &line[at..]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn push_plain(line: &mut Vec<Span>, text: &str, style: u32) {
    if text.is_empty() {
        return;
    }
    let decoded = decode_entities(text);
    if let Some(last) = line.last_mut()
        && last.plain()
        && last.style == style
    {
        last.text.push_str(&decoded);
        return;
    }
    line.push(Span {
        text: decoded,
        style,
        width: 0.0,
        ..Default::default()
    });
}

fn push_text(line: &mut Vec<Span>, text: &str, style: u32, marks: Option<&Marks>) {
    if text.is_empty() {
        return;
    }
    if let Some(found) = marks {
        let mut last = 0;
        for (at, ch) in text.char_indices() {
            if !(0xe000..=0xf8ff).contains(&(ch as u32)) {
                continue;
            }
            push_plain(line, &text[last..at], style);
            let i = ch as usize - 0xe000;
            if let Some(Some(math)) = found.math.get(i) {
                line.push(Span {
                    style,
                    width: math.width(),
                    math: Some(math.clone()),
                    ..Default::default()
                });
            } else if let Some(tex) = found.tex.get(i) {
                push_plain(line, &format!("$${tex}$$"), style);
            }
            last = at + ch.len_utf8();
        }
        if last > 0 {
            if last < text.len() {
                push_plain(line, &text[last..], style);
            }
            return;
        }
    }
    push_plain(line, text, style);
}

fn push_icons(line: &mut Vec<Span>, text: &str, style: u32, measurer: &dyn TextMeasurer, marks: Option<&Marks>) {
    if !text.contains(":fa-") {
        push_text(line, text, style, marks);
        return;
    }
    let mut last = 0;
    for m in re_icon().find_iter(text) {
        push_text(line, &text[last..m.start()], style, marks);
        last = m.end();
        let markup = measurer.icon_markup(m.as_str());
        if !markup.is_empty() {
            line.push(Span {
                style,
                icon: Some(m.as_str().to_string()),
                icon_markup: Some(markup),
                icon_resolver: measurer.icon_resolver(),
                ..Default::default()
            });
        } else if text[last..].starts_with(' ') {
            last += 1;
        } else if last == text.len()
            && let Some(before) = line.last_mut().filter(|s| s.plain())
        {
            before.text = trim_end(&before.text).to_string();
        }
    }
    push_text(line, &text[last..], style, marks);
}

struct Open {
    style: u32,
    // Sub/sup stack; only the innermost tag shifts text.
    shifts: Vec<u32>,
}

fn is_ascii_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

// Splits inline HTML and, for markdown labels, emphasis markers into styled spans.
fn inline(
    line: &mut Vec<Span>,
    source: &str,
    markdown: bool,
    base: u32,
    open: &mut Open,
    measurer: &dyn TextMeasurer,
    marks: Option<&Marks>,
) {
    let mut style = open.style;
    let mut marked = 0u32;
    let mut text = String::new();
    let n = source.len();
    let mut i = 0;
    let flush = |line: &mut Vec<Span>, text: &mut String, style: u32, open: &Open| {
        push_icons(line, text, style | open.shifts.last().copied().unwrap_or(0), measurer, marks);
        text.clear();
    };
    while i < n {
        let ch = source[i..].chars().next().unwrap();
        if ch == '<' {
            if let Some(m) = re_tag().captures(&source[i..]) {
                let whole = m.get(0).unwrap().as_str();
                let tag = m.get(1).unwrap().as_str().to_ascii_lowercase();
                let closing = source[i + 1..].starts_with('/');
                let bit = tag_style(&tag);
                if bit & SHIFT != 0 {
                    flush(line, &mut text, style, open);
                    let at = open.shifts.iter().rposition(|&s| s == bit);
                    if !closing {
                        open.shifts.push(bit);
                    } else if let Some(at) = at {
                        open.shifts.remove(at);
                    }
                } else if bit != 0 {
                    flush(line, &mut text, style, open);
                    open.style = if closing {
                        (open.style & !bit) | (base & bit)
                    } else {
                        open.style | bit
                    };
                    style = if closing { (style & !bit) | (base & bit) } else { style | bit };
                }
                // Other HTML tags are dropped and their text kept. Anything else shaped like a
                // tag, such as the type in List<T>, is text.
                if bit != 0 || html_tags().contains(tag.as_str()) {
                    i += whole.len();
                    continue;
                }
            }
        } else if markdown && (ch == '*' || ch == '_') {
            let double = source[i + 1..].starts_with(ch);
            let bit = if double { BOLD } else { ITALIC };
            let len = if double { 2 } else { 1 };
            let opening = marked & bit == 0;
            let prev = if i > 0 { source[..i].chars().next_back().unwrap() } else { ' ' };
            let next = if i + len < n {
                source[i + len..].chars().next().unwrap()
            } else {
                ' '
            };
            let needle: String = if double { [ch, ch].iter().collect() } else { ch.to_string() };
            let can_open = opening && !is_js_space(next) && source[i + len..].contains(needle.as_str());
            let can_close = !opening && !is_js_space(prev);
            let wordy = ch == '_' && is_ascii_word(prev) && is_ascii_word(next);
            if (can_open || can_close) && !wordy {
                flush(line, &mut text, style, open);
                marked = if can_open { marked | bit } else { marked & !bit };
                style = if can_open { style | bit } else { (style & !bit) | (base & bit) };
                i += len;
                continue;
            }
        }
        text.push(ch);
        i += ch.len_utf8();
    }
    flush(line, &mut text, style, open);
}

fn measure_spans(line: &mut [Span], measurer: &dyn TextMeasurer, size: f64) -> f64 {
    let mut total = 0.0;
    for span in line.iter_mut() {
        span.width = if span.icon.is_some() {
            size * 1.25
        } else if let Some(math) = &span.math {
            math.width()
        } else {
            measurer.width(&span.text, size, span.style)
        };
        total += span.width;
    }
    total
}

struct Wrapping<'a> {
    measurer: &'a dyn TextMeasurer,
    size: f64,
    current: Vec<Span>,
    width: f64,
}

impl Wrapping<'_> {
    fn commit(&mut self, hyphen: bool, out: &mut Vec<Vec<Span>>, widths: &mut Vec<f64>) {
        if let Some(last) = self.current.last_mut().filter(|s| s.plain()) {
            let mut trimmed = trim_end(&last.text).to_string();
            if hyphen && last.text.ends_with('\u{ad}') {
                trimmed.pop();
                trimmed.push('-');
            }
            if trimmed != last.text {
                let w = self.measurer.width(&trimmed, self.size, last.style);
                self.width += w - last.width;
                last.text = trimmed;
                last.width = w;
            }
        }
        out.push(std::mem::take(&mut self.current));
        widths.push(self.width);
        self.width = 0.0;
    }
}

fn wrap(line: &[Span], measurer: &dyn TextMeasurer, size: f64, max: f64, out: &mut Vec<Vec<Span>>, widths: &mut Vec<f64>) {
    let mut w = Wrapping {
        measurer,
        size,
        current: Vec::new(),
        width: 0.0,
    };
    for (si, span) in line.iter().enumerate() {
        if !span.plain() {
            let width = span.math.as_ref().map_or(size * 1.25, |m| m.width());
            if w.width > 0.0 && w.width + width > max {
                w.commit(false, out, widths);
            }
            let mut placed = span.clone();
            placed.width = width;
            w.current.push(placed);
            w.width += width;
            continue;
        }
        let pieces: Vec<&str> = re_piece().find_iter(&span.text).map(|m| m.as_str()).collect();
        // A piece ending in a soft hyphen fits if its '-' does, or if the rest of the word does,
        // which hides the hyphen.
        let soft = span.text.contains('\u{ad}');
        let n = if soft { pieces.len() } else { 0 };
        let mut own = vec![0.0f64; n];
        let mut rest = vec![0.0f64; n + 1];
        for q in (0..n).rev() {
            // A soft hyphen before a space is no break of its own: the space is.
            let shy = pieces[q].ends_with('\u{ad}');
            let end = trim_end(pieces[q]);
            let measured = if shy { &end[..end.len() - '\u{ad}'.len_utf8()] } else { end };
            own[q] = measurer.width(measured, size, span.style);
            // Word width beyond this span is unknown.
            rest[q] = if !shy {
                own[q]
            } else if q == n - 1 {
                if si == line.len() - 1 { own[q] } else { f64::INFINITY }
            } else {
                own[q] + rest[q + 1]
            };
        }
        for (p, &piece) in pieces.iter().enumerate() {
            let mut part = piece;
            let part_width = if soft {
                own[p]
            } else {
                measurer.width(trim_end(part), size, span.style)
            };
            let fits = if !soft || !part.ends_with('\u{ad}') {
                w.width + part_width <= max
            } else {
                let hyphened = format!("{}-", &part[..part.len() - '\u{ad}'.len_utf8()]);
                w.width + measurer.width(&hyphened, size, span.style) <= max || w.width + rest[p] <= max
            };
            if w.width > 0.0 && !fits {
                // Breaking at whitespace leaves a preceding soft hyphen hidden.
                w.commit(trim_start(part) == part, out, widths);
                part = trim_start(part);
            }
            match w.current.last_mut() {
                Some(last) if last.plain() && last.style == span.style => {
                    last.text.push_str(part);
                    // Remeasuring for each zero-width piece would take quadratic time.
                    if part_width > 0.0 || trim_end(part) != part {
                        let before = last.width;
                        last.width = measurer.width(&last.text, size, last.style);
                        w.width += last.width - before;
                    }
                }
                _ => {
                    let width = measurer.width(part, size, span.style);
                    w.current.push(Span {
                        text: part.to_string(),
                        style: span.style,
                        width,
                        ..Default::default()
                    });
                    w.width += width;
                }
            }
        }
    }
    if !w.current.is_empty() || out.is_empty() {
        w.commit(false, out, widths);
    }
}

pub fn layout_label(raw: Option<&str>, markdown: bool, measurer: &dyn TextMeasurer, size: f64, max_width: f64, base: u32) -> Label {
    let raw = match raw {
        Some(r) if !r.is_empty() => r,
        _ => return Label::default(),
    };
    let line_height = js_round(size * LINE_HEIGHT);

    if measurer.math_active() && raw.contains("$$") && !raw.chars().any(|c| (0xe000..=0xf8ff).contains(&(c as u32))) {
        static RE_MATH: OnceLock<Regex> = OnceLock::new();
        let re = RE_MATH.get_or_init(|| Regex::new(r"\$\$([^\n\r\u{2028}\u{2029}]+?)\$\$").unwrap());
        let mut found = Marks {
            tex: Vec::new(),
            math: Vec::new(),
        };
        let marked = re.replace_all(raw, |c: &regex::Captures| {
            if found.tex.len() == 0x1900 {
                return c[0].to_string();
            }
            found.tex.push(c[1].to_string());
            char::from_u32(0xe000 + found.tex.len() as u32 - 1).unwrap().to_string()
        });
        if !found.tex.is_empty() {
            for tex in &found.tex {
                found.math.push(measurer.typeset(&decode_entities(tex), size));
            }
            return with_heights(layout_lines(&marked, markdown, measurer, size, max_width, base, Some(&found)));
        }
    }

    if !re_markup().is_match(raw) && !raw.contains('&') && !raw.contains('ﬂ') {
        let text = trim(raw);
        let width = measurer.width(text, size, base);
        if width <= max_width || !(text.contains(' ') || re_cjk().is_match(text) || re_hint().is_match(text)) {
            return Label {
                lines: vec![vec![Span {
                    text: text.to_string(),
                    style: base,
                    width,
                    ..Default::default()
                }]],
                widths: vec![width],
                width,
                height: line_height,
                size,
                line_height,
            };
        }
        let mut lines = Vec::new();
        let mut widths = Vec::new();
        wrap(
            &[Span {
                text: text.to_string(),
                style: base,
                width,
                ..Default::default()
            }],
            measurer,
            size,
            max_width,
            &mut lines,
            &mut widths,
        );
        let mut widest: f64 = 0.0;
        for &w in &widths {
            if w > widest {
                widest = w;
            }
        }
        let snug = if lines.len() > 1 {
            js_round(size * WRAPPED_LINE_HEIGHT)
        } else {
            line_height
        };
        let height = lines.len() as f64 * snug;
        return Label {
            lines,
            widths,
            width: widest,
            height,
            size,
            line_height: snug,
        };
    }
    layout_lines(raw, markdown, measurer, size, max_width, base, None)
}

fn layout_lines(
    raw: &str,
    markdown: bool,
    measurer: &dyn TextMeasurer,
    size: f64,
    max_width: f64,
    base: u32,
    marks: Option<&Marks>,
) -> Label {
    let source = if markdown {
        trim(&dedent(&raw.replace("\r\n", "\n").replace('\r', "\n"))).to_string()
    } else {
        trim(raw).to_string()
    };
    let mut lines: Vec<Vec<Span>> = Vec::new();
    let mut widths: Vec<f64> = Vec::new();
    let mut width: f64 = 0.0;
    // HTML tags can span explicit breaks; Markdown emphasis stays local to each line.
    let mut open = Open {
        style: base,
        shifts: Vec::new(),
    };
    for part in re_break().split(&source) {
        let mut line = Vec::new();
        inline(&mut line, trim(part), markdown, base, &mut open, measurer, marks);
        let w = measure_spans(&mut line, measurer, size);
        if w > max_width {
            wrap(&line, measurer, size, max_width, &mut lines, &mut widths);
        } else {
            lines.push(line);
            widths.push(w);
        }
    }
    for &w in &widths {
        if w > width {
            width = w;
        }
    }
    let snug = if lines.len() > 1 {
        js_round(size * WRAPPED_LINE_HEIGHT)
    } else {
        js_round(size * LINE_HEIGHT)
    };
    let height = lines.len() as f64 * snug;
    Label {
        lines,
        widths,
        width,
        height,
        size,
        line_height: snug,
    }
}

pub fn line_height(label: &Label, i: usize) -> f64 {
    label.lines[i]
        .iter()
        .filter_map(|s| s.math.as_ref())
        .fold(label.line_height, |h, m| h.max((m.height() + label.size / 4.0).ceil()))
}
pub fn with_heights(mut label: Label) -> Label {
    label.height = (0..label.lines.len()).map(|i| line_height(&label, i)).sum();
    label
}
pub fn has_math(line: &[Span]) -> bool {
    line.iter().any(|s| s.math.is_some())
}
pub fn line_middle(label: &Label, i: usize) -> f64 {
    (0..i).map(|k| line_height(label, k)).sum::<f64>() + line_height(label, i) / 2.0
}
pub fn label_line(label: &Label, i: usize) -> Label {
    Label {
        lines: vec![label.lines[i].clone()],
        widths: vec![label.widths[i]],
        width: label.widths[i],
        height: line_height(label, i),
        size: label.size,
        line_height: label.line_height,
    }
}
