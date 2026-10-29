// Compares the port with what pele's JavaScript renders for the same sources, as recorded by
// scripts/fixtures.ts. Diagram types not ported yet are skipped.

use serde_json::Value;

fn yaml_error(message: &str) -> bool {
    message.contains(" is not valid YAML.")
}

// Renders every case of one type and fails with the first difference of each that differs.
// `least` guards against a fixture file that has lost its cases.
fn compare(kind: &str, fixtures: &str, least: usize) {
    let cases: Vec<Value> = serde_json::from_str(fixtures).unwrap();
    let mut failures = Vec::new();
    let mut compared = 0;
    for (i, case) in cases.iter().enumerate() {
        let source = case["source"].as_str().unwrap();
        if pele_rs::detect_type(source) != Some(kind) {
            continue;
        }
        compared += 1;
        let got = pele_rs::render(source, &Default::default());
        match (case.get("svg").and_then(|v| v.as_str()), &got) {
            (Some(want), Ok(r)) if r.svg == want => {}
            (None, Err(e)) if e.message == case["error"].as_str().unwrap() => {}
            // The YAML reader is a different one, which words its errors its own way.
            (None, Err(e)) if yaml_error(&e.message) && yaml_error(case["error"].as_str().unwrap()) => {}
            (want, got) => {
                let got = match got {
                    Ok(r) => r.svg.clone(),
                    Err(e) => format!("error: {}", e.message),
                };
                let want = want
                    .map(String::from)
                    .unwrap_or_else(|| format!("error: {}", case["error"].as_str().unwrap()));
                let at = got
                    .bytes()
                    .zip(want.bytes())
                    .position(|(a, b)| a != b)
                    .unwrap_or(got.len().min(want.len()));
                let from = at.saturating_sub(60);
                failures.push(format!(
                    "case {i}:\n{source}\n  want …{}…\n  got  …{}…",
                    want.get(from..(at + 120).min(want.len())).unwrap_or(""),
                    got.get(from..(at + 120).min(got.len())).unwrap_or("")
                ));
            }
        }
    }
    eprintln!("compared {compared} {kind} cases");
    assert!(compared >= least, "only {compared} {kind} cases");
    if !failures.is_empty() {
        panic!("{} of {} cases differ:\n\n{}", failures.len(), compared, failures.join("\n\n"));
    }
}

#[test]
fn flowcharts_match_pele() {
    compare("flowchart", include_str!("fixtures/flowchart.json"), 150);
}

#[test]
fn pies_match_pele() {
    compare("pie", include_str!("fixtures/pie.json"), 50);
}
