//! Native plugin adapter, used to compare its inlined SVG with the WASM ABI.
use serde_json::{Value, json};
use std::io::{self, BufRead};
fn main() {
    for line in io::stdin().lock().lines() {
        let input: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let options = serde_json::to_vec(&input["options"]).unwrap();
        let result = match pele_typst_plugin::render(input["source"].as_str().unwrap().as_bytes(), &options) {
            Ok(svg) => json!({"svg":String::from_utf8(svg).unwrap()}),
            Err(error) => json!({"error":error}),
        };
        println!("{result}");
    }
}
