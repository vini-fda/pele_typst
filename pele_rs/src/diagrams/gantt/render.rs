use super::{Model, Schedule, Task, axis, dates};
use crate::diagrams::common::Rendered;
use crate::errors::PeleError;
use crate::preprocess::Config;
use crate::svg::builder::{esc, esc_text, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{CRITICAL, ROUNDED, resolve_style};
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{js_round, num};
use crate::{LinkInfo, RenderOptions};
use indexmap::IndexMap;
use regex::Regex;
use std::sync::OnceLock;
fn cap_width(options: &RenderOptions, natural: f64, least: f64) -> f64 {
    options
        .max_width
        .filter(|&v| v > 0.0)
        .map_or(natural, |available| least.min(natural).max(natural.min(available)))
}
fn setting(config: &Config, key: &str, min: f64, max: f64) -> Option<f64> {
    config
        .get("gantt")?
        .get(key)?
        .as_f64()
        .filter(|v| v.is_finite())
        .map(|v| v.clamp(min, max))
}
fn text<'a>(config: &'a Config, key: &str) -> Option<&'a str> {
    config.get("gantt")?.get(key)?.as_str()
}
fn flag(config: &Config, key: &str) -> bool {
    config.get("gantt").and_then(|c| c.get(key)).and_then(|v| v.as_bool()) == Some(true)
}
fn one_line(raw: &str, measurer: &dyn TextMeasurer, size: f64) -> Label {
    static BREAK: OnceLock<Regex> = OnceLock::new();
    let re = BREAK.get_or_init(|| Regex::new(r"(?i)<br\s*/?>").unwrap());
    let mut label = layout_label(Some(&re.replace_all(raw, " ")), false, measurer, size, 4000.0, 0);
    if label.lines.len() > 1 {
        label.lines.truncate(1);
        label.widths.truncate(1);
        label.width = label.widths[0];
        label.height = label.line_height;
    }
    label
}
fn scheduled(task: &Task) -> bool {
    task.start_time.is_some_and(f64::is_finite) && task.end_time.is_some_and(f64::is_finite)
}
struct Bar {
    task: usize,
    label: Label,
    row: usize,
    x0: f64,
    x1: f64,
    label_x: f64,
    inside: bool,
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Result<Rendered, PeleError> {
    let size = options.font_size.unwrap_or(16.0);
    let pad = options.padding.unwrap_or(8.0);
    let now = dates::clock(options)?;
    let zone = dates::Zone::new(options.time_zone.as_deref())?;
    let label_size = setting(config, "fontSize", 4.0, 72.0).unwrap_or_else(|| js_round(size * 0.8125));
    let section_size = setting(config, "sectionFontSize", 4.0, 72.0).unwrap_or(label_size);
    let tick_size = js_round(size * 0.75);
    let bar_height = setting(config, "barHeight", 4.0, 400.0).unwrap_or_else(|| js_round(label_size * 1.85));
    let bar_gap = setting(config, "barGap", 0.0, 400.0).unwrap_or_else(|| js_round(bar_height / 4.0));
    let row_height = bar_height + bar_gap;
    let section_styles = js_round(setting(config, "numberSectionStyles", 1.0, 1000.0).unwrap_or(4.0)) as usize;
    let compact = model.display_mode == "compact";
    let top_axis = model.top_axis || flag(config, "topAxis");
    let mut schedule = Schedule::new(model, now, zone.clone())?;
    let tasks = schedule.model.tasks.clone();
    let row_tasks: Vec<_> = tasks.iter().enumerate().filter(|(_, t)| !t.vert).map(|(i, _)| i).collect();
    let mut verts: Vec<_> = tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.vert && scheduled(t))
        .map(|(i, _)| i)
        .collect();
    let mut categories = IndexMap::<String, usize>::new();
    for &i in &row_tasks {
        let n = categories.len();
        categories.entry(tasks[i].section.clone()).or_insert(n);
    }
    let mut row_of = vec![None; tasks.len()];
    let mut row_section = vec![];
    if compact {
        let mut groups = IndexMap::<String, Vec<usize>>::new();
        for &i in &row_tasks {
            groups.entry(tasks[i].section.clone()).or_default().push(i);
        }
        for (section, group) in groups {
            let offset = row_section.len();
            let mut sorted: Vec<_> = group.iter().copied().filter(|&i| scheduled(&tasks[i])).collect();
            sorted.sort_by(|&a, &b| {
                tasks[a]
                    .start_time
                    .unwrap()
                    .total_cmp(&tasks[b].start_time.unwrap())
                    .then(tasks[a].order.cmp(&tasks[b].order))
            });
            let mut free = vec![];
            for i in sorted {
                let start = tasks[i].start_time.unwrap();
                let row = free.iter().position(|&t| start >= t).unwrap_or(free.len());
                if row == free.len() {
                    free.push(tasks[i].end_time.unwrap());
                } else {
                    free[row] = tasks[i].end_time.unwrap();
                }
                row_of[i] = Some(offset + row);
            }
            let mut rows = free.len();
            for i in group {
                if row_of[i].is_none() {
                    row_of[i] = Some(offset + rows);
                    rows += 1;
                }
            }
            for _ in 0..rows.max(1) {
                row_section.push(section.clone());
            }
        }
    } else {
        for &i in &row_tasks {
            row_of[i] = Some(row_section.len());
            row_section.push(tasks[i].section.clone());
        }
    }
    let row_count = row_section.len();
    let mut start = f64::INFINITY;
    let mut stop = f64::NEG_INFINITY;
    for task in &tasks {
        if scheduled(task) {
            start = start.min(task.start_time.unwrap());
            stop = stop.max(task.end_time.unwrap());
        }
    }
    let title = layout_label(model.title.as_deref(), false, measurer, size, 4000.0, BOLD);
    let title_height = if title.height > 0.0 { title.height + 16.0 } else { 0.0 };
    let accessible = Accessible {
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    };
    if start > stop {
        let width = (title.width.max(1.0) + 2.0 * pad).ceil();
        let height = (title.height.max(1.0) + 2.0 * pad).ceil();
        let content = label_svg(
            &title,
            width / 2.0,
            pad + title.height / 2.0,
            " class=\"pele-title\" font-weight=\"var(--_tw)\"",
        );
        return Ok(Rendered {
            svg: svg_document("gantt", width, height, size, options, &accessible, &content),
            width,
            height,
            links: vec![],
        });
    }
    let mut section_labels = IndexMap::new();
    let mut section_width: f64 = 0.0;
    for section in categories.keys() {
        let label = layout_label(Some(section), false, measurer, section_size, 180.0, 0);
        section_width = section_width.max(label.width);
        section_labels.insert(section.clone(), label);
    }
    let axis_format = if !model.axis_format.is_empty() {
        model.axis_format.as_str()
    } else if model.date_format == "D" {
        "%d"
    } else {
        text(config, "axisFormat").unwrap_or("%Y-%m-%d")
    };
    let tick_label = |t| axis::time_format(axis_format, t, &zone);
    let sample_width = measurer
        .width(&tick_label(start), tick_size, 0)
        .max(measurer.width(&tick_label(stop), tick_size, 0));
    let left = pad
        + setting(config, "leftPadding", 0.0, 2000.0)
            .unwrap_or((if section_width > 0.0 { section_width + 24.0 } else { 0.0 }).max(sample_width / 2.0));
    let right_padding = setting(config, "rightPadding", 0.0, 2000.0).unwrap_or(16f64.max(sample_width / 2.0));
    let target = cap_width(options, setting(config, "useWidth", 100.0, 20000.0).unwrap_or(800.0), 280.0);
    let plot_width = 240f64.max(target - left - right_padding - pad);
    let right = left + plot_width;
    let scale = if stop > start { plot_width / (stop - start) } else { 0.0 };
    let x = |t| {
        if scale == 0.0 {
            left + plot_width / 2.0
        } else {
            left + (t - start) * scale
        }
    };
    let tick_height = js_round(tick_size * 1.75);
    let top_padding = setting(config, "topPadding", 0.0, 2000.0);
    let grid_start = setting(config, "gridLineStartPadding", 0.0, 2000.0);
    let head_height = pad + title_height + if top_axis { tick_height } else { 0.0 };
    let plot_top = head_height.max(top_padding.unwrap_or(0.0));
    let grid_lead = if let (Some(a), Some(b)) = (top_padding, grid_start) {
        (a - b).min(plot_top - head_height).max(0.0)
    } else {
        0.0
    };
    let plot_height = row_count.max(1) as f64 * row_height;
    let plot_bottom = plot_top + plot_height;
    let mut bars = vec![];
    let mut extent = right + right_padding;
    for &i in &row_tasks {
        let task = &tasks[i];
        let label = one_line(&task.task, measurer, label_size);
        let row = row_of[i].unwrap();
        if !scheduled(task) {
            let label_x = left + 8.0 + label.width / 2.0;
            extent = extent.max(label_x + label.width / 2.0);
            bars.push(Bar {
                task: i,
                label,
                row,
                x0: f64::NAN,
                x1: f64::NAN,
                label_x,
                inside: false,
            });
            continue;
        }
        let mut x0 = x(task.start_time.unwrap());
        let mut x1 = x(task.render_end_time.or(task.end_time).unwrap()).max(x0 + if task.milestone { 0.0 } else { 2.0 });
        if task.milestone {
            let mid = x0 + (x(task.end_time.unwrap()) - x0) / 2.0;
            x0 = mid - bar_height / 2.0;
            x1 = mid + bar_height / 2.0;
        }
        let inside = !task.milestone && label.width + 16.0 <= x1 - x0;
        let mut label_x = (x0 + x1) / 2.0;
        if !inside {
            let fits_right = x1 + 8.0 + label.width <= right + right_padding;
            let fits_left = x0 - 8.0 - label.width >= left;
            label_x = if fits_right || !fits_left {
                x1 + 8.0 + label.width / 2.0
            } else {
                x0 - 8.0 - label.width / 2.0
            };
            extent = extent.max(label_x + label.width / 2.0);
        }
        bars.push(Bar {
            task: i,
            label,
            row,
            x0,
            x1,
            label_x,
            inside,
        });
    }
    let interval = model
        .tick_interval
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| text(config, "tickInterval"))
        .unwrap_or("");
    let weekday = model.weekday.as_deref().or_else(|| text(config, "weekday")).unwrap_or("sunday");
    let count = (plot_width / (sample_width + 24.0)).floor().clamp(2.0, 10.0);
    let ticks = axis::interval_ticks(interval, weekday, start, stop, (plot_width / 3.0).ceil(), &zone)
        .unwrap_or_else(|| axis::auto_ticks(start, stop, count, &zone));
    let (mut grid, mut axis_bottom, mut axis_top) = (String::new(), String::new(), String::new());
    let mut last_right = f64::NEG_INFINITY;
    let mut last_label = String::new();
    let bottom_y = plot_bottom + tick_height / 2.0 + 2.0;
    let top_y = plot_top - grid_lead - tick_height / 2.0 - 2.0;
    for t in ticks {
        let tx = x(t);
        grid += &format!("M{},{}V{}", num(tx), num(plot_top - grid_lead), num(plot_bottom));
        let label = tick_label(t);
        let width = measurer.width(&label, tick_size, 0);
        if tx - width / 2.0 < last_right + 8.0 || label == last_label {
            continue;
        }
        last_right = tx + width / 2.0;
        last_label = label.clone();
        extent = extent.max(last_right);
        let body = format!("text-anchor=\"middle\">{}</text>", esc(&label));
        axis_bottom += &format!("<text x=\"{}\" y=\"{}\" {body}", num(tx), num(bottom_y + tick_size * 0.35));
        if top_axis {
            axis_top += &format!("<text x=\"{}\" y=\"{}\" {body}", num(tx), num(top_y + tick_size * 0.35));
        }
    }
    let (mut vert_lines, mut vert_labels) = (String::new(), String::new());
    let mut vert_ends = vec![];
    let vert_top = plot_bottom + tick_height + 4.0;
    let vert_line = js_round(label_size * 1.5);
    verts.sort_by(|&a, &b| tasks[a].start_time.unwrap().total_cmp(&tasks[b].start_time.unwrap()));
    for i in verts {
        let task = &tasks[i];
        let vx = x(task.start_time.unwrap());
        let label = one_line(&task.task, measurer, label_size);
        let cx = (pad + label.width / 2.0).max(vx);
        let line = vert_ends
            .iter()
            .position(|&end| cx - label.width / 2.0 >= end + 8.0)
            .unwrap_or(vert_ends.len());
        let end = cx + label.width / 2.0;
        if line == vert_ends.len() {
            vert_ends.push(end);
        } else {
            vert_ends[line] = end;
        }
        extent = extent.max(end);
        vert_lines += &format!(
            "<path class=\"pele-vert\" data-id=\"{}\" d=\"M{},{}V{}\"/>",
            esc_text(&task.id),
            num(vx),
            num(plot_top - grid_lead),
            num(plot_bottom)
        );
        vert_labels += &label_svg(
            &label,
            cx,
            vert_top + line as f64 * vert_line + vert_line / 2.0,
            &format!(" class=\"pele-vert-label\" data-id=\"{}\"", esc_text(&task.id)),
        );
    }
    let width = (extent.max(pad + title.width) + pad).ceil();
    let height = (plot_bottom
        + tick_height
        + if vert_ends.is_empty() {
            0.0
        } else {
            4.0 + vert_ends.len() as f64 * vert_line
        }
        + pad)
        .ceil();
    let (mut bands, mut section_out) = (String::new(), String::new());
    let mut row = 0;
    while row < row_count {
        let section = &row_section[row];
        let mut end = row + 1;
        while end < row_count && row_section[end] == *section {
            end += 1;
        }
        let index = categories[section] % section_styles;
        let y = plot_top + row as f64 * row_height;
        let h = (end - row) as f64 * row_height;
        if categories.len() > 1 || !section.is_empty() {
            bands += &format!(
                "<rect class=\"pele-section pele-section-{index}\" x=\"0\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                num(y),
                num(width),
                num(h),
                if index.is_multiple_of(2) { "var(--_s)" } else { "none" }
            );
        }
        let label = &section_labels[section];
        section_out += &label_svg(
            label,
            pad + 8.0 + label.width / 2.0,
            y + h / 2.0,
            &format!(" class=\"pele-section-label\" data-id=\"{}\"", esc_text(section)),
        );
        row = end;
    }
    let mut excluded = String::new();
    if (!model.excludes.is_empty() || !model.includes.is_empty()) && dates::add(start, 6.0, "y", &zone) > stop {
        let mut from = None;
        let mut to = None;
        let flush = |from: &mut Option<f64>, to: &mut Option<f64>, out: &mut String| {
            if let (Some(a), Some(b)) = (*from, *to) {
                let day_start = dates::start_day(a, &zone);
                let day_end = dates::set(b, &zone, None, None, None, Some(23.0), Some(59.0), Some(59.0), Some(999.0));
                let ex0 = left.max(x(day_start));
                let ex1 = right.min(x(day_end));
                if ex1 > ex0 {
                    *out += &format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                        num(ex0),
                        num(plot_top),
                        num(ex1 - ex0),
                        num(plot_height)
                    );
                }
                *from = None;
                *to = None;
            }
        };
        let mut day = start;
        while day <= stop {
            if schedule.is_invalid(day)? {
                if from.is_none() {
                    from = Some(day);
                }
                to = Some(day);
            } else {
                flush(&mut from, &mut to, &mut excluded);
            }
            day = dates::add_days(day, 1.0, &zone);
        }
        flush(&mut from, &mut to, &mut excluded);
    }
    let mut links: Vec<LinkInfo> = vec![];
    let mut tasks_out = String::new();
    for bar in bars {
        let task = &tasks[bar.task];
        let cy = plot_top + bar.row as f64 * row_height + row_height / 2.0;
        let state = if task.active {
            "active"
        } else if task.done {
            "done"
        } else {
            ""
        };
        let mut classes = "pele-task".to_string();
        if !state.is_empty() {
            classes += " pele-";
            classes += state;
        }
        if task.crit {
            classes += " pele-crit";
        }
        if task.milestone {
            classes += " pele-milestone";
        }
        if task.classes.iter().any(|s| s == "clickable") {
            classes += " pele-clickable";
        }
        let mut shape = String::new();
        let mut text_fill = "";
        if bar.x0.is_nan() {
            classes += " pele-unscheduled";
            text_fill = " fill=\"var(--_m)\"";
        } else {
            let color = if task.crit { CRITICAL } else { "var(--_c)" };
            let paint = if state == "done" {
                "fill=\"var(--_a)\"".into()
            } else if state == "active" {
                format!("fill=\"{color}\" fill-opacity=\"0.25\"")
            } else {
                format!("fill=\"{color}\"")
            };
            let stroke = format!("stroke=\"{}\"", if state == "done" && !task.crit { "var(--_b)" } else { color });
            if task.milestone {
                let cx = (bar.x0 + bar.x1) / 2.0;
                let r = bar_height / 2.0;
                shape = format!(
                    "<path d=\"M{},{}L{},{}L{},{}L{},{}Z\" {paint} {stroke} stroke-linejoin=\"round\"/>",
                    num(cx),
                    num(cy - r),
                    num(cx + r),
                    num(cy),
                    num(cx),
                    num(cy + r),
                    num(cx - r),
                    num(cy)
                );
            } else {
                shape = format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" {paint} {stroke}{ROUNDED}/>",
                    num(bar.x0),
                    num(cy - bar_height / 2.0),
                    num(bar.x1 - bar.x0),
                    num(bar_height)
                );
            }
            if bar.inside {
                text_fill = if state.is_empty() { " fill=\"var(--_bg)\"" } else { "" };
            } else if state == "done" {
                text_fill = " fill=\"var(--_m)\"";
            }
        }
        let mut body = shape + &label_svg(&bar.label, bar.label_x, cy, &format!(" class=\"pele-label\"{text_fill}"));
        if let Some(url) = model.links.get(&task.id) {
            body = crate::url::linked(&body, url, &task.id, &mut links, options, "");
        }
        tasks_out += &format!("<g class=\"{classes}\" data-id=\"{}\">{body}</g>", esc_text(&task.id));
    }
    let mut today = String::new();
    if model.today_marker != "off" && now >= start && now <= stop && stop > start {
        let tx = x(now);
        let top = plot_top - grid_lead;
        let style = if model.today_marker.is_empty() {
            String::new()
        } else {
            resolve_style(&model.today_marker.split(',').map(str::to_string).collect::<Vec<_>>()).line
        };
        today = format!(
            "<g class=\"pele-today\"><path d=\"M{},{}V{}\" fill=\"none\" stroke=\"var(--_c)\" stroke-width=\"2\"{style}/>",
            num(tx),
            num(top),
            num(plot_bottom)
        );
        if style.is_empty() {
            today += &format!(
                "<path d=\"M{},{}H{}L{},{}Z\" fill=\"var(--_c)\"/>",
                num(tx - 4.0),
                num(top - 5.0),
                num(tx + 4.0),
                num(tx),
                num(top + 1.0)
            );
        }
        today += "</g>";
    }
    let mut axis_line = format!("M{},{}H{}", num(left), num(plot_bottom), num(right));
    if top_axis {
        axis_line += &format!("M{},{}H{}", num(left), num(plot_top - grid_lead), num(right));
    }
    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !bands.is_empty() {
        content += &format!("<g class=\"pele-sections\">{bands}</g>");
    }
    if !excluded.is_empty() {
        content += &format!("<g class=\"pele-excluded\" fill=\"var(--_a)\">{excluded}</g>");
    }
    content += &format!(
        "<g class=\"pele-grid\" fill=\"none\"><path d=\"{grid}\" stroke=\"var(--_b)\" stroke-dasharray=\"1 3\"/><path d=\"{axis_line}\" stroke=\"var(--_b)\"/></g>"
    );
    if !vert_lines.is_empty() {
        content += &format!(
            "<g class=\"pele-verts\" fill=\"none\" stroke=\"var(--_l)\" stroke-width=\"1.5\" stroke-dasharray=\"4 3\">{vert_lines}</g>"
        );
    }
    content += &today;
    content += &format!(
        "<g class=\"pele-tasks\" font-size=\"{}\">{tasks_out}</g><g class=\"pele-section-labels\" font-size=\"{}\">{section_out}</g><g class=\"pele-axis\" font-size=\"{}\" fill=\"var(--_m)\">{axis_bottom}{axis_top}</g>",
        num(label_size),
        num(section_size),
        num(tick_size)
    );
    if !vert_labels.is_empty() {
        content += &format!("<g class=\"pele-vert-labels\" font-size=\"{}\">{vert_labels}</g>", num(label_size));
    }
    Ok(Rendered {
        svg: svg_document("gantt", width, height, size, options, &accessible, &content),
        width,
        height,
        links,
    })
}
