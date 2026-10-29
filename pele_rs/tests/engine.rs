//! Behavioral contracts translated from the pinned engine, entries and math suites.
use pele_rs::*;
use serde_json::json;
use std::sync::{Arc, Mutex};

const FLOW: &str = "flowchart LR\n A --> B";

struct Replacement(&'static str);
impl Diagram for Replacement {
    fn metadata(&self) -> DiagramMetadata {
        DiagramMetadata::new("flowchart")
    }
    fn parse(&self, _: &str, _: &Config, title: Option<&str>, _: usize, _: usize) -> Result<DiagramModel, PeleError> {
        let value = title.map_or_else(|| self.0.to_string(), |title| format!("{}:{title}", self.0));
        Ok(DiagramModel::Custom(Arc::new(value)))
    }
    fn render(
        &self,
        model: &DiagramModel,
        _: &Config,
        _: &RenderOptions,
        _: &dyn TextMeasurer,
    ) -> Result<diagrams::common::Rendered, PeleError> {
        let DiagramModel::Custom(value) = model else {
            panic!("wrong handler for model")
        };
        let value = value.downcast_ref::<String>().unwrap();
        Ok(diagrams::common::Rendered {
            svg: format!("{}:{value}", self.0),
            width: 1.0,
            height: 1.0,
            links: vec![],
        })
    }
}

#[test]
fn selective_registry_and_original_handler_retention() {
    let mut registry = Renderer::empty();
    assert!(!registry.supports(FLOW));
    let error = registry.render(FLOW, &RenderOptions::default()).unwrap_err();
    assert_eq!(error.code, PeleErrorCode::UnsupportedDiagram);
    assert_eq!(error.mermaid_type.as_deref(), Some("flowchart-v2"));
    assert!(registry.register_builtin("flowchart"));
    assert!(!registry.register_builtin("not-a-type"));
    assert!(registry.registered("flowchart"));
    let original = registry.parse(FLOW, &ParseOptions::default()).unwrap();
    let svg = registry.render_parsed(&original, &RenderOptions::default()).unwrap().svg;
    registry.register(Arc::new(Replacement("first")));
    let first = registry.parse(FLOW, &ParseOptions::default()).unwrap();
    registry.register(Arc::new(Replacement("second")));
    assert_eq!(registry.render(FLOW, &RenderOptions::default()).unwrap().svg, "second:second");
    assert_eq!(
        registry.render_parsed(&first, &RenderOptions::default()).unwrap().svg,
        "first:first"
    );
    assert_eq!(render_parsed(&original, &RenderOptions::default()).unwrap().svg, svg);
    assert!(!Renderer::empty().supports(FLOW));
    assert!(supports(FLOW));
    let empty_title = format!("---\ntitle: []\n---\n{FLOW}");
    assert_eq!(
        registry.render(&empty_title, &RenderOptions::default()).unwrap().svg,
        "second:second:"
    );
}

#[test]
fn config_precedence_and_parse_only_options_are_retained() {
    let source = "---\nconfig:\n flowchart:\n  curve: linear\n  useMaxWidth: false\n---\nflowchart LR\n A --> B";
    let options = ParseOptions {
        config: Some(
            json!({"flowchart":{"curve":"basis","nodeSpacing":999},"host":true})
                .as_object()
                .unwrap()
                .clone(),
        ),
        ..Default::default()
    };
    let parsed = parse(source, &options).unwrap();
    assert_eq!(parsed.config()["host"], true);
    assert_eq!(parsed.config()["flowchart"]["curve"], "linear");
    assert!(parsed.config()["flowchart"].get("nodeSpacing").is_none());
    let baseline = render_parsed(&parsed, &RenderOptions::default()).unwrap();
    assert!(!baseline.svg.contains("max-width:100%"));
    let ignored = RenderOptions {
        limit: Some(0),
        max_edges: Some(0),
        config: Some(json!({"flowchart":{"curve":"step"}}).as_object().unwrap().clone()),
        ..Default::default()
    };
    assert_eq!(render_parsed(&parsed, &ignored).unwrap().svg, baseline.svg);
    let responsive = RenderOptions {
        responsive: Some(true),
        ..Default::default()
    };
    assert!(render_parsed(&parsed, &responsive).unwrap().svg.contains("max-width:100%"));
}

#[test]
fn fallible_detection_and_utf16_limits() {
    let bad = "---\ntitle: [unclosed\n---\nflowchart LR\n A";
    assert!(try_detect_type(bad).is_err());
    assert_eq!(detect_type(bad), None);
    assert!(!supports(bad));
    let source = "flowchart LR\n A[😀]";
    let count = source.encode_utf16().count();
    assert!(
        parse(
            source,
            &ParseOptions {
                limit: Some(count),
                ..Default::default()
            }
        )
        .is_ok()
    );
    let error = parse(
        source,
        &ParseOptions {
            limit: Some(count - 1),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.code, PeleErrorCode::Limit);
    assert_eq!(error.mermaid_type, None);
    let error = render(
        FLOW,
        &RenderOptions {
            output_limit: Some(1),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.code, PeleErrorCode::Limit);
    assert_eq!(error.mermaid_type.as_deref(), Some("flowchart-v2"));
}

#[test]
fn resolver_fallbacks_reentrancy_and_unwind_recovery() {
    let source = "flowchart LR\n A[\"$$x$$ fa:fa-car\"] --> B[\"$$x$$\"]";
    let calls = Arc::new(Mutex::new(Vec::new()));
    let recorded = calls.clone();
    let icons = IconResolver::new(|name| Some(format!("<path data-name=\"{name}\"/>")));
    let nested_icons = icons.clone();
    let math = MathResolver::new(move |tex, size| {
        recorded.lock().unwrap().push((tex.to_string(), size));
        render(
            "flowchart LR\n B[\"fa:fa-star <b></b> $$y$$\"]",
            &RenderOptions {
                icons: Some(nested_icons.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        Some(MathMarkup::Svg {
            svg: format!("<g id=\"{tex}\"/>"),
            width: 30.0,
            height: 18.0,
        })
    });
    let options = RenderOptions {
        icons: Some(icons),
        math: Some(math),
        ..Default::default()
    };
    let parsed = parse(source, &ParseOptions::default()).unwrap();
    let rendered = render_parsed(&parsed, &options).unwrap();
    assert!(rendered.svg.contains("<g id=\"x\"/>"));
    assert!(!rendered.svg.contains("fa-star"));
    assert_eq!(*calls.lock().unwrap(), vec![("x".into(), 16.0)]);
    let plain = render_parsed(&parsed, &RenderOptions::default()).unwrap().svg;
    for math in [
        MathResolver::new(|_, _| None),
        MathResolver::new(|_, _| {
            Some(MathMarkup::Svg {
                svg: String::new(),
                width: f64::NAN,
                height: 10.0,
            })
        }),
    ] {
        assert_eq!(
            render_parsed(
                &parsed,
                &RenderOptions {
                    math: Some(math),
                    ..Default::default()
                }
            )
            .unwrap()
            .svg,
            plain
        );
    }
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = render(
            source,
            &RenderOptions {
                math: Some(MathResolver::new(|_, _| panic!("host callback"))),
                ..Default::default()
            },
        );
    }));
    assert!(panic.is_err());
    assert_eq!(render(source, &RenderOptions::default()).unwrap().svg, plain);
    assert_eq!(render_parsed(&parsed, &options).unwrap().svg, rendered.svg);
}

#[test]
fn concurrent_renders_do_not_share_assets_or_widths() {
    let handles: Vec<_> = (1..=8)
        .map(|id| {
            std::thread::spawn(move || {
                struct Measure(f64);
                impl TextMeasurer for Measure {
                    fn width(&self, text: &str, size: f64, style: u32) -> f64 {
                        MetricsMeasurer.width(text, size, style) * self.0
                    }
                }
                let source = "flowchart LR\n A[\"$$x$$ fa:fa-car words\"]";
                let options = RenderOptions {
                    math: Some(MathResolver::new(move |_, _| {
                        Some(MathMarkup::Svg {
                            svg: format!("<g id=\"render-{id}\"/>"),
                            width: 30.0,
                            height: 18.0,
                        })
                    })),
                    ..Default::default()
                };
                let expected = render_with(source, &options, &Measure(id as f64)).unwrap().svg;
                for _ in 0..10 {
                    assert_eq!(render_with(source, &options, &Measure(id as f64)).unwrap().svg, expected);
                }
                expected
            })
        })
        .collect();
    let output: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    for (i, svg) in output.iter().enumerate() {
        assert!(svg.contains(&format!("render-{}", i + 1)));
    }
    for pair in output.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
}

#[test]
fn source_context_survives_nested_parse_errors() {
    let source = "---\ntitle: Mapping\n---\n%% comment\nflowchart LR\n A[😀] -->";
    let expected = render(source, &RenderOptions::default()).unwrap_err();
    let resolver = MathResolver::new(move |_, _| {
        let error = parse(source, &ParseOptions::default()).unwrap_err();
        assert_eq!(error.line, 6);
        None
    });
    render(
        "flowchart LR\n A[\"$$x$$\"]",
        &RenderOptions {
            math: Some(resolver),
            ..Default::default()
        },
    )
    .unwrap();
    let again = render(source, &RenderOptions::default()).unwrap_err();
    assert_eq!(again.snippet, expected.snippet);
    assert_eq!(again.column, expected.column);
}
