//! Versioned public data repairs with explicit migration prerequisites.
//!
//! The pre-release repair implementations were folded into the public baseline.
//! Future repairs remain independently versioned, checksum-protected, and
//! transactional rather than being hidden inside schema SQL.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::{document_domain::SourceLocator, util::unix_time_millis};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepairPhase {
    AfterMigration(i64),
}

impl RepairPhase {
    fn is_satisfied_by(self, current: Self) -> bool {
        match (self, current) {
            (Self::AfterMigration(required), Self::AfterMigration(applied)) => required <= applied,
        }
    }
}

struct DataRepair {
    version: i64,
    name: &'static str,
    contract: &'static str,
    checksum: &'static str,
    phase: RepairPhase,
    apply: fn(&Transaction<'_>) -> Result<i64, String>,
}

const CANONICAL_SEMANTIC_HASH_REPAIR_CONTRACT: &str =
    "canonical_semantic_chunk_hashes_v1|sha256(locator={stable_identity}|types={element_types_csv}|heading={heading_or_empty}|text={full_text})";
const CANONICAL_BLOCK_HASH_REPAIR_CONTRACT: &str =
    "canonical_block_hashes_v1|sha256(unit={source_unit_number}|ordinal={block_ordinal}|role={role}|headings={heading_path_gt_join}|assets={asset_ids_csv}|text={text})";
const DOCUMENT_ARTIFACT_USER_EDIT_REPAIR_CONTRACT: &str =
    "document_artifact_user_edits_v1|restore pre-stage7 source_title when source_title is not a recognized filename-derived import title and stage7 ledger says generated document title was applied; clear applied ownership";

const REPAIRS: &[DataRepair] = &[
    DataRepair {
        version: 1,
        name: "canonical_semantic_chunk_hashes",
        contract: CANONICAL_SEMANTIC_HASH_REPAIR_CONTRACT,
        checksum: "779d93574a75f1e694fc4476bbf278ce7a540155cc4c35cf113f9ddb03d2a46f",
        phase: RepairPhase::AfterMigration(2),
        apply: repair_canonical_semantic_chunk_hashes,
    },
    DataRepair {
        version: 2,
        name: "canonical_block_hashes",
        contract: CANONICAL_BLOCK_HASH_REPAIR_CONTRACT,
        checksum: "02a07faa3635627246ef361160e94fe80a4634f7d36c97dbc3d6cb0819846085",
        phase: RepairPhase::AfterMigration(2),
        apply: repair_canonical_block_hashes,
    },
    DataRepair {
        version: 3,
        name: "document_artifact_user_edits",
        contract: DOCUMENT_ARTIFACT_USER_EDIT_REPAIR_CONTRACT,
        checksum: "14c6f90de653f8227f0d43a5ab51c8d9be5651a5cab589e4e1cefe29b90e6031",
        phase: RepairPhase::AfterMigration(7),
        apply: repair_document_artifact_user_edits,
    },
];

pub(crate) fn apply_required_repairs(
    connection: &mut Connection,
    phase: RepairPhase,
) -> Result<(), String> {
    ensure_ledger_exists(connection)?;
    for repair in REPAIRS
        .iter()
        .filter(|repair| repair.phase.is_satisfied_by(phase))
    {
        verify_repair_definition(repair)?;
        let existing = recorded_repair(connection, repair.version)?;
        if let Some((name, checksum)) = existing {
            if name != repair.name || checksum != repair.checksum {
                return Err(format!(
                    "Data repair {} does not match this application build. The database was not modified.",
                    repair.version
                ));
            }
            continue;
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start data repair {}: {error}", repair.version))?;
        let changed_rows = (repair.apply)(&transaction)?;
        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        transaction
            .execute(
                "INSERT INTO data_repairs (version, name, checksum, applied_at, changed_rows)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    repair.version,
                    repair.name,
                    repair.checksum,
                    applied_at,
                    changed_rows.max(0)
                ],
            )
            .map_err(|error| format!("Could not record data repair {}: {error}", repair.version))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit data repair {}: {error}", repair.version))?;
    }
    Ok(())
}

pub(crate) fn verify_repair_ledger(connection: &Connection) -> Result<(), String> {
    ensure_ledger_exists(connection)?;
    for repair in REPAIRS {
        verify_repair_definition(repair)?;
    }
    let mut statement = connection
        .prepare("SELECT version, name, checksum FROM data_repairs ORDER BY version")
        .map_err(|error| format!("Could not inspect the data repair ledger: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("Could not read the data repair ledger: {error}"))?;
    for row in rows {
        let (version, name, checksum) =
            row.map_err(|error| format!("Could not decode a data repair row: {error}"))?;
        let expected = REPAIRS.iter().find(|repair| repair.version == version);
        if expected.map(|repair| (repair.name, repair.checksum))
            != Some((name.as_str(), checksum.as_str()))
        {
            return Err(format!(
                "Data repair {version} is not recognized by this application build."
            ));
        }
    }
    Ok(())
}

fn verify_repair_definition(repair: &DataRepair) -> Result<(), String> {
    let actual = format!("{:x}", Sha256::digest(repair.contract.as_bytes()));
    if actual != repair.checksum {
        return Err(format!(
            "Data repair {} has a stale implementation-contract checksum in this application build.",
            repair.version
        ));
    }
    Ok(())
}

fn repair_canonical_semantic_chunk_hashes(transaction: &Transaction<'_>) -> Result<i64, String> {
    let rows = {
        let mut statement = transaction
            .prepare(
                "SELECT document_id, chunk_index, source_locator_json, full_text,
                        element_types_json, heading_context, content_sha256
                 FROM document_semantic_chunk_sources
                 ORDER BY document_id, chunk_index",
            )
            .map_err(|error| {
                format!("Could not prepare canonical semantic hash repair: {error}")
            })?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|error| format!("Could not read canonical semantic hash rows: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not decode canonical semantic hash rows: {error}"))?;
        rows
    };

    let mut changed_rows = 0_i64;
    for (
        document_id,
        chunk_index,
        locator_json,
        full_text,
        element_types_json,
        heading_context,
        stored_hash,
    ) in rows
    {
        let locator: SourceLocator = serde_json::from_str(&locator_json).map_err(|error| {
            format!(
                "Could not decode canonical source locator for document {document_id} chunk {chunk_index}: {error}"
            )
        })?;
        let element_types: Vec<String> = serde_json::from_str(&element_types_json).map_err(|error| {
            format!(
                "Could not decode canonical element types for document {document_id} chunk {chunk_index}: {error}"
            )
        })?;
        let expected_hash = canonical_semantic_chunk_hash(
            &locator,
            &full_text,
            &element_types,
            heading_context.as_deref(),
        );
        if stored_hash == expected_hash {
            continue;
        }
        let changed = transaction
            .execute(
                "UPDATE document_semantic_chunk_sources
                 SET content_sha256 = ?1
                 WHERE document_id = ?2 AND chunk_index = ?3 AND content_sha256 = ?4",
                params![expected_hash, document_id, chunk_index, stored_hash],
            )
            .map_err(|error| {
                format!(
                    "Could not repair canonical semantic hash for document {document_id} chunk {chunk_index}: {error}"
                )
            })?;
        if changed != 1 {
            return Err(format!(
                "Canonical semantic hash repair lost ownership of document {document_id} chunk {chunk_index}."
            ));
        }
        changed_rows = changed_rows.saturating_add(1);
    }
    Ok(changed_rows)
}

fn repair_canonical_block_hashes(transaction: &Transaction<'_>) -> Result<i64, String> {
    let rows = {
        let mut statement = transaction
            .prepare(
                "SELECT document_id, block_index, source_unit_number, block_ordinal,
                        role, text, heading_path_json, asset_ids_json, content_sha256
                 FROM document_source_blocks
                 ORDER BY document_id, block_index",
            )
            .map_err(|error| format!("Could not prepare canonical block hash repair: {error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .map_err(|error| format!("Could not read canonical block hash rows: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not decode canonical block hash rows: {error}"))?;
        rows
    };

    let mut changed_rows = 0_i64;
    for (
        document_id,
        block_index,
        source_unit_number,
        block_ordinal,
        role,
        text,
        heading_path_json,
        asset_ids_json,
        stored_hash,
    ) in rows
    {
        let heading_path: Vec<String> = serde_json::from_str(&heading_path_json).map_err(|error| {
            format!(
                "Could not decode canonical heading path for document {document_id} block {block_index}: {error}"
            )
        })?;
        let asset_ids: Vec<String> = serde_json::from_str(&asset_ids_json).map_err(|error| {
            format!(
                "Could not decode canonical asset identities for document {document_id} block {block_index}: {error}"
            )
        })?;
        let expected_hash = canonical_block_hash(
            source_unit_number,
            block_ordinal,
            &role,
            &text,
            &heading_path,
            &asset_ids,
        );
        if stored_hash == expected_hash {
            continue;
        }
        let changed = transaction
            .execute(
                "UPDATE document_source_blocks
                 SET content_sha256 = ?1
                 WHERE document_id = ?2 AND block_index = ?3 AND content_sha256 = ?4",
                params![expected_hash, document_id, block_index, stored_hash],
            )
            .map_err(|error| {
                format!(
                    "Could not repair canonical block hash for document {document_id} block {block_index}: {error}"
                )
            })?;
        if changed != 1 {
            return Err(format!(
                "Canonical block hash repair lost ownership of document {document_id} block {block_index}."
            ));
        }
        changed_rows = changed_rows.saturating_add(1);
    }
    Ok(changed_rows)
}

fn repair_document_artifact_user_edits(transaction: &Transaction<'_>) -> Result<i64, String> {
    let rows = {
        let mut statement = transaction
            .prepare(
                "SELECT metadata.document_id, metadata.document_title,
                        metadata.applied_document_title,
                        citation.source_title, citation.original_filename, citation.source_format,
                        document.title
                 FROM document_artifact_metadata AS metadata
                 JOIN source_citation_snapshots AS citation
                   ON citation.source_document_id = metadata.document_id
                 JOIN documents AS document
                   ON document.id = metadata.document_id
                 ORDER BY metadata.document_id",
            )
            .map_err(|error| {
                format!("Could not prepare document artifact user-edit repair: {error}")
            })?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|error| format!("Could not read document artifact metadata rows: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not decode document artifact metadata rows: {error}"))?
    };

    let now = unix_time_millis().map_err(|error| error.message)?;
    let mut changed_rows = 0_i64;
    for (
        document_id,
        generated_document_title,
        applied_document_title,
        source_title,
        original_filename,
        source_format,
        current_document_title,
    ) in rows
    {
        if is_filename_derived_import_title(&source_title, &original_filename, &source_format) {
            continue;
        }
        if source_title == generated_document_title
            || current_document_title != generated_document_title
            || applied_document_title.as_deref() != Some(generated_document_title.as_str())
        {
            continue;
        }

        let changed = transaction
            .execute(
                "UPDATE documents
                 SET title = ?1, updated_at = MAX(updated_at, ?2), revision = revision + 1
                 WHERE id = ?3 AND title = ?4",
                params![source_title, now, document_id, generated_document_title],
            )
            .map_err(|error| {
                format!("Could not restore user-edited document title {document_id}: {error}")
            })?;
        if changed != 1 {
            return Err(format!(
                "Document artifact user-edit repair lost ownership of document {document_id}."
            ));
        }
        transaction
            .execute(
                "UPDATE document_artifact_metadata
                 SET applied_document_title = NULL, updated_at = MAX(updated_at, ?1)
                 WHERE document_id = ?2
                   AND applied_document_title = ?3",
                params![now, document_id, generated_document_title],
            )
            .map_err(|error| {
                format!("Could not clear generated document-title ownership {document_id}: {error}")
            })?;
        changed_rows = changed_rows.saturating_add(1);
    }
    Ok(changed_rows)
}

fn is_filename_derived_import_title(title: &str, original_filename: &str, source_format: &str) -> bool {
    let title = title.trim();
    let filename = original_filename.trim();
    let extension = format!(".{source_format}");
    let stem = if filename
        .to_ascii_lowercase()
        .ends_with(&extension.to_ascii_lowercase())
    {
        &filename[..filename.len().saturating_sub(extension.len())]
    } else {
        filename
    };
    [
        filename.to_string(),
        stem.to_string(),
        stem.replace('_', " "),
        stem.replace('-', " "),
    ]
    .into_iter()
    .any(|candidate| title.eq_ignore_ascii_case(candidate.trim()))
}

fn canonical_semantic_chunk_hash(
    locator: &SourceLocator,
    full_text: &str,
    element_types: &[String],
    heading_context: Option<&str>,
) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            format!(
                "locator={}|types={}|heading={}|text={full_text}",
                locator.stable_identity(),
                element_types.join(","),
                heading_context.unwrap_or_default()
            )
            .as_bytes()
        )
    )
}

