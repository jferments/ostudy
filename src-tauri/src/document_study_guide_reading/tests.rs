use super::*;

const DOCUMENT: &str = "11111111111111111111111111111111";
const PLAN: &str = "22222222222222222222222222222222";
const FIRST_GUIDE: &str = "33333333333333333333333333333333";
const SECOND_GUIDE: &str = "44444444444444444444444444444444";
const TOPICS: [&str; 3] = [
    "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1",
    "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2",
    "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3",
];

/// A library with one document whose active guide has the three topics, each
/// section's text hashed with `seed`.
fn library() -> Connection {
    let connection =
        crate::database::subjects_fixture::release_library(crate::database::CURRENT_SCHEMA_VERSION);
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .unwrap();
    crate::collections::test_support::seed_document(&connection, DOCUMENT, 3, 'a').unwrap();
    connection
        .execute(
            "INSERT INTO document_learning_plans(
                 id, document_id, revision, compiler_version, outline_fingerprint,
                 plan_fingerprint, summary_text, learning_objectives_json,
                 key_concepts_json, state, created_at, updated_at
             ) VALUES (?1, ?2, 1, 1, ?3, ?4, 'Summary', '[]', '[]', 'active', 1, 1)",
            params![PLAN, DOCUMENT, "5".repeat(64), "6".repeat(64)],
        )
        .unwrap();
    for (index, topic) in TOPICS.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO document_learning_topics(id, document_id, anchor_hash, created_at)
                 VALUES (?1, ?2, ?3, 1)",
                params![topic, DOCUMENT, index.to_string().repeat(64)],
            )
            .unwrap();
    }
    publish_guide(&connection, FIRST_GUIDE, 1, &['a', 'b', 'c']);
    connection
}

/// Publishes a guide revision whose sections' hashes are the given seeds, and
/// supersedes the guide before it.
fn publish_guide(connection: &Connection, guide: &str, revision: i64, hashes: &[char]) {
    connection
        .execute(
            "UPDATE document_study_guides SET state = 'superseded' WHERE document_id = ?1",
            [DOCUMENT],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO document_study_guides(
                 id, document_id, plan_id, revision, schema_version, prompt_version,
                 assembly_version, overview_json, overview_text, state, created_at,
                 updated_at, published_at
             ) VALUES (?1, ?2, ?3, ?4, 1, 1, 1, '{}', 'Overview', 'active', 1, 1, 1)",
            params![guide, DOCUMENT, PLAN, revision],
        )
        .unwrap();
    for (index, (topic, hash)) in TOPICS.iter().zip(hashes).enumerate() {
        connection
            .execute(
                "INSERT INTO document_study_guide_sections(
                     guide_id, topic_id, sort_index, content_json, content_text,
                     content_hash, source_locator_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, '{}', 'Text', ?4, '{}', 1, 1)",
                params![guide, topic, index as i64, hash.to_string().repeat(64)],
            )
            .unwrap();
    }
}

fn open(
    connection: &mut Connection,
    guide: &str,
    topic: &str,
    now: i64,
) -> DocumentStudyGuideReadingDto {
    open_section(
        connection,
        OpenDocumentStudyGuideSectionInput {
            document_id: DOCUMENT.to_string(),
            guide_id: guide.to_string(),
            topic_id: topic.to_string(),
        },
        now,
    )
    .unwrap()
}

fn mark(
    connection: &mut Connection,
    guide: &str,
    topic: &str,
    read: bool,
    now: i64,
) -> Result<DocumentStudyGuideReadingDto, CommandError> {
    set_section_read(
        connection,
        SetDocumentStudyGuideSectionReadInput {
            document_id: DOCUMENT.to_string(),
            guide_id: guide.to_string(),
            topic_id: topic.to_string(),
            read,
        },
        now,
    )
}

fn read_at(connection: &Connection, topic: &str) -> Option<i64> {
    connection
        .query_row(
            "SELECT read_at FROM document_study_guide_section_reads WHERE topic_id = ?1",
            [topic],
            |row| row.get(0),
        )
        .optional()
        .unwrap()
}

#[test]
fn a_new_guide_has_nothing_read_and_no_place_to_resume() {
    let connection = library();
    let state = reading(&connection, DOCUMENT, FIRST_GUIDE).unwrap();
    assert_eq!(state.section_count, 3);
    assert!(state.read_topic_ids.is_empty());
    assert_eq!((state.last_topic_id, state.last_opened_at), (None, None));
}

#[test]
fn opening_a_section_is_where_the_guide_reopens() {
    let mut connection = library();
    open(&mut connection, FIRST_GUIDE, TOPICS[0], 10);
    let state = open(&mut connection, FIRST_GUIDE, TOPICS[1], 20);
    assert_eq!(state.last_topic_id.as_deref(), Some(TOPICS[1]));
    assert_eq!(state.last_opened_at, Some(20));
    assert!(state.read_topic_ids.is_empty());
}

