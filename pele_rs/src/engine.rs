//! Synchronous parsing, renderer registration, and reusable typed models.
use std::any::Any;
use std::fmt;
use std::sync::{Arc, OnceLock};

use indexmap::IndexMap;
use serde_json::Value;

use crate::diagrams::common::Rendered;
use crate::{Config, MetricsMeasurer, PeleError, PeleErrorCode, RenderOptions, RenderResult, TextMeasurer};

pub const COMPATIBILITY_VERSION: &str = "1.12.0";
pub const UPSTREAM_REVISION: &str = "36e584d59152181927e0d8bf70fddcd2a5f33d6c";
const DEFAULT_LIMIT: usize = 50_000;
const DEFAULT_OUTPUT_LIMIT: usize = 4_000_000;
const DEFAULT_MAX_EDGES: usize = 5_000;

#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    pub limit: Option<usize>,
    pub max_edges: Option<usize>,
    pub config: Option<Config>,
}

impl From<&RenderOptions> for ParseOptions {
    fn from(options: &RenderOptions) -> Self {
        Self {
            limit: options.limit,
            max_edges: options.max_edges,
            config: options.config.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiagramMetadata {
    pub kind: &'static str,
    pub config_section: Option<&'static str>,
    pub keep_comments: bool,
    pub math: bool,
}

impl DiagramMetadata {
    pub const fn new(kind: &'static str) -> Self {
        Self {
            kind,
            config_section: match kind.as_bytes() {
                b"xychart" => Some("xyChart"),
                b"wardley" => Some("wardley-beta"),
                _ => None,
            },
            keep_comments: matches!(kind.as_bytes(), b"agentflow" | b"eventmodeling"),
            math: matches!(
                kind.as_bytes(),
                b"block"
                    | b"class"
                    | b"er"
                    | b"flowchart"
                    | b"kanban"
                    | b"mindmap"
                    | b"requirement"
                    | b"sequence"
                    | b"state"
                    | b"swimlane"
                    | b"usecase"
            ),
        }
    }
}

/// A synchronous handler. Registering the same canonical kind replaces its handler.
pub trait Diagram: Send + Sync {
    fn metadata(&self) -> DiagramMetadata;
    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        line_offset: usize,
        max_edges: usize,
    ) -> Result<DiagramModel, PeleError>;
    fn render(
        &self,
        model: &DiagramModel,
        config: &Config,
        options: &RenderOptions,
        measurer: &dyn TextMeasurer,
    ) -> Result<Rendered, PeleError>;
}

trait IntoRendered {
    fn into_result(self) -> Result<Rendered, PeleError>;
}
impl IntoRendered for Rendered {
    fn into_result(self) -> Result<Rendered, PeleError> {
        Ok(self)
    }
}
impl IntoRendered for Result<Rendered, PeleError> {
    fn into_result(self) -> Result<Rendered, PeleError> {
        self
    }
}

macro_rules! builtins {
    ($(($variant:ident, $kind:literal, $model:ty, $parse:path, $render:path)),* $(,)?) => {
        pub const SUPPORTED: &[&str] = &[$($kind),*];

        #[derive(Clone)]
        pub enum DiagramModel {
            $($variant(Box<$model>),)*
            Custom(Arc<dyn Any + Send + Sync>),
        }

        impl DiagramModel {
            /// The original model discriminant; swimlane intentionally reuses
            /// the flowchart model while ParsedDiagram retains its canonical kind.
            pub fn kind(&self) -> Option<&'static str> {
                match self {
                    $(Self::$variant(_) => Some(if $kind == "swimlane" { "flowchart" } else { $kind }),)*
                    Self::Custom(_) => None,
                }
            }
        }

        impl fmt::Debug for DiagramModel {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(Self::$variant(model) => f.debug_tuple(stringify!($variant)).field(model).finish(),)*
                    Self::Custom(_) => f.write_str("Custom(..)"),
                }
            }
        }

