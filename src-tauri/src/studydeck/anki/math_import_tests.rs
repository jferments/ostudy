//! Generated Anki packages exercise the same converter and transaction as imports.

use super::*;
use crate::cards::validate_document;

fn math_fixture(cases: &[(i64, String)]) -> Fixture {
    let fixture = fixture(&[], "{}");
    let connection = Connection::open(fixture.root.join("source.sqlite3")).unwrap();
    connection
        .execute_batch("DELETE FROM notes; DELETE FROM cards; DELETE FROM revlog;")
        .unwrap();
    for (index, (model, fields)) in cases.iter().enumerate() {
        let note = 10 + index as i64;
        let card = 100 + index as i64;
        connection
            .execute(
                "INSERT INTO notes VALUES (?1, ?2, ?3, '')",
                params![note, model, fields],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO cards VALUES (?1, ?2, 1, 0, 0)",
                params![card, note],
            )
            .unwrap();
    }
    drop(connection);
    let mut zip = ZipWriter::new(fs::File::create(&fixture.path).unwrap());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("collection.anki2", options).unwrap();
    zip.write_all(&fs::read(fixture.root.join("source.sqlite3")).unwrap())
        .unwrap();
    zip.start_file("media", options).unwrap();
    zip.write_all(b"{}").unwrap();
    zip.finish().unwrap();
    fixture
}

fn basic(front: &str, back: &str) -> (i64, String) {
    (1, format!("{front}\u{1f}{back}\u{1f}"))
}

fn convert(source: &str) -> html::Document {
    html::document(source, &mut |_| panic!("math must not resolve a media URL")).unwrap()
}

#[test]
fn document_anki_math_preserves_source_marks_and_block_order() {
    let document = convert(r"<b>Before \(x &lt; y\) after</b>\[\frac{x}{2}\]tail");
    assert!(document.contains_math);
    assert!(document.warnings.is_empty());
    assert_eq!(
        document.value,
        json!({"type": "doc", "attrs": {"schemaVersion": 3}, "content": [
            {"type": "paragraph", "content": [
                {"type": "text", "text": "Before ", "marks": [{"type": "bold"}]},
                {"type": "inlineMath", "attrs": {"latex": "x < y"}},
                {"type": "text", "text": " after", "marks": [{"type": "bold"}]}
            ]},
            {"type": "blockMath", "attrs": {"latex": "\\frac{x}{2}"}},
            {"type": "paragraph", "content": [{"type": "text", "text": "tail"}]}
        ]})
    );
    validate_document(&document.value).unwrap();
}

#[test]
fn document_anki_math_keeps_code_currency_and_unsupported_regions_literal() {
    for source in [
        r"<code>\(x\)</code>",
        r"<pre>\[x\]</pre>",
        "$5 and $10",
        r"[latex]\documentclass{article}\(x\)[/latex]",
        r"before \(unfinished",
    ] {
        let document = convert(source);
        assert!(!document.contains_math, "{source}");
        assert!(document.value.get("attrs").is_none());
        validate_document(&document.value).unwrap();
    }
    assert!(convert(r"before \(unfinished")
        .warnings
        .iter()
        .any(|message| message.contains("Unclosed math")));
    assert!(convert(r"[latex]\(x\)[/latex]")
        .warnings
        .iter()
        .any(|message| message.contains("system TeX")));
    // A delimiter split by HTML structure is not silently reconstructed.
    let split = convert(r"\(x<b>+y</b>\)");
    assert!(!split.contains_math);
    assert!(split
        .warnings
        .iter()
        .any(|message| message.contains("Unclosed math")));
    assert_eq!(
        convert("old plain card").value,
        json!({"type": "doc", "content": [{"type": "paragraph", "content": [
            {"type": "text", "text": "old plain card"}
        ]}]})
    );
}

#[test]
fn document_anki_math_does_not_turn_formula_source_into_html_or_fetches() {
    let document =
        convert(r"<script>unsafe()</script>\(\text{&lt;img src=x onerror=alert(1)&gt;}\)");
    assert!(document.script_omitted);
    assert!(html::has_study_content(&document.value));
    assert_eq!(
        document.value["content"][0]["content"][0],
        json!({"type": "inlineMath", "attrs": {
            "latex": "\\text{<img src=x onerror=alert(1)>}"
        }})
    );
    validate_document(&document.value).unwrap();
    for active in [
        r"<iframe src='https://example.invalid'>\(x\)</iframe>",
        r"<span hidden>\(x\)</span>",
    ] {
        assert!(html::document(active, &mut |_| panic!("no media expected")).is_err());
    }
}

