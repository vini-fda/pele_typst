// Exact upstream oracle comparisons; no case is skipped based on Rust detection.
use serde_json::{Value, json};
mod support;
use std::sync::{Arc, Mutex};
use support::options;

fn compare(kind: &str, fixtures: &str) {
    let cases: Vec<Value> = serde_json::from_str(fixtures).unwrap();
    assert!(!cases.is_empty(), "missing cases for {kind}");
    let mut failures = Vec::new();
    let mut total = 0;
    for (i, case) in cases.iter().enumerate() {
        let source = case["source"].as_str().unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let opts = options(&case["options"], calls.clone());
        let got = pele_rs::render(source, &opts);
        let detected = pele_rs::detect_type(source);
        let mut problem = None;
        if detected != case["detected"].as_str() {
            problem = Some(format!("detection: got {detected:?}, want {}", case["detected"]));
        }
        if let Some(want) = case.get("parsed") {
            let parsed = match pele_rs::parse(source, &pele_rs::ParseOptions::from(&opts)) {
                Ok(p) => json!({"type":p.model.kind(),"mermaidType":p.mermaid_type}),
                Err(e) => {
                    json!({"error":support::error_value(&e)})
                }
            };
            if parsed != *want {
                problem = Some(format!("parse differs\n  want: {want}\n  got: {parsed}"));
            }
        }
        match got {
            Ok(got) => {
                if let Some(want) = case.get("result") {
                    if got.svg != want["svg"].as_str().unwrap() {
                        let wanted = want["svg"].as_str().unwrap();
                        let at = got
                            .svg
                            .bytes()
                            .zip(wanted.bytes())
                            .position(|(a, b)| a != b)
                            .unwrap_or(got.svg.len().min(wanted.len()));
                        let fragment = |s: &str| {
                            let start = s
                                .char_indices()
                                .take_while(|(i, _)| *i < at.min(s.len()))
                                .count()
                                .saturating_sub(50);
                            s.chars().skip(start).take(180).collect::<String>()
                        };
                        problem = Some(format!(
                            "SVG differs at byte {at}\n  want: {}\n  got:  {}",
                            fragment(wanted),
                            fragment(&got.svg)
                        ));
                    } else if got.width != want["width"].as_f64().unwrap() || got.height != want["height"].as_f64().unwrap() {
                        problem = Some(format!(
                            "dimensions: got {}x{}, want {}x{}",
                            got.width, got.height, want["width"], want["height"]
                        ));
                    } else if got.kind != want["type"] || got.mermaid_type != want["mermaidType"] {
                        problem = Some("result type metadata differs".to_string());
                    } else {
                        let links = Value::Array(
                            got.links
                                .iter()
                                .map(|l| json!({"id":l.id,"href":l.href,"internal":l.internal}))
                                .collect(),
                        );
                        if links != want["links"] {
                            problem = Some(format!("links differ: {links}"));
                        }
                    }
                } else {
                    problem = Some(format!("render succeeded, expected {}", case["error"]));
                }
            }
            Err(error) => {
                let got = support::error_value(&error);
                if got != case["error"] {
                    problem = Some(format!(
                        "error differs\n  want: {}\n  got: {got}",
                        case.get("error").unwrap_or(&Value::Null)
                    ));
                }
            }
        }
        if let Some(expected) = case.get("calls") {
            let actual = Value::Array(calls.lock().unwrap().clone());
            if actual != *expected {
                problem.get_or_insert_with(|| format!("resolver calls differ\n  want: {expected}\n  got: {actual}"));
            }
        }
        if case.get("reuseWidths").is_some() && problem.is_none() {
            let parsed = pele_rs::parse(source, &pele_rs::ParseOptions::from(&opts)).unwrap();
            for width in [None, Some(320.0), Some(500.0), None] {
                let mut options = opts.clone();
                options.max_width = width;
                let reused = pele_rs::render_parsed(&parsed, &options);
                let fresh = pele_rs::render(source, &options);
                if format!("{reused:?}") != format!("{fresh:?}") {
                    problem = Some(format!("parsed-model reuse differs at width {width:?}"));
                    break;
                }
            }
        }
        if let Some(problem) = problem {
            total += 1;
            if failures.len() < 8 {
                failures.push(format!("case {i}:\n{source}\n{problem}"));
            }
        }
    }
    assert_eq!(total, 0, "{total}/{} {kind} cases differ:\n{}", cases.len(), failures.join("\n\n"));
}

macro_rules! fixture {
    ($test:ident,$kind:literal) => {
        #[test]
        fn $test() {
            compare($kind, include_str!(concat!("fixtures/", $kind, ".json")));
        }
    };
}
fixture!(flowchart, "flowchart");
fixture!(pie, "pie");
fixture!(er, "er");
fixture!(mindmap, "mindmap");
fixture!(kanban, "kanban");
fixture!(timeline, "timeline");
fixture!(journey, "journey");
fixture!(class, "class");
fixture!(quadrantchart, "quadrantChart");
fixture!(xychart, "xychart");
fixture!(state, "state");
fixture!(gantt, "gantt");
fixture!(architecture, "architecture");
fixture!(requirement, "requirement");
fixture!(sankey, "sankey");
fixture!(packet, "packet");
fixture!(radar, "radar");
fixture!(treemap, "treemap");
fixture!(info, "info");
fixture!(sequence, "sequence");
fixture!(gitgraph, "gitGraph");
fixture!(c4, "c4");
fixture!(block, "block");
fixture!(railroad, "railroad");
fixture!(agentflow, "agentflow");
fixture!(treeview, "treeView");
fixture!(cynefin, "cynefin");
fixture!(wardley, "wardley");
fixture!(eventmodeling, "eventmodeling");
fixture!(usecase, "usecase");
fixture!(venn, "venn");
fixture!(ishikawa, "ishikawa");
fixture!(swimlane, "swimlane");

#[test]
fn every_upstream_type_is_registered() {
    let manifest: Value = serde_json::from_str(include_str!("../scripts/reference.json")).unwrap();
    let mut expected: Vec<_> = manifest["types"].as_object().unwrap().keys().map(String::as_str).collect();
    let mut actual = pele_rs::SUPPORTED.to_vec();
    expected.sort();
    actual.sort();
    assert_eq!(actual, expected);
}
