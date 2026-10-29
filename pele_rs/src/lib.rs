//! A Rust port of [pele](https://github.com/obsidianmd/pele), a Mermaid renderer that writes
//! themeable SVG. Identical inputs give the same SVG as pele's JavaScript, byte for byte.
//!
//! Implements the synchronous engine and all 33 diagram types from Pele 1.12.0,
//! revision `36e584d59152181927e0d8bf70fddcd2a5f33d6c`.

// The port mirrors pele's TypeScript loop for loop, and keeps JavaScript's handling of NaN, -0
// and the sign of `%`, so that the SVG stays identical. These lints would trade that for idiom.
#![allow(
    clippy::needless_range_loop,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::manual_clamp,
    clippy::manual_rem_euclid,
    clippy::too_many_arguments,
    // The public error carries complete messages, aliases, source snippets, and
    // optional exact UTF-16 diagnostics. Keep Result<_, PeleError> compatible.
    clippy::result_large_err
)]

pub mod detect;
pub mod diagrams;
pub mod engine;
pub mod errors;
pub mod layout;
pub mod preprocess;
pub mod svg;
pub mod text;
pub mod unicode;
pub mod url;
pub mod util;
mod yaml;

use std::fmt;
use std::sync::Arc;

pub use engine::{
    Diagram, DiagramMetadata, DiagramModel, ParseOptions, ParsedDiagram, Renderer, parse, render_parsed, render_parsed_with,
    try_detect_type,
};
pub use errors::{PeleError, PeleErrorCode};
pub use preprocess::Config;
pub use text::measurer::{MetricsMeasurer, TextMeasurer};
pub use url::LinkInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonospacePart {
    Members,
    Attributes,
    Files,
    Messages,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MathMarkup {
    Svg { svg: String, width: f64, height: f64 },
    Html { html: String, width: f64, height: f64 },
}

type IconCallback = dyn Fn(&str) -> Option<String> + Send + Sync;
type MathCallback = dyn Fn(&str, f64) -> Option<MathMarkup> + Send + Sync;

#[derive(Clone)]
pub struct IconResolver(Arc<IconCallback>);
impl PartialEq for IconResolver {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl IconResolver {
    pub fn new(f: impl Fn(&str) -> Option<String> + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }
    pub fn resolve(&self, name: &str) -> Option<String> {
        (self.0)(name)
    }
}
impl fmt::Debug for IconResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IconResolver(..)")
    }
}

#[derive(Clone)]
pub struct MathResolver(Arc<MathCallback>);
impl MathResolver {
    pub fn new(f: impl Fn(&str, f64) -> Option<MathMarkup> + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }
    pub fn resolve(&self, tex: &str, size: f64) -> Option<MathMarkup> {
        (self.0)(tex, size)
    }
}
impl fmt::Debug for MathResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MathResolver(..)")
    }
}

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
    pub monospace: Option<Vec<MonospacePart>>,
    pub icons: Option<IconResolver>,
    pub math: Option<MathResolver>,
    /// Unix time in milliseconds. Native rendering defaults to the current time.
    pub now: Option<f64>,
    /// An IANA timezone; native rendering defaults to the system's local zone.
    pub time_zone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RenderResult {
    pub svg: String,
    pub width: f64,
    pub height: f64,
    pub kind: String,
    pub mermaid_type: String,
    pub links: Vec<LinkInfo>,
}

/// Canonical diagram kinds registered by the complete renderer.
pub use engine::SUPPORTED;

pub fn detect_type(text: &str) -> Option<&'static str> {
    try_detect_type(text).ok().flatten()
}

pub fn supports(text: &str) -> bool {
    engine::default_renderer().supports(text)
}

/// Render diagram text using Pele's metrics measurer.
pub fn render(text: &str, options: &RenderOptions) -> Result<RenderResult, PeleError> {
    engine::default_renderer().render(text, options)
}

/// Render diagram text with a host-provided text measurer.
pub fn render_with(text: &str, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Result<RenderResult, PeleError> {
    engine::default_renderer().render_with(text, options, measurer)
}
