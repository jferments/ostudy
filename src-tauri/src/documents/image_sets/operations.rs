//! Minimal idempotency receipts survive deletion without resurrecting sources.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{document_canonical::sha256_hex, error::CommandError, util::validate_entity_id};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditResult {
    pub document_id: String,
    pub revision: i64,
    pub filter_revision: i64,
}

pub(crate) fn request_hash(input: &impl Serialize) -> Result<String, CommandError> {
    Ok(sha256_hex(&serde_json::to_vec(input).map_err(CommandError::serialization)?))
}

pub(crate) fn load(connection: &Connection, operation: &str, document: &str, hash: &str) -> Result<Option<EditResult>, CommandError> {
    validate_entity_id(operation, "image edit operation")?;
    validate_entity_id(document, "image document")?;
    let saved: Option<(Option<String>, String, String)> = connection.query_row(
        "SELECT document_id, request_sha256, result_json FROM document_image_edit_receipts WHERE operation_id = ?1", [operation],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(CommandError::database)?;
    saved.map(|(owner, expected, json)| {
        if expected != hash { return Err(CommandError::new("image_set_operation_conflict", "This operation already has different image-edit inputs.")); }
        if owner.as_deref() != Some(document) { return Err(CommandError::new("image_set_edit_removed", "The edited source no longer exists. Retrying cannot recreate it.")); }
        let result = decode(&json)?;
        if result.document_id != document { return Err(invalid()); }
        Ok(result)
    }).transpose()
}

pub(crate) fn save(connection: &Connection, operation: &str, hash: &str, result: &EditResult, now: i64) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO document_image_edit_receipts (operation_id, document_id, request_sha256, result_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![operation, result.document_id, hash, serde_json::to_string(result).map_err(CommandError::serialization)?, now],
    ).map_err(CommandError::database)?;
    Ok(())
}


fn decode(json: &str) -> Result<EditResult, CommandError> {
    if json.len() > 1024 { return Err(invalid()); }
    let result: EditResult = serde_json::from_str(json).map_err(|_| invalid())?;
    validate_entity_id(&result.document_id, "image edit result")?;
    if result.revision < 1 || result.filter_revision < 1 { return Err(invalid()); }
    Ok(result)
}

pub(crate) fn validate(connection: &Connection) -> Result<(), CommandError> {
    let mut statement = connection.prepare(
        "SELECT e.operation_id, e.document_id, e.request_sha256, e.result_json, s.document_id
         FROM document_image_edit_receipts e LEFT JOIN document_image_sets s ON s.document_id = e.document_id",
    ).map_err(CommandError::database)?;
    let mut rows = statement.query([]).map_err(CommandError::database)?;
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let operation: String = row.get(0).map_err(CommandError::database)?;
        let owner: Option<String> = row.get(1).map_err(CommandError::database)?;
        let hash: String = row.get(2).map_err(CommandError::database)?;
        let json: String = row.get(3).map_err(CommandError::database)?;
        let current: Option<String> = row.get(4).map_err(CommandError::database)?;
        validate_entity_id(&operation, "image edit operation")?;
        super::manifest::validate_hash(&hash)?;
        let result = decode(&json)?;
        if owner != current || owner.as_ref().is_some_and(|id| id != &result.document_id) {
            return Err(invalid());
        }
    }
    Ok(())
}

fn invalid() -> CommandError {
    CommandError::new("invalid_image_edit_receipt", "A saved image-edit result is unsupported or belongs to another source.")
}