        struct Builtin(&'static str);
        impl Diagram for Builtin {
            fn metadata(&self) -> DiagramMetadata { DiagramMetadata::new(self.0) }
            fn parse(&self, source: &str, config: &Config, title: Option<&str>, line_offset: usize, max_edges: usize) -> Result<DiagramModel, PeleError> {
                match self.0 {
                    $($kind => $parse(source, config, title, line_offset, max_edges).map(|m| DiagramModel::$variant(Box::new(m))),)*
                    _ => unreachable!("built-in handler without a registered kind"),
                }
            }
            fn render(&self, model: &DiagramModel, config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Result<Rendered, PeleError> {
                match (self.0, model) {
                    $(($kind, DiagramModel::$variant(model)) => IntoRendered::into_result($render(model, config, options, measurer)),)*
                    _ => Err(PeleError::new("The parsed model does not belong to this diagram handler.", PeleErrorCode::Semantic)),
                }
            }
        }
    }
}

builtins! {
    (Flowchart, "flowchart", crate::diagrams::flowchart::db::FlowDb, parse_flowchart, crate::diagrams::flowchart::render::render_flowchart),
    (Pie, "pie", crate::diagrams::pie::model::PieModel, parse_pie, crate::diagrams::pie::render::render_pie),
    (Cynefin, "cynefin", crate::diagrams::cynefin::Model, crate::diagrams::cynefin::parse, crate::diagrams::cynefin::render),
    (Quadrant, "quadrantChart", crate::diagrams::quadrant::Model, crate::diagrams::quadrant::parse, crate::diagrams::quadrant::render),
    (Er, "er", crate::diagrams::er::Model, crate::diagrams::er::parse, crate::diagrams::er::render),
    (Xychart, "xychart", crate::diagrams::xychart::Model, crate::diagrams::xychart::parse, crate::diagrams::xychart::render),
    (Kanban, "kanban", crate::diagrams::kanban::Model, crate::diagrams::kanban::parse, crate::diagrams::kanban::render),
    (Treemap, "treemap", crate::diagrams::treemap::Model, crate::diagrams::treemap::parse, crate::diagrams::treemap::render),
    (Requirement, "requirement", crate::diagrams::requirement::Model, crate::diagrams::requirement::parse, crate::diagrams::requirement::render),
    (TreeView, "treeView", crate::diagrams::treeview::Model, crate::diagrams::treeview::parse, crate::diagrams::treeview::render),
    (Sankey, "sankey", crate::diagrams::sankey::Model, crate::diagrams::sankey::parse, crate::diagrams::sankey::render),
    (Ishikawa, "ishikawa", crate::diagrams::ishikawa::Model, crate::diagrams::ishikawa::parse, crate::diagrams::ishikawa::render),
    (Git, "gitGraph", crate::diagrams::git::Model, crate::diagrams::git::parse, crate::diagrams::git::render),
    (Architecture, "architecture", crate::diagrams::architecture::Model, crate::diagrams::architecture::parse, crate::diagrams::architecture::render),
    (EventModeling, "eventmodeling", crate::diagrams::eventmodeling::Model, crate::diagrams::eventmodeling::parse, crate::diagrams::eventmodeling::render),
    (Sequence, "sequence", crate::diagrams::sequence::Model, crate::diagrams::sequence::parse, crate::diagrams::sequence::render),
    (Class, "class", crate::diagrams::class::Model, crate::diagrams::class::parse, crate::diagrams::class::render),
    (Wardley, "wardley", crate::diagrams::wardley::Model, crate::diagrams::wardley::parse, crate::diagrams::wardley::render),
    (Venn, "venn", crate::diagrams::venn::Model, crate::diagrams::venn::parse, crate::diagrams::venn::render),
    (Gantt, "gantt", crate::diagrams::gantt::Model, crate::diagrams::gantt::parse, crate::diagrams::gantt::render),
    (Railroad, "railroad", crate::diagrams::railroad::Model, crate::diagrams::railroad::parse, crate::diagrams::railroad::render),
    (State, "state", crate::diagrams::state::Model, crate::diagrams::state::parse, crate::diagrams::state::render),
    (Usecase, "usecase", crate::diagrams::usecase::Model, crate::diagrams::usecase::parse, crate::diagrams::usecase::render),
    (Block, "block", crate::diagrams::block::Model, crate::diagrams::block::parse, crate::diagrams::block::render),
    (Agentflow, "agentflow", crate::diagrams::agentflow::Model, crate::diagrams::agentflow::parse, crate::diagrams::agentflow::render),
    (C4, "c4", crate::diagrams::c4::Model, crate::diagrams::c4::parse, crate::diagrams::c4::render),
    (Info, "info", crate::diagrams::info::Model, crate::diagrams::info::parse, crate::diagrams::info::render),
    (Packet, "packet", crate::diagrams::packet::Model, crate::diagrams::packet::parse, crate::diagrams::packet::render),
    (Radar, "radar", crate::diagrams::radar::Model, crate::diagrams::radar::parse, crate::diagrams::radar::render),
    (Timeline, "timeline", crate::diagrams::timeline::Model, crate::diagrams::timeline::parse, crate::diagrams::timeline::render),
    (Journey, "journey", crate::diagrams::journey::Model, crate::diagrams::journey::parse, crate::diagrams::journey::render),
    (Mindmap, "mindmap", crate::diagrams::mindmap::Model, crate::diagrams::mindmap::parse, crate::diagrams::mindmap::render),
    (Swimlane, "swimlane", crate::diagrams::swimlane::Model, crate::diagrams::swimlane::parse, crate::diagrams::swimlane::render),
}

fn parse_flowchart(
    source: &str,
    config: &Config,
    title: Option<&str>,
    _: usize,
    max_edges: usize,
) -> Result<crate::diagrams::flowchart::db::FlowDb, PeleError> {
    let inherit = config.get("flowchart").and_then(|v| v.get("inheritDir")) == Some(&Value::Bool(true));
    let mut db = crate::diagrams::flowchart::db::FlowDb::new(inherit, Some(max_edges));
    db.title = title.map(String::from);
    crate::diagrams::flowchart::parser::parse_flowchart(source, &mut db)?;
    Ok(db)
}

fn parse_pie(
    source: &str,
    _: &Config,
    title: Option<&str>,
    _: usize,
    _: usize,
) -> Result<crate::diagrams::pie::model::PieModel, PeleError> {
    crate::diagrams::pie::model::build_pie(crate::diagrams::pie::parser::parse_pie(source)?, title)
}

/// A model and its parsing context. Only parsing constructs this context.
#[derive(Clone)]
pub struct ParsedDiagram {
    pub model: DiagramModel,
    pub kind: String,
    pub mermaid_type: String,
    config: Config,
    handler: Arc<dyn Diagram>,
}

impl fmt::Debug for ParsedDiagram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParsedDiagram")
            .field("model", &self.model)
            .field("kind", &self.kind)
            .field("mermaid_type", &self.mermaid_type)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl ParsedDiagram {
    pub fn config(&self) -> &Config {
        &self.config
    }
}

#[derive(Clone)]
pub struct Renderer {
    diagrams: IndexMap<&'static str, Arc<dyn Diagram>>,
}

impl Default for Renderer {
    fn default() -> Self {
        let mut renderer = Self::empty();
        for kind in SUPPORTED {
            renderer.register(Arc::new(Builtin(kind)));
        }
        renderer
    }
}

fn tag(mut error: PeleError, kind: &str, mermaid_type: &str) -> PeleError {
    error.diagram.get_or_insert_with(|| kind.to_string());
    error.mermaid_type.get_or_insert_with(|| mermaid_type.to_string());
    error.message = crate::text::entities::written_codes(&error.message);
    error.snippet = crate::text::entities::written_codes(&error.snippet);
    error
}

impl Renderer {
    pub fn empty() -> Self {
        Self { diagrams: IndexMap::new() }
    }
    /// Register one of the shipped handlers in a selective renderer.
    pub fn register_builtin(&mut self, kind: &str) -> bool {
        if let Some(&kind) = SUPPORTED.iter().find(|&&value| value == kind) {
            self.register(Arc::new(Builtin(kind)));
            true
        } else {
            false
        }
    }
    pub fn register(&mut self, diagram: Arc<dyn Diagram>) {
        self.diagrams.insert(diagram.metadata().kind, diagram);
    }
    pub fn registered(&self, kind: &str) -> bool {
        self.diagrams.contains_key(kind)
    }
    pub fn supports(&self, text: &str) -> bool {
        try_detect_type(text).ok().flatten().is_some_and(|kind| self.registered(kind))
    }

