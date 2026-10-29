pub(crate) mod story;
use crate::diagrams::common::{Rendered, turn_to_fit};
use crate::errors::PeleErrorCode;
use crate::preprocess::Config;
use crate::svg::builder::{esc_text, label_svg};
use crate::svg::edges::{ARROW, marker};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{rounded, series_color};
use crate::text::label::Label;
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{js_num, js_round, num, trim};
use crate::{PeleError, RenderOptions};
use story::{fit, plain};
#[derive(Clone, Debug)]
pub struct Period {
    pub text: String,
    pub events: Vec<String>,
    pub section: String,
    pub section_index: isize,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub direction: String,
    pub sections: Vec<String>,
    pub periods: Vec<Period>,
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let tokens = story::tokenize(source, true);
    let mut model = Model {
        title: title.map(str::to_string),
        direction: "LR".into(),
        ..Default::default()
    };
    match tokens[0].kind {
        "timeline" | "timeline_lr" => {}
        "timeline_td" => model.direction = "TD".into(),
        _ => {
            return Err(story::fail(
                "timeline",
                source,
                &tokens[0],
                "'timeline', 'timeline_lr', 'timeline_td'",
            ));
        }
    }
    let mut section = String::new();
    let mut i = 1;
    while i < tokens.len() {
        let t = &tokens[i];
        match t.kind {
            "NEWLINE" => {}
            "title" => model.title = Some(story::substring(&t.text, 6)),
            "acc_title" => {
                i += 1;
                model.acc_title = Some(story::acc(trim(&tokens[i].text)))
            }
            "acc_descr" => {
                i += 1;
                model.acc_descr = Some(story::squeeze(trim(&tokens[i].text)))
            }
            "acc_descr_multiline_value" => model.acc_descr = Some(story::squeeze(trim(&t.text))),
            "section" => {
                section = story::substring(&t.text, 8);
                model.sections.push(section.clone())
            }
            "period" => model.periods.push(Period {
                text: t.text.clone(),
                events: Vec::new(),
                section: section.clone(),
                section_index: model.sections.len() as isize - 1,
            }),
            "event" => {
                let p = model
                    .periods
                    .last_mut()
                    .ok_or_else(|| PeleError::new("An event needs a time period before it.", PeleErrorCode::Semantic).of("timeline"))?;
                p.events.push(story::substring(&t.text, 2))
            }
            "EOF" => return Ok(model),
            _ => {
                return Err(story::fail(
                    "timeline",
                    source,
                    t,
                    "'EOF', 'SPACE', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'period', 'event'",
                ));
            }
        }
        i += 1
    }
    Ok(model)
}
fn rect(x: f64, y: f64, w: f64, h: f64, attrs: &str) -> String {
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
        num(x),
        num(y),
        num(w),
        num(h),
        rounded(attrs)
    )
}
pub fn render(model: &Model, config: &Config, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    if model.direction == "TD" {
        draw(model, config, o, m, true)
    } else {
        turn_to_fit(o, |down| draw(model, config, o, m, down))
    }
}
fn draw(model: &Model, config: &Config, o: &RenderOptions, m: &dyn TextMeasurer, vertical: bool) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let pad = o.padding.unwrap_or(8.0);
    let multicolor = config
        .get("timeline")
        .and_then(|v| v.get("disableMulticolor"))
        .and_then(|v| v.as_bool())
        != Some(true);
    let periods = &model.periods;
    let has_sections = !model.sections.is_empty();
    let mut groups: Vec<(isize, Vec<usize>)> = model.sections.iter().enumerate().map(|(i, _)| (i as isize, Vec::new())).collect();
    let mut leading = Vec::new();
    for (i, p) in periods.iter().enumerate() {
        if p.section_index >= 0 && (p.section_index as usize) < groups.len() {
            groups[p.section_index as usize].1.push(i)
        } else {
            leading.push(i)
        }
    }
    if !leading.is_empty() {
        groups.insert(0, (-1, leading));
    }
    let room = if vertical {
        o.max_width.filter(|x| *x > 0.0).map(|x| x - 2.0 * pad).unwrap_or(f64::INFINITY)
    } else {
        f64::INFINITY
    };
    let taken = 96.0;
    let period_text = 152.0_f64.min(80.0_f64.max(js_round((room - taken) * 0.38)));
    let event_text = 264.0_f64.min(110.0_f64.max(room - taken - period_text));
    let mut text_w = period_text;
    let mut period_labels: Vec<Label> = periods.iter().map(|p| plain(Some(&p.text), m, size, text_w, 0)).collect();
    let mut event_labels: Vec<Vec<Label>> = periods
        .iter()
        .map(|p| {
            p.events
                .iter()
                .map(|e| plain(Some(e), m, small, if vertical { event_text } else { text_w }, 0))
                .collect()
        })
        .collect();
    let mut widest = 0.0_f64;
    for l in &period_labels {
        widest = widest.max(l.width)
    }
    if !vertical {
        for ls in &event_labels {
            for l in ls {
                widest = widest.max(l.width)
            }
        }
    }
    let overflow = widest > text_w || (vertical && event_labels.iter().flatten().any(|l| l.width > event_text));
    if overflow {
        text_w = widest
            .ceil()
            .max(period_text)
            .min(if room == f64::INFINITY { 216.0 } else { period_text });
        period_labels = periods.iter().map(|p| fit(Some(&p.text), m, size, text_w, 0)).collect();
        event_labels = periods
            .iter()
            .map(|p| {
                p.events
                    .iter()
                    .map(|e| fit(Some(e), m, small, if vertical { event_text } else { text_w }, 0))
                    .collect()
            })
            .collect();
    }
    let lh = js_round(size * 1.5);
    let col_w = text_w + 24.0;
    let event_w = if vertical { event_text } else { text_w } + 24.0;
    let chip_h = |l: &Label| l.height.max(lh) + 16.0;
    let event_h = |l: &Label| l.height.max(js_round(small * 1.5)) + 16.0;
    let color_of = |g: isize, p: usize| {
        if g >= 0 {
            series_color(g as i64)
        } else if has_sections {
            "var(--_m)".into()
        } else {
            series_color(if multicolor { p as i64 } else { 0 })
        }
    };
    let has_events = event_labels.iter().any(|l| !l.is_empty());
    let slots: usize = groups.iter().map(|g| g.1.len().max(1)).sum();
    let pitch = col_w + 20.0;
    let drawn = !periods.is_empty();
    let body_w = if vertical {
        col_w
            + if has_events {
                48.0 + event_w
            } else if drawn {
                32.0
            } else {
                0.0
            }
    } else if slots > 0 {
        slots as f64 * pitch - 20.0 + if drawn { 12.0 + ARROW } else { 0.0 }
    } else {
        0.0
    };
    let title = fit(model.title.as_deref(), m, size, body_w.max(320.0), BOLD);
    let title_h = if title.height > 0.0 { title.height + 16.0 } else { 0.0 };
    let width = body_w.max(title.width);
    let left = pad + (width - body_w) / 2.0;
    let top = pad + title_h;
    let section_label = |g: isize, w: f64| fit(model.sections.get(g as usize).map(String::as_str), m, size, w - 24.0, BOLD);
    let header = |g: isize, l: &Label, x: f64, y: f64, w: f64, h: f64| {
        rect(x, y, w, h, &format!(" fill=\"{}\"", series_color(g as i64)))
            + &label_svg(l, x + w / 2.0, y + h / 2.0, " font-weight=\"var(--_hw)\" fill=\"var(--_bg)\"")
    };
    let period = |i: usize, color: &str, x: f64, y: f64, h: f64| {
        rect(x, y, col_w, h, &format!(" fill=\"{color}\""))
            + &label_svg(&period_labels[i], x + col_w / 2.0, y + h / 2.0, " fill=\"var(--_bg)\"")
    };
    let event = |l: &Label, color: &str, x: f64, y: f64, h: f64| {
        "<g class=\"pele-node pele-event\">".to_string()
            + &rect(x, y, event_w, h, &format!(" fill=\"{color}\" fill-opacity=\"0.16\""))
            + &label_svg(l, x + event_w / 2.0, y + h / 2.0, &format!(" font-size=\"{}\"", js_num(small)))
            + "</g>"
    };
    let dot = |x: f64, y: f64, color: &str| {
        format!(
            "<circle class=\"pele-dot\" cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"{color}\" stroke=\"var(--_bg)\" stroke-width=\"2\"/>",
            num(x),
            num(y)
        )
    };
    let open = |g: isize| {
        if g >= 0 {
            format!(
                "<g class=\"pele-cluster pele-section\" data-id=\"{}\">",
                esc_text(trim(&model.sections[g as usize]))
            )
        } else {
            String::new()
        }
    };
    let open_period = |i: usize| {
        format!(
            "<g class=\"pele-node pele-period\" data-id=\"{}\">",
            esc_text(trim(&periods[i].text))
        )
    };
    let mut body = String::new();

    let height = if vertical {
        let axis_x = left + col_w + 20.0;
        let spine_x = axis_x + 14.0;
        let events_x = spine_x + 14.0;
        let mut y = top;
        for (g, indices) in &groups {
            body += &open(*g);
            if *g >= 0 {
                let l = section_label(*g, body_w);
                let h = chip_h(&l);
                body += &header(*g, &l, left, y, body_w, h);
                y += h + 12.0
            }
            for &i in indices {
                let color = color_of(*g, i);
                let h = chip_h(&period_labels[i]);
                let first = event_labels[i].first().map(event_h).unwrap_or(h);
                let cy = y + h.max(first) / 2.0;
                let mut d = format!("M{},{}H{}", num(left + col_w), num(cy), num(axis_x));
                let mut cards = String::new();
                let mut ey = cy - first / 2.0;
                let mut last = cy;
                for l in &event_labels[i] {
                    let eh = event_h(l);
                    last = ey + eh / 2.0;
                    d += &format!("M{},{}H{}", num(spine_x), num(last), num(events_x));
                    cards += &event(l, &color, events_x, ey, eh);
                    ey += eh + 8.0
                }
                if !cards.is_empty() {
                    d += &format!("M{},{}H{}V{}", num(axis_x), num(cy), num(spine_x), num(last))
                }
                body += &(open_period(i)
                    + &period(i, &color, left, cy - h / 2.0, h)
                    + &format!("<path class=\"pele-edge\" d=\"{d}\" fill=\"none\" stroke=\"var(--_b)\"/>")
                    + &dot(axis_x, cy, &color)
                    + &cards
                    + "</g>");
                y = (cy + h / 2.0).max(ey - 8.0) + 20.0;
            }
            if indices.is_empty() {
                y += 8.0
            }
            if *g >= 0 {
                body += "</g>"
            }
        }
        if !groups.is_empty() {
            y -= 20.0
        }
        if drawn {
            let tip = y + 12.0 + ARROW;
            body = format!(
                "<path class=\"pele-axis\" d=\"M{},{}V{}\" fill=\"none\" stroke=\"var(--_l)\"/>",
                num(axis_x),
                num(top),
                num(tip - ARROW + 1.0)
            ) + &marker("arrow_point", axis_x, tip, 0.0, 1.0, "var(--_l)", "none")
                + &body;
            y = tip;
        }
        y + pad
    } else {
        let mut header_h = 0.0_f64;
        let section_labels: Vec<Option<Label>> = groups
            .iter()
            .map(|(g, indices)| {
                if *g < 0 {
                    None
                } else {
                    let l = section_label(*g, indices.len().max(1) as f64 * pitch - 20.0);
                    header_h = header_h.max(chip_h(&l));
                    Some(l)
                }
            })
            .collect();
        let chip = period_labels.iter().map(&chip_h).fold(0.0_f64, f64::max);
        let periods_y = top + if has_sections { header_h + 8.0 } else { 0.0 };
        let axis_y = periods_y + chip + if chip > 0.0 { 20.0 } else { 0.0 };
        let events_y = axis_y + 20.0;
        let mut bottom = if drawn { axis_y + 12.0 } else { top + header_h };
        let mut slot = 0;
        for (gi, (g, indices)) in groups.iter().enumerate() {
            let span = indices.len().max(1);
            let gx = left + slot as f64 * pitch;
            body += &open(*g);
            if let Some(l) = &section_labels[gi] {
                body += &header(*g, l, gx, top, span as f64 * pitch - 20.0, header_h)
            }
            for (k, &i) in indices.iter().enumerate() {
                let color = color_of(*g, i);
                let x = gx + k as f64 * pitch;
                let cx = x + col_w / 2.0;
                let mut d = format!(
                    "M{},{}V{}",
                    num(cx),
                    num(periods_y + chip),
                    num(if !event_labels[i].is_empty() { events_y } else { axis_y })
                );
                let mut cards = String::new();
                let mut ey = events_y;
                for l in &event_labels[i] {
                    let eh = event_h(l);
                    if ey > events_y {
                        d += &format!("M{},{}V{}", num(cx), num(ey - 8.0), num(ey))
                    }
                    cards += &event(l, &color, x, ey, eh);
                    ey += eh + 8.0
                }
                bottom = bottom.max(ey - 8.0);
                body += &(open_period(i)
                    + &period(i, &color, x, periods_y, chip)
                    + &format!("<path class=\"pele-edge\" d=\"{d}\" fill=\"none\" stroke=\"var(--_b)\"/>")
                    + &dot(cx, axis_y, &color)
                    + &cards
                    + "</g>");
            }
            if *g >= 0 {
                body += "</g>"
            }
            slot += span;
        }
        if drawn {
            let tip = left + body_w;
            body = format!(
                "<path class=\"pele-axis\" d=\"M{},{}H{}\" fill=\"none\" stroke=\"var(--_l)\"/>",
                num(left),
                num(axis_y),
                num(tip - ARROW + 1.0)
            ) + &marker("arrow_point", tip, axis_y, 1.0, 0.0, "var(--_l)", "none")
                + &body
        }
        bottom + pad
    };
    let total_width = (width + 2.0 * pad).ceil();
    let total_height = height.max(pad * 2.0).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &body;
    Rendered {
        svg: svg_document(
            "timeline",
            total_width,
            total_height,
            size,
            o,
            &Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            &content,
        ),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}