#[test]
fn document_anki_math_real_package_preserves_formulas_and_static_script_salvage() {
    let fixture = math_fixture(&[
        basic(r"[$]x^2[/$]", r"[$$]\frac{x}{2}[/$$]"),
        (3, r"\(y\)".into()),
    ]);
    let (package, report) = read_package(&fixture.path, "math.apkg").unwrap();
    assert_eq!(report.source_card_count, 2);
    assert_eq!(report.imported_card_count, 2);
    assert_eq!(report.skipped_card_count, 0);
    assert_eq!(report.javascript_salvaged_card_count, 1);
    assert_eq!(package.notes.len(), 2);
    for note in &package.notes {
        assert_eq!(note.front_doc["attrs"]["schemaVersion"], 3);
        // The transport envelope remains independent of the rich-root schema.
        assert_eq!(note.content_schema_version, 1);
        assert_eq!(note.note_type, "basic");
        validate_document(&note.front_doc).unwrap();
        validate_document(&note.back_doc).unwrap();
    }
    let squared = package
        .notes
        .iter()
        .find(|note| note.front_text.contains("x^2"))
        .unwrap();
    assert_eq!(
        squared.front_doc["content"][0]["content"][0]["attrs"]["latex"],
        "x^2"
    );
    assert_eq!(squared.back_doc["content"][0]["type"], "blockMath");
    assert_eq!(
        squared.back_doc["content"][0]["attrs"]["latex"],
        r"\frac{x}{2}"
    );
}

#[test]
fn document_anki_math_cloze_keeps_rich_formulas_in_the_existing_basic_fallback() {
    let fixture = math_fixture(&[(2, r"Value {{c1::\(x^2\)}} and \(y\)".into())]);
    let (package, report) = read_package(&fixture.path, "math-cloze.apkg").unwrap();
    assert_eq!(report.imported_card_count, 1);
    assert_eq!(report.native_cloze_card_count, 0);
    assert_eq!(report.cloze_fallback_card_count, 1);
    let note = &package.notes[0];
    assert_eq!(note.note_type, "basic");
    assert!(note.cloze_source.is_empty());
    assert_eq!(note.front_doc["attrs"]["schemaVersion"], 3);
    assert_eq!(note.back_doc["attrs"]["schemaVersion"], 3);
    let back = note.back_doc.to_string();
    assert!(back.contains("inlineMath"));
    assert!(back.contains("x^2"));
    validate_document(&note.front_doc).unwrap();
    validate_document(&note.back_doc).unwrap();
}

#[test]
fn document_anki_math_reports_rejected_sources_without_losing_other_cards() {
    let oversized = format!(
        "\\({}\\)",
        "x".repeat(crate::cards::math::MAX_LATEX_BYTES + 1)
    );
    let fixture = math_fixture(&[
        basic(&oversized, "answer"),
        basic(r"\(\)", "answer"),
        basic(r"before \(unfinished", "answer"),
        basic(r"[latex]\(x\)[/latex]", "answer"),
        basic(r"\(valid\)", "answer"),
    ]);
    let (package, report) = read_package(&fixture.path, "bounded-math.apkg").unwrap();
    assert_eq!(report.source_card_count, 5);
    assert_eq!(report.imported_card_count, 3);
    assert_eq!(report.skipped_card_count, 2);
    assert_eq!(package.notes.len(), 3);
    for id in ["100", "101"] {
        assert!(report.issues.iter().any(|issue| {
            issue.source_card_id == id && issue.message.contains("16,384 UTF-8 bytes")
        }));
    }
    assert!(report.issues.iter().any(|issue| {
        issue.source_card_id == "102" && issue.message.contains("Unclosed math")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.source_card_id == "103" && issue.message.contains("system TeX")
    }));
}

#[test]
fn document_anki_math_import_replay_and_database_reopen_preserve_exact_nodes() {
    let fixture = math_fixture(&[basic(r"\(x+α\)", r"\[\frac{x}{2}\]")]);
    let paths = AppPaths::new(fixture.root.join("target")).unwrap();
    let database = Database::open(paths.database_path()).unwrap();
    let operation = "c".repeat(32);
    let fingerprint = crate::studydeck::hash_file(&fixture.path).unwrap();
    let (package, _) = read_package(&fixture.path, "durable-math.apkg").unwrap();
    let expected_front = package.notes[0].front_doc.clone();
    let expected_back = package.notes[0].back_doc.clone();
    let first = database
        .with_connection(|connection| {
            crate::studydeck::import::import_package(
                connection,
                &paths,
                &operation,
                &fingerprint,
                package,
            )
        })
        .unwrap();
    let (package, _) = read_package(&fixture.path, "durable-math.apkg").unwrap();
    let replay = database
        .with_connection(|connection| {
            crate::studydeck::import::import_package(
                connection,
                &paths,
                &operation,
                &fingerprint,
                package,
            )
        })
        .unwrap();
    assert_eq!(first.deck.id, replay.deck.id);
    assert_eq!(first.card_count, 1);
    drop(database);
    let reopened = Database::open(paths.database_path()).unwrap();
    reopened
        .with_connection(|connection| {
            let (front, back): (String, String) = connection
                .query_row("SELECT front_doc, back_doc FROM notes", [], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .map_err(CommandError::database)?;
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&front).unwrap(),
                expected_front
            );
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&back).unwrap(),
                expected_back
            );
            let counts: (i64, i64, i64, i64) = connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM decks), (SELECT COUNT(*) FROM cards),
                     (SELECT COUNT(*) FROM review_events), (SELECT MAX(reps) FROM card_schedules)",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(counts, (1, 1, 0, 0));
            Ok(())
        })
        .unwrap();
}
