//! Real generated APKG fixtures for native audio/video conversion.

use std::{fs, io::Write, path::PathBuf};

use base64::Engine;
use rusqlite::{params, Connection};
use serde_json::json;
use zip::{write::SimpleFileOptions, ZipWriter};

use super::*;
use crate::{cards::validate_document, database::Database, paths::AppPaths};

struct MediaFixture {
    root: PathBuf,
    path: PathBuf,
}

impl Drop for MediaFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn fixture(cases: &[(i64, String)], files: &[(&str, Vec<u8>)]) -> MediaFixture {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "ostudy-anki-media-test-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    let database_path = root.join("source.sqlite3");
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE col (ver INTEGER, models TEXT, decks TEXT);
             CREATE TABLE notes (id INTEGER PRIMARY KEY, mid INTEGER, flds TEXT, tags TEXT);
             CREATE TABLE cards (id INTEGER PRIMARY KEY, nid INTEGER, did INTEGER, ord INTEGER, odid INTEGER);
             CREATE TABLE revlog (id INTEGER PRIMARY KEY, cid INTEGER, ease INTEGER);",
        )
        .unwrap();
    let models = json!({
        "1": {"name": "Basic", "type": 0, "css": "",
            "flds": [{"name": "Front", "ord": 0}, {"name": "Back", "ord": 1}, {"name": "Reverse", "ord": 2}],
            "tmpls": [{"name": "Card 1", "ord": 0, "qfmt": "{{Front}}", "afmt": "{{FrontSide}}<hr id=answer>{{Back}}"}]},
        "2": {"name": "Cloze", "type": 1, "css": "", "flds": [{"name": "Text", "ord": 0}],
            "tmpls": [{"name": "Cloze", "ord": 0, "qfmt": "{{cloze:Text}}", "afmt": "{{cloze:Text}}"}]}
    });
    connection
        .execute(
            "INSERT INTO col VALUES (11, ?1, ?2)",
            params![models.to_string(), json!({"1": {"name": "Media"}}).to_string()],
        )
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

    let path = root.join("media.apkg");
    let mut zip = ZipWriter::new(fs::File::create(&path).unwrap());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("collection.anki2", options).unwrap();
    zip.write_all(&fs::read(&database_path).unwrap()).unwrap();
    let mapping = files
        .iter()
        .enumerate()
        .map(|(index, (name, _))| (index.to_string(), json!(name)))
        .collect::<serde_json::Map<_, _>>();
    zip.start_file("media", options).unwrap();
    zip.write_all(serde_json::Value::Object(mapping).to_string().as_bytes())
        .unwrap();
    for (index, (_, bytes)) in files.iter().enumerate() {
        zip.start_file(index.to_string(), options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
    MediaFixture { root, path }
}

fn basic(front: &str, back: &str) -> (i64, String) {
    (1, format!("{front}\u{1f}{back}\u{1f}"))
}

fn wav() -> Vec<u8> {
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&40_u32.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&44_100_u32.to_le_bytes());
    wav.extend_from_slice(&88_200_u32.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&4_u32.to_le_bytes());
    wav.extend_from_slice(&[0, 0, 0, 0]);
    wav
}

