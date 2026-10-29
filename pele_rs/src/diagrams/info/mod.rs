//! The pinned upstream Pele information diagram.
use crate::RenderOptions;
use crate::diagrams::common::Rendered;
use crate::diagrams::tokens::*;
use crate::errors::PeleError;
use crate::preprocess::Config;
use crate::svg::builder::label_svg;
use crate::svg::root::{Accessible, svg_document};
use crate::text::label::layout_label;
use crate::text::measurer::{BOLD, TextMeasurer};

#[derive(Debug, Clone, Default)]
pub struct Model {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
}

pub fn parse(source: &str, _config: &Config, title: Option<&str>, _line_offset: usize, _max_edges: usize) -> Result<Model, PeleError> {
    let types = vec![
        keyword("showInfo"),
        keyword("info"),
        acc_descr_type(),
        acc_title_type(),
        title_type(),
        newline_type(),
        whitespace_type(),
        yaml_type(),
        directive_type(),
        comment_type(),
    ];
    let tokens = tokenize(&source.encode_utf16().collect::<Vec<_>>(), &types, "info")?;
    let mut r = Reader {
        i: 0,
        tokens,
        types: &types,
        diagram: "info",
    };
    let mut model = Model::default();
    while r.accept(5) {}
    r.expect(1)?;
    while r.accept(5) {}
    if r.accept(0) {
        while r.accept(5) {}
    }
    while r.kind() != -1 {
        match r.kind() {
            2 => model.acc_descr = Some(acc_descr_value(&r.take())),
            3 => model.acc_title = Some(acc_title_value(&r.take())),
            4 => model.title = Some(title_value(&r.take())),
            _ => return Err(r.fail("a title or the end of input")),
        }
        if r.kind() == -1 {
            break;
        }
        if r.kind() != 5 {
            return Err(r.fail("a line break or the end of input"));
        }
        while r.accept(5) {}
    }
    model.title = model.title.filter(|v| !v.is_empty()).or_else(|| title.map(String::from));
    model.acc_title = model
        .acc_title
        .filter(|v| !v.is_empty())
        .map(|v| crate::util::trim_start(&v).to_string());
    model.acc_descr = model.acc_descr.filter(|v| !v.is_empty()).map(|v| normalize_description(&v));
    Ok(model)
}

fn normalize_description(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '\n' {
            while chars.peek().is_some_and(|&c| crate::util::is_js_space(c)) {
                chars.next();
            }
        }
    }
    out
}

pub fn render(model: &Model, _config: &Config, options: &RenderOptions, measurer: &dyn TextMeasurer) -> Rendered {
    let size = options.font_size.unwrap_or(16.0);
    let pad = options.padding.unwrap_or(8.0);
    let title = layout_label(model.title.as_deref(), false, measurer, size, 4000.0, BOLD);
    let title_height = if title.height > 0.0 { title.height + 8.0 } else { 0.0 };
    let text = layout_label(Some("Pele v1.12.0"), false, measurer, size, 4000.0, 0);
    let width = (title.width.max(text.width) + 2.0 * pad).ceil();
    let height = (title_height + text.height + 2.0 * pad).ceil();
    let content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        " class=\"pele-title\" font-weight=\"var(--_tw)\"",
    ) + &label_svg(
        &text,
        width / 2.0,
        pad + title_height + text.height / 2.0,
        " class=\"pele-version\" fill=\"var(--_m)\"",
    );
    let acc = Accessible {
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    };
    Rendered {
        svg: svg_document("info", width, height, size, options, &acc, &content),
        width,
        height,
        links: vec![],
    }
}
