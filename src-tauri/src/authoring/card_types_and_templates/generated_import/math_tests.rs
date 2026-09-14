use super::super::import_generated_flashcards_bytes_with_commit;
use super::markdown_document;
use crate::{cards::validate_document, database::Database};
use serde_json::{json, Value};

fn formulas(node: &Value) -> Vec<Value> {
    let mut result = Vec::new();
    if matches!(node["type"].as_str(), Some("inlineMath" | "blockMath")) {
        result.push(node.clone());
    }
    if let Some(children) = node.get("content").and_then(Value::as_array) {
        for child in children {
            result.extend(formulas(child));
        }
    }
    result
}

fn text(node: &Value) -> String {
    if node["type"] == "text" {
        return node["text"].as_str().unwrap().to_string();
    }
    node.get("content")
        .and_then(Value::as_array)
        .map(|children| children.iter().map(text).collect::<Vec<_>>().join(""))
        .unwrap_or_default()
}

fn parse(source: &str) -> Value {
    let document = markdown_document(source, "cards[0].front").unwrap();
    validate_document(&document).unwrap();
    document
}

#[test]
fn document_generated_math_preserves_source_before_emphasis_and_links() {
    let document = parse(r"α **before \(x_i*y_j\) after** [value \(a_b\)](https://example.org)");
    assert_eq!(document["attrs"]["schemaVersion"], 3);
    assert_eq!(
        formulas(&document),
        vec![
            json!({"type":"inlineMath","attrs":{"latex":"x_i*y_j"}}),
            json!({"type":"inlineMath","attrs":{"latex":"a_b"}}),
        ]
    );
    assert_eq!(text(&document), "α before  after value ");
    assert_eq!(
        document["content"][0]["content"][1]["marks"],
        json!([{"type":"bold"}])
    );
}

#[test]
fn document_generated_math_preserves_multiline_display_and_surrounding_text() {
    let source =
        "before \\[\\begin{matrix}\na & b \\\\\nc & d\n\\end{matrix}\\] after\n\n$$\\frac{x}{2}$$";
    let document = parse(source);
    assert_eq!(
        formulas(&document),
        vec![
            json!({"type":"blockMath","attrs":{"latex":"\\begin{matrix}\na & b \\\\\nc & d\n\\end{matrix}"}}),
            json!({"type":"blockMath","attrs":{"latex":"\\frac{x}{2}"}}),
        ]
    );
    assert_eq!(document["content"][0]["type"], "paragraph");
    assert_eq!(document["content"][1]["type"], "blockMath");
    assert_eq!(document["content"][2]["type"], "paragraph");
    assert_eq!(text(&document), "before  after");
}

#[test]
fn document_generated_math_keeps_quote_list_and_heading_containers() {
    let document =
        parse("# Rate \\(r_i\\)\n\n> before \\[x\\] after\n\n- **Answer: \\(y_j\\)**\n- next");
    assert_eq!(document["content"][0]["type"], "heading");
    assert_eq!(document["content"][1]["type"], "blockquote");
    assert_eq!(document["content"][1]["content"][1]["type"], "blockMath");
    assert_eq!(document["content"][2]["type"], "bulletList");
    assert_eq!(formulas(&document).len(), 3);
}

#[test]
fn document_generated_math_leaves_code_currency_and_escaped_openings_literal() {
    let source = "`\\(x_i\\)`\n\n```tex\n\\[x^2\\]\n$$y$$\n```\n\n    \\(z\\)\n\n$5 and $10; \\\\(not math\\\\)";
    let document = parse(source);
    assert!(formulas(&document).is_empty());
    assert!(document.get("attrs").is_none());
    assert!(text(&document).contains(r"\(x_i\)"));
    assert!(text(&document).contains("$5 and $10"));
    assert!(text(&document).contains(r"\[x^2\]"));
}