fn decodable_mp4() -> Vec<u8> {
    // 0.2-second 16x16 black H.264 Baseline + AAC MP4, generated with FFmpeg.
    // Keeping a genuinely decodable fixture prevents container-shape tests from
    // being mistaken for playback evidence.
    base64::engine::general_purpose::STANDARD
        .decode("AAAAIGZ0eXBpc29tAAACAGlzb21pc28yYXZjMW1wNDEAAAVjbW9vdgAAAGxtdmhkAAAAAAAAAAAAAAAAAAAD6AAAAMgAAQAAAQAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAwAAAjl0cmFrAAAAXHRraGQAAAADAAAAAAAAAAAAAAABAAAAAAAAAMgAAAAAAAAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAABAAAAAABAAAAAQAAAAAAAkZWR0cwAAABxlbHN0AAAAAAAAAAEAAADIAAAAAAABAAAAAAGxbWRpYQAAACBtZGhkAAAAAAAAAAAAAAAAAAAoAAAACABVxAAAAAAALWhkbHIAAAAAAAAAAHZpZGUAAAAAAAAAAAAAAABWaWRlb0hhbmRsZXIAAAABXG1pbmYAAAAUdm1oZAAAAAEAAAAAAAAAAAAAACRkaW5mAAAAHGRyZWYAAAAAAAAAAQAAAAx1cmwgAAAAAQAAARxzdGJsAAAAuHN0c2QAAAAAAAAAAQAAAKhhdmMxAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAAAABAAEABIAAAASAAAAAAAAAABFUxhdmM2MS4xOS4xMDEgbGlieDI2NAAAAAAAAAAAAAAAGP//AAAALmF2Y0MBQsAK/+EAFmdCwArZHsBEAAADAAQAAAMAKDxImSABAAVoy4PLIAAAABBwYXNwAAAAAQAAAAEAAAAUYnRydAAAAAAAAGRQAAAAAAAAABhzdHRzAAAAAAAAAAEAAAABAAAIAAAAABxzdHNjAAAAAAAAAAEAAAABAAAAAQAAAAEAAAAUc3RzegAAAAAAAAKCAAAAAQAAABRzdGNvAAAAAAAAAAEAAAWoAAACVXRyYWsAAABcdGtoZAAAAAMAAAAAAAAAAAAAAAIAAAAAAAAAyAAAAAAAAAAAAAAAAQEAAAAAAQAAAAAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAACRlZHRzAAAAHGVsc3QAAAAAAAAAAQAAAMgAAAQAAAEAAAAAAc1tZGlhAAAAIG1kaGQAAAAAAAAAAAAAAAAAAB9AAAAKQFXEAAAAAAAtaGRscgAAAAAAAAAAc291bgAAAAAAAAAAAAAAAFNvdW5kSGFuZGxlcgAAAAF4bWluZgAAABBzbWhkAAAAAAAAAAAAAAAkZGluZgAAABxkcmVmAAAAAAAAAAEAAAAMdXJsIAAAAAEAAAE8c3RibAAAAH5zdHNkAAAAAAAAAAEAAABubXA0YQAAAAAAAAABAAAAAAAAAAAAAQAQAAAAAB9AAAAAAAA2ZXNkcwAAAAADgICAJQACAASAgIAXQBUAAAAAALuAAAACwwWAgIAFFYhW5QAGgICAAQIAAAAUYnRydAAAAAAAALuAAAACwwAAACBzdHRzAAAAAAAAAAIAAAACAAAEAAAAAAEAAAJAAAAAKHN0c2MAAAAAAAAAAgAAAAEAAAABAAAAAQAAAAIAAAACAAAAAQAAACBzdHN6AAAAAAAAAAAAAAADAAAAFQAAAAQAAAAEAAAAGHN0Y28AAAAAAAAAAgAABZMAAAgqAAAAGnNncGQBAAAAcm9sbAAAAAIAAAAB//8AAAAcc2JncAAAAAByb2xsAAAAAQAAAAMAAAABAAAAYXVkdGEAAABZbWV0YQAAAAAAAAAhaGRscgAAAAAAAAAAbWRpcmFwcGwAAAAAAAAAAAAAAAAsaWxzdAAAACSpdG9vAAAAHGRhdGEAAAABAAAAAExhdmY2MS43LjEwMwAAAAhmcmVlAAACp21kYXTeAgBMYXZjNjEuMTkuMTAxAAIwQA4AAAJwBgX//2zcRem95tlIt5Ys2CDZI+7veDI2NCAtIGNvcmUgMTY0IHIzMTA4IDMxZTE5ZjkgLSBILjI2NC9NUEVHLTQgQVZDIGNvZGVjIC0gQ29weWxlZnQgMjAwMy0yMDIzIC0gaHR0cDovL3d3dy52aWRlb2xhbi5vcmcveDI2NC5odG1sIC0gb3B0aW9uczogY2FiYWM9MCByZWY9MyBkZWJsb2NrPTE6MDowIGFuYWx5c2U9MHgxOjB4MTExIG1lPWhleCBzdWJtZT03IHBzeT0xIHBzeV9yZD0xLjAwOjAuMDAgbWl4ZWRfcmVmPTEgbWVfcmFuZ2U9MTYgY2hyb21hX21lPTEgdHJlbGxpcz0xIDh4OGRjdD0wIGNxbT0wIGRlYWR6b25lPTIxLDExIGZhc3RfcHNraXA9MSBjaHJvbWFfcXBfb2Zmc2V0PS0yIHRocmVhZHM9MSBsb29rYWhlYWRfdGhyZWFkcz0xIHNsaWNlZF90aHJlYWRzPTAgbnI9MCBkZWNpbWF0ZT0xIGludGVybGFjZWQ9MCBibHVyYXlfY29tcGF0PTAgY29uc3RyYWluZWRfaW50cmE9MCBiZnJhbWVzPTAgd2VpZ2h0cD0wIGtleWludD0yNTAga2V5aW50X21pbj01IHNjZW5lY3V0PTQwIGludHJhX3JlZnJlc2g9MCByY19sb29rYWhlYWQ9NDAgcmM9Y3JmIG1idHJlZT0xIGNyZj0yMy4wIHFjb21wPTAuNjAgcXBtaW49MCBxcG1heD02OSBxcHN0ZXA9NCBpcF9yYXRpbz0xLjQwIGFxPTE6MS4wMACAAAAACmWIhA/yYoAAw+4BGCAHARggBw==")
        .unwrap()
}