    pub fn parse(&self, text: &str, options: &ParseOptions) -> Result<ParsedDiagram, PeleError> {
        let limit = options.limit.unwrap_or(DEFAULT_LIMIT);
        if text.encode_utf16().count() > limit {
            return Err(PeleError::new(
                format!("Diagram source is longer than the limit of {limit} characters."),
                PeleErrorCode::Limit,
            ));
        }
        let pre = crate::preprocess::preprocess(text)?;
        let found = crate::detect::detect_diagram(&pre.text)
            .ok_or_else(|| PeleError::new("No diagram type detected for the given text.", PeleErrorCode::UnsupportedDiagram))?;
        let handler = self
            .diagrams
            .get(found.kind)
            .ok_or_else(|| {
                tag(
                    PeleError::new(
                        format!("Diagram type \"{}\" is not supported yet.", found.kind),
                        PeleErrorCode::UnsupportedDiagram,
                    ),
                    found.kind,
                    found.mermaid_type,
                )
            })?
            .clone();
        let metadata = handler.metadata();
        let mut config = options.config.clone().unwrap_or_default();
        config.extend(pre.config);
        let source = crate::preprocess::encode_entities(if metadata.keep_comments { &pre.with_comments } else { &pre.text }) + "\n";
        let model = crate::errors::locating_in(&source, text, metadata.keep_comments, || {
            handler.parse(
                &source,
                &config,
                pre.title.as_deref(),
                pre.line_offset,
                options.max_edges.unwrap_or(DEFAULT_MAX_EDGES),
            )
        })
        .map_err(|e| tag(e, found.kind, found.mermaid_type))?;
        Ok(ParsedDiagram {
            model,
            kind: found.kind.to_string(),
            mermaid_type: found.mermaid_type.to_string(),
            config,
            handler,
        })
    }

