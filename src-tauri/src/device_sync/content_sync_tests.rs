//! Notebooks, marks and lessons across device sync: a notebook travels with
//! its pictures, and rows whose parent the other device deleted follow it
//! instead of breaking the merge.

use std::{fs, path::PathBuf};

use rusqlite::{params, Connection};

use super::*;
use crate::device_sync::{
    merge::{merge_remote_database, remember_sent_database},
    store::SyncStore,
};

const THIS_DEVICE: &str = "000000000000000000000000000000a1";
const OTHER_DEVICE: &str = "000000000000000000000000000000b2";
const SUBJECT: &str = "55555555555555555555555555555555";
const NOTEBOOK: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PAGE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const LESSON: &str = "cccccccccccccccccccccccccccccccc";

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-content-sync-{label}-{}-{}",
            std::process::id(),
            crate::platform::now_utc_millis().unwrap_or_default()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// A library at the current schema, opened as a plain connection.
    fn library(&self, name: &str) -> Connection {
        let path = self.path(name);
        drop(Database::open(&path).unwrap());
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        connection
    }

    fn copy(&self, connection: &Connection, name: &str) -> Connection {
        let path = self.path(name);
        connection
            .execute_batch(&format!("VACUUM INTO '{}'", path.display()))
            .unwrap();
        let copy = Connection::open(&path).unwrap();
        copy.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        copy
    }

    fn store(&self) -> SyncStore {
        let paths = AppPaths::new(self.path("store")).unwrap();
        let store = SyncStore::open(&paths).unwrap();
        store
            .upsert_peer(OTHER_DEVICE, "Other device", "public-key", &"b".repeat(64), "")
            .unwrap();
        store
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn notes_and_lessons() -> SyncSelection {
    SyncSelection {
        flashcards: false,
        documents: false,
        study_guides: false,
        notes: true,
        lessons: true,
        api_settings: false,
        api_credentials: false,
    }
}

fn count(connection: &Connection, sql: &str) -> i64 {
    connection.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn add_subject(connection: &Connection) {
    connection
        .execute(
            "INSERT INTO document_collections(id, name, sort_order, created_at, updated_at, revision)
             VALUES (?1, 'Cardiology', 1000, 1, 1, 1)",
            [SUBJECT],
        )
        .unwrap();
}

fn add_notebook(connection: &Connection, subject: Option<&str>) {
    connection
        .execute(
            "INSERT INTO notebooks(id, subject_id, title, paper, page_size, revision, created_at, updated_at)
             VALUES (?1, ?2, 'Cardiac cycle', 'grid', 'letter', 1, 1, 1)",
            params![NOTEBOOK, subject],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO notebook_pages(id, notebook_id, position, paper, width, height, created_at)
             VALUES (?1, ?2, 1, 'grid', 612, 792, 1)",
            params![PAGE, NOTEBOOK],
        )
        .unwrap();
}

fn add_line(connection: &Connection, id: char) {
    connection
        .execute(
            "INSERT INTO notebook_items(id, page_id, kind, payload_json, revision, created_at, updated_at)
             VALUES (?1, ?2, 'line', '{\"x1\":1,\"y1\":1,\"x2\":9,\"y2\":9,\"color\":\"#111111\",\"strokeWidth\":2}', 1, 1, 1)",
            params![id.to_string().repeat(32), PAGE],
        )
        .unwrap();
}

fn add_lesson(connection: &Connection) {
    connection
        .execute_batch(&format!(
            "INSERT INTO lessons(id, subject_id, sort_order, title, revision, created_at, updated_at)
             VALUES ('{LESSON}', '{SUBJECT}', 1000, 'Heart sounds', 1, 1, 1);
             INSERT INTO lesson_revisions(lesson_id, revision_number, origin, content_json,
                 content_hash, provenance_json, created_at)
             VALUES ('{LESSON}', 1, 'authored', '{{}}', '{hash}', NULL, 1);",
            hash = "b".repeat(64),
        ))
        .unwrap();
}

/// Merges `remote` into `local`, with `agreed` as what both devices last
/// agreed on.
fn merge(scratch: &Scratch, local: &mut Connection, remote: &Connection, agreed: &Connection) {
    let store = scratch.store();
    remember_sent_database(agreed, &store, OTHER_DEVICE, notes_and_lessons()).unwrap();
    merge_remote_database(
        local,
        remote,
        &store,
        THIS_DEVICE,
        OTHER_DEVICE,
        notes_and_lessons(),
    )
    .unwrap();
}

#[test]
fn a_notebook_travels_with_its_picture_without_flashcards() {
    let scratch = Scratch::new("picture");
    let sender_paths = AppPaths::new(scratch.path("sender-files")).unwrap();
    let sender = Database::open(&scratch.path("sender.sqlite3")).unwrap();
    let asset = "f".repeat(32);
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(2, 2, image::Rgba([200, 30, 30, 255]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let relative = AppPaths::image_relative_path(&asset, "png");
    let file = sender_paths.resolve_relative(&relative);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, &png).unwrap();
    sender
        .with_connection(|connection| {
            add_subject(connection);
            add_notebook(connection, Some(SUBJECT));
            connection
                .execute_batch(&format!(
                    "INSERT INTO assets(id, original_filename, mime_type, byte_size,
                         relative_path, created_at, lifecycle_state, ready_at, verified_at,
                         updated_at)
                     VALUES ('{asset}', 'sketch.png', 'image/png', {size}, '{relative}', 1,
                         'ready', 1, 1, 1);
                     INSERT INTO notebook_items(id, page_id, kind, payload_json, revision,
                         created_at, updated_at)
                     VALUES ('{item}', '{PAGE}', 'image', '{{}}', 1, 1, 1);
                     INSERT INTO notebook_item_assets(item_id, asset_id)
                     VALUES ('{item}', '{asset}');",
                    size = png.len(),
                    item = "e".repeat(32),
                ))
                .map_err(CommandError::database)
        })
        .unwrap();

    let package = scratch.path("notes.syncpkg");
    let selection = SyncSelection {
        lessons: false,
        ..notes_and_lessons()
    };
    let summary = create_package(&sender, &sender_paths, &package, selection).unwrap();
    assert_eq!(summary.asset_count, 1);
    let extracted = extract_package(&package, &scratch.path("staging")).unwrap();
    assert!(extracted.selection.notes);
    let receiver_paths = AppPaths::new(scratch.path("receiver-files")).unwrap();
    install_assets(&receiver_paths, &extracted.assets).unwrap();
    assert_eq!(fs::read(receiver_paths.resolve_relative(&relative)).unwrap(), png);

    let mut receiver = scratch.library("receiver.sqlite3");
    let remote = Connection::open(&extracted.database_path).unwrap();
    merge_remote_database(
        &mut receiver,
        &remote,
        &scratch.store(),
        THIS_DEVICE,
        OTHER_DEVICE,
        extracted.selection,
    )
    .unwrap();
    assert_eq!(
        count(&receiver, "SELECT COUNT(*) FROM notebooks WHERE subject_id IS NOT NULL"),
        1
    );
    assert_eq!(count(&receiver, "SELECT COUNT(*) FROM notebook_items"), 1);
    assert_eq!(
        count(&receiver, "SELECT COUNT(*) FROM assets WHERE lifecycle_state = 'ready'"),
        1
    );
}

#[test]
fn marks_on_a_notebook_the_other_device_deleted_go_with_it() {
    let scratch = Scratch::new("deleted-notebook");
    let agreed = scratch.library("agreed.sqlite3");
    add_notebook(&agreed, None);
    add_line(&agreed, '1');
    let mut local = scratch.copy(&agreed, "local.sqlite3");
    let remote = scratch.copy(&agreed, "remote.sqlite3");
    // This device drew another line; the other device deleted the notebook.
    add_line(&local, '2');
    remote
        .execute("DELETE FROM notebooks WHERE id = ?1", [NOTEBOOK])
        .unwrap();

    merge(&scratch, &mut local, &remote, &agreed);
    assert_eq!(count(&local, "SELECT COUNT(*) FROM notebooks"), 0);
    assert_eq!(count(&local, "SELECT COUNT(*) FROM notebook_pages"), 0);
    assert_eq!(count(&local, "SELECT COUNT(*) FROM notebook_items"), 0);
}

#[test]
fn a_notebook_filed_in_a_subject_the_other_device_deleted_goes_to_no_subject() {
    let scratch = Scratch::new("deleted-subject");
    let agreed = scratch.library("agreed.sqlite3");
    add_subject(&agreed);
    let mut local = scratch.copy(&agreed, "local.sqlite3");
    let remote = scratch.copy(&agreed, "remote.sqlite3");
    add_notebook(&local, Some(SUBJECT));
    add_line(&local, '1');
    remote
        .execute("DELETE FROM document_collections WHERE id = ?1", [SUBJECT])
        .unwrap();

    merge(&scratch, &mut local, &remote, &agreed);
    assert_eq!(count(&local, "SELECT COUNT(*) FROM document_collections"), 0);
    assert_eq!(
        count(&local, "SELECT COUNT(*) FROM notebooks WHERE subject_id IS NULL"),
        1
    );
    assert_eq!(count(&local, "SELECT COUNT(*) FROM notebook_items"), 1);
}

#[test]
fn a_lesson_in_a_subject_the_other_device_deleted_goes_with_it() {
    let scratch = Scratch::new("deleted-lesson-subject");
    let agreed = scratch.library("agreed.sqlite3");
    add_subject(&agreed);
    add_lesson(&agreed);
    let mut local = scratch.copy(&agreed, "local.sqlite3");
    let remote = scratch.copy(&agreed, "remote.sqlite3");
    // This device studied the lesson; the other device deleted its subject.
    local
        .execute(
            "INSERT INTO lesson_progress(lesson_id, revision_number, current_step,
                 furthest_step, checks_json, revision, started_at, updated_at)
             VALUES (?1, 1, 2, 2, '{}', 1, 1, 1)",
            [LESSON],
        )
        .unwrap();
    remote
        .execute("DELETE FROM document_collections WHERE id = ?1", [SUBJECT])
        .unwrap();

    merge(&scratch, &mut local, &remote, &agreed);
    assert_eq!(count(&local, "SELECT COUNT(*) FROM lessons"), 0);
    assert_eq!(count(&local, "SELECT COUNT(*) FROM lesson_progress"), 0);
}

#[test]
fn lesson_progress_made_on_the_other_device_arrives() {
    let scratch = Scratch::new("lesson-progress");
    let agreed = scratch.library("agreed.sqlite3");
    add_subject(&agreed);
    add_lesson(&agreed);
    let mut local = scratch.copy(&agreed, "local.sqlite3");
    let remote = scratch.copy(&agreed, "remote.sqlite3");
    remote
        .execute(
            "INSERT INTO lesson_progress(lesson_id, revision_number, current_step,
                 furthest_step, checks_json, revision, started_at, updated_at)
             VALUES (?1, 1, 3, 4, '{}', 1, 1, 1)",
            [LESSON],
        )
        .unwrap();

    merge(&scratch, &mut local, &remote, &agreed);
    assert_eq!(
        count(&local, "SELECT furthest_step FROM lesson_progress"),
        4
    );
}