fn nodes_of_type<'a>(value: &'a serde_json::Value, kind: &str, output: &mut Vec<&'a serde_json::Value>) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                nodes_of_type(value, kind, output);
            }
        }
        serde_json::Value::Object(object) => {
            if object.get("type").and_then(serde_json::Value::as_str) == Some(kind) {
                output.push(value);
            }
            for value in object.values() {
                nodes_of_type(value, kind, output);
            }
        }
        _ => {}
    }
}

#[test]
fn document_anki_media_real_package_converts_audio_video_and_reopens_from_database() {
    let fixture = fixture(
        &[
            basic("Listen [sound:voice.wav] now", "answer"),
            basic("<video><source src='clip.mp4'></video>", "movie"),
        ],
        &[("voice.wav", wav()), ("clip.mp4", decodable_mp4())],
    );
    let (package, report) = read_package(&fixture.path, "media.apkg").unwrap();
    assert_eq!((report.imported_card_count, report.skipped_card_count), (2, 0));
    assert_eq!(package.assets.len(), 2);
    assert_eq!(package.note_assets.len(), 2);
    assert!(package.assets.iter().any(|asset| asset.mime_type == "audio/wav"));
    assert!(package.assets.iter().any(|asset| asset.mime_type == "video/mp4"));
    let mut audio = Vec::new();
    let mut video = Vec::new();
    for note in &package.notes {
        validate_document(&note.front_doc).unwrap();
        assert_eq!(note.front_doc["attrs"]["schemaVersion"], 3);
        nodes_of_type(&note.front_doc, "audio", &mut audio);
        nodes_of_type(&note.front_doc, "video", &mut video);
    }
    assert_eq!(audio.len(), 1);
    assert_eq!(video.len(), 1);
    assert_eq!(audio[0]["attrs"]["src"].as_str().unwrap().starts_with("study-asset://"), true);
    assert_eq!(video[0]["attrs"]["src"].as_str().unwrap().starts_with("study-asset://"), true);

    let target = AppPaths::new(fixture.root.join("target")).unwrap();
    let database = Database::open(target.database_path()).unwrap();
    let fingerprint = crate::studydeck::hash_file(&fixture.path).unwrap();
    let result = database
        .with_connection(|connection| {
            super::super::import::import_package(
                connection,
                &target,
                &"c".repeat(32),
                &fingerprint,
                package,
            )
        })
        .unwrap();
    assert_eq!((result.card_count, result.asset_count), (2, 2));
    drop(database);
    let reopened = Database::open(target.database_path()).unwrap();
    reopened
        .with_connection(|connection| {
            let assets: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM assets WHERE mime_type IN ('audio/wav', 'video/mp4') AND lifecycle_state = 'ready'",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let relationships: i64 = connection
                .query_row("SELECT COUNT(*) FROM note_assets", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!((assets, relationships), (2, 2));
            Ok(())
        })
        .unwrap();
}

#[test]
fn document_anki_media_cloze_uses_rich_basic_fallback_instead_of_losing_audio() {
    let fixture = fixture(
        &[(2, "Say {{c1::hello}} [sound:voice.wav]".to_string())],
        &[("voice.wav", wav())],
    );
    let (package, report) = read_package(&fixture.path, "cloze-media.apkg").unwrap();
    assert_eq!(report.imported_card_count, 1);
    assert_eq!(report.native_cloze_card_count, 0);
    assert_eq!(report.cloze_fallback_card_count, 1);
    let note = &package.notes[0];
    assert_eq!(note.note_type, "basic");
    assert!(note.cloze_source.is_empty());
    let mut front = Vec::new();
    let mut back = Vec::new();
    nodes_of_type(&note.front_doc, "audio", &mut front);
    nodes_of_type(&note.back_doc, "audio", &mut back);
    assert!(!front.is_empty() || !back.is_empty());
}

#[test]
fn document_anki_media_missing_unsupported_and_type_mismatch_skip_only_affected_cards() {
    let fixture = fixture(
        &[
            basic("plain", "ok"),
            basic("[sound:missing.wav]", "missing"),
            basic("[sound:bad.mp3]", "bad"),
            basic("<audio src='clip.mp4'></audio>", "mismatch"),
        ],
        &[("bad.mp3", b"not an mp3".to_vec()), ("clip.mp4", decodable_mp4())],
    );
    let (package, report) = read_package(&fixture.path, "bad-media.apkg").unwrap();
    assert_eq!((report.imported_card_count, report.skipped_card_count), (1, 3));
    assert_eq!(package.notes.len(), 1);
    assert!(package.assets.is_empty());
    assert!(report.issues.iter().any(|issue| {
        issue.source_card_id == "101" && issue.message.contains("missing")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.source_card_id == "102" && issue.message.contains("unsupported")
    }));
    assert!(report.issues.iter().any(|issue| {
        issue.source_card_id == "103" && issue.message.contains("declared type")
    }));
}