#[test]
fn document_generated_math_bounds_sources_without_partial_or_silent_conversion() {
    for source in [
        r"\(unfinished".to_string(),
        r"\(\)".to_string(),
        "\\(x\u{000c}y\\)".to_string(),
        format!("\\({}\\)", "α".repeat(8193)),
        r"\(x\)".repeat(65),
        format!("\\({0}\\)\\({0}\\)\\(z\\)", "x".repeat(16384)),
    ] {
        let error = markdown_document(&source, "cards[4].back").unwrap_err();
        assert_eq!(error.code, "invalid_import_math");
        assert!(error.message.starts_with("cards[4].back:"));
    }
    assert_eq!(
        formulas(&parse(&format!("\\({}\\)", "α".repeat(8192)))).len(),
        1
    );
    assert_eq!(formulas(&parse(&r"\(x\)".repeat(64))).len(), 64);
}

#[test]
fn document_generated_math_private_markers_cannot_collide_with_source() {
    let source = "literal \u{e000}0\u{e001} and \u{e000}\u{e000}1\u{e001} then \\(x\\)";
    let document = parse(source);
    assert_eq!(formulas(&document).len(), 1);
    assert_eq!(
        text(&document),
        "literal \u{e000}0\u{e001} and \u{e000}\u{e000}1\u{e001} then "
    );
}

#[test]
fn document_generated_math_retains_inert_source_without_admitting_raw_html() {
    let source = r"\(\text{<img src=x>}\)";
    let document = parse(source);
    assert_eq!(
        formulas(&document)[0]["attrs"]["latex"],
        r"\text{<img src=x>}"
    );
    assert!(markdown_document(r"\(x\) <img src=x>", "front").is_err());
    assert!(markdown_document(r"\(x\) ![image](remote)", "front").is_err());
}

fn file(cards: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "format":"ostudy-flashcards", "formatVersion":1,
        "deck":{"mode":"create","name":"Generated math"}, "cards":cards
    }))
    .unwrap()
}

#[test]
fn document_generated_math_import_persists_canonical_nodes_and_reverse_presentations() {
    let database = Database::open_in_memory().unwrap();
    let front = r"**Find \(x_i\)**";
    let back = r"\[\frac{x}{2}\]";
    let result = import_generated_flashcards_bytes_with_commit(
        &database,
        &file(json!([{"type":"basic","front":front,"back":back,"reverseEnabled":true}])),
        |_, _, _| Ok(()),
    )
    .unwrap();
    assert_eq!(
        (result.created_note_count, result.created_card_count),
        (1, 2)
    );
    database
        .with_connection(|connection| {
            let stored: (String, String) = connection
                .query_row(
                    "SELECT front_doc, back_doc FROM notes WHERE deck_id = ?1",
                    [&result.deck_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&stored.0).unwrap(),
                parse(front)
            );
            assert_eq!(
                serde_json::from_str::<Value>(&stored.1).unwrap(),
                parse(back)
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn document_generated_math_invalid_final_candidate_keeps_the_whole_import_atomic() {
    let database = Database::open_in_memory().unwrap();
    let error = import_generated_flashcards_bytes_with_commit(
        &database,
        &file(json!([
            {"type":"basic","front":"valid \\(x\\)","back":"answer"},
            {"type":"basic","front":"invalid \\(unclosed","back":"answer"}
        ])),
        |_, _, _| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, "invalid_flashcard_import_cards");
    database
        .with_connection(|connection| {
            let count: i64 = connection
                .query_row("SELECT count(*) FROM notes", [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn document_generated_math_marker_allocation_is_bounded_for_private_use_runs() {
    let literal = "\u{e000}".repeat(2048);
    let source = format!("{literal} {}", r"\(x\)".repeat(64));
    let protected = super::math::ProtectedMath::new(&source, "front").unwrap();
    assert!(protected.source.len() <= source.len() + 64 * 80);
    let document = parse(&source);
    assert_eq!(formulas(&document).len(), 64);
    assert_eq!(text(&document), format!("{literal} "));
}
