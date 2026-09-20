//! Revision-checked user decisions. Committed decisions never depend on a later
//! refresh, OCR run, or optional provider request succeeding.

use std::collections::{BTreeMap, HashSet};

use rusqlite::{params, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{database::Database, error::CommandError, util::{unix_time_millis, validate_entity_id}};

use super::{filter::RegionOverride, operations::{self, EditResult}, receipt::invalid, repository, storage};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetImageFiltersInput {
    pub operation_id: String,
    pub document_id: String,
    pub expected_revision: i64,
    pub expected_filter_revision: i64,
    pub include_interface: bool,
    pub overrides: Vec<FilterChange>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilterChange {
    pub region_id: String,
    pub action: Option<RegionOverride>,
}

pub(crate) fn apply(database: &Database, input: &SetImageFiltersInput) -> Result<EditResult, CommandError> {
    validate_entity_id(&input.operation_id, "image filter operation")?;
    validate_entity_id(&input.document_id, "image document")?;
    if input.overrides.len() > super::receipt::MAX_REGIONS {
        return Err(invalid("Too many content-filter changes."));
    }
    let hash = operations::request_hash(input)?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(CommandError::database)?;
        if let Some(saved) = operations::load(&transaction, &input.operation_id, &input.document_id, &hash)? {
            return Ok(saved);
        }
        let manifest = repository::load(&transaction, &input.document_id)?;
        let settings = storage::settings(&transaction, &input.document_id)?;
        if manifest.revision != input.expected_revision || settings.revision != input.expected_filter_revision {
            return Err(CommandError::new("image_set_changed", "The image source or filters changed. Reload before applying these edits."));
        }
        let receipt = storage::load(&transaction, &input.document_id)?;
        let regions = receipt.as_ref().map(|receipt| receipt.members.iter().flat_map(|member| {
            member.regions.iter().map(move |region| (region.id.as_str(), member.member_id.as_str()))
        }).collect::<BTreeMap<_, _>>()).unwrap_or_default();
        let mut seen = HashSet::new();
        for change in &input.overrides {
            if !seen.insert(&change.region_id) || !regions.contains_key(change.region_id.as_str()) {
                return Err(invalid("A content-filter selection does not belong to this inspected source."));
            }
        }
        if settings.include_interface == input.include_interface && input.overrides.iter().all(|change| {
            settings.overrides.get(&change.region_id).copied() == change.action
        }) {
            let result = EditResult { document_id: input.document_id.clone(), revision: manifest.revision, filter_revision: settings.revision };
            operations::save(&transaction, &input.operation_id, &hash, &result, now)?;
            transaction.commit().map_err(CommandError::database)?;
            return Ok(result);
        }
        let revision = settings.revision.checked_add(1).ok_or_else(|| invalid("Filter revision limit reached."))?;
        transaction.execute(
            "INSERT INTO document_image_settings (document_id, revision, include_interface) VALUES (?1, ?2, ?3)
             ON CONFLICT(document_id) DO UPDATE SET revision = excluded.revision, include_interface = excluded.include_interface",
            params![input.document_id, revision, input.include_interface],
        ).map_err(CommandError::database)?;
        for change in &input.overrides {
            transaction.execute("DELETE FROM document_image_overrides WHERE document_id = ?1 AND region_id = ?2",
                params![input.document_id, change.region_id]).map_err(CommandError::database)?;
            if let Some(action) = change.action {
                let member = regions.get(change.region_id.as_str()).ok_or_else(|| invalid("Missing inspected image region."))?;
                let action = match action { RegionOverride::Include => "include", RegionOverride::Exclude => "exclude", RegionOverride::Metadata => "metadata" };
                transaction.execute(
                    "INSERT INTO document_image_overrides (document_id, member_id, region_id, action) VALUES (?1, ?2, ?3, ?4)",
                    params![input.document_id, member, change.region_id, action],
                ).map_err(CommandError::database)?;
            }
        }
        storage::invalidate(&transaction, &input.document_id, now)?;
        let result = EditResult { document_id: input.document_id.clone(), revision: manifest.revision, filter_revision: revision };
        operations::save(&transaction, &input.operation_id, &hash, &result, now)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(result)
    })
}
