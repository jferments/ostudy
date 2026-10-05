//! Range-safe projection of document collections into the shared semantic scope.
//! A collection grants only retained document evidence whose canonical locator is
//! wholly inside the current SourceSetSnapshot. Generated derivatives and prior
//! conversation text are not implicitly authorized by folder membership.

use std::collections::{BTreeMap, HashSet};

use rusqlite::Connection;

use crate::{
    collections::{self, SourceSelectionInput, SourceSetSnapshotDto},
    document_domain::SourceLocator,
    error::CommandError,
};

use super::{
    adapters::SourceSeed,
    contracts::{invalid, ScopeKind, SourceKind, SourceRecord, StudyScope},
    index::IndexKey,
    repository,
};

const MAX_COLLECTION_INDEX_ENTRIES: usize = 250_000;

pub(crate) fn has_collection(scope: &StudyScope) -> bool {
    scope
        .material_scopes()
        .iter()
        .any(|member| member.kind == ScopeKind::Collection)
}

pub(crate) fn snapshot(
    connection: &Connection,
    scope: &StudyScope,
) -> Result<Option<SourceSetSnapshotDto>, CommandError> {
    let mut collection_ids = scope
        .material_scopes()
        .into_iter()
        .filter(|member| member.kind == ScopeKind::Collection)
        .filter_map(|member| member.id)
        .collect::<Vec<_>>();
    if collection_ids.is_empty() {
        return Ok(None);
    }
    collection_ids.sort();
    collection_ids.dedup();
    collections::resolve_selection(
        connection,
        SourceSelectionInput {
            collection_ids,
            documents: Vec::new(),
            exclusions: Vec::new(),
        },
    )
    .map(Some)
}

pub(crate) fn documents(
    connection: &Connection,
    scope: &StudyScope,
) -> Result<Vec<String>, CommandError> {
    Ok(snapshot(connection, scope)?
        .map(|snapshot| {
            snapshot
                .members
                .into_iter()
                .map(|member| member.document_id)
                .collect()
        })
        .unwrap_or_default())
}

pub(crate) fn document_seeds(
    connection: &Connection,
    scope: &StudyScope,
) -> Result<Vec<SourceSeed>, CommandError> {
    let Some(snapshot) = snapshot(connection, scope)? else {
        return Ok(Vec::new());
    };
    let document_ids = snapshot
        .members
        .iter()
        .map(|member| member.document_id.as_str())
        .collect::<Vec<_>>();
    let document_json =
        serde_json::to_string(&document_ids).map_err(CommandError::serialization)?;
    let mut statement = connection
        .prepare(
            "SELECT document_id,chunk_index,source_locator_json
             FROM document_semantic_chunk_sources
             WHERE document_id IN (SELECT value FROM json_each(?1))
             ORDER BY document_id,chunk_index LIMIT 100001",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([document_json], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if rows.len() > 100_000 {
        return Err(invalid(
            "The selected document folders contain too many semantic source windows.",
        ));
    }
    let members = snapshot
        .members
        .iter()
        .map(|member| (member.document_id.as_str(), member))
        .collect::<BTreeMap<_, _>>();
    let mut result = Vec::new();
    for (document_id, chunk_index, locator_json) in rows {
        let locator: SourceLocator =
            serde_json::from_str(&locator_json).map_err(CommandError::serialization)?;
        if members
            .get(document_id.as_str())
            .is_some_and(|member| locator_allowed(member, &locator))
        {
            result.push(SourceSeed {
                kind: SourceKind::Document,
                owner_id: document_id,
                parent_key: chunk_index.to_string(),
            });
        }
    }
    Ok(result)
}

