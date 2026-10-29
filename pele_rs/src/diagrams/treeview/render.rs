use super::{
    Model, Node,
    literal::{left_label, literal_label},
};
use crate::diagrams::common::{Rendered, tighten, title_room};
use crate::preprocess::{Config, truthy};
use crate::svg::builder::{esc, esc_text, label_svg};
use crate::svg::root::{Accessible, svg_document};
use crate::svg::theme::{FONT_MONO, ROUNDED, class_names};
use crate::text::entities::decode_entities;
use crate::text::label::{Label, layout_label};
use crate::text::measurer::{BOLD, ITALIC, MONO, TextMeasurer};
use crate::util::{js_round, num};
use crate::{MonospacePart, RenderOptions};
const FOLDER: &str = "M3 6.5A1.5 1.5 0 0 1 4.5 5h4.6l2.4 2.5h8A1.5 1.5 0 0 1 21 9v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18z";
const FILE: &str = "M6.5 3h7L19 8.5v11a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 5 19.5v-15A1.5 1.5 0 0 1 6.5 3zM13.5 3v5.5H19";
fn node_icon(node: &Node, config: &Config) -> Option<String> {
    let c = config.get("treeView");
    let pack = c.and_then(|v| v.get("defaultIconPack")).and_then(|v| v.as_str()).unwrap_or("");
    let qualify = |icon: &str| {
        if icon.contains(':') {
            icon.to_string()
        } else if icon == "file" || icon == "folder" || pack.is_empty() {
            format!("mermaid-treeview:{icon}")
        } else {
            format!("{pack}:{icon}")
        }
    };
    if node.icon.as_deref() == Some("none") {
        return None;
    }
    if let Some(icon) = node.icon.as_deref().filter(|s| !s.is_empty()) {
        return Some(qualify(icon));
    }
    if !truthy(c.and_then(|v| v.get("showIcons"))) {
        return None;
    }
    if !node.directory {
        let named = c
            .and_then(|v| v.get("filenameIcons"))
            .and_then(|v| v.as_object())
            .and_then(|v| v.get(&node.name))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let detected = named.or_else(|| {
            node.name.rfind('.').filter(|i| *i > 0).and_then(|i| {
                let ext = node.name[i..].to_lowercase();
                let map = c.and_then(|v| v.get("extensionIcons")).and_then(|v| v.as_object())?;
                map.get(&ext)
                    .and_then(|v| v.as_str())
                    .or_else(|| map.get(&ext[1..]).and_then(|v| v.as_str()))
            })
        });
        if detected == Some("none") {
            return None;
        }
        if let Some(icon) = detected.filter(|s| !s.is_empty()) {
            return Some(qualify(icon));
        }
    }
    Some(format!("mermaid-treeview:{}", if node.directory { "folder" } else { "file" }))
}
fn setting(config: &Config, key: &str, fallback: f64, min: f64, max: f64) -> f64 {
    config
        .get("treeView")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_f64())
        .filter(|v| *v >= min && *v <= max)
        .unwrap_or(fallback)
}
pub fn render(model: &Model, config: &Config, o: &RenderOptions, m: &dyn TextMeasurer) -> Rendered {
    tighten(o, |tight| draw(model, config, o, m, tight))
}
fn draw(model: &Model, config: &Config, o: &RenderOptions, m: &dyn TextMeasurer, tight: bool) -> Rendered {
    let size = o.font_size.unwrap_or(16.0);
    let pad = o.padding.unwrap_or(8.0);
    let mono = o.monospace.as_ref().is_some_and(|v| v.contains(&MonospacePart::Files));
    let family = if mono {
        format!(" font-family=\"{FONT_MONO}\"")
    } else {
        String::new()
    };
    let px = setting(config, "paddingX", 5.0, 0.0, 100.0);
    let py = setting(config, "paddingY", 2.0, 0.0, 100.0);
    let row_indent = setting(config, "rowIndent", 15.0, 0.0, 400.0);
    let line_thickness = setting(config, "lineThickness", 1.0, 0.25, 12.0);
    let mut max_depth = 0;
    let mut depths: Vec<(usize, usize)> = model.nodes[0].children.iter().map(|i| (*i, 0)).collect();
    let mut i = 0;
    while tight && i < depths.len() {
        let (node, depth) = depths[i];
        max_depth = max_depth.max(depth);
        for &child in &model.nodes[node].children {
            depths.push((child, depth + 1))
        }
        i += 1
    }
    let step = if tight {
        (row_indent + px).min(2.0_f64.max((o.max_width.unwrap_or(320.0) - 120.0) / max_depth.max(1) as f64))
    } else {
        row_indent + px
    };
    let row_height = js_round(size * 1.5) + 2.0 * py;
    let icon_size = size;
    let title = layout_label(
        model.title.as_deref(),
        false,
        m,
        size,
        if tight { title_room(o) } else { 4000.0 },
        BOLD,
    );
    let title_height = if title.height > 0.0 { title.height + 12.0 } else { 0.0 };
    struct Row {
        node: usize,
        x: f64,
        cy: f64,
        icon: Option<String>,
        text: String,
        label_x: f64,
        highlighted: bool,
        label: Option<Label>,
        description: Option<Label>,
        top: f64,
        height: f64,
    }
    let mut rows: Vec<Row> = Vec::new();
    let mut row_of = vec![0; model.nodes.len()];
    let mut pending: Vec<(usize, usize, bool)> = Vec::new();
    let push = |pending: &mut Vec<(usize, usize, bool)>, list: &[usize], depth: usize| {
        let slot = list.iter().any(|i| node_icon(&model.nodes[*i], config).is_some());
        for &i in list.iter().rev() {
            pending.push((i, depth, slot))
        }
    };
    push(&mut pending, &model.nodes[0].children, 0);
    let mut label_right = 0.0_f64;
    let mut any_highlight = false;
    let mut rows_height = 0.0;
    while let Some((index, depth, slot)) = pending.pop() {
        let node = &model.nodes[index];
        let x = depth as f64 * step;
        let text = decode_entities(&node.name);
        let label_x = x + if slot { icon_size + 6.0 } else { 0.0 };
        let room = 40.0_f64.max(o.max_width.unwrap_or(320.0) - 2.0 * pad - 8.0 - label_x);
        let style = if node.directory { BOLD } else { 0 } | if mono { MONO } else { 0 };
        let label = tight.then(|| literal_label(&text, m, size, room, style));
        let description = if tight {
            node.description
                .as_ref()
                .filter(|s| !s.is_empty())
                .map(|s| literal_label(&decode_entities(s), m, size, room, ITALIC))
        } else {
            None
        };
        let width = label.as_ref().map(|l| l.width).unwrap_or_else(|| m.width(&text, size, style));
        label_right = label_right.max(label_x + width);
        let highlighted = node
            .css_class
            .as_ref()
            .is_some_and(|s| s.split_whitespace().any(|s| s == "highlight"));
        any_highlight |= highlighted;
        let height = if tight {
            row_height
                .max(label.as_ref().map(|l| l.height).unwrap_or(0.0) + description.as_ref().map(|l| l.height).unwrap_or(0.0) + 2.0 * py)
        } else {
            row_height
        };
        let cy = rows_height
            + if tight {
                py + icon_size.max(label.as_ref().map(|l| l.height).unwrap_or(0.0)) / 2.0
            } else {
                height / 2.0
            };
        row_of[index] = rows.len();
        rows.push(Row {
            node: index,
            x,
            cy,
            icon: node_icon(node, config),
            text,
            label_x,
            highlighted,
            label,
            description,
            top: rows_height,
            height,
        });
        rows_height += height;
        push(&mut pending, &node.children, depth + 1)
    }
    let desc_x = label_right + 16.0;
    let mut width = label_right;
    for row in &rows {
        if let Some(description) = model.nodes[row.node].description.as_ref().filter(|s| !s.is_empty()) {
            let w = row
                .description
                .as_ref()
                .map(|l| l.width)
                .unwrap_or_else(|| m.width(&decode_entities(description), size, ITALIC));
            width = width.max(if tight { row.label_x } else { desc_x } + w)
        }
    }
    width = width.max(title.width);
    let inset = pad + if any_highlight { 4.0 } else { 0.0 };
    let top = pad + title_height;
    let tw = (width + 2.0 * inset).ceil();
    let th = (top + rows_height + pad).ceil();
    let mut guides = String::new();
    for row in &rows {
        let children = &model.nodes[row.node].children;
        if children.is_empty() {
            continue;
        }
        let gx = inset + row.x + 7.0;
        let tick = gx.max(inset + row.x + step - px);
        let last = top + rows[row_of[*children.last().unwrap()]].cy;
        let r = 4.0_f64.min(tick - gx);
        let mut d = format!("M{},{}V{}", num(gx), num(top + row.top + row.height - 2.0), num(last - r));
        if r > 0.0 {
            d += &format!("Q{},{} {},{}H{}", num(gx), num(last), num(gx + r), num(last), num(tick))
        }
        if tick > gx {
            for &c in &children[..children.len() - 1] {
                d += &format!("M{},{}H{}", num(gx), num(top + rows[row_of[c]].cy), num(tick))
            }
        }
        guides += &format!("<path d=\"{d}\"/>")
    }
    let baseline = size * 0.35;
    let mut body = String::new();
    for row in &rows {
        let node = &model.nodes[row.node];
        let cy = top + row.cy;
        let mut inner = String::new();
        if row.highlighted {
            let x = inset + row.x - 4.0;
            inner += &format!(
                "<rect class=\"pele-tree-highlight\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_c)\" fill-opacity=\"0.14\"{ROUNDED}/>",
                num(x),
                num(top + row.top + 1.0),
                num(tw - inset + 4.0 - x),
                num(row.height - 2.0)
            )
        }
        if let Some(icon) = &row.icon {
            let name = icon.strip_prefix("mermaid-treeview:").unwrap_or("");
            let glyph = match name {
                "folder" => FOLDER,
                "file" => FILE,
                _ => {
                    if node.directory {
                        FOLDER
                    } else {
                        FILE
                    }
                }
            };
            let drawn = o
                .icons
                .as_ref()
                .and_then(|r| r.resolve(icon))
                .unwrap_or_else(|| format!("<path d=\"{glyph}\" stroke=\"var(--_m)\" stroke-width=\"1.5\"/>"));
            inner += &format!(
                "<svg class=\"pele-icon\" data-icon=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{drawn}</svg>",
                esc_text(icon),
                num(inset + row.x),
                num(cy - icon_size / 2.0),
                num(icon_size),
                num(icon_size)
            )
        }
        let weight = if node.directory { " font-weight=\"var(--_hw)\"" } else { "" };
        if let Some(label) = &row.label {
            inner += &left_label(
                label,
                inset + row.label_x,
                top + row.top + py,
                &format!(" class=\"pele-label\"{weight}{family}"),
            )
        } else if !row.text.is_empty() {
            inner += &format!(
                "<text class=\"pele-label\" x=\"{}\" y=\"{}\"{weight}{family} xml:space=\"preserve\">{}</text>",
                num(inset + row.label_x),
                num(cy + baseline),
                esc(&row.text)
            )
        }
        if let Some(description) = &row.description {
            inner += &left_label(
                description,
                inset + row.label_x,
                top + row.top + py + row.label.as_ref().map(|l| l.height).unwrap_or(0.0),
                " class=\"pele-tree-description\" font-style=\"italic\" fill=\"var(--_m)\"",
            )
        } else if let Some(description) = node.description.as_ref().filter(|s| !s.is_empty()) {
            inner += &format!(
                "<text class=\"pele-tree-description\" x=\"{}\" y=\"{}\" font-style=\"italic\" fill=\"var(--_m)\" xml:space=\"preserve\">{}</text>",
                num(inset + desc_x),
                num(cy + baseline),
                esc(&decode_entities(description))
            )
        }
        body += &format!(
            "<g class=\"pele-node pele-tree-{}{}\" data-id=\"{}\">{inner}</g>",
            if node.directory { "dir" } else { "file" },
            class_names(node.css_class.as_deref().unwrap_or("")),
            esc_text(&node.name)
        )
    }
    let mut content = label_svg(
        &title,
        tw / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    );
    if !guides.is_empty() {
        content += &format!(
            "<g class=\"pele-tree-guides\" fill=\"none\" stroke=\"var(--_b)\" stroke-width=\"{}\">{guides}</g>",
            num(line_thickness)
        )
    }
    content += &body;
    Rendered {
        svg: svg_document(
            "treeView",
            tw,
            th,
            size,
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