#[test]
fn marking_read_is_repeatable_and_keeps_the_first_read_time() {
    let mut connection = library();
    let state = mark(&mut connection, FIRST_GUIDE, TOPICS[2], true, 10).unwrap();
    assert_eq!(state.read_topic_ids, vec![TOPICS[2].to_string()]);
    let changes = connection.total_changes();
    let again = mark(&mut connection, FIRST_GUIDE, TOPICS[2], true, 30).unwrap();
    assert_eq!(again, state);
    assert_eq!(connection.total_changes(), changes);
    assert_eq!(read_at(&connection, TOPICS[2]), Some(10));

    mark(&mut connection, FIRST_GUIDE, TOPICS[0], true, 40).unwrap();
    let state = reading(&connection, DOCUMENT, FIRST_GUIDE).unwrap();
    // Guide order, not the order they were marked in.
    assert_eq!(
        state.read_topic_ids,
        vec![TOPICS[0].to_string(), TOPICS[2].to_string()]
    );

    let state = mark(&mut connection, FIRST_GUIDE, TOPICS[2], false, 50).unwrap();
    assert_eq!(state.read_topic_ids, vec![TOPICS[0].to_string()]);
    let state = mark(&mut connection, FIRST_GUIDE, TOPICS[2], false, 60).unwrap();
    assert_eq!(state.read_topic_ids, vec![TOPICS[0].to_string()]);
    // Marking is not opening: the place to resume is unchanged.
    assert_eq!(state.last_topic_id, None);
}

#[test]
fn a_remade_guide_keeps_marks_only_on_unchanged_sections() {
    let mut connection = library();
    open(&mut connection, FIRST_GUIDE, TOPICS[1], 10);
    for topic in TOPICS {
        mark(&mut connection, FIRST_GUIDE, topic, true, 20).unwrap();
    }
    // The second section is rewritten; the other two keep their text.
    publish_guide(&connection, SECOND_GUIDE, 2, &['a', 'x', 'c']);

    let state = reading(&connection, DOCUMENT, SECOND_GUIDE).unwrap();
    assert_eq!(
        state.read_topic_ids,
        vec![TOPICS[0].to_string(), TOPICS[2].to_string()]
    );
    assert_eq!(state.last_topic_id.as_deref(), Some(TOPICS[1]));

    // Reading the rewritten text marks it again, with a new time.
    mark(&mut connection, SECOND_GUIDE, TOPICS[1], true, 40).unwrap();
    assert_eq!(read_at(&connection, TOPICS[1]), Some(40));
}

#[test]
fn a_reader_open_on_a_replaced_guide_cannot_save() {
    let mut connection = library();
    publish_guide(&connection, SECOND_GUIDE, 2, &['a', 'b', 'c']);
    let changes = connection.total_changes();
    let error = mark(&mut connection, FIRST_GUIDE, TOPICS[0], true, 10).unwrap_err();
    assert_eq!(error.code, "study_guide_changed");
    let error = open_section(
        &mut connection,
        OpenDocumentStudyGuideSectionInput {
            document_id: DOCUMENT.to_string(),
            guide_id: FIRST_GUIDE.to_string(),
            topic_id: TOPICS[0].to_string(),
        },
        10,
    )
    .unwrap_err();
    assert_eq!(error.code, "study_guide_changed");
    assert_eq!(connection.total_changes(), changes);

    let stranger = "d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4d4";
    let error = mark(&mut connection, SECOND_GUIDE, stranger, true, 10).unwrap_err();
    assert_eq!(error.code, "study_guide_section_missing");
    let error = mark(&mut connection, SECOND_GUIDE, "not-an-id", true, 10).unwrap_err();
    assert_eq!(error.code, "invalid_identifier");
}

#[test]
fn deleting_the_document_or_its_guide_clears_its_reading() {
    let mut connection = library();
    open(&mut connection, FIRST_GUIDE, TOPICS[0], 10);
    mark(&mut connection, FIRST_GUIDE, TOPICS[0], true, 10).unwrap();
    delete_document_reading(&connection, DOCUMENT).unwrap();
    let count = |connection: &Connection, table: &str| -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    };
    assert_eq!(count(&connection, "document_study_guide_section_reads"), 0);
    assert_eq!(count(&connection, "document_study_guide_positions"), 0);

    open(&mut connection, FIRST_GUIDE, TOPICS[0], 20);
    mark(&mut connection, FIRST_GUIDE, TOPICS[0], true, 20).unwrap();
    connection
        .execute("DELETE FROM documents WHERE id = ?1", [DOCUMENT])
        .unwrap();
    assert_eq!(count(&connection, "document_study_guide_section_reads"), 0);
    assert_eq!(count(&connection, "document_study_guide_positions"), 0);
}
