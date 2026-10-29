use pele_rs::{MonospacePart, RenderOptions};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

pub fn error_value(e: &pele_rs::PeleError) -> Value {
    let mut value = json!({"message":e.message,"code":e.code.as_str(),"type":e.diagram,"mermaidType":e.mermaid_type,"line":e.line,"column":e.column,"snippet":e.snippet});
    if let Some(units) = &e.message_utf16 {
        value["messageUtf16"] = json!(units);
    }
    value
}

pub fn options(v: &Value, calls: Arc<Mutex<Vec<Value>>>) -> RenderOptions {
    let num = |k: &str| v.get(k).and_then(Value::as_f64);
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(String::from);
    let boolean = |k: &str| v.get(k).and_then(Value::as_bool);
    let strings = |k: &str| {
        v.get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|v| v.as_str().unwrap().to_string()).collect())
    };
    let icons = v["iconMode"].as_str().map(|mode| {
        let mode = mode.to_string();
        let calls = calls.clone();
        pele_rs::IconResolver::new(move |name| {
            calls.lock().unwrap().push(json!(["icon", name]));
            match mode.as_str() {
                "missing" => None,
                "empty" => Some(String::new()),
                _ => Some("<path d=\"M2 2H22V22H2Z\"/>".into()),
            }
        })
    });
    let math = v["mathMode"].as_str().map(|mode| {
        let mode = mode.to_string();
        pele_rs::MathResolver::new(move |tex, size| {
            let recorded_size = if size.fract() == 0.0 && size.abs() < 9e15 {
                json!(size as i64)
            } else {
                json!(size)
            };
            calls.lock().unwrap().push(json!(["math", tex, recorded_size]));
            let height = match mode.as_str() {
                "invalid" => -1.0,
                "tall" => 60.0,
                _ => 18.0,
            };
            match mode.as_str() {
                "missing" => None,
                "html" => Some(pele_rs::MathMarkup::Html {
                    html: "<span>x<br></span>".into(),
                    width: 30.0,
                    height,
                }),
                _ => Some(pele_rs::MathMarkup::Svg {
                    svg: "<path d=\"M1,1H9\"/>".into(),
                    width: 30.0,
                    height,
                }),
            }
        })
    });
    RenderOptions {
        font_family: text("fontFamily"),
        font_family_mono: text("fontFamilyMono"),
        font_size: num("fontSize"),
        id_prefix: text("idPrefix"),
        responsive: boolean("responsive"),
        max_width: num("maxWidth"),
        auto_direction: boolean("autoDirection"),
        direction_breakpoint: num("directionBreakpoint"),
        padding: num("padding"),
        limit: num("limit").map(|v| v as usize),
        output_limit: num("outputLimit").map(|v| v as usize),
        max_edges: num("maxEdges").map(|v| v as usize),
        link_schemes: strings("linkSchemes"),
        image_schemes: strings("imageSchemes"),
        links: boolean("links"),
        images: boolean("images"),
        link_rel: text("linkRel"),
        config: v.get("config").and_then(Value::as_object).cloned(),
        now: num("now"),
        time_zone: text("timeZone").or_else(|| Some("UTC".into())),
        icons,
        math,
        monospace: v.get("monospace").and_then(Value::as_array).map(|a| {
            a.iter()
                .map(|s| match s.as_str().unwrap() {
                    "members" => MonospacePart::Members,
                    "attributes" => MonospacePart::Attributes,
                    "files" => MonospacePart::Files,
                    "messages" => MonospacePart::Messages,
                    _ => panic!("unknown monospace part"),
                })
                .collect()
        }),
    }
}
