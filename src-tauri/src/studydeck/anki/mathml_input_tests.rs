use super::*;

const FRACTION: &str = "<math><mfrac><mi>x</mi><mn>2</mn></mfrac></math>";

#[test]
fn document_anki_mathml_enters_shared_atoms_without_text_marks() {
    let converted = document(&format!("<b>Before {FRACTION} after</b>"), &mut |_| unreachable!()).unwrap();
    crate::cards::validate_document(&converted.value).unwrap();
    assert!(converted.contains_math);
    assert_eq!(converted.value["attrs"]["schemaVersion"], 3);
    let nodes = converted.value["content"][0]["content"].as_array().unwrap();
    assert_eq!(nodes[1], json!({"type":"inlineMath","attrs":{"latex":r"\frac{x}{2}"}}));
    assert_eq!(nodes[0]["marks"][0]["type"], "bold");
    assert_eq!(nodes[2]["marks"][0]["type"], "bold");
    assert_eq!(plain_text(FRACTION).unwrap(), r"\(\frac{x}{2}\)");
}

#[test]
fn document_anki_mathml_display_unsupported_and_code_boundaries() {
    let block = FRACTION.replacen("<math>", "<math display='block'>", 1);
    let converted = document(&format!("before{block}after"), &mut |_| unreachable!()).unwrap();
    assert_eq!(converted.value["content"][1]["type"], "blockMath");
    assert_eq!(converted.value["content"][2]["content"][0]["text"], "after");
    assert!(document("<math><maction><mi>x</mi></maction></math>", &mut |_| unreachable!()).is_err());
    assert!(document(&format!("<span hidden>{FRACTION}</span>"), &mut |_| unreachable!()).is_err());
    let code = document(&format!("<code>{FRACTION}</code>"), &mut |_| unreachable!()).unwrap();
    assert!(!code.contains_math);
    assert_eq!(code.value["content"][0]["content"][0]["text"], FRACTION);
    let ignored = document(&format!("<style>{FRACTION}</style>visible"), &mut |_| unreachable!()).unwrap();
    assert!(!ignored.contains_math);
}

#[test]
fn document_anki_mathml_real_package_survives_native_commit_and_reopen() {
    use std::{fs, io::Write};
    use rusqlite::{params, Connection};
    use zip::{write::SimpleFileOptions, ZipWriter};
    let root = std::env::temp_dir().join(format!("ostudy-mathml-apkg-{}-{}", std::process::id(), super::super::TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source.sqlite");
    let db = Connection::open(&source).unwrap();
    db.execute_batch("CREATE TABLE col (ver INTEGER, models TEXT, decks TEXT); CREATE TABLE notes (id INTEGER PRIMARY KEY, mid INTEGER, flds TEXT, tags TEXT); CREATE TABLE cards (id INTEGER PRIMARY KEY, nid INTEGER, did INTEGER, ord INTEGER, odid INTEGER); CREATE TABLE revlog (id INTEGER PRIMARY KEY, cid INTEGER, ease INTEGER);").unwrap();
    let models = json!({"1":{"name":"Basic","type":0,"css":"","flds":[{"name":"Front","ord":0},{"name":"Back","ord":1}],"tmpls":[{"name":"Card","ord":0,"qfmt":"{{Front}}","afmt":"{{Back}}"}]}});
    db.execute("INSERT INTO col VALUES (11, ?1, ?2)", params![models.to_string(), json!({"1":{"name":"MathML"}}).to_string()]).unwrap();
    db.execute("INSERT INTO notes VALUES (1, 1, ?1, '')", [format!("{FRACTION}\u{1f}answer")]).unwrap();
    db.execute("INSERT INTO cards VALUES (1, 1, 1, 0, 0)", []).unwrap();
    drop(db);
    let apkg = root.join("math.apkg");
    let mut zip = ZipWriter::new(fs::File::create(&apkg).unwrap());
    zip.start_file("collection.anki2", SimpleFileOptions::default()).unwrap();
    zip.write_all(&fs::read(source).unwrap()).unwrap();
    zip.start_file("media", SimpleFileOptions::default()).unwrap();
    zip.write_all(b"{}").unwrap();
    zip.finish().unwrap();
    let (package, report) = super::super::read_package(&apkg, "math.apkg").unwrap();
    assert_eq!(report.imported_card_count, 1);
    let expected = package.notes[0].front_doc.clone();
    let paths = crate::paths::AppPaths::new(root.join("target")).unwrap();
    let database = crate::database::Database::open(paths.database_path()).unwrap();
    let fingerprint = crate::studydeck::hash_file(&apkg).unwrap();
    database.with_connection(|connection| {
        super::super::super::import::import_package(connection, &paths, &"d".repeat(32), &fingerprint, package)
    }).unwrap();
    drop(database);
    let reopened = crate::database::Database::open(paths.database_path()).unwrap();
    reopened.with_connection(|connection| {
        let stored: String = connection.query_row("SELECT front_doc FROM notes", [], |row| row.get(0)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&stored).unwrap(), expected);
        Ok(())
    }).unwrap();
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}
