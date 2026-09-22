use rusqlite::{params, Connection};

use super::*;
use crate::{
    database::Database,
    document_domain::{SourceLocator, SourceUnitKind},
    error::CommandError,
};

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
                 ?3,'ready',1,1,'pdf','page',?4,'application/pdf'
             )",
            params![
                document_id,
                sha256,
                format!("documents/{document_id}/source.pdf"),
                units
            ],
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
            expected_organization_revision: revision,
        },
    )
}

fn seed_prepared_knowledge(
    connection: &Connection,
    document_id: &str,
) -> Result<(String, String), CommandError> {
    let source_id = id('e');
    let window_id = id('f');
    let source_hash = "b".repeat(64);
    let contract = "c".repeat(64);
    let record = serde_json::json!({
        "identityVersion": 2,
        "id": source_id,
        "kind": "document",
        "ownerId": document_id,
        "segmentKey": "0",
        "ownerRevision": "prepared-v1",
        "contentHash": source_hash,
        "title": "Prepared source",
        "text": "page 1",
        "origin": "source_document",
        "documentId": document_id,
        "noteId": null,
        "cardId": null,
        "guideId": null,
        "conversationId": null,
        "turnId": null,
        "locator": {
            "unitKind": "page",
            "unitNumbers": [1],
            "sectionLabel": null,
            "assetIds": [],
            "timeRangeMs": null
        },
        "images": []
    })
    .to_string();
    connection
        .execute(
            "INSERT INTO knowledge_sources(
                 id,kind,owner_id,segment_key,parent_key,owner_revision,content_hash,
                 record_json,active,document_id,extraction_contract,extracted_hash,
                 created_at,updated_at
             ) VALUES(?1,'document',?2,'0','0','prepared-v1',?3,?4,1,?2,?5,?3,1,1)",
            params![source_id, document_id, source_hash, record, contract],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO knowledge_analysis_windows(
                 id,document_id,contract,state,accepted_count,rejected_count,
                 report_json,created_at,updated_at
             ) VALUES(?1,?2,?3,'complete',1,0,'{\"accepted\":1}',1,1)",
            params![window_id, document_id, contract],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO knowledge_analysis_sources(window_id,source_id,source_hash)
             VALUES(?1,?2,?3)",
            params![window_id, source_id, source_hash],
        )
        .map_err(CommandError::database)?;
    Ok((source_id, window_id))
}

#[test]
fn one_level_collection_contract_rejects_nested_storage() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let organization = create_root(connection, '1', "Week 1", 1)?;
            let first = organization.collections[0].clone();
            let error = connection
                .execute(
                    "UPDATE document_collections SET parent_id=?1 WHERE id=?2",
                    params![first.id, first.id],
                )
                .unwrap_err();
            assert!(error
                .to_string()
                .contains("document_collections_parent_must_be_null"));
            Ok(())
        })
        .unwrap();
}

#[test]
fn renamed_collection_remains_explicitly_resolvable() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let document = id('d');
            seed_document(connection, &document, 2, 'a')?;
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
            let renamed = update_collection(
                connection,
                UpdateDocumentCollectionInput {
                    operation_id: id('3'),
                    collection_id: folder.id.clone(),
                    name: "Week 1 revised".to_string(),
                    revision: folder.revision,
                    expected_organization_revision: organization.revision,
                },
            )?;
            assert_eq!(
                renamed
                    .collections
                    .iter()
                    .find(|collection| collection.id == folder.id)
                    .unwrap()
                    .name,
                "Week 1 revised"
            );
            let snapshot = resolve_selection(
                connection,
                SourceSelectionInput {
                    collection_ids: vec![folder.id],
                    documents: vec![],
                    exclusions: vec![],
                },
            )?;
            assert_eq!(snapshot.members.len(), 1);
            assert_eq!(snapshot.members[0].document_id, document);
            Ok(())
        })
        .unwrap();
}

#[test]
fn membership_lifecycle_does_not_rewrite_prepared_knowledge() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let document = id('d');
            seed_document(connection, &document, 1, 'a')?;
            let (source_id, window_id) = seed_prepared_knowledge(connection, &document)?;
            let source_before: (String, String, String, String, i64) = connection
                .query_row(
                    "SELECT owner_revision,content_hash,extraction_contract,extracted_hash,active
                     FROM knowledge_sources WHERE id=?1",
                    [&source_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .map_err(CommandError::database)?;
            let window_before: (String, String, i64, i64, String) = connection
                .query_row(
                    "SELECT contract,state,accepted_count,rejected_count,report_json
                     FROM knowledge_analysis_windows WHERE id=?1",
                    [&window_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .map_err(CommandError::database)?;

            let first = create_root(connection, '1', "Week 1", 1)?;
            let first_folder = first.collections[0].clone();
            let second = create_root(connection, '2', "Week 2", first.revision)?;
            let second_folder = second
                .collections
                .iter()
                .find(|collection| collection.name == "Week 2")
                .unwrap()
                .clone();
            let added = add_membership(
                connection,
                AddDocumentCollectionMembershipInput {
                    operation_id: id('3'),
                    collection_id: first_folder.id,
                    document_id: document.clone(),
                    expected_organization_revision: second.revision,
                },
            )?;
            let membership = added.memberships[0].clone();
            let moved = move_membership(
                connection,
                MoveDocumentCollectionMembershipInput {
                    operation_id: id('4'),
                    membership_id: membership.id.clone(),
                    target_collection_id: second_folder.id,
                    revision: membership.revision,
                    expected_organization_revision: added.revision,
                },
            )?;
            let moved_membership = moved
                .memberships
                .iter()
                .find(|candidate| candidate.id == membership.id)
                .unwrap()
                .clone();
            let removed = remove_membership(
                connection,
                RemoveDocumentCollectionMembershipInput {
                    operation_id: id('5'),
                    membership_id: moved_membership.id,
                    revision: moved_membership.revision,
                    expected_organization_revision: moved.revision,
                },
            )?;
            assert!(removed.memberships.is_empty());
            assert_eq!(
                connection
                    .query_row(
                        "SELECT owner_revision,content_hash,extraction_contract,extracted_hash,active
                         FROM knowledge_sources WHERE id=?1",
                        [&source_id],
                        |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get(2)?,
                                row.get(3)?,
                                row.get(4)?,
                            ))
                        },
                    )
                    .map_err(CommandError::database)?,
                source_before
            );
            assert_eq!(
                connection
                    .query_row(
                        "SELECT contract,state,accepted_count,rejected_count,report_json
                         FROM knowledge_analysis_windows WHERE id=?1",
                        [&window_id],
                        |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get(2)?,
                                row.get(3)?,
                                row.get(4)?,
                            ))
                        },
                    )
                    .map_err(CommandError::database)?,
                window_before
            );
            let analysis_source_hash: String = connection
                .query_row(
                    "SELECT source_hash FROM knowledge_analysis_sources
                     WHERE window_id=?1 AND source_id=?2",
                    params![window_id, source_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(analysis_source_hash, "b".repeat(64));
            Ok(())
        })
        .unwrap();
}
