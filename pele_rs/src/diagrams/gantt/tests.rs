use super::*;
use serde_json::{Value, json};
#[test]
fn calendar_and_model_oracle() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("../../../tests/fixtures/gantt-calendar.json")).unwrap();
    let mut failures = vec![];
    for (i, case) in cases.iter().enumerate() {
        let zone = Zone::new(case["zone"].as_str()).unwrap();
        let args = &case["args"];
        let n = |i: usize| args[i].as_f64().unwrap();
        let s = |i: usize| args[i].as_str().unwrap();
        let now = case["now"].as_f64().unwrap();
        let actual = match case["op"].as_str().unwrap() {
            "format" => json!(dates::format(n(0), s(1), &zone)),
            "parse" => json!(dates::parse_strict(s(0), s(1), now, &zone)),
            "axis" => json!(axis::time_format(s(1), n(0), &zone)),
            "add" => json!(dates::add(n(0), n(1), s(2), &zone)),
            "auto" => json!(axis::auto_ticks(n(0), n(1), n(2), &zone)),
            "interval" => json!(axis::interval_ticks(s(0), s(1), n(2), n(3), n(4), &zone)),
            "schedule" => {
                let model = parse(s(0), &Config::new(), None, 0, 500).unwrap();
                let scheduled = Schedule::new(&model, n(1), zone).unwrap();
                json!(scheduled.model.tasks.iter().map(|t|json!({"id":t.id,"start":t.start_time,"end":t.end_time,"renderEnd":t.render_end_time,"manual":t.manual_end_time,"processed":t.processed,"order":t.order})).collect::<Vec<_>>())
            }
            "render" => {
                let options = crate::RenderOptions {
                    now: Some(now),
                    time_zone: case["zone"].as_str().map(str::to_string),
                    ..Default::default()
                };
                let rendered = crate::render(s(0), &options).unwrap();
                json!({"svg":rendered.svg,"width":rendered.width,"height":rendered.height,"type":rendered.kind,"mermaidType":rendered.mermaid_type,"links":[]})
            }
            _ => unreachable!(),
        };
        if !same(&actual, &case["value"]) && failures.len() < 20 {
            failures.push(format!(
                "case {i} {} {} {:?}: wanted {}, got {}",
                case["zone"], case["op"], args, case["value"], actual
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b)),
        (Value::Object(a), Value::Object(b)) => a.len() == b.len() && a.iter().all(|(k, a)| b.get(k).is_some_and(|b| same(a, b))),
        _ => a == b,
    }
}