pub(crate) fn eligible_entries(
    connection: &Connection,
    scope: &StudyScope,
) -> Result<Vec<IndexKey>, CommandError> {
    let Some(snapshot) = snapshot(connection, scope)? else {
        return Ok(Vec::new());
    };
    let ids = snapshot
        .members
        .iter()
        .map(|member| member.document_id.as_str())
        .collect::<Vec<_>>();
    let json = serde_json::to_string(&ids).map_err(CommandError::serialization)?;
    let mut statement = connection
        .prepare(
            "SELECT record_json FROM knowledge_sources
             WHERE active=1 AND kind='document'
               AND document_id IN (SELECT value FROM json_each(?1))
             ORDER BY document_id,id LIMIT 250001",
        )
        .map_err(CommandError::database)?;
    let records = statement
        .query_map([json], |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if records.len() > MAX_COLLECTION_INDEX_ENTRIES {
        return Err(invalid(
            "The selected document folders contain too much searchable material.",
        ));
    }

    let members = snapshot
        .members
        .iter()
        .map(|member| (member.document_id.as_str(), member))
        .collect::<BTreeMap<_, _>>();
    let mut sources = Vec::new();
    for serialized in records {
        let source: SourceRecord =
            serde_json::from_str(&serialized).map_err(CommandError::serialization)?;
        if !repository::source_is_current(connection, &source)? {
            continue;
        }
        if members
            .get(source.document_id.as_deref().unwrap_or_default())
            .is_some_and(|member| source_allowed(member, &source))
        {
            sources.push(source);
        }
    }

    let visible = sources
        .iter()
        .map(|source| (source.id.clone(), source.content_hash.clone()))
        .collect::<HashSet<_>>();
    let mut entries = Vec::new();
    for source in &sources {
        entries.push(IndexKey {
            id: format!("source:{}", source.id),
            source_id: source.id.clone(),
            content_hash: source.content_hash.clone(),
        });
        for unit in repository::load_units(connection, &source.id)? {
            if !visible.contains(&(unit.source_id.clone(), unit.source_hash.clone()))
                || unit
                    .content
                    .grounding
                    .as_ref()
                    .is_some_and(|grounding| !grounding.is_visible_in(&sources))
                || !unit_dependencies_visible(connection, &unit.id, &visible)?
            {
                continue;
            }
            entries.push(IndexKey {
                id: format!("unit:{}", unit.id),
                source_id: unit.source_id,
                content_hash: super::contracts::hash(
                    &serde_json::to_string(&unit.content).map_err(CommandError::serialization)?,
                ),
            });
            if entries.len() > MAX_COLLECTION_INDEX_ENTRIES {
                return Err(invalid(
                    "The selected document folders contain too much searchable material.",
                ));
            }
        }
    }
    Ok(entries)
}

fn unit_dependencies_visible(
    connection: &Connection,
    unit_id: &str,
    visible: &HashSet<(String, String)>,
) -> Result<bool, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT source_id,source_hash FROM knowledge_unit_evidence
             WHERE unit_id=?1 ORDER BY source_id",
        )
        .map_err(CommandError::database)?;
    let dependencies = statement
        .query_map([unit_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?;
    for dependency in dependencies {
        if !visible.contains(&dependency.map_err(CommandError::database)?) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn source_allowed(member: &collections::SourceSetMemberDto, source: &SourceRecord) -> bool {
    source.kind == SourceKind::Document
        && source.document_id.as_deref() == Some(member.document_id.as_str())
        && source
            .locator
            .as_ref()
            .is_some_and(|locator| locator_allowed(member, locator))
}

fn locator_allowed(member: &collections::SourceSetMemberDto, locator: &SourceLocator) -> bool {
    let kind = locator.unit_kind.as_str();
    !locator.unit_numbers.is_empty()
        && locator.unit_numbers.iter().all(|number| {
            member.ranges.iter().any(|range| {
                range.unit_kind == kind && *number >= range.start_unit && *number <= range.end_unit
            })
        })
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::{
        database::Database,
        document_domain::{SourceLocator, SourceUnitKind},
        knowledge::{
            contracts::{ContentOrigin, SourceRecord},
            learning_context::{self, ContextBudget, ContextPolicy, LearningContextRequest},
        },
    };

    fn id(value: char) -> String {
        value.to_string().repeat(32)
    }

    fn seed_fixture(
        connection: &Connection,
    ) -> Result<(StudyScope, Vec<SourceRecord>), CommandError> {
        let document = id('d');
        let collection = id('c');
        let sha = "a".repeat(64);
        connection
            .execute(
                "INSERT INTO documents(
                 id,title,original_filename,mime_type,byte_size,sha256,source_relative_path,
                 lifecycle_state,created_at,updated_at,source_format,source_unit_kind,
                 source_unit_count,source_media_type
             ) VALUES(?1,'Reading','reading.pdf','application/pdf',100,?2,?3,
                 'ready',1,1,'pdf','page',2,'application/pdf')",
                params![document, sha, format!("documents/{document}/source.pdf")],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO document_source_files(
                 document_id,file_ordinal,original_filename,media_type,relative_path,
                 byte_size,sha256,created_at,updated_at
             ) VALUES(?1,1,'reading.pdf','application/pdf',?2,100,?3,1,1)",
                params![document, format!("documents/{document}/source.pdf"), sha],
            )
            .map_err(CommandError::database)?;
        let mut sources = Vec::new();
        for unit in 1..=2_i64 {
            let locator = SourceLocator::new(SourceUnitKind::Page, vec![unit], None)?;
            let locator_json =
                serde_json::to_string(&locator).map_err(CommandError::serialization)?;
            let text = format!("page {unit}");
            let text_hash = super::super::contracts::hash(&text);
            connection
                .execute(
                    "INSERT INTO document_source_units(
                     document_id,unit_number,unit_kind,label,title,plain_text,text_sha256,
                     source_file_ordinal,locator_json,created_at,updated_at
                 ) VALUES(?1,?2,'page',?3,NULL,?4,?5,1,?6,1,1)",
                    params![
                        document,
                        unit,
                        format!("Page {unit}"),
                        text,
                        text_hash,
                        locator_json
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO document_semantic_chunk_sources(
                     document_id,chunk_index,text,full_text,source_locator_json,
                     visual_asset_ids_json,element_types_json,heading_context,token_estimate,
                     is_oversized,content_sha256,canonical_schema_version,created_at,updated_at
                 ) VALUES(?1,?2,?3,?3,?4,'[]','[\"paragraph\"]',NULL,2,0,?5,1,1,1)",
                    params![document, unit - 1, text, locator_json, text_hash],
                )
                .map_err(CommandError::database)?;
            let mut source = SourceRecord {
                identity_version: 2,
                id: String::new(),
                kind: SourceKind::Document,
                owner_id: document.clone(),
                segment_key: (unit - 1).to_string(),
                owner_revision: format!("page-{unit}"),
                content_hash: String::new(),
                title: format!("Reading page {unit}"),
                text,
                origin: ContentOrigin::SourceDocument,
                document_id: Some(document.clone()),
                note_id: None,
                card_id: None,
                guide_id: None,
                conversation_id: None,
                turn_id: None,
                locator: Some(locator),
                images: Vec::new(),
            };
            source.refresh_identity()?;
            repository::publish_source(connection, &source, &(unit - 1).to_string(), 1)?;
            sources.push(source);
        }
        connection
            .execute(
                "INSERT INTO document_collections(
                 id,name,sort_order,created_at,updated_at,revision
             ) VALUES(?1,'Week 1',1000,1,1,1)",
                [&collection],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "UPDATE documents SET subject_id=?1 WHERE id=?2",
                params![collection, document],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO document_study_ranges(
                 id,document_id,unit_kind,start_unit,end_unit,source_sha256,created_at
             ) VALUES(?1,?2,'page',1,1,?3,1)",
                params![id('r'), document, sha],
            )
            .map_err(CommandError::database)?;
        Ok((
            StudyScope::single(ScopeKind::Collection, Some(collection)),
            sources,
        ))
    }

    #[test]
    fn collection_scope_admits_only_sources_inside_saved_ranges() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let (scope, sources) = seed_fixture(connection)?;
                let entries = eligible_entries(connection, &scope)?;
                assert!(entries.iter().any(|entry| entry.source_id == sources[0].id));
                assert!(!entries.iter().any(|entry| entry.source_id == sources[1].id));
                let seeds = document_seeds(connection, &scope)?;
                assert_eq!(seeds.len(), 1);
                assert_eq!(seeds[0].parent_key, "0");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn learning_context_stamp_rejects_membership_changes_after_hydration() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let (scope, sources) = seed_fixture(connection)?;
                let context = learning_context::build(
                    connection,
                    LearningContextRequest {
                        scope: &scope,
                        include_conversations: false,
                        candidates: &sources,
                        policy: ContextPolicy::Retrieval,
                        target_units: None,
                        budget: ContextBudget {
                            sources: 12,
                            units: 768,
                            bytes: 1024 * 1024,
                        },
                    },
                )?;
                assert_eq!(context.sources.len(), 1);
                connection
                    .execute(
                        "UPDATE document_collection_organization SET revision=revision+1,updated_at=2
                 WHERE singleton_id=1",
                        [],
                    )
                    .map_err(CommandError::database)?;
                let error = context.stamp.ensure_current(connection).unwrap_err();
                assert_eq!(error.code, "source_set_changed");
                Ok(())
            })
            .unwrap();
    }
}
