use crate::diagrams::common::{Rendered, turn_to_fit};
use crate::diagrams::timeline::story::{self, fit, plain};
use crate::preprocess::Config;
use crate::svg::builder::{esc_text, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{ROUNDED, series_color};
use crate::text::label::Label;
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{js_num, js_number, js_round, num, trim};
use crate::{PeleError, RenderOptions};
#[derive(Clone, Debug)]
pub struct Task {
    pub task: String,
    pub score: f64,
    pub people: Vec<String>,
    pub section: String,
}
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub sections: Vec<String>,
    pub tasks: Vec<Task>,
}
pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let tokens = story::tokenize(source, false);
    let mut model = Model {
        title: title.map(str::to_string),
        ..Default::default()
    };
    if tokens[0].kind != "journey" {
        return Err(story::fail("journey", source, &tokens[0], "'journey'"));
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
            "taskName" => {
                i += 1;
                if tokens[i].kind != "taskData" {
                    return Err(story::fail("journey", source, &tokens[i], "'taskData'"));
                }
                let pieces: Vec<&str> = tokens[i].text[1..].split(':').collect();
                model.tasks.push(Task {
                    task: t.text.clone(),
                    score: js_number(pieces[0]),
                    people: pieces
                        .get(1)
                        .map(|s| s.split(',').map(|p| trim(p).into()).collect())
                        .unwrap_or_default(),
                    section: section.clone(),
                })
            }
            "EOF" => return Ok(model),
            _ => {
                return Err(story::fail(
                    "journey",
                    source,
                    t,
                    "'EOF', 'SPACE', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'section', 'taskName'",
                ));
            }
        }
        i += 1
    }
    Ok(model)
}
pub fn actors(tasks: &[Task]) -> Vec<String> {
    let mut names: Vec<String> = tasks.iter().flat_map(|t| t.people.iter().cloned()).collect();
    names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    names.dedup();
    names
}
fn face(x: f64, y: f64, score: f64) -> String {
    let mood = if score > 3.0 {
        "happy"
    } else if score < 3.0 {
        "sad"
    } else {
        "neutral"
    };
    let mouth = match mood {
        "happy" => "M-5,2A6,6 0 0 0 5,2",
        "sad" => "M-5,6.5A6,6 0 0 1 5,6.5",
        _ => "M-4.5,4.5H4.5",
    };
    format!(
        "<g class=\"pele-face pele-face-{mood}\" transform=\"translate({},{})\"><circle r=\"11\" fill=\"var(--_bg)\" stroke=\"var(--_l)\"/><g fill=\"var(--_l)\"><circle cx=\"-3.6\" cy=\"-2.8\" r=\"1.3\"/><circle cx=\"3.6\" cy=\"-2.8\" r=\"1.3\"/></g><path d=\"{mouth}\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\"/></g>",
        num(x),
        num(y)
    )
}
pub fn render(model: &Model, _config: &Config, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    turn_to_fit(o, |down| if down { vertical(model, o, m) } else { draw(model, o, m) })
}
fn finish(model: &Model, o: &RenderOptions, size: f64, width: f64, height: f64, content: String) -> Rendered {
    Rendered {
        svg: svg_document(
            "journey",
            width,
            height,
            size,
            o,
            &Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            &content,
        ),
        width,
        height,
        links: Vec::new(),
    }
}
fn draw(model: &Model, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let tiny = js_round(size * 0.75);
    let pad = o.padding.unwrap_or(8.0);
    let tasks = &model.tasks;
    let n = tasks.len();
    let names: Vec<String> = actors(tasks).into_iter().filter(|s| !s.is_empty()).collect();
    let actor = |name: &str| names.iter().position(|s| s == name).unwrap_or(0) as i64;
    let mut text_w = 124.0;
    let mut labels: Vec<Label> = tasks.iter().map(|t| plain(Some(&t.task), m, small, text_w, 0)).collect();
    let widest = labels.iter().map(|l| l.width).fold(0.0_f64, f64::max);
    if widest > text_w {
        text_w = widest.ceil().min(200.0);
        labels = tasks.iter().map(|t| fit(Some(&t.task), m, small, text_w, 0)).collect()
    }
    let card_w = text_w + 20.0;
    let pitch = card_w + 12.0;
    let plot_w = if n > 0 { n as f64 * pitch - 12.0 } else { 0.0 };
    let line_h = js_round(small * 1.5);
    let limit = plot_w.max(240.0);
    struct Item {
        name: String,
        label: Label,
        x: f64,
        y: f64,
        h: f64,
    }
    let mut legend: Vec<Item> = Vec::new();
    let mut legend_w = 0.0_f64;
    let mut legend_h = 0.0_f64;
    let mut lx = 0.0;
    for name in &names {
        let label = fit(Some(name), m, small, limit - 16.0, 0);
        let w = 16.0 + label.width;
        if lx > 0.0 && lx + w > limit {
            lx = 0.0;
            legend_h += 4.0
        }
        let y = if lx > 0.0 { legend.last().unwrap().y } else { legend_h };
        let h = label.height.max(line_h);
        legend.push(Item {
            name: name.clone(),
            label,
            x: lx,
            y,
            h,
        });
        lx += w;
        legend_w = legend_w.max(lx);
        legend_h = legend_h.max(y + h);
        lx += 20.0
    }
    let body_w = if n > 0 { 24.0 + plot_w } else { 0.0 };
    let title = fit(model.title.as_deref(), m, size, body_w.max(320.0), BOLD);
    let title_h = if title.height > 0.0 { title.height + 16.0 } else { 0.0 };
    let width = body_w.max(title.width).max(if n > 0 { 24.0 + legend_w } else { 0.0 });
    let left = pad + (width - body_w) / 2.0 + 24.0;
    let top = pad + title_h;
    let mut runs: Vec<(usize, usize, Option<Label>)> = Vec::new();
    let mut band_h = 0.0_f64;
    let mut i = 0;
    while i < n {
        let mut end = i + 1;
        while end < n && tasks[end].section == tasks[i].section {
            end += 1
        }
        let label = if !tasks[i].section.is_empty() {
            let l = fit(Some(&tasks[i].section), m, small, (end - i) as f64 * pitch - 32.0, BOLD);
            band_h = band_h.max(l.height.max(line_h) + 12.0);
            Some(l)
        } else {
            None
        };
        runs.push((i, end - i, label));
        i = end
    }
    let text_h = labels.iter().map(|l| l.height).fold(line_h, f64::max);
    let dots_h = if !names.is_empty() { 16.0 } else { 0.0 };
    let card_h = text_h + dots_h + 16.0;
    let cards_y = top + if band_h > 0.0 { band_h + 6.0 } else { 0.0 };
    let chart_top = cards_y + card_h + 27.0;
    let chart_bottom = chart_top + 104.0;
    let right = left + plot_w;
    let mut body = String::new();
    let mut bottom = top;
    if n > 0 {
        let mut grid = String::new();
        let mut scale = String::new();
        for score in (1..=5).rev() {
            let y = chart_top + (5 - score) as f64 * 26.0;
            grid += &format!("M{},{}H{}", num(left - 6.0), num(y), num(right));
            scale += &format!("<text x=\"{}\" y=\"{}\">{score}</text>", num(left - 12.0), num(y + tiny * 0.35));
        }
        body += &format!(
            "<path class=\"pele-grid\" d=\"{grid}\" fill=\"none\" stroke=\"var(--_a)\"/><g class=\"pele-scale\" font-size=\"{}\" text-anchor=\"end\" fill=\"var(--_m)\">{scale}</g>",
            js_num(tiny)
        );
        let mut line = String::new();
        let mut stems = String::new();
        let mut faces = String::new();
        let mut cards = Vec::new();
        for (i, t) in tasks.iter().enumerate() {
            let x = left + i as f64 * pitch;
            let cx = x + card_w / 2.0;
            let level = if t.score.is_nan() { 3.0 } else { t.score.max(1.0).min(5.0) };
            let y = chart_top + (5.0 - level) * 26.0;
            line += &format!("{}{},{}", if i == 0 { "M" } else { "L" }, num(cx), num(y));
            stems += &format!("M{},{}V{}", num(cx), num(cards_y + card_h), num(y - 11.0));
            faces += &face(cx, y, t.score);
            let people: Vec<&String> = t.people.iter().filter(|s| !s.is_empty()).collect();
            let step = 13.0_f64.min((card_w - 30.0) / people.len().saturating_sub(1).max(1) as f64);
            let mut dots = String::new();
            for (k, name) in people.iter().enumerate() {
                dots += &format!(
                    "<circle class=\"pele-actor\" cx=\"{}\" cy=\"{}\" r=\"5\" fill=\"{}\" stroke=\"var(--_s)\"><title>{}</title></circle>",
                    num(cx + (k as f64 - (people.len() as f64 - 1.0) / 2.0) * step),
                    num(cards_y + card_h - 13.0),
                    series_color(actor(name)),
                    esc_text(name)
                )
            }
            cards.push(format!("<g class=\"pele-node pele-task\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{ROUNDED}/>",esc_text(trim(&t.task)),num(x),num(cards_y),num(card_w),num(card_h))+&label_svg(&labels[i],cx,cards_y+8.0+text_h/2.0,&format!(" font-size=\"{}\"",js_num(small)))+&dots+"</g>");
        }
        body += &format!("<path class=\"pele-stems\" d=\"{stems}\" fill=\"none\" stroke=\"var(--_b)\" stroke-dasharray=\"2 3\"/>");
        if n > 1 {
            body += &format!("<path class=\"pele-edge\" d=\"{line}\" fill=\"none\" stroke=\"var(--_l)\" stroke-linejoin=\"round\"/>")
        }
        body += &faces;
        for (start, count, label) in runs {
            let members = cards[start..start + count].join("");
            let Some(label) = label else {
                body += &members;
                continue;
            };
            let x = left + start as f64 * pitch;
            let w = count as f64 * pitch - 12.0;
            body += &(format!(
                "<g class=\"pele-cluster pele-section\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\"{ROUNDED}/>",
                esc_text(trim(&tasks[start].section)),
                num(x),
                num(top),
                num(w),
                num(band_h)
            ) + &label_svg(
                &label,
                x + w / 2.0,
                top + band_h / 2.0,
                &format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", js_num(small)),
            ) + &members
                + "</g>");
        }
        bottom = chart_bottom + 11.0;
        if !legend.is_empty() {
            let x0 = left + (plot_w - legend_w) / 2.0;
            let y0 = bottom + 16.0;
            let mut items = String::new();
            for it in &legend {
                let cy = y0 + it.y + it.h / 2.0;
                items += &(format!(
                    "<g class=\"pele-legend-item\" data-id=\"{}\"><circle cx=\"{}\" cy=\"{}\" r=\"5\" fill=\"{}\"/>",
                    esc_text(&it.name),
                    num(x0 + it.x + 5.0),
                    num(cy),
                    series_color(actor(&it.name))
                ) + &label_svg(&it.label, x0 + it.x + 16.0 + it.label.width / 2.0, cy, "")
                    + "</g>")
            }
            body += &format!("<g class=\"pele-legend\" font-size=\"{}\">{items}</g>", js_num(small));
            bottom = y0 + legend_h
        }
    }
    let tw = (width + 2.0 * pad).ceil();
    let th = (bottom.max(pad) + pad).ceil();
    let content = label_svg(
        &title,
        pad + width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &body;
    finish(model, o, size, tw, th, content)
}
fn vertical(model: &Model, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.875);
    let pad = o.padding.unwrap_or(8.0);
    let room = 140.0_f64.max(o.max_width.unwrap_or(320.0) - 2.0 * pad);
    let title = fit(model.title.as_deref(), m, size, room, BOLD);
    let names: Vec<String> = actors(&model.tasks).into_iter().filter(|s| !s.is_empty()).collect();
    let mut y = pad + if title.height > 0.0 { title.height + 16.0 } else { 0.0 };
    let mut section: Option<&str> = None;
    let mut body = String::new();
    let mut width = title.width;
    for (index, t) in model.tasks.iter().enumerate() {
        if section != Some(&t.section) {
            section = Some(&t.section);
            let head = fit(section, m, size, room - 16.0, BOLD);
            if head.height > 0.0 {
                body += &label_svg(
                    &head,
                    pad + 8.0 + head.width / 2.0,
                    y + head.height / 2.0,
                    " class=\"pele-cluster-label\" font-weight=\"var(--_hw)\"",
                );
                width = width.max(head.width + 16.0);
                y += head.height + 8.0
            }
        }
        let label = fit(Some(&t.task), m, small, room - 72.0, 0);
        let people: Vec<(&str, Label)> = t
            .people
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| (s.as_str(), fit(Some(s), m, small, room - 44.0, 0)))
            .collect();
        let text_h = label.height.max(42.0);
        let h = 16.0 + text_h + people.iter().map(|(_, l)| l.height.max(16.0) + 4.0).sum::<f64>();
        let w = room.max(label.width + 72.0);
        body += &format!(
            "<g class=\"pele-node pele-journey-step\" data-id=\"{index}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{ROUNDED}/>",
            num(pad),
            num(y),
            num(w),
            num(h)
        );
        body += &label_svg(
            &label,
            pad + 10.0 + label.width / 2.0,
            y + 8.0 + label.height / 2.0,
            " class=\"pele-label\"",
        );
        let cx = pad + w - 28.0;
        body += &face(cx, y + 19.0, t.score);
        if !t.score.is_nan() {
            body += &format!(
                "<text class=\"pele-journey-score\" x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"middle\">{}</text>",
                num(cx),
                num(y + 30.0 + small),
                js_num(small),
                num(t.score.max(1.0).min(5.0))
            )
        }
        let mut py = y + 12.0 + text_h;
        for (name, l) in people {
            let ph = l.height.max(16.0);
            body += &format!(
                "<circle class=\"pele-actor\" cx=\"{}\" cy=\"{}\" r=\"5\" fill=\"{}\"/>",
                num(pad + 16.0),
                num(py + ph / 2.0),
                series_color(names.iter().position(|s| s == name).unwrap_or(0) as i64)
            );
            body += &label_svg(
                &l,
                pad + 28.0 + l.width / 2.0,
                py + ph / 2.0,
                &format!(" class=\"pele-journey-actor\" font-size=\"{}\"", js_num(small)),
            );
            py += ph + 4.0
        }
        body += "</g>";
        y += h;
        if index + 1 < model.tasks.len() {
            body += &format!(
                "<path class=\"pele-edge\" d=\"M{},{}v16\" fill=\"none\" stroke=\"var(--_l)\"/>",
                num(pad + w / 2.0),
                num(y)
            )
        }
        y += 16.0;
        width = width.max(w);
    }
    let tw = (width + 2.0 * pad).ceil();
    let h = ((pad + title.height).max(y - if model.tasks.is_empty() { 0.0 } else { 16.0 }) + pad).ceil();
    let content = label_svg(
        &title,
        tw / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &body;
    finish(model, o, size, tw, h, content)
}
