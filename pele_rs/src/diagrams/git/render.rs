use super::{Commit, Model};
use crate::RenderOptions;
use crate::diagrams::common::{Rendered, turn_to_fit};
use crate::preprocess::Config;
use crate::svg::builder::{esc_text, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{ROUNDED, series_color};
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, TextMeasurer};
use crate::util::{is_js_space, js_num, js_round, num};
use std::collections::HashMap;
const HALF: f64 = std::f64::consts::FRAC_1_SQRT_2;
struct View<'a> {
    commit: &'a Commit,
    index: usize,
    lane: usize,
    m: f64,
    label: Option<Label>,
    tags: Vec<Label>,
}
struct Bounds {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}
impl Bounds {
    fn grow(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        if x0 < self.x0 {
            self.x0 = x0;
        }
        if y0 < self.y0 {
            self.y0 = y0;
        }
        if x1 > self.x1 {
            self.x1 = x1;
        }
        if y1 > self.y1 {
            self.y1 = y1;
        }
    }
}
fn flag(config: &Config, key: &str, fallback: bool) -> bool {
    config
        .get("gitGraph")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_bool())
        .unwrap_or(fallback)
}
fn tag_width(tag: &Label) -> f64 {
    7.0 + 4.0 + tag.width + 6.0
}
fn plain(raw: &str, measurer: &dyn TextMeasurer, size: f64) -> Label {
    let mut text = String::new();
    for (i, c) in raw.char_indices() {
        if c == '<' {
            let rest = &raw[i + 1..];
            let br = rest.get(..2).is_some_and(|v| v.eq_ignore_ascii_case("br"));
            let end = if br { rest[2..].trim_start_matches(is_js_space) } else { "" };
            if !(br && (end.starts_with('>') || end.starts_with("/>"))) {
                text += "&lt;";
                continue;
            }
        }
        text.push(c);
    }
    layout_label(Some(&text.replace(":fa-", "&colon;fa-")), false, measurer, size, 4000.0, 0)
}
fn symbol(kind: usize, x: f64, y: f64, color: &str) -> String {
    let at = format!("cx=\"{}\" cy=\"{}\"", num(x), num(y));
    if kind == 2 {
        return format!(
            "<rect x=\"{}\" y=\"{}\" width=\"16\" height=\"16\" rx=\"2\" fill=\"{color}\"/>",
            num(x - 8.0),
            num(y - 8.0)
        );
    }
    if kind == 3 {
        return format!(
            "<circle {at} r=\"6\" fill=\"var(--_bg)\" stroke=\"{color}\" stroke-width=\"2\"/><circle {at} r=\"2.5\" fill=\"{color}\"/>"
        );
    }
    let dot = format!("<circle {at} r=\"7\" fill=\"{color}\"/>");
    if kind == 1 {
        return dot
            + &format!(
                "<path d=\"M{},{}l6,6M{},{}l6,-6\" fill=\"none\" stroke=\"var(--_bg)\" stroke-width=\"1.5\" stroke-linecap=\"round\"/>",
                num(x - 3.0),
                num(y - 3.0),
                num(x - 3.0),
                num(y + 3.0)
            );
    }
    if kind == 4 {
        return dot
            + &format!(
                "<path d=\"M{},{}L{},{}L{},{}\" fill=\"none\" stroke=\"var(--_bg)\" stroke-linejoin=\"round\"/><circle cx=\"{}\" cy=\"{}\" r=\"1.8\" fill=\"var(--_bg)\"/><circle cx=\"{}\" cy=\"{}\" r=\"1.8\" fill=\"var(--_bg)\"/>",
                num(x - 2.6),
                num(y + 1.0),
                num(x + 0.8),
                num(y - 4.2),
                num(x + 2.6),
                num(y + 1.0),
                num(x - 2.6),
                num(y + 2.2),
                num(x + 2.6),
                num(y + 2.2)
            );
    }
    dot
}
pub fn render(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    if model.direction != "LR" {
        return draw(model, config, options, measurer, &model.direction);
    }
    turn_to_fit(options, |down| {
        draw(model, config, options, measurer, if down { "TB" } else { "LR" })
    })
}
fn draw(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer, direction: &str) -> Rendered {
    let size = options.font_size.unwrap_or(16.0);
    let small = js_round(size * 0.75);
    let pad = options.padding.unwrap_or(8.0);
    let show_branches = flag(config, "showBranches", true);
    let show_label = flag(config, "showCommitLabel", true);
    let rotate = flag(config, "rotateCommitLabel", true);
    let parallel = flag(config, "parallelCommits", false);
    let vertical = direction != "LR";
    let flip = direction == "BT";
    let lane_count = model.lanes.len();
    let medium = js_round(size * 0.875);
    let names: Vec<_> = model
        .lanes
        .iter()
        .map(|name| plain(if show_branches { name } else { "" }, measurer, medium))
        .collect();
    let mut commits: Vec<_> = model.commits.values().collect();
    commits.sort_by_key(|c| c.seq);
    let mut views = vec![];
    let mut by_id = HashMap::new();
    for commit in commits {
        let labelled = show_label && commit.kind != 4 && (commit.kind != 3 || commit.custom_id == Some(true));
        let tags = commit
            .tags
            .iter()
            .map(|t| plain(t, measurer, small))
            .filter(|l| !l.lines.is_empty())
            .collect();
        let index = views.len();
        let lane = model.lanes.iter().position(|v| v == &commit.branch).unwrap_or(0);
        views.push(View {
            commit,
            index,
            lane,
            m: 0.0,
            label: labelled.then(|| plain(&commit.id, measurer, small)),
            tags,
        });
        by_id.insert(commit.id.as_str(), index);
    }
    let mut half = vec![0.0_f64; views.len()];
    let mut tag_half = half.clone();
    let mut lo = vec![7.0_f64; lane_count];
    let mut hi = lo.clone();
    for (k, view) in views.iter().enumerate() {
        let (mut tags_main, mut tags_cross) = (0.0_f64, 0.0_f64);
        for tag in &view.tags {
            let w = tag_width(tag);
            if vertical {
                tags_main = tags_main.max(tag.height);
                tags_cross += w + 4.0;
            } else {
                tags_main = tags_main.max(w);
                tags_cross += tag.height + 3.0;
            }
        }
        let (mut label_main, mut label_cross) = (0.0_f64, 0.0_f64);
        if let Some(label) = view.label.as_ref().filter(|l| !l.lines.is_empty()) {
            if rotate {
                label_cross = 3.0 + HALF * (label.width + label.height);
            } else {
                label_main = if vertical { label.height } else { label.width };
                label_cross = 6.0 + if vertical { label.width } else { label.height };
            }
        }
        half[k] = 7.0_f64.max(tags_main / 2.0).max(label_main / 2.0);
        tag_half[k] = tags_main / 2.0;
        let tag_side = 7.0 + if view.tags.is_empty() { 0.0 } else { 6.0 + tags_cross };
        let label_side = 7.0 + label_cross;
        lo[view.lane] = lo[view.lane].max(if vertical { label_side } else { tag_side });
        hi[view.lane] = hi[view.lane].max(if vertical { tag_side } else { label_side });
    }
    let mut lead = vec![0.0; views.len()];
    if !vertical {
        for (k, view) in views.iter().enumerate() {
            if tag_half[k] == 0.0 {
                continue;
            }
            for id in &view.commit.parents {
                if by_id.get(id.as_str()).is_some_and(|&i| views[i].lane < view.lane) {
                    lead[k] = tag_half[k] + 4.0;
                }
            }
        }
    }
    let space = |p: usize, k: usize| {
        44.0_f64
            .max(half[p] + half[k] + 8.0)
            .max(if lead[k] > 0.0 { half[p] + 36.0 + lead[k] } else { 0.0 })
    };
    for k in 0..views.len() {
        if !parallel {
            views[k].m = if k == 0 { 0.0 } else { views[k - 1].m + space(k - 1, k) };
            continue;
        }
        let mut m = 0.0_f64;
        for id in &views[k].commit.parents {
            if let Some(&p) = by_id.get(id.as_str())
                && p < k
            {
                m = m.max(views[p].m + space(p, k));
            }
        }
        views[k].m = m;
    }
    let end = views.iter().map(|v| v.m).fold(0.0, f64::max);
    let mut cross = vec![0.0; lane_count];
    for i in 0..lane_count {
        let name = &names[i];
        let reach = if name.lines.is_empty() {
            0.0
        } else if vertical {
            name.width / 2.0 + 8.0
        } else {
            name.height / 2.0 + 2.0
        };
        lo[i] = lo[i].max(reach);
        hi[i] = hi[i].max(reach);
        if i > 0 {
            cross[i] = cross[i - 1] + 44.0_f64.max(hi[i - 1] + lo[i] + 8.0);
        }
    }
    let px = |m: f64, c: f64| if vertical { c } else { m };
    let py = |m: f64, c: f64| if vertical { if flip { -m } else { m } } else { c };
    let point = |m: f64, c: f64| format!("{},{}", num(px(m, c)), num(py(m, c)));
    let mut bounds = Bounds {
        x0: f64::INFINITY,
        y0: f64::INFINITY,
        x1: f64::NEG_INFINITY,
        y1: f64::NEG_INFINITY,
    };
    let mut branches = String::new();
    if show_branches {
        for i in 0..lane_count {
            let name = &names[i];
            let color = series_color(i as i64);
            let c = cross[i];
            let w = name.width + 16.0;
            let h = name.height + 4.0;
            let from = if name.lines.is_empty() {
                -20.0
            } else {
                -20.0 - if vertical { h } else { w }
            };
            let cx = px(from + if vertical { h } else { w } / 2.0, c);
            let cy = py(from + if vertical { h } else { w } / 2.0, c);
            bounds.grow(px(-20.0, c), py(-20.0, c), px(-20.0, c), py(-20.0, c));
            bounds.grow(px(end + 20.0, c), py(end + 20.0, c), px(end + 20.0, c), py(end + 20.0, c));
            branches += &format!(
                "<g class=\"pele-branch\" data-id=\"{}\"><path class=\"pele-lane\" d=\"M{}L{}\" stroke=\"{color}\" stroke-dasharray=\"2 4\"/>",
                esc_text(&model.lanes[i]),
                point(-20.0, c),
                point(end + 20.0, c)
            );
            if !name.lines.is_empty() {
                bounds.grow(cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0);
                branches += &format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{color}\"{ROUNDED}/>{}",
                    num(cx - w / 2.0),
                    num(cy - h / 2.0),
                    num(w),
                    num(h),
                    label_svg(name, cx, cy, " class=\"pele-label\" fill=\"var(--_bg)\"")
                );
            }
            branches += "</g>";
        }
    }
    let mut taken = vec![vec![]; lane_count];
    for view in &views {
        taken[view.lane].push(view.m);
    }
    if parallel {
        for list in &mut taken {
            list.sort_by(|a, b| crate::util::ord(a - b));
        }
    }
    let busy = |lane: usize, from: f64, to: f64| {
        let list = &taken[lane];
        let i = list.partition_point(|&v| v <= from);
        i < list.len() && list[i] < to
    };
    let mut curves = String::new();
    let mut lines = String::new();
    for view in &views {
        let commit = view.commit;
        let c2 = cross[view.lane];
        let m2 = view.m;
        for (j, id) in commit.parents.iter().enumerate() {
            let Some(&p) = by_id.get(id.as_str()) else {
                continue;
            };
            if p == view.index {
                continue;
            }
            let parent = &views[p];
            let c1 = cross[parent.lane];
            let m1 = parent.m;
            if parent.lane == view.lane {
                lines += &format!(
                    "<path class=\"pele-edge\" d=\"M{}L{}\" stroke=\"{}\"/>",
                    point(m1, c1),
                    point(m2, c2),
                    series_color(view.lane as i64)
                );
                continue;
            }
            let merged = j > 0 && commit.kind == 3;
            let picked = j > 0 && !merged;
            let mut d = format!("M{}", point(m1, c1));
            if m2 <= m1 {
                let mid = (m1 + m2) / 2.0;
                d += &format!("C{} {} {}", point(mid, c1), point(mid, c2), point(m2, c2));
            } else if !merged && !busy(view.lane, m1, m2) {
                let s = 36.0_f64.min(m2 - m1);
                d += &format!(
                    "C{} {} {}L{}",
                    point(m1 + s / 2.0, c1),
                    point(m1 + s / 2.0, c2),
                    point(m1 + s, c2),
                    point(m2, c2)
                );
            } else {
                let to = if parent.lane < view.lane && m2 - lead[view.index] > m1 {
                    m2 - lead[view.index]
                } else {
                    m2
                };
                let mut s = 36.0_f64.min(to - m1);
                let mut along = c1;
                if !merged || busy(parent.lane, m1, m2) {
                    along = if c1 > c2 {
                        c2 + hi[view.lane] + 4.0
                    } else {
                        c2 - lo[view.lane] - 4.0
                    };
                    s = 36.0_f64.min((to - m1) / 2.0);
                    d += &format!(
                        "C{} {} {}",
                        point(m1 + s / 2.0, c1),
                        point(m1 + s / 2.0, along),
                        point(m1 + s, along)
                    );
                }
                d += &format!(
                    "L{}C{} {} {}",
                    point(to - s, along),
                    point(to - s / 2.0, along),
                    point(to - s / 2.0, c2),
                    point(to, c2)
                );
                if to < m2 {
                    d += &format!("L{}", point(m2, c2));
                }
            }
            curves += &format!(
                "<path class=\"pele-edge{}\" d=\"{d}\" stroke=\"{}\"{}/>",
                if merged {
                    " pele-edge-merge"
                } else if picked {
                    " pele-edge-cherry-pick"
                } else {
                    ""
                },
                series_color(if merged { parent.lane } else { view.lane } as i64),
                if picked { " stroke-dasharray=\"4 3\"" } else { "" }
            );
        }
    }
    let mut commits = String::new();
    for view in &views {
        let commit = view.commit;
        let x = px(view.m, cross[view.lane]);
        let y = py(view.m, cross[view.lane]);
        let kind = commit.custom_type.unwrap_or(commit.kind);
        bounds.grow(x - 8.0, y - 8.0, x + 8.0, y + 8.0);
        let written = commit.kind < 3 && !commit.message.is_empty();
        let mut body = if written {
            format!("<title>{}</title>", esc_text(&commit.message))
        } else {
            String::new()
        };
        body += &symbol(kind, x, y, &series_color(view.lane as i64));
        if let Some(label) = view.label.as_ref().filter(|l| !l.lines.is_empty()) {
            let w = label.width;
            let h = label.height;
            let back = |cx: f64, cy: f64| {
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\"/>",
                    num(cx - w / 2.0 - 2.0),
                    num(cy - h / 2.0 + 2.0),
                    num(w + 4.0),
                    num(h - 4.0)
                )
            };
            if rotate {
                let k = HALF * h / 2.0;
                let ax = if vertical { x - 7.0 - 3.0 - k } else { x + 3.0 };
                let ay = if vertical { y } else { y + 7.0 + 3.0 + k };
                let reach = HALF * w + k;
                bounds.grow(
                    ax - reach,
                    if flip { ay - reach } else { ay - k },
                    ax + k,
                    if flip { ay + k } else { ay + reach },
                );
                body += &format!(
                    "<g class=\"pele-commit-label\" transform=\"rotate({} {} {})\">{}{}</g>",
                    if flip { 45 } else { -45 },
                    num(ax),
                    num(ay),
                    back(ax - w / 2.0, ay),
                    label_svg(label, ax - w / 2.0, ay, "")
                );
            } else {
                let cx = if vertical { x - 7.0 - 6.0 - w / 2.0 } else { x };
                let cy = if vertical { y } else { y + 7.0 + 6.0 + h / 2.0 };
                bounds.grow(cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0);
                body += &format!(
                    "<g class=\"pele-commit-label\">{}{}</g>",
                    back(cx, cy),
                    label_svg(label, cx, cy, "")
                );
            }
        }
        let mut offset = 13.0;
        let order: Vec<_> = if vertical {
            (0..view.tags.len()).collect()
        } else {
            (0..view.tags.len()).rev().collect()
        };
        for t in order {
            let tag = &view.tags[t];
            let w = tag_width(tag);
            let h = tag.height;
            let left = if vertical { x + offset } else { x - w / 2.0 };
            let cy = if vertical { y } else { y - offset - h / 2.0 };
            offset += if vertical { w + 4.0 } else { h + 3.0 };
            bounds.grow(left, cy - h / 2.0, left + w, cy + h / 2.0);
            body += &format!(
                "<g class=\"pele-tag\"><path d=\"M{},{}L{},{}H{}V{}H{}Z\" fill=\"var(--_s)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"/><circle cx=\"{}\" cy=\"{}\" r=\"1.5\" fill=\"var(--_b)\"/>{}</g>",
                num(left),
                num(cy),
                num(left + 7.0),
                num(cy - h / 2.0),
                num(left + w),
                num(cy + h / 2.0),
                num(left + 7.0),
                num(left + 7.0),
                num(cy),
                label_svg(tag, left + 7.0 + 4.0 + tag.width / 2.0, cy, " fill=\"var(--_fg)\"")
            );
        }
        let merge = if commit.kind == 3 && kind != 3 { " pele-commit-merge" } else { "" };
        let class = ["normal", "reverse", "highlight", "merge", "cherry-pick"]
            .get(kind)
            .copied()
            .unwrap_or("normal");
        commits += &format!(
            "<g class=\"pele-commit pele-commit-{class}{merge}\" data-id=\"{}\">{body}</g>",
            esc_text(&commit.id)
        );
    }
    if bounds.x0 > bounds.x1 {
        bounds.grow(0.0, 0.0, 0.0, 0.0);
    }
    let title = layout_label(model.title.as_deref(), false, measurer, size, 4000.0, BOLD);
    let title_height = if title.height > 0.0 { title.height + 16.0 } else { 0.0 };
    let inner = (bounds.x1 - bounds.x0).max(title.width);
    let width = (inner + 2.0 * pad).ceil();
    let height = (bounds.y1 - bounds.y0 + title_height + 2.0 * pad).ceil();
    let dx = pad - bounds.x0 + (inner - (bounds.x1 - bounds.x0)) / 2.0;
    let dy = pad + title_height - bounds.y0;
    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &format!("<g transform=\"translate({},{})\">", num(dx), num(dy));
    if !branches.is_empty() {
        content += &format!(
            "<g class=\"pele-branches\" fill=\"none\" font-size=\"{}\">{branches}</g>",
            js_num(medium)
        );
    }
    if !curves.is_empty() || !lines.is_empty() {
        content += &format!("<g class=\"pele-edges\" fill=\"none\" stroke-width=\"2\" stroke-linecap=\"round\">{curves}{lines}</g>");
    }
    if !commits.is_empty() {
        content += &format!(
            "<g class=\"pele-commits\" font-size=\"{}\" fill=\"var(--_m)\">{commits}</g>",
            js_num(small)
        );
    }
    content += "</g>";
    let acc = Accessible {
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    };
    Rendered {
        svg: svg_document("gitGraph", width, height, size, options, &acc, &content),
        width,
        height,
        links: vec![],
    }
}
