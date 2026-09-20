//! Document collection organization and permission-safe source-set resolution.
//!
//! Collections reference retained documents; they never own or duplicate source
//! bytes or prepared knowledge. Reading ranges bind to canonical source-unit
//! identities and to the document SHA that was current when the range was saved.

use std::collections::{btree_map::Entry, BTreeMap, BTreeSet, HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_COLLECTION_NAME_CHARS: usize = 120;
const MAX_COLLECTION_DEPTH: usize = 8;
const MAX_COLLECTIONS: usize = 500;
const MAX_MEMBERSHIPS: usize = 4_096;
const MAX_SELECTION_COLLECTIONS: usize = 64;
const MAX_SELECTION_DOCUMENTS: usize = 256;
const MAX_RANGES_PER_DOCUMENT: usize = 64;
const MAX_EXPANDED_DOCUMENTS: usize = 256;
const MAX_EXPANDED_UNITS: usize = 16_384;
const MAX_EXPANDED_TEXT_BYTES: usize = 16 * 1024 * 1024;
const ORDER_STEP: i64 = 1_000;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCollectionDto {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub archived_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCollectionMembershipDto {
    pub id: String,
    pub collection_id: String,
    pub document_id: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCollectionRangeDto {
    pub id: String,
    pub membership_id: String,
    pub unit_kind: String,
    pub start_unit: i64,
    pub end_unit: i64,
    pub source_sha256: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCollectionOrganizationDto {
    pub revision: i64,
    pub collections: Vec<DocumentCollectionDto>,
    pub memberships: Vec<DocumentCollectionMembershipDto>,
    pub ranges: Vec<DocumentCollectionRangeDto>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateDocumentCollectionInput {
    pub operation_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub expected_organization_revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateDocumentCollectionInput {
    pub operation_id: String,
    pub collection_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub archived: bool,
    pub revision: i64,
    pub expected_organization_revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteDocumentCollectionInput {
    pub operation_id: String,
    pub collection_id: String,
    pub revision: i64,
    pub expected_organization_revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddDocumentCollectionMembershipInput {
    pub operation_id: String,
    pub collection_id: String,
    pub document_id: String,
    pub expected_organization_revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveDocumentCollectionMembershipInput {
    pub operation_id: String,
    pub membership_id: String,
    pub target_collection_id: String,
    pub revision: i64,
    pub expected_organization_revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveDocumentCollectionMembershipInput {
    pub operation_id: String,
    pub membership_id: String,
    pub revision: i64,
    pub expected_organization_revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceRangeInput {
    pub unit_kind: String,
    pub start_unit: i64,
    pub end_unit: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetDocumentCollectionRangesInput {
    pub operation_id: String,
    pub membership_id: String,
    pub ranges: Vec<SourceRangeInput>,
    pub revision: i64,
    pub expected_organization_revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectedDocumentInput {
    pub document_id: String,
    #[serde(default)]
    pub ranges: Vec<SourceRangeInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSelectionInput {
    #[serde(default)]
    pub collection_ids: Vec<String>,
    #[serde(default)]
    pub documents: Vec<SelectedDocumentInput>,
    #[serde(default)]
    pub exclusions: Vec<SelectedDocumentInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSourceRangeDto {
    pub unit_kind: String,
    pub start_unit: i64,
    pub end_unit: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceMembershipReferenceDto {
    pub collection_id: String,
    pub membership_id: String,
    pub membership_revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceSetMemberDto {
    pub document_id: String,
    pub source_sha256: String,
    pub document_revision: i64,
    pub source_format: String,
    pub source_unit_kind: String,
    pub source_unit_count: i64,
    pub ranges: Vec<ResolvedSourceRangeDto>,
    pub collection_references: Vec<SourceMembershipReferenceDto>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceDispositionDto {
    pub document_id: Option<String>,
    pub collection_id: Option<String>,
    pub membership_id: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceSetSnapshotDto {
    pub snapshot_id: String,
    pub organization_revision: i64,
    pub members: Vec<SourceSetMemberDto>,
    pub dispositions: Vec<SourceDispositionDto>,
    pub expanded_unit_count: i64,
    pub expanded_text_bytes: i64,
}

#[derive(Clone, Debug)]
struct DocumentShape {
    id: String,
    sha256: String,
    revision: i64,
    source_format: String,
    unit_kind: String,
    unit_count: i64,
}

#[derive(Clone, Debug)]
struct MembershipSource {
    collection_id: String,
    membership_id: String,
    membership_revision: i64,
    document_id: String,
    ranges: Vec<DocumentCollectionRangeDto>,
}

#[tauri::command]
pub fn get_document_collection_organization(
    database: State<'_, Database>,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    database.with_connection(|connection| load_organization(connection))
}

#[tauri::command]
pub fn create_document_collection(
    database: State<'_, Database>,
    input: CreateDocumentCollectionInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_optional_id(input.parent_id.as_deref(), "parent document collection")?;
    database.with_connection(|connection| create_collection(connection, input))
}

#[tauri::command]
pub fn update_document_collection(
    database: State<'_, Database>,
    input: UpdateDocumentCollectionInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_entity_id(&input.collection_id, "document collection")?;
    validate_optional_id(input.parent_id.as_deref(), "parent document collection")?;
    database.with_connection(|connection| update_collection(connection, input))
}

#[tauri::command]
pub fn delete_document_collection(
    database: State<'_, Database>,
    input: DeleteDocumentCollectionInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_entity_id(&input.collection_id, "document collection")?;
    database.with_connection(|connection| delete_collection(connection, input))
}

#[tauri::command]
pub fn add_document_collection_membership(
    database: State<'_, Database>,
    input: AddDocumentCollectionMembershipInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_entity_id(&input.collection_id, "document collection")?;
    validate_entity_id(&input.document_id, "document")?;
    database.with_connection(|connection| add_membership(connection, input))
}

#[tauri::command]
pub fn move_document_collection_membership(
    database: State<'_, Database>,
    input: MoveDocumentCollectionMembershipInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_entity_id(&input.membership_id, "document collection membership")?;
    validate_entity_id(&input.target_collection_id, "document collection")?;
    database.with_connection(|connection| move_membership(connection, input))
}

#[tauri::command]
pub fn remove_document_collection_membership(
    database: State<'_, Database>,
    input: RemoveDocumentCollectionMembershipInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_entity_id(&input.membership_id, "document collection membership")?;
    database.with_connection(|connection| remove_membership(connection, input))
}

#[tauri::command]
pub fn set_document_collection_ranges(
    database: State<'_, Database>,
    input: SetDocumentCollectionRangesInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_operation_id(&input.operation_id)?;
    validate_entity_id(&input.membership_id, "document collection membership")?;
    database.with_connection(|connection| set_membership_ranges(connection, input))
}

#[tauri::command]
pub fn resolve_document_source_selection(
    database: State<'_, Database>,
    input: SourceSelectionInput,
) -> Result<SourceSetSnapshotDto, CommandError> {
    database.with_connection(|connection| resolve_selection(connection, input))
}

pub(crate) fn sanitize_for_device_transfer(connection: &Connection) -> Result<(), CommandError> {
    connection
        .execute("DELETE FROM document_collection_operation_receipts", [])
        .map_err(CommandError::database)?;
    Ok(())
}

fn load_organization(
    connection: &Connection,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    let revision = organization_revision(connection)?;
    let collections = {
        let mut statement = connection
            .prepare(
                "SELECT id,parent_id,name,sort_order,archived_at,created_at,updated_at,revision
                 FROM document_collections
                 ORDER BY parent_id IS NOT NULL,parent_id,sort_order,name COLLATE NOCASE,id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| {
                Ok(DocumentCollectionDto {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    sort_order: row.get(3)?,
                    archived_at: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    revision: row.get(7)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    let memberships = {
        let mut statement = connection
            .prepare(
                "SELECT id,collection_id,document_id,sort_order,created_at,updated_at,revision
                 FROM document_collection_memberships
                 ORDER BY collection_id,sort_order,id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| {
                Ok(DocumentCollectionMembershipDto {
                    id: row.get(0)?,
                    collection_id: row.get(1)?,
                    document_id: row.get(2)?,
                    sort_order: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    revision: row.get(6)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    let ranges = {
        let mut statement = connection
            .prepare(
                "SELECT id,membership_id,unit_kind,start_unit,end_unit,source_sha256,created_at
                 FROM document_collection_ranges
                 ORDER BY membership_id,start_unit,end_unit,id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| {
                Ok(DocumentCollectionRangeDto {
                    id: row.get(0)?,
                    membership_id: row.get(1)?,
                    unit_kind: row.get(2)?,
                    start_unit: row.get(3)?,
                    end_unit: row.get(4)?,
                    source_sha256: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    Ok(DocumentCollectionOrganizationDto {
        revision,
        collections,
        memberships,
        ranges,
    })
}

fn create_collection(
    connection: &mut Connection,
    input: CreateDocumentCollectionInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    let name = validate_name(input.name.clone())?;
    validate_revision(
        input.expected_organization_revision,
        "document collection organization",
    )?;
    let request_hash = request_hash(&input)?;
    if let Some(replayed) = replay_receipt(connection, &input.operation_id, &request_hash)? {
        return Ok(replayed);
    }
    let now = unix_time_millis()?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&tx, input.expected_organization_revision)?;
    let count: i64 = tx
        .query_row("SELECT COUNT(*) FROM document_collections", [], |row| {
            row.get(0)
        })
        .map_err(CommandError::database)?;
    if count >= MAX_COLLECTIONS as i64 {
        return Err(invalid(
            "The document library contains too many collections.",
        ));
    }
    let id = generate_id(&tx)?;
    let mut parents = collection_parent_map(&tx)?;
    parents.insert(id.clone(), input.parent_id.clone());
    validate_hierarchy(&parents)?;
    let sort_order = next_collection_order(&tx, input.parent_id.as_deref())?;
    tx.execute(
        "INSERT INTO document_collections(
             id,parent_id,name,sort_order,archived_at,created_at,updated_at,revision
         ) VALUES(?1,?2,?3,?4,NULL,?5,?5,1)",
        params![id, input.parent_id, name, sort_order, now],
    )
    .map_err(CommandError::database)?;
    bump_organization(&tx, input.expected_organization_revision, now)?;
    let result = load_organization(&tx)?;
    store_receipt(&tx, &input.operation_id, &request_hash, &result, now)?;
    tx.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn update_collection(
    connection: &mut Connection,
    input: UpdateDocumentCollectionInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    let name = validate_name(input.name.clone())?;
    validate_revision(input.revision, "document collection")?;
    validate_revision(
        input.expected_organization_revision,
        "document collection organization",
    )?;
    let request_hash = request_hash(&input)?;
    if let Some(replayed) = replay_receipt(connection, &input.operation_id, &request_hash)? {
        return Ok(replayed);
    }
    let now = unix_time_millis()?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&tx, input.expected_organization_revision)?;
    let current: Option<i64> = tx
        .query_row(
            "SELECT revision FROM document_collections WHERE id=?1",
            [&input.collection_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    match current {
        None => return Err(collection_not_found()),
        Some(revision) if revision != input.revision => return Err(collection_changed()),
        Some(_) => {}
    }
    let mut parents = collection_parent_map(&tx)?;
    parents.insert(input.collection_id.clone(), input.parent_id.clone());
    validate_hierarchy(&parents)?;
    let archived_at = if input.archived { Some(now) } else { None };
    let changed = tx
        .execute(
            "UPDATE document_collections
             SET parent_id=?1,name=?2,archived_at=?3,updated_at=?4,revision=revision+1
             WHERE id=?5 AND revision=?6",
            params![
                input.parent_id,
                name,
                archived_at,
                now,
                input.collection_id,
                input.revision
            ],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(collection_changed());
    }
    bump_organization(&tx, input.expected_organization_revision, now)?;
    let result = load_organization(&tx)?;
    store_receipt(&tx, &input.operation_id, &request_hash, &result, now)?;
    tx.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn delete_collection(
    connection: &mut Connection,
    input: DeleteDocumentCollectionInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_revision(input.revision, "document collection")?;
    validate_revision(
        input.expected_organization_revision,
        "document collection organization",
    )?;
    let request_hash = request_hash(&input)?;
    if let Some(replayed) = replay_receipt(connection, &input.operation_id, &request_hash)? {
        return Ok(replayed);
    }
    let now = unix_time_millis()?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&tx, input.expected_organization_revision)?;
    let current: Option<i64> = tx
        .query_row(
            "SELECT revision FROM document_collections WHERE id=?1",
            [&input.collection_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    match current {
        None => return Err(collection_not_found()),
        Some(revision) if revision != input.revision => return Err(collection_changed()),
        Some(_) => {}
    }
    let child_count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM document_collections WHERE parent_id=?1",
            [&input.collection_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if child_count != 0 {
        return Err(invalid(
            "Move or delete nested document folders before deleting this folder.",
        ));
    }
    let deleted = tx
        .execute(
            "DELETE FROM document_collections WHERE id=?1 AND revision=?2",
            params![input.collection_id, input.revision],
        )
        .map_err(CommandError::database)?;
    if deleted != 1 {
        return Err(collection_changed());
    }
    bump_organization(&tx, input.expected_organization_revision, now)?;
    let result = load_organization(&tx)?;
    store_receipt(&tx, &input.operation_id, &request_hash, &result, now)?;
    tx.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn add_membership(
    connection: &mut Connection,
    input: AddDocumentCollectionMembershipInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_revision(
        input.expected_organization_revision,
        "document collection organization",
    )?;
    let request_hash = request_hash(&input)?;
    if let Some(replayed) = replay_receipt(connection, &input.operation_id, &request_hash)? {
        return Ok(replayed);
    }
    let now = unix_time_millis()?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&tx, input.expected_organization_revision)?;
    require_collection(&tx, &input.collection_id)?;
    require_document(&tx, &input.document_id)?;
    let count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM document_collection_memberships",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if count >= MAX_MEMBERSHIPS as i64 {
        return Err(invalid(
            "The document library contains too many collection memberships.",
        ));
    }
    let duplicate: bool = tx
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM document_collection_memberships
                 WHERE collection_id=?1 AND document_id=?2
             )",
            params![input.collection_id, input.document_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if duplicate {
        return Err(invalid("That document is already in this folder."));
    }
    let id = generate_id(&tx)?;
    let sort_order = next_membership_order(&tx, &input.collection_id)?;
    tx.execute(
        "INSERT INTO document_collection_memberships(
             id,collection_id,document_id,sort_order,created_at,updated_at,revision
         ) VALUES(?1,?2,?3,?4,?5,?5,1)",
        params![id, input.collection_id, input.document_id, sort_order, now],
    )
    .map_err(CommandError::database)?;
    bump_organization(&tx, input.expected_organization_revision, now)?;
    let result = load_organization(&tx)?;
    store_receipt(&tx, &input.operation_id, &request_hash, &result, now)?;
    tx.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn move_membership(
    connection: &mut Connection,
    input: MoveDocumentCollectionMembershipInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_revision(input.revision, "document collection membership")?;
    validate_revision(
        input.expected_organization_revision,
        "document collection organization",
    )?;
    let request_hash = request_hash(&input)?;
    if let Some(replayed) = replay_receipt(connection, &input.operation_id, &request_hash)? {
        return Ok(replayed);
    }
    let now = unix_time_millis()?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&tx, input.expected_organization_revision)?;
    require_collection(&tx, &input.target_collection_id)?;
    let row: Option<(String, i64)> = tx
        .query_row(
            "SELECT document_id,revision
             FROM document_collection_memberships WHERE id=?1",
            [&input.membership_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((document_id, revision)) = row else {
        return Err(membership_not_found());
    };
    if revision != input.revision {
        return Err(membership_changed());
    }
    let duplicate: bool = tx
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM document_collection_memberships
                 WHERE collection_id=?1 AND document_id=?2 AND id<>?3
             )",
            params![input.target_collection_id, document_id, input.membership_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if duplicate {
        return Err(invalid(
            "That document is already in the destination folder.",
        ));
    }
    let sort_order = next_membership_order(&tx, &input.target_collection_id)?;
    let changed = tx
        .execute(
            "UPDATE document_collection_memberships
             SET collection_id=?1,sort_order=?2,updated_at=?3,revision=revision+1
             WHERE id=?4 AND revision=?5",
            params![
                input.target_collection_id,
                sort_order,
                now,
                input.membership_id,
                input.revision
            ],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(membership_changed());
    }
    bump_organization(&tx, input.expected_organization_revision, now)?;
    let result = load_organization(&tx)?;
    store_receipt(&tx, &input.operation_id, &request_hash, &result, now)?;
    tx.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn remove_membership(
    connection: &mut Connection,
    input: RemoveDocumentCollectionMembershipInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_revision(input.revision, "document collection membership")?;
    validate_revision(
        input.expected_organization_revision,
        "document collection organization",
    )?;
    let request_hash = request_hash(&input)?;
    if let Some(replayed) = replay_receipt(connection, &input.operation_id, &request_hash)? {
        return Ok(replayed);
    }
    let now = unix_time_millis()?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&tx, input.expected_organization_revision)?;
    let deleted = tx
        .execute(
            "DELETE FROM document_collection_memberships
             WHERE id=?1 AND revision=?2",
            params![input.membership_id, input.revision],
        )
        .map_err(CommandError::database)?;
    if deleted != 1 {
        return Err(membership_changed());
    }
    bump_organization(&tx, input.expected_organization_revision, now)?;
    let result = load_organization(&tx)?;
    store_receipt(&tx, &input.operation_id, &request_hash, &result, now)?;
    tx.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn set_membership_ranges(
    connection: &mut Connection,
    input: SetDocumentCollectionRangesInput,
) -> Result<DocumentCollectionOrganizationDto, CommandError> {
    validate_revision(input.revision, "document collection membership")?;
    validate_revision(
        input.expected_organization_revision,
        "document collection organization",
    )?;
    if input.ranges.len() > MAX_RANGES_PER_DOCUMENT {
        return Err(invalid("A document selection contains too many ranges."));
    }
    let request_hash = request_hash(&input)?;
    if let Some(replayed) = replay_receipt(connection, &input.operation_id, &request_hash)? {
        return Ok(replayed);
    }
    let now = unix_time_millis()?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&tx, input.expected_organization_revision)?;
    let row: Option<(String, i64)> = tx
        .query_row(
            "SELECT document_id,revision
             FROM document_collection_memberships WHERE id=?1",
            [&input.membership_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((document_id, revision)) = row else {
        return Err(membership_not_found());
    };
    if revision != input.revision {
        return Err(membership_changed());
    }
    let shape = load_document_shape(&tx, &document_id)?;
    let normalized = validate_and_normalize_ranges(&tx, &shape, input.ranges.clone())?;
    tx.execute(
        "DELETE FROM document_collection_ranges WHERE membership_id=?1",
        [&input.membership_id],
    )
    .map_err(CommandError::database)?;
    for range in normalized {
        let range_id = generate_id(&tx)?;
        tx.execute(
            "INSERT INTO document_collection_ranges(
                 id,membership_id,unit_kind,start_unit,end_unit,source_sha256,created_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                range_id,
                input.membership_id,
                range.unit_kind,
                range.start_unit,
                range.end_unit,
                shape.sha256,
                now
            ],
        )
        .map_err(CommandError::database)?;
    }
    let changed = tx
        .execute(
            "UPDATE document_collection_memberships
             SET updated_at=?1,revision=revision+1
             WHERE id=?2 AND revision=?3",
            params![now, input.membership_id, input.revision],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(membership_changed());
    }
    bump_organization(&tx, input.expected_organization_revision, now)?;
    let result = load_organization(&tx)?;
    store_receipt(&tx, &input.operation_id, &request_hash, &result, now)?;
    tx.commit().map_err(CommandError::database)?;
    Ok(result)
}

pub(crate) fn resolve_selection(
    connection: &Connection,
    mut input: SourceSelectionInput,
) -> Result<SourceSetSnapshotDto, CommandError> {
    canonicalize_selection(&mut input)?;
    let organization_revision = organization_revision(connection)?;
    let parents = collection_parent_map(connection)?;
    validate_hierarchy(&parents)?;
    let selected_collections = expand_collection_ids(connection, &input.collection_ids)?;
    let membership_sources = load_membership_sources(connection, &selected_collections)?;

    #[derive(Default)]
    struct Pending {
        whole: bool,
        ranges: Vec<SourceRangeInput>,
        refs: Vec<SourceMembershipReferenceDto>,
    }

    let mut pending: BTreeMap<String, Pending> = BTreeMap::new();
    let mut dispositions = Vec::new();
    for membership in membership_sources {
        let shape = load_document_shape(connection, &membership.document_id)?;
        if membership
            .ranges
            .iter()
            .any(|range| range.source_sha256 != shape.sha256)
        {
            dispositions.push(SourceDispositionDto {
                document_id: Some(membership.document_id),
                collection_id: Some(membership.collection_id),
                membership_id: Some(membership.membership_id),
                reason: "range_source_changed".to_string(),
            });
            continue;
        }
        let entry = pending.entry(membership.document_id.clone()).or_default();
        if membership.ranges.is_empty() {
            entry.whole = true;
            entry.ranges.clear();
        } else if !entry.whole {
            entry
                .ranges
                .extend(membership.ranges.iter().map(|range| SourceRangeInput {
                    unit_kind: range.unit_kind.clone(),
                    start_unit: range.start_unit,
                    end_unit: range.end_unit,
                }));
        }
        entry.refs.push(SourceMembershipReferenceDto {
            collection_id: membership.collection_id,
            membership_id: membership.membership_id,
            membership_revision: membership.membership_revision,
        });
    }

    for selected in &input.documents {
        let shape = load_document_shape(connection, &selected.document_id)?;
        let normalized =
            validate_and_normalize_ranges(connection, &shape, selected.ranges.clone())?;
        let entry = pending.entry(selected.document_id.clone()).or_default();
        if normalized.is_empty() {
            entry.whole = true;
            entry.ranges.clear();
        } else if !entry.whole {
            entry.ranges.extend(normalized);
        }
    }

    if pending.len() > MAX_EXPANDED_DOCUMENTS {
        return Err(invalid(
            "The selected folders expand to too many documents.",
        ));
    }

    let exclusions = exclusion_map(connection, &input.exclusions)?;
    let mut members = Vec::new();
    let mut expanded_unit_count = 0usize;
    let mut expanded_text_bytes = 0usize;

    for (document_id, mut selected) in pending {
        let shape = load_document_shape(connection, &document_id)?;
        if shape.unit_count <= 0 {
            dispositions.push(SourceDispositionDto {
                document_id: Some(document_id),
                collection_id: None,
                membership_id: None,
                reason: "empty_source".to_string(),
            });
            continue;
        }

        let mut ranges = if selected.whole {
            vec![SourceRangeInput {
                unit_kind: shape.unit_kind.clone(),
                start_unit: 1,
                end_unit: shape.unit_count,
            }]
        } else {
            validate_and_normalize_ranges(connection, &shape, std::mem::take(&mut selected.ranges))?
        };

        if let Some(excluded) = exclusions.get(&document_id) {
            if excluded.is_empty() {
                dispositions.push(SourceDispositionDto {
                    document_id: Some(document_id),
                    collection_id: None,
                    membership_id: None,
                    reason: "excluded".to_string(),
                });
                continue;
            }
            ranges = subtract_ranges(ranges, excluded);
        }

        if ranges.is_empty() {
            dispositions.push(SourceDispositionDto {
                document_id: Some(document_id),
                collection_id: None,
                membership_id: None,
                reason: "excluded".to_string(),
            });
            continue;
        }

        let (units, bytes) = measure_ranges(connection, &shape, &ranges)?;
        expanded_unit_count = expanded_unit_count
            .checked_add(units)
            .ok_or_else(|| invalid("The selected source window is too large."))?;
        expanded_text_bytes = expanded_text_bytes
            .checked_add(bytes)
            .ok_or_else(|| invalid("The selected source window is too large."))?;
        if expanded_unit_count > MAX_EXPANDED_UNITS || expanded_text_bytes > MAX_EXPANDED_TEXT_BYTES
        {
            return Err(invalid(
                "The selected folders exceed the bounded source window. Narrow the folders or reading ranges.",
            ));
        }

        selected.refs.sort_by(|left, right| {
            (&left.collection_id, &left.membership_id)
                .cmp(&(&right.collection_id, &right.membership_id))
        });
        selected.refs.dedup_by(|left, right| {
            left.collection_id == right.collection_id && left.membership_id == right.membership_id
        });

        members.push(SourceSetMemberDto {
            document_id: shape.id,
            source_sha256: shape.sha256,
            document_revision: shape.revision,
            source_format: shape.source_format,
            source_unit_kind: shape.unit_kind,
            source_unit_count: shape.unit_count,
            ranges: ranges
                .into_iter()
                .map(|range| ResolvedSourceRangeDto {
                    unit_kind: range.unit_kind,
                    start_unit: range.start_unit,
                    end_unit: range.end_unit,
                })
                .collect(),
            collection_references: selected.refs,
        });
    }

    dispositions.sort_by(|left, right| {
        (
            &left.document_id,
            &left.collection_id,
            &left.membership_id,
            &left.reason,
        )
            .cmp(&(
                &right.document_id,
                &right.collection_id,
                &right.membership_id,
                &right.reason,
            ))
    });
    let identity = serde_json::to_vec(&(
        organization_revision,
        &members,
        &dispositions,
        expanded_unit_count,
        expanded_text_bytes,
    ))
    .map_err(CommandError::serialization)?;

    let snapshot = SourceSetSnapshotDto {
        snapshot_id: hash_bytes(&identity),
        organization_revision,
        members,
        dispositions,
        expanded_unit_count: i64::try_from(expanded_unit_count)
            .map_err(|_| invalid("The selected source window is too large."))?,
        expanded_text_bytes: i64::try_from(expanded_text_bytes)
            .map_err(|_| invalid("The selected source window is too large."))?,
    };
    ensure_snapshot_current(connection, &snapshot)?;
    Ok(snapshot)
}

pub(crate) fn ensure_snapshot_current(
    connection: &Connection,
    snapshot: &SourceSetSnapshotDto,
) -> Result<(), CommandError> {
    if organization_revision(connection)? != snapshot.organization_revision {
        return Err(source_set_changed());
    }
    for member in &snapshot.members {
        let current = load_document_shape(connection, &member.document_id)?;
        if current.sha256 != member.source_sha256
            || current.revision != member.document_revision
            || current.unit_count != member.source_unit_count
            || current.unit_kind != member.source_unit_kind
        {
            return Err(source_set_changed());
        }
    }
    Ok(())
}

fn canonicalize_selection(input: &mut SourceSelectionInput) -> Result<(), CommandError> {
    if input.collection_ids.len() > MAX_SELECTION_COLLECTIONS
        || input.documents.len() > MAX_SELECTION_DOCUMENTS
        || input.exclusions.len() > MAX_SELECTION_DOCUMENTS
    {
        return Err(invalid("The source selection is too large."));
    }
    for id in &input.collection_ids {
        validate_entity_id(id, "document collection")?;
    }
    input.collection_ids.sort();
    input.collection_ids.dedup();
    for selected in input.documents.iter().chain(input.exclusions.iter()) {
        validate_entity_id(&selected.document_id, "document")?;
        if selected.ranges.len() > MAX_RANGES_PER_DOCUMENT {
            return Err(invalid("A document selection contains too many ranges."));
        }
    }
    input
        .documents
        .sort_by(|left, right| left.document_id.cmp(&right.document_id));
    input
        .exclusions
        .sort_by(|left, right| left.document_id.cmp(&right.document_id));
    if input.collection_ids.is_empty() && input.documents.is_empty() {
        return Err(invalid("Select at least one document or document folder."));
    }
    Ok(())
}

fn expand_collection_ids(
    connection: &Connection,
    roots: &[String],
) -> Result<BTreeSet<String>, CommandError> {
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut known = HashSet::new();
    let mut statement = connection
        .prepare("SELECT id,parent_id FROM document_collections")
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(CommandError::database)?;
    for row in rows {
        let (id, parent) = row.map_err(CommandError::database)?;
        known.insert(id.clone());
        if let Some(parent) = parent {
            children.entry(parent).or_default().push(id);
        }
    }

    let mut expanded = BTreeSet::new();
    let mut stack = roots.to_vec();
    while let Some(id) = stack.pop() {
        if !known.contains(&id) {
            return Err(collection_not_found());
        }
        if expanded.insert(id.clone()) {
            if expanded.len() > MAX_COLLECTIONS {
                return Err(invalid("The selected folder hierarchy is too large."));
            }
            if let Some(next) = children.get(&id) {
                stack.extend(next.iter().cloned());
            }
        }
    }
    Ok(expanded)
}

fn load_membership_sources(
    connection: &Connection,
    collection_ids: &BTreeSet<String>,
) -> Result<Vec<MembershipSource>, CommandError> {
    if collection_ids.is_empty() {
        return Ok(Vec::new());
    }
    let ids = serde_json::to_string(collection_ids).map_err(CommandError::serialization)?;
    let mut statement = connection
        .prepare(
            "SELECT m.collection_id,m.id,m.revision,m.document_id,
                    r.id,r.unit_kind,r.start_unit,r.end_unit,r.source_sha256,r.created_at
             FROM document_collection_memberships m
             LEFT JOIN document_collection_ranges r ON r.membership_id=m.id
             WHERE m.collection_id IN (SELECT value FROM json_each(?1))
             ORDER BY m.collection_id,m.sort_order,m.id,r.start_unit,r.end_unit,r.id",
        )
        .map_err(CommandError::database)?;
    let mut rows = statement.query([ids]).map_err(CommandError::database)?;
    let mut map: BTreeMap<String, MembershipSource> = BTreeMap::new();

    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let collection_id: String = row.get(0).map_err(CommandError::database)?;
        let membership_id: String = row.get(1).map_err(CommandError::database)?;
        let membership_revision: i64 = row.get(2).map_err(CommandError::database)?;
        let document_id: String = row.get(3).map_err(CommandError::database)?;
        let entry = map
            .entry(membership_id.clone())
            .or_insert_with(|| MembershipSource {
                collection_id,
                membership_id: membership_id.clone(),
                membership_revision,
                document_id,
                ranges: Vec::new(),
            });
        let range_id: Option<String> = row.get(4).map_err(CommandError::database)?;
        if let Some(range_id) = range_id {
            entry.ranges.push(DocumentCollectionRangeDto {
                id: range_id,
                membership_id: membership_id.clone(),
                unit_kind: row.get(5).map_err(CommandError::database)?,
                start_unit: row.get(6).map_err(CommandError::database)?,
                end_unit: row.get(7).map_err(CommandError::database)?,
                source_sha256: row.get(8).map_err(CommandError::database)?,
                created_at: row.get(9).map_err(CommandError::database)?,
            });
        }
    }

    if map.len() > MAX_MEMBERSHIPS {
        return Err(invalid(
            "The selected folders expand to too many memberships.",
        ));
    }
    Ok(map.into_values().collect())
}

fn exclusion_map(
    connection: &Connection,
    exclusions: &[SelectedDocumentInput],
) -> Result<BTreeMap<String, Vec<SourceRangeInput>>, CommandError> {
    let mut map: BTreeMap<String, Vec<SourceRangeInput>> = BTreeMap::new();
    for excluded in exclusions {
        let shape = load_document_shape(connection, &excluded.document_id)?;
        let ranges = validate_and_normalize_ranges(connection, &shape, excluded.ranges.clone())?;
        match map.entry(excluded.document_id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(ranges);
            }
            Entry::Occupied(mut entry) => {
                if entry.get().is_empty() {
                    continue;
                }
                if ranges.is_empty() {
                    entry.get_mut().clear();
                } else {
                    entry.get_mut().extend(ranges);
                }
            }
        }
    }
    for ranges in map.values_mut() {
        if !ranges.is_empty() {
            *ranges = normalize_ranges(std::mem::take(ranges))?;
        }
    }
    Ok(map)
}

fn subtract_ranges(
    included: Vec<SourceRangeInput>,
    excluded: &[SourceRangeInput],
) -> Vec<SourceRangeInput> {
    let mut output = Vec::new();
    for include in included {
        let unit_kind = include.unit_kind.clone();
        let mut pieces = vec![include];
        for exclude in excluded.iter().filter(|range| range.unit_kind == unit_kind) {
            let mut next = Vec::new();
            for piece in pieces {
                if exclude.end_unit < piece.start_unit || exclude.start_unit > piece.end_unit {
                    next.push(piece);
                    continue;
                }
                if exclude.start_unit > piece.start_unit {
                    next.push(SourceRangeInput {
                        unit_kind: piece.unit_kind.clone(),
                        start_unit: piece.start_unit,
                        end_unit: exclude.start_unit - 1,
                    });
                }
                if exclude.end_unit < piece.end_unit {
                    next.push(SourceRangeInput {
                        unit_kind: piece.unit_kind,
                        start_unit: exclude.end_unit + 1,
                        end_unit: piece.end_unit,
                    });
                }
            }
            pieces = next;
            if pieces.is_empty() {
                break;
            }
        }
        output.extend(pieces);
    }
    output
}

fn validate_and_normalize_ranges(
    connection: &Connection,
    shape: &DocumentShape,
    ranges: Vec<SourceRangeInput>,
) -> Result<Vec<SourceRangeInput>, CommandError> {
    if ranges.len() > MAX_RANGES_PER_DOCUMENT {
        return Err(invalid("A document selection contains too many ranges."));
    }
    let normalized = normalize_ranges(ranges)?;
    for range in &normalized {
        if range.unit_kind != shape.unit_kind
            || range.start_unit < 1
            || range.end_unit < range.start_unit
            || range.end_unit > shape.unit_count
        {
            return Err(invalid(
                "A reading range does not match the document's current source-unit precision.",
            ));
        }
        let present: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM document_source_units
                 WHERE document_id=?1 AND unit_kind=?2
                   AND unit_number BETWEEN ?3 AND ?4",
                params![shape.id, range.unit_kind, range.start_unit, range.end_unit],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if present != range.end_unit - range.start_unit + 1 {
            return Err(source_set_changed());
        }
    }
    Ok(normalized)
}

fn normalize_ranges(
    mut ranges: Vec<SourceRangeInput>,
) -> Result<Vec<SourceRangeInput>, CommandError> {
    for range in &ranges {
        if !valid_unit_kind(&range.unit_kind)
            || range.start_unit < 1
            || range.end_unit < range.start_unit
        {
            return Err(invalid("A reading range is invalid."));
        }
    }
    ranges.sort();
    let mut output: Vec<SourceRangeInput> = Vec::new();
    for range in ranges {
        if let Some(last) = output.last_mut() {
            if last.unit_kind == range.unit_kind
                && range.start_unit <= last.end_unit.saturating_add(1)
            {
                last.end_unit = last.end_unit.max(range.end_unit);
                continue;
            }
        }
        output.push(range);
    }
    Ok(output)
}

fn measure_ranges(
    connection: &Connection,
    shape: &DocumentShape,
    ranges: &[SourceRangeInput],
) -> Result<(usize, usize), CommandError> {
    let mut units = 0usize;
    let mut bytes = 0usize;
    for range in ranges {
        let (count, size): (i64, i64) = connection
            .query_row(
                "SELECT COUNT(*),COALESCE(SUM(length(CAST(plain_text AS BLOB))),0)
                 FROM document_source_units
                 WHERE document_id=?1 AND unit_kind=?2
                   AND unit_number BETWEEN ?3 AND ?4",
                params![shape.id, range.unit_kind, range.start_unit, range.end_unit],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(CommandError::database)?;
        if count != range.end_unit - range.start_unit + 1 {
            return Err(source_set_changed());
        }
        units = units
            .checked_add(usize::try_from(count).map_err(|_| source_set_changed())?)
            .ok_or_else(source_set_changed)?;
        bytes = bytes
            .checked_add(usize::try_from(size).map_err(|_| source_set_changed())?)
            .ok_or_else(source_set_changed)?;
    }
    Ok((units, bytes))
}

fn load_document_shape(
    connection: &Connection,
    document_id: &str,
) -> Result<DocumentShape, CommandError> {
    connection
        .query_row(
            "SELECT id,sha256,revision,source_format,source_unit_kind,source_unit_count
             FROM documents WHERE id=?1",
            [document_id],
            |row| {
                Ok(DocumentShape {
                    id: row.get(0)?,
                    sha256: row.get(1)?,
                    revision: row.get(2)?,
                    source_format: row.get(3)?,
                    unit_kind: row.get(4)?,
                    unit_count: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "document_not_found",
                "That document is no longer in the library.",
            )
        })
}

fn require_document(connection: &Connection, document_id: &str) -> Result<(), CommandError> {
    load_document_shape(connection, document_id).map(|_| ())
}

fn require_collection(connection: &Connection, collection_id: &str) -> Result<(), CommandError> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM document_collections WHERE id=?1
             )",
            [collection_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists {
        Ok(())
    } else {
        Err(collection_not_found())
    }
}

fn collection_parent_map(
    connection: &Connection,
) -> Result<BTreeMap<String, Option<String>>, CommandError> {
    let mut statement = connection
        .prepare("SELECT id,parent_id FROM document_collections ORDER BY id")
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(CommandError::database)?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

fn validate_hierarchy(parents: &BTreeMap<String, Option<String>>) -> Result<(), CommandError> {
    if parents.len() > MAX_COLLECTIONS {
        return Err(invalid(
            "The document library contains too many collections.",
        ));
    }
    for id in parents.keys() {
        let mut seen = HashSet::new();
        let mut current = Some(id.as_str());
        let mut depth = 0usize;
        while let Some(node) = current {
            if !seen.insert(node.to_string()) {
                return Err(invalid("Document folders cannot contain themselves."));
            }
            depth += 1;
            if depth > MAX_COLLECTION_DEPTH {
                return Err(invalid("Document folders are nested too deeply."));
            }
            current = parents.get(node).and_then(|value| value.as_deref());
            if current.is_some_and(|parent| !parents.contains_key(parent)) {
                return Err(collection_not_found());
            }
        }
    }
    Ok(())
}

fn next_collection_order(
    connection: &Connection,
    parent_id: Option<&str>,
) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(sort_order),0)+?1
             FROM document_collections WHERE parent_id IS ?2",
            params![ORDER_STEP, parent_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

fn next_membership_order(
    connection: &Connection,
    collection_id: &str,
) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(sort_order),0)+?1
             FROM document_collection_memberships WHERE collection_id=?2",
            params![ORDER_STEP, collection_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

fn organization_revision(connection: &Connection) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT revision FROM document_collection_organization
             WHERE singleton_id=1",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

fn assert_organization_revision(
    connection: &Connection,
    expected: i64,
) -> Result<(), CommandError> {
    if organization_revision(connection)? == expected {
        Ok(())
    } else {
        Err(organization_changed())
    }
}

fn bump_organization(tx: &Transaction<'_>, expected: i64, now: i64) -> Result<(), CommandError> {
    let changed = tx
        .execute(
            "UPDATE document_collection_organization
             SET revision=revision+1,updated_at=?1
             WHERE singleton_id=1 AND revision=?2",
            params![now, expected],
        )
        .map_err(CommandError::database)?;
    if changed == 1 {
        Ok(())
    } else {
        Err(organization_changed())
    }
}

fn request_hash<T: Serialize>(input: &T) -> Result<String, CommandError> {
    let bytes = serde_json::to_vec(input).map_err(CommandError::serialization)?;
    Ok(hash_bytes(&bytes))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn replay_receipt<T: DeserializeOwned>(
    connection: &Connection,
    operation_id: &str,
    request_hash: &str,
) -> Result<Option<T>, CommandError> {
    let stored: Option<(String, String)> = connection
        .query_row(
            "SELECT request_sha256,result_json
             FROM document_collection_operation_receipts
             WHERE operation_id=?1",
            [operation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((stored_hash, result)) = stored else {
        return Ok(None);
    };
    if stored_hash != request_hash {
        return Err(CommandError::new(
            "operation_id_conflict",
            "That collection operation identifier was already used for a different request.",
        ));
    }
    serde_json::from_str(&result)
        .map(Some)
        .map_err(CommandError::serialization)
}

fn store_receipt<T: Serialize>(
    tx: &Transaction<'_>,
    operation_id: &str,
    request_hash: &str,
    result: &T,
    now: i64,
) -> Result<(), CommandError> {
    let json = serde_json::to_string(result).map_err(CommandError::serialization)?;
    tx.execute(
        "INSERT INTO document_collection_operation_receipts(
             operation_id,request_sha256,result_json,created_at
         ) VALUES(?1,?2,?3,?4)",
        params![operation_id, request_hash, json, now],
    )
    .map_err(CommandError::database)?;
    Ok(())
}

fn validate_operation_id(value: &str) -> Result<(), CommandError> {
    validate_entity_id(value, "document collection operation")
}

fn validate_optional_id(value: Option<&str>, label: &str) -> Result<(), CommandError> {
    if let Some(value) = value {
        validate_entity_id(value, label)?;
    }
    Ok(())
}

fn validate_name(value: String) -> Result<String, CommandError> {
    let value = value.trim();
    let count = value.chars().count();
    if count == 0 || count > MAX_COLLECTION_NAME_CHARS {
        return Err(invalid(
            "Document folder names must contain between 1 and 120 characters.",
        ));
    }
    Ok(value.to_string())
}

fn validate_revision(value: i64, label: &str) -> Result<(), CommandError> {
    if value >= 1 {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_revision",
            format!("The {label} revision is invalid."),
        ))
    }
}

fn valid_unit_kind(value: &str) -> bool {
    matches!(
        value,
        "page" | "slide" | "chapter" | "section" | "segment" | "image" | "time_segment"
    )
}

fn invalid(message: &str) -> CommandError {
    CommandError::new("invalid_document_collection", message)
}

fn organization_changed() -> CommandError {
    CommandError::new(
        "document_collection_organization_changed",
        "Document folder organization changed. Reload it and try again.",
    )
}

fn collection_changed() -> CommandError {
    CommandError::new(
        "document_collection_changed",
        "This document folder changed. Reload it and try again.",
    )
}

fn collection_not_found() -> CommandError {
    CommandError::new(
        "document_collection_not_found",
        "That document folder no longer exists.",
    )
}

fn membership_changed() -> CommandError {
    CommandError::new(
        "document_collection_membership_changed",
        "This document folder membership changed. Reload it and try again.",
    )
}

fn membership_not_found() -> CommandError {
    CommandError::new(
        "document_collection_membership_not_found",
        "That document folder membership no longer exists.",
    )
}

fn source_set_changed() -> CommandError {
    CommandError::new(
        "source_set_changed",
        "The selected source material changed. Resolve the source selection again before continuing.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_domain::{SourceLocator, SourceUnitKind};

    fn id(byte: char) -> String {
        byte.to_string().repeat(32)
    }

    fn seed_document(
        connection: &Connection,
        document_id: &str,
        units: i64,
        sha: char,
    ) -> Result<(), CommandError> {
        let sha256 = sha.to_string().repeat(64);
        connection
            .execute(
                "INSERT INTO documents(
                     id,title,original_filename,mime_type,byte_size,sha256,
                     source_relative_path,lifecycle_state,created_at,updated_at,
                     source_format,source_unit_kind,source_unit_count,source_media_type
                 ) VALUES(
                     ?1,'Source','source.pdf','application/pdf',100,?2,
                     'documents/source.pdf','ready',1,1,'pdf','page',?3,'application/pdf'
                 )",
                params![document_id, sha256, units],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO document_source_files(
                     document_id,file_ordinal,original_filename,media_type,
                     relative_path,byte_size,sha256,created_at,updated_at
                 ) VALUES(?1,1,'source.pdf','application/pdf',?2,100,?3,1,1)",
                params![
                    document_id,
                    format!("documents/{document_id}/source.pdf"),
                    sha256
                ],
            )
            .map_err(CommandError::database)?;
        for unit in 1..=units {
            let locator =
                serde_json::to_string(&SourceLocator::new(SourceUnitKind::Page, vec![unit], None)?)
                    .map_err(CommandError::serialization)?;
            connection
                .execute(
                    "INSERT INTO document_source_units(
                         document_id,unit_number,unit_kind,label,title,plain_text,
                         text_sha256,source_file_ordinal,locator_json,created_at,updated_at
                     ) VALUES(?1,?2,'page',?3,NULL,?4,?5,1,?6,1,1)",
                    params![
                        document_id,
                        unit,
                        format!("Page {unit}"),
                        format!("page {unit}"),
                        sha256,
                        locator
                    ],
                )
                .map_err(CommandError::database)?;
        }
        Ok(())
    }

    fn create_root(
        connection: &mut Connection,
        operation: char,
        name: &str,
        revision: i64,
    ) -> Result<DocumentCollectionOrganizationDto, CommandError> {
        create_collection(
            connection,
            CreateDocumentCollectionInput {
                operation_id: id(operation),
                name: name.into(),
                parent_id: None,
                expected_organization_revision: revision,
            },
        )
    }

    #[test]
    fn nested_cycles_are_native_rules() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let first = create_root(connection, '1', "One", 1)?;
                let root = first.collections[0].clone();
                let second = create_collection(
                    connection,
                    CreateDocumentCollectionInput {
                        operation_id: id('2'),
                        name: "Two".into(),
                        parent_id: Some(root.id.clone()),
                        expected_organization_revision: first.revision,
                    },
                )?;
                let child = second
                    .collections
                    .iter()
                    .find(|collection| collection.name == "Two")
                    .unwrap()
                    .clone();
                let error = update_collection(
                    connection,
                    UpdateDocumentCollectionInput {
                        operation_id: id('3'),
                        collection_id: root.id,
                        name: root.name,
                        parent_id: Some(child.id),
                        archived: false,
                        revision: root.revision,
                        expected_organization_revision: second.revision,
                    },
                )
                .unwrap_err();
                assert_eq!(error.code, "invalid_document_collection");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn ranges_normalize_and_changed_source_is_never_guessed() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let document = id('d');
                seed_document(connection, &document, 6, 'a')?;
                let organization = create_root(connection, '1', "Week 1", 1)?;
                let folder = organization.collections[0].clone();
                let organization = add_membership(
                    connection,
                    AddDocumentCollectionMembershipInput {
                        operation_id: id('2'),
                        collection_id: folder.id.clone(),
                        document_id: document.clone(),
                        expected_organization_revision: organization.revision,
                    },
                )?;
                let membership = organization.memberships[0].clone();
                let organization = set_membership_ranges(
                    connection,
                    SetDocumentCollectionRangesInput {
                        operation_id: id('3'),
                        membership_id: membership.id,
                        ranges: vec![
                            SourceRangeInput {
                                unit_kind: "page".into(),
                                start_unit: 1,
                                end_unit: 2,
                            },
                            SourceRangeInput {
                                unit_kind: "page".into(),
                                start_unit: 2,
                                end_unit: 4,
                            },
                        ],
                        revision: membership.revision,
                        expected_organization_revision: organization.revision,
                    },
                )?;
                assert_eq!(organization.ranges.len(), 1);
                assert_eq!(
                    (
                        organization.ranges[0].start_unit,
                        organization.ranges[0].end_unit
                    ),
                    (1, 4)
                );
                let snapshot = resolve_selection(
                    connection,
                    SourceSelectionInput {
                        collection_ids: vec![folder.id.clone()],
                        documents: vec![],
                        exclusions: vec![],
                    },
                )?;
                assert_eq!(snapshot.members[0].ranges[0].end_unit, 4);
                connection
                    .execute(
                        "UPDATE documents
                         SET sha256=?1,revision=revision+1 WHERE id=?2",
                        params!["b".repeat(64), document],
                    )
                    .map_err(CommandError::database)?;
                let stale = resolve_selection(
                    connection,
                    SourceSelectionInput {
                        collection_ids: vec![folder.id],
                        documents: vec![],
                        exclusions: vec![],
                    },
                )?;
                assert!(stale.members.is_empty());
                assert_eq!(stale.dispositions[0].reason, "range_source_changed");
                assert!(ensure_snapshot_current(connection, &snapshot).is_err());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn parent_child_selection_deduplicates_and_excludes_ranges() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let document = id('d');
                seed_document(connection, &document, 5, 'a')?;
                let root_organization = create_root(connection, '1', "Course", 1)?;
                let root = root_organization.collections[0].clone();
                let child_organization = create_collection(
                    connection,
                    CreateDocumentCollectionInput {
                        operation_id: id('2'),
                        name: "Week 1".into(),
                        parent_id: Some(root.id.clone()),
                        expected_organization_revision: root_organization.revision,
                    },
                )?;
                let child = child_organization
                    .collections
                    .iter()
                    .find(|collection| collection.name == "Week 1")
                    .unwrap()
                    .clone();
                let organization = add_membership(
                    connection,
                    AddDocumentCollectionMembershipInput {
                        operation_id: id('3'),
                        collection_id: child.id.clone(),
                        document_id: document.clone(),
                        expected_organization_revision: child_organization.revision,
                    },
                )?;
                let snapshot = resolve_selection(
                    connection,
                    SourceSelectionInput {
                        collection_ids: vec![root.id, child.id],
                        documents: vec![],
                        exclusions: vec![SelectedDocumentInput {
                            document_id: document,
                            ranges: vec![SourceRangeInput {
                                unit_kind: "page".into(),
                                start_unit: 2,
                                end_unit: 4,
                            }],
                        }],
                    },
                )?;
                assert_eq!(snapshot.members.len(), 1);
                assert_eq!(snapshot.members[0].collection_references.len(), 1);
                assert_eq!(
                    snapshot.members[0].ranges,
                    vec![
                        ResolvedSourceRangeDto {
                            unit_kind: "page".into(),
                            start_unit: 1,
                            end_unit: 1,
                        },
                        ResolvedSourceRangeDto {
                            unit_kind: "page".into(),
                            start_unit: 5,
                            end_unit: 5,
                        },
                    ]
                );
                assert_eq!(snapshot.organization_revision, organization.revision);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn deletion_preserves_source_and_duplicate_retry_is_stable() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let document = id('d');
                seed_document(connection, &document, 1, 'a')?;
                let input = CreateDocumentCollectionInput {
                    operation_id: id('1'),
                    name: "Week 1".into(),
                    parent_id: None,
                    expected_organization_revision: 1,
                };
                let created = create_collection(connection, input.clone())?;
                let replay = create_collection(connection, input)?;
                assert_eq!(created, replay);
                let folder = created.collections[0].clone();
                let with_member = add_membership(
                    connection,
                    AddDocumentCollectionMembershipInput {
                        operation_id: id('2'),
                        collection_id: folder.id.clone(),
                        document_id: document.clone(),
                        expected_organization_revision: created.revision,
                    },
                )?;
                let deleted = delete_collection(
                    connection,
                    DeleteDocumentCollectionInput {
                        operation_id: id('3'),
                        collection_id: folder.id,
                        revision: folder.revision,
                        expected_organization_revision: with_member.revision,
                    },
                )?;
                assert!(deleted.collections.is_empty());
                assert!(deleted.memberships.is_empty());
                let source_exists: bool = connection
                    .query_row(
                        "SELECT EXISTS(
                             SELECT 1 FROM documents WHERE id=?1
                         )",
                        [document],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert!(source_exists);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn stale_revision_and_unsupported_precision_fail_usefully() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let document = id('d');
                seed_document(connection, &document, 2, 'a')?;
                let organization = create_root(connection, '1', "Week 1", 1)?;
                let folder = organization.collections[0].clone();
                let stale = add_membership(
                    connection,
                    AddDocumentCollectionMembershipInput {
                        operation_id: id('2'),
                        collection_id: folder.id.clone(),
                        document_id: document.clone(),
                        expected_organization_revision: 1,
                    },
                )
                .unwrap_err();
                assert_eq!(stale.code, "document_collection_organization_changed");
                let organization = add_membership(
                    connection,
                    AddDocumentCollectionMembershipInput {
                        operation_id: id('3'),
                        collection_id: folder.id,
                        document_id: document,
                        expected_organization_revision: organization.revision,
                    },
                )?;
                let membership = organization.memberships[0].clone();
                let error = set_membership_ranges(
                    connection,
                    SetDocumentCollectionRangesInput {
                        operation_id: id('4'),
                        membership_id: membership.id,
                        ranges: vec![SourceRangeInput {
                            unit_kind: "slide".into(),
                            start_unit: 1,
                            end_unit: 1,
                        }],
                        revision: membership.revision,
                        expected_organization_revision: organization.revision,
                    },
                )
                .unwrap_err();
                assert_eq!(error.code, "invalid_document_collection");
                Ok(())
            })
            .unwrap();
    }
}
