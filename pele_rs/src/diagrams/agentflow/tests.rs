use super::*;
use serde_json::{Value, json};
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b)),
        (Value::Object(a), Value::Object(b)) => a.len() == b.len() && a.iter().all(|(k, a)| b.get(k).is_some_and(|b| same(a, b))),
        _ => a == b,
    }
}
fn difference(a: &Value, b: &Value, path: &str) -> String {
    match (a, b) {
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if !same(a, b) {
                    return difference(a, b, &format!("{path}[{i}]"));
                }
            }
        }
        (Value::Object(a), Value::Object(b)) if a.len() == b.len() => {
            for (k, a) in a {
                if let Some(b) = b.get(k)
                    && !same(a, b)
                {
                    return difference(a, b, &format!("{path}.{k}"));
                }
            }
        }
        _ => {}
    }
    format!("{path}: wanted {b}, got {a}")
}
#[test]
fn agentflow_semantics_and_source_oracle() {
    let fixture: Value = serde_json::from_str(include_str!("../../../tests/fixtures/agentflow-model.json")).unwrap();
    let mut failures = vec![];
    let mut count = 0;
    for (i, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let source = case["source"].as_str().unwrap();
        let options = crate::ParseOptions {
            limit: case["options"].get("limit").and_then(Value::as_u64).map(|v| v as usize),
            config: case["options"].get("config").and_then(Value::as_object).cloned(),
            max_edges: case["options"].get("maxEdges").and_then(Value::as_u64).map(|v| v as usize),
        };
        let actual = match crate::parse(source, &options) {
            Ok(parsed) => {
                let crate::DiagramModel::Agentflow(db) = parsed.model else {
                    panic!("wrong diagram model")
                };
                json!({"model":{"semantic":db.semantic_model(),"diagnostics":db.diagnostics.iter().map(db::Diagnostic::to_json).collect::<Vec<_>>(),"mappings":db.mappings.iter().map(|m|json!([m.id,m.kind,m.position.start_line,m.position.start_column,m.position.end_line,m.position.end_column,m.position.start_index,m.position.end_index])).collect::<Vec<_>>(),"metadata":{"vertices":db.nodes.iter().map(|(id,n)|json!([id,n.metadata])).collect::<Vec<_>>(),"edges":db.edges.iter().map(|e|json!([e.id,e.metadata])).collect::<Vec<_>>(),"flows":db.subgraphs.iter().map(|g|json!([g.id,g.metadata])).collect::<Vec<_>>()}}})
            }
            Err(e) => {
                json!({"error":{"message":e.message,"code":e.code.as_str(),"type":e.diagram,"mermaidType":e.mermaid_type,"line":e.line,"column":e.column,"snippet":e.snippet}})
            }
        };
        let mut wanted = case.clone();
        wanted.as_object_mut().unwrap().remove("source");
        wanted.as_object_mut().unwrap().remove("options");
        if !same(&actual, &wanted) {
            count += 1;
            if failures.len() < 12 {
                failures.push(format!("case{i}:\n{source}\n{}", difference(&actual, &wanted, "model")));
            }
        }
    }
    for (i, case) in fixture["tokens"].as_array().unwrap().iter().enumerate() {
        let source = case["source"].as_str().unwrap();
        let t = lexer::tokenize(&source.encode_utf16().collect::<Vec<_>>());
        let got = json!({"types":t.types.iter().map(|t|*t as usize).collect::<Vec<_>>(),"texts":t.texts,"starts":t.starts,"locs":t.locs.iter().flat_map(|p|[p.first_line,p.first_column,p.last_line,p.last_column]).collect::<Vec<_>>()});
        if !same(&got, &case["tokens"]) {
            count += 1;
            if failures.len() < 12 {
                failures.push(format!("tokens{i}: {source:?}\n{}", difference(&got, &case["tokens"], "tokens")));
            }
        }
    }
    assert_eq!(count, 0, "{count} agentflow model/token cases differ:\n{}", failures.join("\n\n"));
}