fn canonical_block_hash(
    source_unit_number: i64,
    block_ordinal: i64,
    role: &str,
    text: &str,
    heading_path: &[String],
    asset_ids: &[String],
) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            format!(
                "unit={source_unit_number}|ordinal={block_ordinal}|role={role}|headings={}|assets={}|text={text}",
                heading_path.join(" > "),
                asset_ids.join(",")
            )
            .as_bytes()
        )
    )
}

fn recorded_repair(
    connection: &Connection,
    version: i64,
) -> Result<Option<(String, String)>, String> {
    connection
        .query_row(
            "SELECT name, checksum FROM data_repairs WHERE version = ?1",
            [version],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("Could not inspect data repair {version}: {error}"))
}

fn ensure_ledger_exists(connection: &Connection) -> Result<(), String> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'data_repairs')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect the data repair ledger: {error}"))?;
    if exists == 0 {
        return Err(
            "The data repair ledger is missing from the public schema baseline.".to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_contract_checksums_match_declared_contracts() {
        for repair in REPAIRS {
            verify_repair_definition(repair).expect("repair checksum should match its contract");
        }
    }

    #[test]
    fn later_migration_phases_include_prerequisite_repairs() {
        assert!(RepairPhase::AfterMigration(2).is_satisfied_by(RepairPhase::AfterMigration(3)));
        assert!(!RepairPhase::AfterMigration(3).is_satisfied_by(RepairPhase::AfterMigration(2)));
    }

    #[test]
    fn document_artifact_user_edit_repair_restores_pre_stage7_titles() {
        let mut connection = Connection::open_in_memory().expect("repair fixture should open");
        connection
            .execute_batch(
                "CREATE TABLE documents (
                     id TEXT PRIMARY KEY,
                     title TEXT NOT NULL,
                     updated_at INTEGER NOT NULL,
                     revision INTEGER NOT NULL
                 );
                 CREATE TABLE source_citation_snapshots (
                     source_document_id TEXT PRIMARY KEY,
                     source_title TEXT NOT NULL,
                     original_filename TEXT NOT NULL,
                     source_format TEXT NOT NULL
                 );
                 CREATE TABLE document_artifact_metadata (
                     document_id TEXT PRIMARY KEY,
                     document_title TEXT NOT NULL,
                     applied_document_title TEXT,
                     updated_at INTEGER NOT NULL
                 );
                 INSERT INTO documents VALUES (
                     'custom-doc', 'Vasopressor pharmacology', 3, 2
                 );
                 INSERT INTO source_citation_snapshots VALUES (
                     'custom-doc', 'My vasopressor notes', 'vasopressors.pdf', 'pdf'
                 );
                 INSERT INTO document_artifact_metadata VALUES (
                     'custom-doc', 'Vasopressor pharmacology',
                     'Vasopressor pharmacology', 3
                 );
                 INSERT INTO documents VALUES (
                     'generated-doc', 'Distributive shock physiology', 3, 2
                 );
                 INSERT INTO source_citation_snapshots VALUES (
                     'generated-doc', 'shock_notes', 'shock_notes.pdf', 'pdf'
                 );
                 INSERT INTO document_artifact_metadata VALUES (
                     'generated-doc', 'Distributive shock physiology',
                     'Distributive shock physiology', 3
                 );",
            )
            .expect("repair fixture schema should load");

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("repair transaction should start");
        let changed = repair_document_artifact_user_edits(&transaction)
            .expect("user-edit repair should succeed");
        transaction.commit().expect("repair should commit");
        assert_eq!(changed, 1);

        let custom: (String, Option<String>) = connection
            .query_row(
                "SELECT document.title, metadata.applied_document_title
                 FROM documents AS document
                 JOIN document_artifact_metadata AS metadata
                   ON metadata.document_id = document.id
                 WHERE document.id = 'custom-doc'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("custom row should load");
        assert_eq!(custom.0, "My vasopressor notes");
        assert_eq!(custom.1, None);

        let generated: String = connection
            .query_row(
                "SELECT title FROM documents WHERE id = 'generated-doc'",
                [],
                |row| row.get(0),
            )
            .expect("generated row should load");
        assert_eq!(generated, "Distributive shock physiology");
    }

    #[test]
    fn semantic_hash_includes_locator_structure_heading_and_full_text() {
        let locator = SourceLocator::with_evidence(
            crate::document_domain::SourceUnitKind::Page,
            vec![1, 2],
            Some("Gas exchange".to_string()),
            vec!["unit-render:000002".to_string()],
            None,
        )
        .unwrap();
        let baseline = canonical_semantic_chunk_hash(
            &locator,
            "Diffusion depends on surface area.",
            &["paragraph".to_string()],
            Some("Gas exchange"),
        );
        assert_ne!(
            baseline,
            canonical_semantic_chunk_hash(
                &locator,
                "Diffusion depends on surface area and thickness.",
                &["paragraph".to_string()],
                Some("Gas exchange"),
            )
        );
        assert_ne!(
            baseline,
            canonical_semantic_chunk_hash(
                &locator,
                "Diffusion depends on surface area.",
                &["table".to_string()],
                Some("Gas exchange"),
            )
        );
    }

    #[test]
    fn block_hash_includes_source_role_structure_assets_and_text() {
        let baseline = canonical_block_hash(
            1,
            0,
            "table",
            "CO = HR × SV",
            &["Hemodynamics".to_string()],
            &["unit-render:000001".to_string()],
        );
        assert_ne!(
            baseline,
            canonical_block_hash(
                1,
                0,
                "paragraph",
                "CO = HR × SV",
                &["Hemodynamics".to_string()],
                &["unit-render:000001".to_string()],
            )
        );
        assert_ne!(
            baseline,
            canonical_block_hash(
                1,
                0,
                "table",
                "CO = HR × SV",
                &["Hemodynamics".to_string()],
                &[],
            )
        );
    }
}
