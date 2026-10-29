//! A Rust port of [pele](https://github.com/obsidianmd/pele), a Mermaid renderer that writes
//! themeable SVG. Identical inputs give the same SVG as pele's JavaScript, byte for byte.
//!
//! Only flowcharts are ported so far; other diagram types report that they are not supported.

// The port mirrors pele's TypeScript loop for loop, and keeps JavaScript's handling of NaN, -0
// and the sign of `%`, so that the SVG stays identical. These lints would trade that for idiom.
#![allow(
    clippy::needless_range_loop,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::manual_clamp,
    clippy::manual_rem_euclid,
    clippy::too_many_arguments
)]

pub mod detect;
pub mod diagrams;
pub mod errors;
pub mod layout;
pub mod preprocess;
pub mod svg;
pub mod text;
pub mod unicode;
pub mod url;
pub mod util;

use serde_json::Value;

pub use errors::{PeleError, PeleErrorCode};
pub use preprocess::Config;
pub use text::measurer::{MetricsMeasurer, TextMeasurer};

#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    // The fonts labels are measured in, and drawn in where the page sets no --pele-font.
    pub font_family: Option<String>,
    pub font_family_mono: Option<String>,
    pub font_size: Option<f64>,
    pub id_prefix: Option<String>,
    pub responsive: Option<bool>,
    // The width the host has for the diagram, in pixels.
    pub max_width: Option<f64>,
    pub auto_direction: Option<bool>,
    pub direction_breakpoint: Option<f64>,
    pub padding: Option<f64>,
    pub limit: Option<usize>,
    pub output_limit: Option<usize>,
    pub max_edges: Option<usize>,
    pub link_schemes: Option<Vec<String>>,
    pub image_schemes: Option<Vec<String>>,
    pub links: Option<bool>,
    pub images: Option<bool>,
    pub link_rel: Option<String>,
    pub config: Option<Config>,
}

#[derive(Debug, Clone)]
pub struct RenderResult {
    pub svg: String,
    pub width: f64,
    pub height: f64,
    pub kind: String,
    pub mermaid_type: String,
}

// Mermaid's default maxTextSize.
const DEFAULT_LIMIT: usize = 50000;
const DEFAULT_OUTPUT_LIMIT: usize = 4_000_000;
const DEFAULT_MAX_EDGES: usize = 5000;

pub fn detect_type(text: &str) -> Option<&'static str> {
    let pre = preprocess::preprocess(text).ok()?;
    detect::detect_diagram(&pre.text).map(|d| d.kind)
}

fn tag(mut error: PeleError, kind: &str) -> PeleError {
    if error.diagram.is_none() {
        error.diagram = Some(kind.to_string());
    }
    error.message = text::entities::written_codes(&error.message);
    error.snippet = text::entities::written_codes(&error.snippet);
    error
}

/// Renders Mermaid text to SVG, measuring text with pele's table of Arial widths.
pub fn render(text: &str, options: &RenderOptions) -> Result<RenderResult, PeleError> {
    render_with(text, options, &MetricsMeasurer)
}

/// Renders Mermaid text to SVG, measuring text with `measurer`.
pub fn render_with(text: &str, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Result<RenderResult, PeleError> {
    let limit = options.limit.unwrap_or(DEFAULT_LIMIT);
    if text.encode_utf16().count() > limit {
        return Err(PeleError::new(
            format!("Diagram source is longer than the limit of {} characters.", limit),
            PeleErrorCode::Limit,
        ));
    }
    let pre = preprocess::preprocess(text)?;
    let Some(found) = detect::detect_diagram(&pre.text) else {
        return Err(PeleError::new(
            "No diagram type detected for the given text.",
            PeleErrorCode::UnsupportedDiagram,
        ));
    };
    if found.kind != "flowchart" {
        return Err(PeleError::new(
            format!("Diagram type \"{}\" is not supported yet.", found.kind),
            PeleErrorCode::UnsupportedDiagram,
        )
        .of(found.kind));
    }
    let mut config = options.config.clone().unwrap_or_default();
    for (k, v) in pre.config.iter() {
        config.insert(k.clone(), v.clone());
    }
    let parsed = preprocess::encode_entities(&pre.text) + "\n";

    let flow = config.get("flowchart").and_then(|v| v.as_object());
    let mut db = diagrams::flowchart::db::FlowDb::new(
        flow.and_then(|f| f.get("inheritDir")) == Some(&Value::Bool(true)),
        Some(options.max_edges.unwrap_or(DEFAULT_MAX_EDGES)),
    );
    if let Some(title) = pre.title.as_deref().filter(|t| !t.is_empty()) {
        db.title = Some(title.to_string());
    }
    diagrams::flowchart::parser::parse_flowchart(&parsed, &mut db).map_err(|e| tag(e, found.kind))?;

    let fixed = flow.and_then(|f| f.get("useMaxWidth")) == Some(&Value::Bool(false));
    let mut opts = options.clone();
    opts.responsive = Some(options.responsive.unwrap_or(!fixed));
    let rendered = diagrams::flowchart::render::render_flowchart(&db, &config, &opts, measurer);
    let output_limit = options.output_limit.unwrap_or(DEFAULT_OUTPUT_LIMIT);
    if rendered.svg.encode_utf16().count() > output_limit {
        return Err(PeleError::new(
            format!("Diagram output is longer than the limit of {} characters.", output_limit),
            PeleErrorCode::Limit,
        )
        .of(found.kind));
    }
    Ok(RenderResult {
        svg: rendered.svg,
        width: rendered.width,
        height: rendered.height,
        kind: found.kind.to_string(),
        mermaid_type: found.mermaid_type.to_string(),
    })
}