#[test]
fn both_devices_narrating_the_same_text_keep_their_own_audio() {
    let scratch = Scratch::new("narration");
    let agreed = scratch.library("agreed.sqlite3");
    let mut local = scratch.copy(&agreed, "local.sqlite3");
    let remote = scratch.copy(&agreed, "remote.sqlite3");
    for (connection, letter) in [(&local, '1'), (&remote, '2')] {
        connection
            .execute_batch(&format!(
                "INSERT INTO assets(id, original_filename, mime_type, byte_size,
                     relative_path, created_at, lifecycle_state, ready_at, verified_at,
                     updated_at)
                 VALUES ('{asset}', 'narration.mp3', 'audio/mpeg', 10,
                     'assets/images/{asset}.mp3', 1, 'ready', 1, 1, 1);
                 INSERT INTO speech_segments(id, text_sha256, engine, model_id, voice,
                     format, asset_id, characters, created_at)
                 VALUES ('{segment}', '{hash}', 'openrouter', 'hexgrad/kokoro-82m',
                     'af_heart', 'mp3', '{asset}', 12, 1);",
                asset = letter.to_string().repeat(32),
                segment = format!("{letter}e").repeat(16),
                hash = "b".repeat(64),
            ))
            .unwrap();
    }

    merge(&scratch, &mut local, &remote, &agreed);
    assert_eq!(
        count(&local, "SELECT COUNT(*) FROM speech_segments WHERE asset_id = '11111111111111111111111111111111'"),
        1
    );
    assert_eq!(count(&local, "SELECT COUNT(*) FROM speech_segments"), 1);
    assert_eq!(count(&local, "SELECT COUNT(*) FROM assets"), 1);
}
