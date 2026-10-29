// The Typst plugin: renders Mermaid with pele_rs and gives back SVG that needs no CSS.

use pele_rs::svg::inline::inline_styles;
use pele_rs::{IconResolver, MathMarkup, MathResolver, MonospacePart, RenderOptions};
use serde_json::Value;
use wasm_minimal_protocol::*;

initiate_protocol!();

// Assets contain SVG fragments, with fixed pixel dimensions for math. Gantt always
// requires an explicit clock because WASM has no ambient clock or local timezone.
#[wasm_func]
pub fn render(source: &[u8], options: &[u8]) -> Result<Vec<u8>, String> {
    let source = std::str::from_utf8(source).map_err(|e| e.to_string())?;
    let options: Value = if options.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(options).map_err(|e| e.to_string())?
    };
    let num = |k: &str| options.get(k).and_then(Value::as_f64);
    let text = |k: &str| options.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from);
    let now = num("now").filter(|v| v.is_finite());
    if pele_rs::try_detect_type(source).map_err(|e| e.message)? == Some("gantt") && now.is_none() {
        return Err("Gantt diagrams require a finite `now` timestamp in Unix milliseconds.".into());
    }
    let time_zone = text("timeZone").unwrap_or_else(|| "UTC".into());
    if time_zone.parse::<chrono_tz::Tz>().is_err() {
        return Err(format!("Unknown IANA timezone: {time_zone}"));
    }
    let monospace = options
        .get("monospace")
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_array()
                .ok_or_else(|| "monospace must be an array".to_string())?
                .iter()
                .map(|v| match v.as_str() {
                    Some("members") => Ok(MonospacePart::Members),
                    Some("attributes") => Ok(MonospacePart::Attributes),
                    Some("files") => Ok(MonospacePart::Files),
                    Some("messages") => Ok(MonospacePart::Messages),
                    _ => Err("monospace entries must be members, attributes, files, or messages".to_string()),
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let icons = options
        .get("icons")
        .filter(|v| !v.is_null())
        .map(|v| {
            let assets = v.as_object().ok_or("icons must be a dictionary of SVG strings")?.clone();
            if assets.values().any(|v| !v.is_string()) {
                return Err("icons must be a dictionary of SVG strings");
            }
            Ok(IconResolver::new(move |name| {
                assets.get(name).and_then(Value::as_str).map(String::from)
            }))
        })
        .transpose()?;
    let math = options
        .get("math")
        .filter(|v| !v.is_null())
        .map(|v| {
            let assets = v.as_object().ok_or("math must be a dictionary of SVG assets")?;
            let mut parsed = std::collections::HashMap::new();
            for (tex, asset) in assets {
                if asset.get("html").is_some() {
                    return Err("Typst math assets must contain SVG; HTML math is unsupported");
                }
                let svg = asset
                    .get("svg")
                    .and_then(Value::as_str)
                    .ok_or("math assets require an SVG string")?;
                let width = asset
                    .get("width")
                    .and_then(Value::as_f64)
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .ok_or("math assets require a positive finite pixel width")?;
                let height = asset
                    .get("height")
                    .and_then(Value::as_f64)
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .ok_or("math assets require a positive finite pixel height")?;
                parsed.insert(
                    tex.clone(),
                    MathMarkup::Svg {
                        svg: svg.into(),
                        width,
                        height,
                    },
                );
            }
            Ok(MathResolver::new(move |tex, _| parsed.get(tex).cloned()))
        })
        .transpose()?;
    let opts = RenderOptions {
        font_size: num("fontSize"),
        font_family: text("fontFamily"),
        font_family_mono: text("fontFamilyMono"),
        padding: num("padding"),
        max_width: num("maxWidth"),
        responsive: Some(false),
        config: options.get("config").and_then(Value::as_object).cloned(),
        now,
        time_zone: Some(time_zone),
        monospace,
        icons,
        math,
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
    if options.get("background").and_then(Value::as_bool) == Some(true)
        && let Some(end) = svg.find('>')
    {
        svg.insert_str(end + 1, "<rect width=\"100%\" height=\"100%\" fill=\"var(--_bg)\"/>");
    }
    Ok(inline_styles(&svg, &theme).into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clock_timezone_and_svg_asset_contracts() {
        let source = b"gantt\n dateFormat YYYY-MM-DD\n Task :2024-03-09, 3d";
        assert!(render(source, b"{}").unwrap_err().contains("finite `now`"));
        assert!(render(source, br#"{"now":1e999}"#).unwrap_err().contains("finite `now`"));
        assert!(render(source, br#"{"now":1710072000000,"timeZone":"America/Los_Angeles"}"#).is_ok());
        assert!(
            render(source, br#"{"now":0,"timeZone":"not/a/zone"}"#)
                .unwrap_err()
                .contains("Unknown IANA")
        );
        let flow = b"flowchart LR\n A[\"$$x$$ fa:fa-star\"]";
        assert!(
            render(flow, br#"{"math":{"x":{"html":"<b>x</b>","width":10,"height":10}}}"#)
                .unwrap_err()
                .contains("HTML")
        );
        assert!(
            render(flow, br#"{"math":{"x":{"svg":"","width":0,"height":10}}}"#)
                .unwrap_err()
                .contains("positive finite")
        );
        let svg = render(
            flow,
            br#"{"math":{"x":{"svg":"<path id='formula'/>","width":30,"height":18}},"icons":{"fa:fa-star":"<path id='icon'/>"}}"#,
        )
        .unwrap();
        let svg = String::from_utf8(svg).unwrap();
        assert!(svg.contains("id=\"formula\""));
        assert!(svg.contains("id=\"icon\""));
        assert!(!svg.contains("var("));
    }
}
