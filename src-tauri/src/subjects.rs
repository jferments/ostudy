//! Subjects: the one grouping for a course's decks, documents and guides.
//!
//! A subject is a `document_collections` row (release schema 6), so study
//! targets, collection guides and knowledge views keep their references. Every
//! deck and document is filed in at most one subject through its `subject_id`;
//! NULL is "No subject". Subjects do not nest.
//!
//! Two revisions guard concurrent edits:
//!
//! - the library revision (`library_organization`) changes with the subject
//!   list and with deck filing and order;
//! - the document filing revision (`document_collection_organization`) changes
//!   with document filing and study ranges. Study-target source snapshots are
//!   stamped with it, so moving a deck never makes a document's study
//!   materials look out of date.
//!
//! Deleting a subject changes both. The deck folder and document collection
//! commands are adapters over the storage helpers here, so the current screens
//! keep working until the Library replaces them.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::{
    collections::{self, SourceRangeInput},
    database::Database,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

pub(crate) const MAX_SUBJECT_NAME_CHARS: usize = 120;
pub(crate) const MAX_SUBJECTS: i64 = 500;
const ORDER_STEP: i64 = 1_000;
const FILING_REVIEW_LABEL_CHARS: i64 = 160;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectDto {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectDeckPlacementDto {
    pub deck_id: String,
    pub subject_id: Option<String>,
    pub sort_order: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectDocumentPlacementDto {
    pub document_id: String,
    pub subject_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentStudyRangeDto {
    pub id: String,
    pub document_id: String,
    pub unit_kind: String,
    pub start_unit: i64,
    pub end_unit: i64,
    pub source_sha256: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectLibraryDto {
    pub library_revision: i64,
    pub document_filing_revision: i64,
    pub subjects: Vec<SubjectDto>,
    pub decks: Vec<SubjectDeckPlacementDto>,
    pub documents: Vec<SubjectDocumentPlacementDto>,
    pub study_ranges: Vec<DocumentStudyRangeDto>,
    /// Documents left out of their subject's study set, sorted.
    pub left_out_document_ids: Vec<String>,
    pub check_these_count: i64,
}

/// One entry of the "Check these" list: a subject, deck, document or kept
/// collection copy where the upgrade or a sync had to choose.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubjectFilingReviewDto {
    pub entity_kind: String,
    pub entity_id: String,
    pub reason: String,
    pub detail: Value,
    pub label: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSubjectInput {
    pub name: String,
    pub expected_library_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameSubjectInput {
    pub subject_id: String,
    pub name: String,
    pub revision: i64,
    pub expected_library_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveSubjectInput {
    pub subject_id: String,
    pub target_index: i64,
    pub expected_library_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteSubjectInput {
    pub subject_id: String,
    pub revision: i64,
    pub expected_library_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveDeckToSubjectInput {
    pub deck_id: String,
    pub subject_id: Option<String>,
    #[serde(default)]
    pub target_index: Option<i64>,
    pub expected_library_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveDocumentToSubjectInput {
    pub document_id: String,
    pub subject_id: Option<String>,
    pub expected_document_filing_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetDocumentStudyRangesInput {
    pub document_id: String,
    pub ranges: Vec<SourceRangeInput>,
    pub expected_document_filing_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetSubjectLeftOutDocumentsInput {
    pub subject_id: String,
    /// The subject's documents its study set leaves out; every other document
    /// filed in it is let back in.
    pub document_ids: Vec<String>,
    pub expected_document_filing_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DismissSubjectFilingReviewInput {
    pub entity_kind: String,
    pub entity_id: String,
}

#[tauri::command]
pub fn get_subject_library(
    database: State<'_, Database>,
) -> Result<SubjectLibraryDto, CommandError> {
    database.with_connection(|connection| load_library(connection))
}

#[tauri::command]
pub fn create_subject(
    database: State<'_, Database>,
    input: CreateSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    database.with_connection(|connection| create(connection, input))
}

#[tauri::command]
pub fn rename_subject(
    database: State<'_, Database>,
    input: RenameSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_entity_id(&input.subject_id, "subject")?;
    database.with_connection(|connection| rename(connection, input))
}

#[tauri::command]
pub fn move_subject(
    database: State<'_, Database>,
    input: MoveSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_entity_id(&input.subject_id, "subject")?;
    database.with_connection(|connection| reorder(connection, input))
}

#[tauri::command]
pub fn delete_subject(
    database: State<'_, Database>,
    input: DeleteSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_entity_id(&input.subject_id, "subject")?;
    database.with_connection(|connection| delete(connection, input))
}

#[tauri::command]
pub fn move_deck_to_subject(
    database: State<'_, Database>,
    input: MoveDeckToSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    if let Some(subject_id) = input.subject_id.as_deref() {
        validate_entity_id(subject_id, "subject")?;
    }
    database.with_connection(|connection| move_deck(connection, input))
}

#[tauri::command]
pub fn move_document_to_subject(
    database: State<'_, Database>,
    input: MoveDocumentToSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    if let Some(subject_id) = input.subject_id.as_deref() {
        validate_entity_id(subject_id, "subject")?;
    }
    database.with_connection(|connection| move_document(connection, input))
}

#[tauri::command]
pub fn set_document_study_ranges(
    database: State<'_, Database>,
    input: SetDocumentStudyRangesInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    database.with_connection(|connection| set_ranges(connection, input))
}

#[tauri::command]
pub fn set_subject_left_out_documents(
    database: State<'_, Database>,
    input: SetSubjectLeftOutDocumentsInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_entity_id(&input.subject_id, "subject")?;
    for document_id in &input.document_ids {
        validate_entity_id(document_id, "document")?;
    }
    database.with_connection(|connection| set_left_out(connection, input))
}

#[tauri::command]
pub fn get_subject_filing_reviews(
    database: State<'_, Database>,
) -> Result<Vec<SubjectFilingReviewDto>, CommandError> {
    database.with_connection(|connection| load_filing_reviews(connection))
}

#[tauri::command]
pub fn dismiss_subject_filing_review(
    database: State<'_, Database>,
    input: DismissSubjectFilingReviewInput,
) -> Result<Vec<SubjectFilingReviewDto>, CommandError> {
    if !matches!(
        input.entity_kind.as_str(),
        "subject" | "deck" | "document" | "note"
    ) {
        return Err(CommandError::new(
            "invalid_filing_review",
            "That item is not on the Check these list.",
        ));
    }
    validate_entity_id(&input.entity_id, "Check these item")?;
    database.with_connection(|connection| dismiss(connection, input))
}

pub(crate) fn load_library(connection: &Connection) -> Result<SubjectLibraryDto, CommandError> {
    Ok(SubjectLibraryDto {
        library_revision: revision(connection, Revision::Library)?,
        document_filing_revision: revision(connection, Revision::DocumentFiling)?,
        subjects: list_subjects(connection)?,
        decks: deck_placements(connection)?,
        documents: document_placements(connection)?,
        study_ranges: study_ranges(connection, None)?,
        left_out_document_ids: left_out_document_ids(connection)?,
        check_these_count: check_these_count(connection)?,
    })
}

fn create(
    connection: &mut Connection,
    input: CreateSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    let name = validate_name(&input.name)?;
    validate_revision(input.expected_library_revision, "library")?;
    let now = unix_time_millis()?;
    let tx = immediate(connection)?;
    expect_revision(&tx, Revision::Library, input.expected_library_revision)?;
    insert_subject(&tx, &name, now)?;
    advance(&tx, Revision::Library, input.expected_library_revision, now)?;
    tx.commit().map_err(CommandError::database)?;
    load_library(connection)
}

fn rename(
    connection: &mut Connection,
    input: RenameSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    let name = validate_name(&input.name)?;
    validate_revision(input.revision, "subject")?;
    validate_revision(input.expected_library_revision, "library")?;
    let now = unix_time_millis()?;
    let tx = immediate(connection)?;
    expect_revision(&tx, Revision::Library, input.expected_library_revision)?;
    require(rename_subject_row(
        &tx,
        &input.subject_id,
        &name,
        input.revision,
        now,
    )?)?;
    advance(&tx, Revision::Library, input.expected_library_revision, now)?;
    tx.commit().map_err(CommandError::database)?;
    load_library(connection)
}

fn reorder(
    connection: &mut Connection,
    input: MoveSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_revision(input.expected_library_revision, "library")?;
    let now = unix_time_millis()?;
    let tx = immediate(connection)?;
    expect_revision(&tx, Revision::Library, input.expected_library_revision)?;
    require(move_subject_row(
        &tx,
        &input.subject_id,
        input.target_index,
        now,
    )?)?;
    advance(&tx, Revision::Library, input.expected_library_revision, now)?;
    tx.commit().map_err(CommandError::database)?;
    load_library(connection)
}

fn delete(
    connection: &mut Connection,
    input: DeleteSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_revision(input.revision, "subject")?;
    validate_revision(input.expected_library_revision, "library")?;
    let now = unix_time_millis()?;
    let tx = immediate(connection)?;
    expect_revision(&tx, Revision::Library, input.expected_library_revision)?;
    require(delete_subject_row(
        &tx,
        &input.subject_id,
        input.revision,
        now,
    )?)?;
    advance(&tx, Revision::Library, input.expected_library_revision, now)?;
    bump(&tx, Revision::DocumentFiling, None, now)?;
    tx.commit().map_err(CommandError::database)?;
    load_library(connection)
}

fn move_deck(
    connection: &mut Connection,
    input: MoveDeckToSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_revision(input.expected_library_revision, "library")?;
    let now = unix_time_millis()?;
    let tx = immediate(connection)?;
    expect_revision(&tx, Revision::Library, input.expected_library_revision)?;
    if let Some(subject_id) = input.subject_id.as_deref() {
        if !subject_exists(&tx, subject_id)? {
            return Err(subject_not_found());
        }
    }
    match file_deck(
        &tx,
        &input.deck_id,
        input.subject_id.as_deref(),
        input.target_index,
        now,
    )? {
        Outcome::Done => {}
        Outcome::Missing => return Err(deck_not_found()),
        Outcome::Stale | Outcome::OutOfRange => return Err(invalid_target()),
    }
    advance(&tx, Revision::Library, input.expected_library_revision, now)?;
    tx.commit().map_err(CommandError::database)?;
    load_library(connection)
}

fn move_document(
    connection: &mut Connection,
    input: MoveDocumentToSubjectInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_revision(input.expected_document_filing_revision, "document filing")?;
    let now = unix_time_millis()?;
    let tx = immediate(connection)?;
    expect_revision(
        &tx,
        Revision::DocumentFiling,
        input.expected_document_filing_revision,
    )?;
    if let Some(subject_id) = input.subject_id.as_deref() {
        if !subject_exists(&tx, subject_id)? {
            return Err(subject_not_found());
        }
    }
    let current = document_subject(&tx, &input.document_id)?.ok_or_else(document_not_found)?;
    if current != input.subject_id {
        file_document(&tx, &input.document_id, input.subject_id.as_deref(), now)?;
        advance(
            &tx,
            Revision::DocumentFiling,
            input.expected_document_filing_revision,
            now,
        )?;
    }
    tx.commit().map_err(CommandError::database)?;
    load_library(connection)
}

fn set_ranges(
    connection: &mut Connection,
    input: SetDocumentStudyRangesInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_revision(input.expected_document_filing_revision, "document filing")?;
    let now = unix_time_millis()?;
    let tx = immediate(connection)?;
    expect_revision(
        &tx,
        Revision::DocumentFiling,
        input.expected_document_filing_revision,
    )?;
    match document_subject(&tx, &input.document_id)? {
        None => return Err(document_not_found()),
        Some(None) => {
            return Err(CommandError::new(
                "document_not_in_subject",
                "Move this document into a subject before choosing which parts to study.",
            ))
        }
        Some(Some(_)) => {}
    }
    replace_study_ranges(&tx, &input.document_id, input.ranges, now)?;
    advance(
        &tx,
        Revision::DocumentFiling,
        input.expected_document_filing_revision,
        now,
    )?;
    tx.commit().map_err(CommandError::database)?;
    load_library(connection)
}

fn set_left_out(
    connection: &mut Connection,
    input: SetSubjectLeftOutDocumentsInput,
) -> Result<SubjectLibraryDto, CommandError> {
    validate_revision(input.expected_document_filing_revision, "document filing")?;
    let now = unix_time_millis()?;
    let tx = immediate(connection)?;
    expect_revision(
        &tx,
        Revision::DocumentFiling,
        input.expected_document_filing_revision,
    )?;
    if !subject_exists(&tx, &input.subject_id)? {
        return Err(subject_not_found());
    }
    let left_out: BTreeSet<&str> = input.document_ids.iter().map(String::as_str).collect();
    for document_id in &left_out {
        let filed_in = document_subject(&tx, document_id)?.flatten();
        if filed_in.as_deref() != Some(input.subject_id.as_str()) {
            return Err(CommandError::new(
                "document_not_in_subject",
                "That document is no longer in this subject. Reload it and try again.",
            ));
        }
    }
    tx.execute(
        "DELETE FROM document_study_exclusions
         WHERE document_id IN (SELECT id FROM documents WHERE subject_id = ?1)",
        [input.subject_id.as_str()],
    )
    .map_err(CommandError::database)?;
    for document_id in left_out {
        tx.execute(
            "INSERT INTO document_study_exclusions(document_id, created_at) VALUES (?1, ?2)",
            params![document_id, now],
        )
        .map_err(CommandError::database)?;
    }
    advance(
        &tx,
        Revision::DocumentFiling,
        input.expected_document_filing_revision,
        now,
    )?;
    tx.commit().map_err(CommandError::database)?;
    load_library(connection)
}

fn left_out_document_ids(connection: &Connection) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare("SELECT document_id FROM document_study_exclusions ORDER BY document_id")
        .map_err(CommandError::database)?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>();
    ids.map_err(CommandError::database)
}

fn dismiss(
    connection: &mut Connection,
    input: DismissSubjectFilingReviewInput,
) -> Result<Vec<SubjectFilingReviewDto>, CommandError> {
    let now = unix_time_millis()?;
    // Dismissing twice, or an item another window already dismissed, is not an
    // error: the result is the same list either way.
    connection
        .execute(
            "UPDATE subject_filing_reviews SET dismissed_at = ?1
             WHERE entity_kind = ?2 AND entity_id = ?3 AND dismissed_at IS NULL",
            params![now, input.entity_kind, input.entity_id],
        )
        .map_err(CommandError::database)?;
    load_filing_reviews(connection)
}

/// The two organization revisions; see the module documentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Revision {
    Library,
    DocumentFiling,
}

impl Revision {
    fn table(self) -> &'static str {
        match self {
            Self::Library => "library_organization",
            Self::DocumentFiling => "document_collection_organization",
        }
    }
}

pub(crate) fn revision(connection: &Connection, which: Revision) -> Result<i64, CommandError> {
    connection
        .query_row(
            &format!(
                "SELECT revision FROM {} WHERE singleton_id = 1",
                which.table()
            ),
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

/// Advances a revision, only while it still equals `expected` when one is
/// given. Returns whether it advanced. Runs in the caller's transaction.
pub(crate) fn bump(
    connection: &Connection,
    which: Revision,
    expected: Option<i64>,
    now: i64,
) -> Result<bool, CommandError> {
    let changed = connection
        .execute(
            &format!(
                "UPDATE {} SET revision = revision + 1, updated_at = ?1
                 WHERE singleton_id = 1 AND (?2 IS NULL OR revision = ?2)",
                which.table()
            ),
            params![now, expected],
        )
        .map_err(CommandError::database)?;
    Ok(changed == 1)
}

fn expect_revision(
    tx: &Transaction<'_>,
    which: Revision,
    expected: i64,
) -> Result<(), CommandError> {
    if revision(tx, which)? == expected {
        Ok(())
    } else {
        Err(revision_changed(which))
    }
}

fn advance(
    tx: &Transaction<'_>,
    which: Revision,
    expected: i64,
    now: i64,
) -> Result<(), CommandError> {
    if bump(tx, which, Some(expected), now)? {
        Ok(())
    } else {
        Err(revision_changed(which))
    }
}

/// The result of a write that names its row, and where relevant its revision
/// and a target position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    Done,
    Missing,
    Stale,
    OutOfRange,
}

fn require(outcome: Outcome) -> Result<(), CommandError> {
    match outcome {
        Outcome::Done => Ok(()),
        Outcome::Missing => Err(subject_not_found()),
        Outcome::Stale => Err(subject_changed()),
        Outcome::OutOfRange => Err(invalid_target()),
    }
}

pub(crate) fn list_subjects(connection: &Connection) -> Result<Vec<SubjectDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, sort_order, created_at, updated_at, revision
             FROM document_collections
             ORDER BY sort_order, name COLLATE NOCASE, id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok(SubjectDto {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_order: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                revision: row.get(5)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>();
    rows.map_err(CommandError::database)
}

pub(crate) fn subject_exists(connection: &Connection, id: &str) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM document_collections WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

pub(crate) fn subject_revision(
    connection: &Connection,
    id: &str,
) -> Result<Option<i64>, CommandError> {
    connection
        .query_row(
            "SELECT revision FROM document_collections WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)
}

/// Adds a subject at the end of the list and returns its id.
pub(crate) fn insert_subject(
    tx: &Transaction<'_>,
    name: &str,
    now: i64,
) -> Result<String, CommandError> {
    let count: i64 = tx
        .query_row("SELECT COUNT(*) FROM document_collections", [], |row| {
            row.get(0)
        })
        .map_err(CommandError::database)?;
    if count >= MAX_SUBJECTS {
        return Err(CommandError::new(
            "too_many_subjects",
            format!("The library cannot hold more than {MAX_SUBJECTS} subjects."),
        ));
    }
    let id = generate_id(tx)?;
    let sort_order: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + ?1 FROM document_collections",
            [ORDER_STEP],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    tx.execute(
        "INSERT INTO document_collections(id, name, sort_order, created_at, updated_at, revision)
         VALUES (?1, ?2, ?3, ?4, ?4, 1)",
        params![id, name, sort_order, now],
    )
    .map_err(CommandError::database)?;
    Ok(id)
}

pub(crate) fn rename_subject_row(
    tx: &Transaction<'_>,
    id: &str,
    name: &str,
    revision: i64,
    now: i64,
) -> Result<Outcome, CommandError> {
    match subject_revision(tx, id)? {
        None => return Ok(Outcome::Missing),
        Some(current) if current != revision => return Ok(Outcome::Stale),
        Some(_) => {}
    }
    let changed = tx
        .execute(
            "UPDATE document_collections
             SET name = ?1, updated_at = ?2, revision = revision + 1
             WHERE id = ?3 AND revision = ?4",
            params![name, now, id, revision],
        )
        .map_err(CommandError::database)?;
    Ok(if changed == 1 {
        Outcome::Done
    } else {
        Outcome::Stale
    })
}

/// Moves a subject to `target_index` in the list of subjects.
pub(crate) fn move_subject_row(
    tx: &Transaction<'_>,
    id: &str,
    target_index: i64,
    now: i64,
) -> Result<Outcome, CommandError> {
    if !subject_exists(tx, id)? {
        return Ok(Outcome::Missing);
    }
    let mut order = list_subjects(tx)?
        .into_iter()
        .map(|subject| subject.id)
        .filter(|subject_id| subject_id != id)
        .collect::<Vec<_>>();
    let Some(index) = index_within(target_index, order.len()) else {
        return Ok(Outcome::OutOfRange);
    };
    order.insert(index, id.to_string());
    for (position, subject_id) in order.iter().enumerate() {
        tx.execute(
            "UPDATE document_collections SET sort_order = ?1, updated_at = ?2
             WHERE id = ?3 AND sort_order <> ?1",
            params![order_for(position)?, now, subject_id],
        )
        .map_err(CommandError::database)?;
    }
    Ok(Outcome::Done)
}

/// Deletes a subject. Its decks join the end of No subject in their order, and
/// its documents go to No subject (which clears their study ranges). Its study
/// targets, overview guide and knowledge views go with it, as a deleted
/// collection's always have.
pub(crate) fn delete_subject_row(
    tx: &Transaction<'_>,
    id: &str,
    revision: i64,
    now: i64,
) -> Result<Outcome, CommandError> {
    match subject_revision(tx, id)? {
        None => return Ok(Outcome::Missing),
        Some(current) if current != revision => return Ok(Outcome::Stale),
        Some(_) => {}
    }
    let mut unfiled = ordered_deck_ids(tx, None, None)?;
    unfiled.extend(ordered_deck_ids(tx, Some(id), None)?);
    write_deck_order(tx, None, &unfiled, now)?;
    let deleted = tx
        .execute(
            "DELETE FROM document_collections WHERE id = ?1 AND revision = ?2",
            params![id, revision],
        )
        .map_err(CommandError::database)?;
    Ok(if deleted == 1 {
        Outcome::Done
    } else {
        Outcome::Stale
    })
}

/// Whether a subject holds documents or study materials, which only the
/// Documents screens show.
pub(crate) fn subject_holds_documents(
    connection: &Connection,
    id: &str,
) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM documents WHERE subject_id = ?1)
                 OR EXISTS(SELECT 1 FROM study_targets WHERE collection_id = ?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

pub(crate) fn ordered_deck_ids(
    connection: &Connection,
    subject_id: Option<&str>,
    excluding: Option<&str>,
) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id FROM decks
             WHERE deleted_at IS NULL
               AND subject_id IS ?1
               AND (?2 IS NULL OR id <> ?2)
             ORDER BY sort_order, name COLLATE NOCASE, created_at",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![subject_id, excluding], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>();
    rows.map_err(CommandError::database)
}

fn write_deck_order(
    connection: &Connection,
    subject_id: Option<&str>,
    ordered_ids: &[String],
    now: i64,
) -> Result<(), CommandError> {
    for (position, deck_id) in ordered_ids.iter().enumerate() {
        let changed = connection
            .execute(
                "UPDATE decks SET subject_id = ?1, sort_order = ?2, updated_at = ?3
                 WHERE id = ?4 AND deleted_at IS NULL",
                params![subject_id, order_for(position)?, now, deck_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "deck_not_found",
                "A deck changed while its library position was being updated.",
            ));
        }
    }
    Ok(())
}

/// Files a live deck in a subject, or in No subject, at `target_index` among
/// that subject's decks or at the end, and closes the gap it leaves. Runs in
/// the caller's transaction.
pub(crate) fn file_deck(
    connection: &Connection,
    deck_id: &str,
    subject_id: Option<&str>,
    target_index: Option<i64>,
    now: i64,
) -> Result<Outcome, CommandError> {
    let previous: Option<Option<String>> = connection
        .query_row(
            "SELECT subject_id FROM decks WHERE id = ?1 AND deleted_at IS NULL",
            [deck_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(previous) = previous else {
        return Ok(Outcome::Missing);
    };
    let mut order = ordered_deck_ids(connection, subject_id, Some(deck_id))?;
    let index = match target_index {
        None => order.len(),
        Some(target_index) => match index_within(target_index, order.len()) {
            Some(index) => index,
            None => return Ok(Outcome::OutOfRange),
        },
    };
    order.insert(index, deck_id.to_string());
    write_deck_order(connection, subject_id, &order, now)?;
    if previous.as_deref() != subject_id {
        let remaining = ordered_deck_ids(connection, previous.as_deref(), None)?;
        write_deck_order(connection, previous.as_deref(), &remaining, now)?;
    }
    Ok(Outcome::Done)
}

/// The subject a document is filed in: `None` when there is no such document,
/// `Some(None)` when it is under No subject.
pub(crate) fn document_subject(
    connection: &Connection,
    document_id: &str,
) -> Result<Option<Option<String>>, CommandError> {
    connection
        .query_row(
            "SELECT subject_id FROM documents WHERE id = ?1",
            [document_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)
}

/// Files a document in a subject or in No subject. A document keeps its study
/// ranges when it moves between subjects and loses them when it leaves every
/// subject (a schema trigger clears them). Its generated deck goes with it; see
/// `file_generated_deck_with_document`.
pub(crate) fn file_document(
    tx: &Transaction<'_>,
    document_id: &str,
    subject_id: Option<&str>,
    now: i64,
) -> Result<(), CommandError> {
    let previous = document_subject(tx, document_id)?.ok_or_else(document_not_found)?;
    tx.execute(
        "UPDATE documents SET subject_id = ?1 WHERE id = ?2",
        params![subject_id, document_id],
    )
    .map_err(CommandError::database)?;
    file_generated_deck_with_document(tx, document_id, previous.as_deref(), now)
}

/// Files a document's generated deck in the document's subject, so a subject
/// keeps a document's flashcards and studying it includes them. The deck
/// follows only from No subject or from `previous_subject_id`, the subject the
/// document just left: a deck the learner filed somewhere else stays there, and
/// so does a deck that another document in its subject also generated or had
/// merged into it. Runs in the caller's transaction.
pub(crate) fn file_generated_deck_with_document(
    connection: &Connection,
    document_id: &str,
    previous_subject_id: Option<&str>,
    now: i64,
) -> Result<(), CommandError> {
    let placement: Option<(String, Option<String>, Option<String>)> = connection
        .query_row(
            "SELECT deck.id, deck.subject_id, document.subject_id
             FROM documents document
             JOIN decks deck ON deck.id = document.generated_deck_id
                            AND deck.deleted_at IS NULL
             WHERE document.id = ?1",
            [document_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((deck_id, deck_subject, document_subject)) = placement else {
        return Ok(());
    };
    if deck_subject == document_subject {
        return Ok(());
    }
    if let Some(deck_subject) = deck_subject.as_deref() {
        let shared: bool = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM documents other
                     WHERE other.id <> ?1 AND other.subject_id = ?2
                       AND (other.generated_deck_id = ?3
                            OR EXISTS (SELECT 1 FROM document_decks link
                                       WHERE link.document_id = other.id
                                         AND link.deck_id = ?3)))",
                params![document_id, deck_subject, deck_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if Some(deck_subject) != previous_subject_id || shared {
            return Ok(());
        }
    }
    file_deck(connection, &deck_id, document_subject.as_deref(), None, now)?;
    bump(connection, Revision::Library, None, now)?;
    Ok(())
}

/// Replaces a document's study ranges with the normalized `ranges`, bound to
/// its current source; an empty list studies the whole document.
pub(crate) fn replace_study_ranges(
    tx: &Transaction<'_>,
    document_id: &str,
    ranges: Vec<SourceRangeInput>,
    now: i64,
) -> Result<(), CommandError> {
    let shape = collections::load_document_shape(tx, document_id)?;
    let normalized = collections::validate_and_normalize_ranges(tx, &shape, ranges)?;
    tx.execute(
        "DELETE FROM document_study_ranges WHERE document_id = ?1",
        [document_id],
    )
    .map_err(CommandError::database)?;
    for range in normalized {
        let id = generate_id(tx)?;
        tx.execute(
            "INSERT INTO document_study_ranges(
                 id, document_id, unit_kind, start_unit, end_unit, source_sha256, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                document_id,
                range.unit_kind,
                range.start_unit,
                range.end_unit,
                shape.sha256,
                now
            ],
        )
        .map_err(CommandError::database)?;
    }
    Ok(())
}

/// Study ranges of every document, or of one, in document and unit order.
pub(crate) fn study_ranges(
    connection: &Connection,
    document_id: Option<&str>,
) -> Result<Vec<DocumentStudyRangeDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, document_id, unit_kind, start_unit, end_unit, source_sha256, created_at
             FROM document_study_ranges
             WHERE ?1 IS NULL OR document_id = ?1
             ORDER BY document_id, start_unit, end_unit, id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([document_id], |row| {
            Ok(DocumentStudyRangeDto {
                id: row.get(0)?,
                document_id: row.get(1)?,
                unit_kind: row.get(2)?,
                start_unit: row.get(3)?,
                end_unit: row.get(4)?,
                source_sha256: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>();
    rows.map_err(CommandError::database)
}

pub(crate) fn deck_placements(
    connection: &Connection,
) -> Result<Vec<SubjectDeckPlacementDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, subject_id, sort_order
             FROM decks
             WHERE deleted_at IS NULL
             ORDER BY subject_id IS NOT NULL DESC, subject_id,
                      sort_order, name COLLATE NOCASE, created_at",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok(SubjectDeckPlacementDto {
                deck_id: row.get(0)?,
                subject_id: row.get(1)?,
                sort_order: row.get(2)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>();
    rows.map_err(CommandError::database)
}

fn document_placements(
    connection: &Connection,
) -> Result<Vec<SubjectDocumentPlacementDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, subject_id FROM documents
             ORDER BY subject_id IS NOT NULL DESC, subject_id, title COLLATE NOCASE, id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok(SubjectDocumentPlacementDto {
                document_id: row.get(0)?,
                subject_id: row.get(1)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>();
    rows.map_err(CommandError::database)
}

/// Undismissed "Check these" entries whose subject, deck, document or kept
/// copy still exists, labeled with its current name. A collection copy that
/// another device edited after this one retired it stays listed while retired.
const OPEN_FILING_REVIEWS: &str = "
    SELECT entity_kind, entity_id, reason, detail_json, label FROM (
        SELECT r.entity_kind, r.entity_id, r.reason, r.detail_json,
               CASE r.entity_kind
                   WHEN 'subject' THEN (SELECT c.name FROM document_collections c
                                        WHERE c.id = r.entity_id)
                   WHEN 'deck' THEN (SELECT k.name FROM decks k
                                     WHERE k.id = r.entity_id AND k.deleted_at IS NULL)
                   WHEN 'document' THEN (SELECT d.title FROM documents d
                                         WHERE d.id = r.entity_id)
                   WHEN 'note' THEN (SELECT substr(n.front_text, 1, ?1) FROM notes n
                                     WHERE n.id = r.entity_id
                                       AND (n.deleted_at IS NULL
                                            OR r.reason = 'sync_edited_retired_copy'))
               END AS label,
               CASE r.entity_kind
                   WHEN 'subject' THEN 0 WHEN 'deck' THEN 1 WHEN 'document' THEN 2 ELSE 3
               END AS kind_order
        FROM subject_filing_reviews r
        WHERE r.dismissed_at IS NULL)
    WHERE label IS NOT NULL";

pub(crate) fn load_filing_reviews(
    connection: &Connection,
) -> Result<Vec<SubjectFilingReviewDto>, CommandError> {
    let mut statement = connection
        .prepare(&format!(
            "{OPEN_FILING_REVIEWS} ORDER BY kind_order, label COLLATE NOCASE, entity_id"
        ))
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([FILING_REVIEW_LABEL_CHARS], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    rows.into_iter()
        .map(|(entity_kind, entity_id, reason, detail_json, label)| {
            Ok(SubjectFilingReviewDto {
                entity_kind,
                entity_id,
                reason,
                detail: serde_json::from_str(&detail_json).map_err(CommandError::serialization)?,
                label,
            })
        })
        .collect()
}

fn check_these_count(connection: &Connection) -> Result<i64, CommandError> {
    connection
        .query_row(
            &format!("SELECT COUNT(*) FROM ({OPEN_FILING_REVIEWS})"),
            [FILING_REVIEW_LABEL_CHARS],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

fn index_within(target_index: i64, len: usize) -> Option<usize> {
    usize::try_from(target_index)
        .ok()
        .filter(|index| *index <= len)
}

fn order_for(position: usize) -> Result<i64, CommandError> {
    i64::try_from(position)
        .ok()
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_mul(ORDER_STEP))
        .ok_or_else(|| {
            CommandError::new(
                "library_order_too_large",
                "The library contains too many ordered items.",
            )
        })
}

fn immediate(connection: &mut Connection) -> Result<Transaction<'_>, CommandError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)
}

/// Trims a subject name and checks its length.
pub(crate) fn trimmed_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let count = trimmed.chars().count();
    (1..=MAX_SUBJECT_NAME_CHARS)
        .contains(&count)
        .then(|| trimmed.to_string())
}

fn validate_name(value: &str) -> Result<String, CommandError> {
    trimmed_name(value).ok_or_else(|| {
        CommandError::new(
            "invalid_subject_name",
            format!("Enter a subject name of 1 to {MAX_SUBJECT_NAME_CHARS} characters."),
        )
    })
}

fn validate_revision(revision: i64, label: &str) -> Result<(), CommandError> {
    if revision >= 1 {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_revision",
            format!("The {label} revision is invalid."),
        ))
    }
}

fn revision_changed(which: Revision) -> CommandError {
    match which {
        Revision::Library => CommandError::new(
            "library_organization_changed",
            "The library changed. Reload it and try again.",
        ),
        Revision::DocumentFiling => CommandError::new(
            "document_filing_changed",
            "Document filing changed. Reload the library and try again.",
        ),
    }
}

fn subject_not_found() -> CommandError {
    CommandError::new("subject_not_found", "That subject no longer exists.")
}

fn subject_changed() -> CommandError {
    CommandError::new(
        "subject_changed",
        "This subject changed in another window. Reload it and try again.",
    )
}

fn deck_not_found() -> CommandError {
    CommandError::new("deck_not_found", "That deck no longer exists.")
}

fn document_not_found() -> CommandError {
    CommandError::new(
        "document_not_found",
        "That document is no longer in the library.",
    )
}

fn invalid_target() -> CommandError {
    CommandError::new(
        "invalid_library_target",
        "The requested library position is no longer available.",
    )
}

#[cfg(test)]
mod tests;
