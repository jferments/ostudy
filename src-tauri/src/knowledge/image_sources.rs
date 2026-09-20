//! Image evidence identity and explicit location rebinding. Native pixel/text
//! proofs, not a model or UI claim, authorize reuse across source ordering edits.

use std::collections::{BTreeMap, HashSet};

use rusqlite::{params, Connection};

use crate::{documents::image_sets::storage, error::CommandError};

use super::{
    adapters::{self, SourceSeed},
    contracts::{SourceKind, SourceRecord},
    repository,
};

mod rebinding;

pub(crate) fn available(
    connection: &Connection,
    source: &SourceRecord,
) -> Result<bool, CommandError> {
    if source.kind != SourceKind::Document {
        return Ok(true);
    }
    // Runtime callers receive the latest database. Historical read-only entry
    // validation explicitly uses load_retained_source instead.
    storage::ready(connection, &source.owner_id)
}

/// Called only inside the guarded canonical image publication transaction.
/// Publish all new locations before retiring anything, so a moved source cannot
/// be deleted by another source's old numeric parent position.
pub(crate) fn reconcile(
    connection: &Connection,
    document_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    let mut statement = connection
        .prepare("SELECT record_json FROM knowledge_sources WHERE kind = 'document' AND document_id = ?1")
        .map_err(CommandError::database)?;
    let old = statement
        .query_map([document_id], |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let old = old
        .into_iter()
        .map(|text| {
            let source: SourceRecord =
                serde_json::from_str(&text).map_err(CommandError::serialization)?;
            source.validate()?;
            Ok((source.id.clone(), source))
        })
        .collect::<Result<BTreeMap<_, _>, CommandError>>()?;
    let mut indexes = connection
        .prepare("SELECT chunk_index FROM document_image_chunk_keys WHERE document_id = ?1 ORDER BY chunk_index")
        .map_err(CommandError::database)?;
    let indexes = indexes
        .query_map([document_id], |row| row.get::<_, i64>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut retained = Vec::new();
    let mut rebound = BTreeMap::new();
    for index in indexes {
        let parent_key = index.to_string();
        let seed = SourceSeed {
            kind: SourceKind::Document,
            owner_id: document_id.to_string(),
            parent_key: parent_key.clone(),
        };
        for mut source in adapters::read_seed(connection, &seed)? {
            source.refresh_identity()?;
            if let Some(previous) = old.get(&source.id) {
                if previous.identity_version == 3
                    && source.identity_version == 3
                    && previous.content_hash == source.content_hash
                    && previous.owner_revision == source.owner_revision
                    && previous.text == source.text
                    && previous.images == source.images
                {
                    rebound.insert(source.id.clone(), (previous.clone(), source.clone()));
                }
            }
            repository::publish_source(connection, &source, &parent_key, now)?;
            connection
                .execute(
                    "UPDATE knowledge_sources SET parent_key = ?1 WHERE id = ?2 AND parent_key <> ?1",
                    params![parent_key, source.id],
                )
                .map_err(CommandError::database)?;
            retained.push(source.id);
        }
    }
    connection
        .execute(
            "DELETE FROM knowledge_sources WHERE kind = 'document' AND document_id = ?1
         AND id NOT IN (SELECT value FROM json_each(?2))",
            params![
                document_id,
                serde_json::to_string(&retained).map_err(CommandError::serialization)?
            ],
        )
        .map_err(CommandError::database)?;
    rebinding::rebind_dependents(connection, &rebound)?;
    Ok(())
}

pub(crate) fn validate_current(
    connection: &Connection,
    document_id: &str,
) -> Result<(), CommandError> {
    if !storage::ready(connection, document_id)? {
        return Ok(());
    }
    let mut statement = connection.prepare(
        "SELECT parent_key, record_json FROM knowledge_sources WHERE kind = 'document' AND document_id = ?1 AND active = 1",
    ).map_err(CommandError::database)?;
    let rows = statement
        .query_map([document_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut checked = HashSet::new();
    for (parent_key, json) in rows {
        let stored: SourceRecord =
            serde_json::from_str(&json).map_err(CommandError::serialization)?;
        if stored.identity_version != 3 || !checked.insert(stored.id.clone()) {
            return Err(repository::stale_source());
        }
        let mut verified = stored.clone();
        verified.refresh_identity()?;
        if verified.id != stored.id || verified.content_hash != stored.content_hash {
            return Err(repository::stale_source());
        }
        let seed = SourceSeed {
            kind: SourceKind::Document,
            owner_id: document_id.to_string(),
            parent_key,
        };
        let mut found = false;
        for mut current in adapters::read_seed(connection, &seed)? {
            current.refresh_identity()?;
            if current.id == stored.id
                && current.content_hash == stored.content_hash
                && current.owner_revision == stored.owner_revision
                && current.locator == stored.locator
            {
                found = true;
            }
        }
        if !found {
            return Err(repository::stale_source());
        }
    }
    Ok(())
}
