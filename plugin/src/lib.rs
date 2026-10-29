// The Typst plugin: renders Mermaid with pele_rs and gives back SVG that needs no CSS.

use pele_rs::svg::inline::inline_styles;
use pele_rs::RenderOptions;
use serde_json::Value;
use wasm_minimal_protocol::*;

initiate_protocol!();

// `options` is a JSON object: fontSize, fontFamily, fontFamilyMono, padding, maxWidth, config
// (Mermaid configuration), theme (CSS custom properties such as "--pele-fg"), and background
// (whether to paint the theme's background behind the diagram).
#[wasm_func]
pub fn render(source: &[u8], options: &[u8]) -> Result<Vec<u8>, String> {
    let source = std::str::from_utf8(source).map_err(|e| e.to_string())?;
    let options: Value = if options.is_empty() { Value::Null } else { serde_json::from_slice(options).map_err(|e| e.to_string())? };
    let num = |k: &str| options.get(k).and_then(Value::as_f64);
    let text = |k: &str| options.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from);
    let opts = RenderOptions {
        font_size: num("fontSize"),
        font_family: text("fontFamily"),
        font_family_mono: text("fontFamilyMono"),
        padding: num("padding"),
        max_width: num("maxWidth"),
        responsive: Some(false),
        config: options.get("config").and_then(Value::as_object).cloned(),
        ..Default::default()
    };
    let rendered = pele_rs::render(source, &opts).map_err(|e| e.message)?;
    let mut theme: Vec<(String, String)> = Vec::new();
    if let Some(Value::Object(t)) = options.get("theme") {
        for (k, v) in t {
            if let Some(s) = v.as_str() {
                theme.push((k.clone(), s.to_string()));
            }
        }
    }
    let mut svg = rendered.svg;
    if options.get("background").and_then(Value::as_bool) == Some(true) {
        if let Some(end) = svg.find('>') {
            svg.insert_str(end + 1, "<rect width=\"100%\" height=\"100%\" fill=\"var(--_bg)\"/>");
        }
    }
    Ok(inline_styles(&svg, &theme).into_bytes())
}
