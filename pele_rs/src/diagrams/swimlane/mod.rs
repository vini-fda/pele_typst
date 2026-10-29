use crate::diagrams::common::Rendered;
use crate::diagrams::flowchart::{
    db::{FlowDb, LabelType},
    graph::{FlowGraph, GraphNode},
    render::{FlowVariant, render_flowchart_variant},
};
use crate::layout::{
    compound::{CEdge, CNode, CompoundResult, Dir},
    lanes::lane_layout,
    layered::LayeredOptions,
};
use crate::svg::{builder::label_svg, ink::ink_fill, theme::ResolvedStyle};
use crate::text::label::Label;
use crate::util::num;
use crate::{Config, PeleError, RenderOptions, TextMeasurer};

pub type Model = FlowDb;

pub fn parse(source: &str, config: &Config, title: Option<&str>, _line_offset: usize, max_edges: usize) -> Result<Model, PeleError> {
    let inherit = config.get("flowchart").and_then(|v| v.get("inheritDir")).and_then(|v| v.as_bool()) == Some(true);
    let mut db = FlowDb::new(inherit, Some(max_edges));
    db.title = title.map(str::to_owned);
    crate::diagrams::flowchart::parser::parse_flowchart(source, &mut db)?;
    Ok(db)
}

pub fn render(model: &Model, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    render_flowchart_variant(model, config, options, measurer, Some(&Swimlane))
}

struct Swimlane;
impl FlowVariant for Swimlane {
    fn kind(&self) -> &'static str {
        "swimlane"
    }
    fn prepare(&self, graph: &mut FlowGraph) {
        let mut loose = false;
        for node in &mut graph.nodes {
            if node.is_group {
                node.dir = None;
            } else if node.parent_id.is_none() {
                loose = true;
            }
        }
        if !loose {
            return;
        }
        const DEFAULT_LANE: &str = "__swimlane_default__";
        for node in &mut graph.nodes {
            if !node.is_group && node.parent_id.is_none() {
                node.parent_id = Some(DEFAULT_LANE.into());
            }
        }
        graph.nodes.push(GraphNode {
            id: DEFAULT_LANE.into(),
            label: None,
            label_type: LabelType::Text,
            shape: "rect".into(),
            is_group: true,
            parent_id: None,
            css_styles: vec![],
            css_compiled_styles: vec![],
            css_classes: String::new(),
            dir: None,
            link: None,
            link_target: None,
            tooltip: None,
            icon: None,
            pos: None,
            img: None,
            asset_width: None,
            asset_height: None,
            constraint: None,
        });
    }
    fn layout(&self, nodes: &mut [CNode], edges: &mut [CEdge], dir: Dir, opt: &LayeredOptions, tight: bool) -> CompoundResult {
        lane_layout(nodes, edges, dir, opt, if tight { 12.0 } else { 24.0 })
    }
    fn draw_group(&self, c: &CNode, label: &Label, style: &ResolvedStyle, classes: &str, id: &str, dir: Dir) -> String {
        let left = c.x - c.w / 2.0;
        let top = c.y - c.h / 2.0;
        let sideways = matches!(dir, Dir::LR | Dir::RL);
        let cx = if sideways { left + c.pad_top / 2.0 } else { c.x };
        let cy = if sideways { c.y } else { top + c.pad_top / 2.0 };
        let text = label_svg(
            label,
            cx,
            cy,
            &format!(
                " class=\"pele-cluster-label\"{}{}",
                ink_fill(&style.shape, &style.text, Some("var(--_m)")),
                style.text
            ),
        );
        let text = if sideways && !text.is_empty() {
            format!("<g transform=\"rotate(-90 {} {})\">{}</g>", num(cx), num(cy), text)
        } else {
            text
        };
        format!(
            "<g class=\"pele-cluster pele-lane{}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"var(--_b)\"/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\" stroke=\"var(--_b)\"{}/>{}</g>",
            classes,
            id,
            num(left),
            num(top),
            num(c.w),
            num(c.h),
            num(left),
            num(top),
            num(if sideways { c.pad_top } else { c.w }),
            num(if sideways { c.h } else { c.pad_top }),
            style.shape,
            text
        )
    }
    fn filled_box(&self, c: &CNode, dir: Dir) -> [f64; 4] {
        let left = c.x - c.w / 2.0;
        let top = c.y - c.h / 2.0;
        if matches!(dir, Dir::LR | Dir::RL) {
            [left, top, left + c.pad_top, top + c.h]
        } else {
            [left, top, left + c.w, top + c.pad_top]
        }
    }
}
