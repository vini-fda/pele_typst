//! JSON-lines adapter for deterministic differential testing against the pinned engine.
use serde_json::{Value, json};
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};
#[path = "../tests/support/mod.rs"]
mod support;
fn main() {
    for line in io::stdin().lock().lines() {
        let input: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let options = support::options(&input["options"], calls);
        let source = input["source"].as_str().unwrap();
        let result = match pele_rs::render(source, &options) {
            Ok(r) => {
                json!({"result":{"svg":r.svg,"width":r.width,"height":r.height,"type":r.kind,"mermaidType":r.mermaid_type,"links":r.links.iter().map(|l|json!({"id":l.id,"href":l.href,"internal":l.internal})).collect::<Vec<_>>()}})
            }
            Err(e) => {
                json!({"error":support::error_value(&e)})
            }
        };
        println!("{result}");
    }
}