    pub fn render(&self, text: &str, options: &RenderOptions) -> Result<RenderResult, PeleError> {
        self.render_with(text, options, &MetricsMeasurer)
    }
    pub fn render_with(&self, text: &str, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Result<RenderResult, PeleError> {
        self.render_parsed_with(&self.parse(text, &ParseOptions::from(options))?, options, measurer)
    }
    pub fn render_parsed(&self, parsed: &ParsedDiagram, options: &RenderOptions) -> Result<RenderResult, PeleError> {
        self.render_parsed_with(parsed, options, &MetricsMeasurer)
    }
    pub fn render_parsed_with(
        &self,
        parsed: &ParsedDiagram,
        options: &RenderOptions,
        measurer: &dyn TextMeasurer,
    ) -> Result<RenderResult, PeleError> {
        let meta = parsed.handler.metadata();
        let fixed = parsed
            .config
            .get(meta.config_section.unwrap_or(meta.kind))
            .and_then(|v| v.get("useMaxWidth"))
            == Some(&Value::Bool(false));
        let mut opts = options.clone();
        opts.responsive = Some(options.responsive.unwrap_or(!fixed));
        if !meta.math {
            opts.math = None;
        }
        let context = crate::text::context::RenderContext::new(measurer, &mut opts);
        let mut rendered = parsed
            .handler
            .render(&parsed.model, &parsed.config, &opts, &context)
            .map_err(|e| tag(e, &parsed.kind, &parsed.mermaid_type))?;
        let limit = options.output_limit.unwrap_or(DEFAULT_OUTPUT_LIMIT);
        if rendered.svg.encode_utf16().count() > limit {
            return Err(tag(
                PeleError::new(
                    format!("Diagram output is longer than the limit of {limit} characters."),
                    PeleErrorCode::Limit,
                ),
                &parsed.kind,
                &parsed.mermaid_type,
            ));
        }
        for link in &mut rendered.links {
            link.id = crate::text::entities::decode_entities(&link.id);
        }
        Ok(RenderResult {
            svg: rendered.svg,
            width: rendered.width,
            height: rendered.height,
            kind: parsed.kind.clone(),
            mermaid_type: parsed.mermaid_type.clone(),
            links: rendered.links,
        })
    }
}

pub(crate) fn default_renderer() -> &'static Renderer {
    static RENDERER: OnceLock<Renderer> = OnceLock::new();
    RENDERER.get_or_init(Renderer::default)
}

pub fn try_detect_type(text: &str) -> Result<Option<&'static str>, PeleError> {
    let pre = crate::preprocess::preprocess(text)?;
    Ok(crate::detect::detect_diagram(&pre.text).map(|d| d.kind))
}
pub fn parse(text: &str, options: &ParseOptions) -> Result<ParsedDiagram, PeleError> {
    default_renderer().parse(text, options)
}
pub fn render_parsed(parsed: &ParsedDiagram, options: &RenderOptions) -> Result<RenderResult, PeleError> {
    default_renderer().render_parsed(parsed, options)
}
pub fn render_parsed_with(parsed: &ParsedDiagram, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Result<RenderResult, PeleError> {
    default_renderer().render_parsed_with(parsed, options, measurer)
}
