//! Stack-safety cases from the pinned railroad/usecase stress suites.
use pele_rs::{ParseOptions, RenderOptions, parse, render};

fn large() -> RenderOptions {
    RenderOptions {
        limit: Some(usize::MAX),
        output_limit: Some(usize::MAX),
        ..Default::default()
    }
}

#[test]
fn railroad_nested_constructs_use_heap_stacks() {
    for source in [
        format!("railroad-ebnf-beta\nr = {}a{};", "[".repeat(12_000), "]".repeat(12_000)),
        format!(
            "railroad-beta\nr = {}terminal(\"a\"){};",
            "optional(oneOrMore(".repeat(2_500),
            "))".repeat(2_500)
        ),
        format!("railroad-abnf-beta\nr = {}a{};", "2*3(".repeat(10_000), ")".repeat(10_000)),
        format!("railroad-peg-beta\nR <- {}a{};", "!( &( ".repeat(5_000), " ) )".repeat(5_000)),
    ] {
        let result = render(&source, &large()).unwrap();
        assert!(result.width.is_finite() && result.height.is_finite());
        assert!(result.svg.contains("pele-rule"));
    }
}

#[test]
fn usecase_deep_ordered_json_uses_heap_stacks() {
    for source in [
        format!("usecase-beta\njson P@{}1{}", "{\"a\":".repeat(12_000), "}".repeat(12_000)),
        format!("usecase-beta\njson P@{{\"a\":{}{}}}", "[".repeat(20_000), "]".repeat(20_000)),
    ] {
        let result = render(&source, &large()).unwrap();
        assert!(result.svg.contains("pele-json"));
        assert!(result.width.is_finite() && result.height.is_finite());
    }
}

#[test]
fn eventmodeling_handles_long_token_streams() {
    let mut source = String::from("eventmodeling\n");
    for i in 0..1000 {
        source.push_str(&format!("tf {i:03} evt E{i}\n"));
    }
    let result = render(&source, &large()).unwrap();
    assert_eq!(result.svg.matches("class=\"pele-node pele-em-event\"").count(), 1000);
}

#[test]
fn non_ascii_header_boundaries_do_not_overflow() {
    // U+FFFF was an unchecked u16 + 32 in the case-insensitive header scanner.
    let _ = parse("ishikawa\u{ffff}", &ParseOptions::default());
}

#[test]
fn json_error_preserves_lone_utf16_token_and_source_column() {
    let error = parse("usecase-beta\njson P@{\"a\":1😀}", &ParseOptions::default()).unwrap_err();
    let prefix = "Invalid JSON: JSON Parse error: Unrecognized token '";
    let suffix = "' (line 2, column 14)";
    let exact: Vec<u16> = prefix.encode_utf16().chain([0xd83d]).chain(suffix.encode_utf16()).collect();
    assert_eq!(error.message_utf16, Some(Box::new(exact)));
    assert_eq!(error.message, format!("{prefix}�{suffix}"));
    assert_eq!((error.line, error.column), (2, 14));
    assert!(error.snippet.is_empty());
    for token in ["é", "?", "/", "\u{a0}"] {
        let source = format!("usecase-beta\njson P@{{\"a\":1{token}}}");
        let error = parse(&source, &ParseOptions::default()).unwrap_err();
        assert!(
            error.message.contains(&format!("Unrecognized token '{token}'")),
            "{}",
            error.message
        );
        assert_eq!(error.message_utf16, None);
    }
}

#[test]
fn state_deep_documents_clone_render_and_drop_on_heap_stacks() {
    let source = format!("stateDiagram-v2\n{}{}", "state s{\n".repeat(6_000), "}\n".repeat(6_000));
    let model = parse(
        &source,
        &ParseOptions {
            limit: Some(usize::MAX),
            ..Default::default()
        },
    )
    .unwrap();
    drop(model.clone());
    drop(model);
    let source = format!(
        "stateDiagram-v2\n{}a-->b\n{}",
        (0..3_000).map(|i| format!("state s{i}{{\n")).collect::<String>(),
        "}\n".repeat(3_000)
    );
    let result = render(&source, &large()).unwrap();
    assert!(result.width.is_finite() && result.height.is_finite());
    assert!(result.svg.contains("pele-cluster"));
}

#[test]
fn c4_deep_boundaries_and_repeated_aliases_stay_on_heap_stacks() {
    for source in [
        format!(
            "C4Context\n{}System(s, \"S\")\n{}",
            (0..2_000).map(|i| format!("Boundary(b{i}, \"B{i}\") {{\n")).collect::<String>(),
            "}\n".repeat(2_000)
        ),
        format!(
            "C4Context\n{}System(s, \"S\")\n{}",
            "Boundary(b, \"B\") {\n".repeat(3_000),
            "}\n".repeat(3_000)
        ),
    ] {
        let result = render(&source, &large()).unwrap();
        assert!(result.width.is_finite() && result.height.is_finite());
    }
}
