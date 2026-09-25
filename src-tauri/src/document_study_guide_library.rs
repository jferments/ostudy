//! Bounded metadata projection for the first-class Study Guides workspace.
//!
//! Document and collection guides share one search/sort/page contract. Full
//! guide bodies are hydrated only after a learner opens the appropriate reader.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{database::Database, error::CommandError};

const DEFAULT_PAGE_SIZE: i64 = 40;
const MAX_PAGE_SIZE: i64 = 100;
const MAX_OFFSET: i64 = 100_000;
const MAX_QUERY_CHARS: usize = 160;

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StudyGuideLibrarySort {
    #[default]
    Recent,
    Oldest,
    TitleAsc,
    TitleDesc,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ListStudyGuidesInput {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub sort: StudyGuideLibrarySort,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_page_size")]
    pub limit: i64,
}

impl Default for ListStudyGuidesInput {
    fn default() -> Self {
        Self {
            query: String::new(),
            sort: StudyGuideLibrarySort::Recent,
            offset: 0,
            limit: DEFAULT_PAGE_SIZE,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudyGuideLibraryItem {
    pub guide_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub title: String,
    pub original_filename: Option<String>,
    pub source_format: Option<String>,
    pub source_unit_kind: Option<String>,
    pub source_unit_count: Option<i64>,
    pub source_count: i64,
    pub guide_revision: i64,
    pub section_count: i64,
    pub note_count: i64,
    pub published_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudyGuideLibraryPage {
    pub items: Vec<StudyGuideLibraryItem>,
    pub total_count: i64,
    pub offset: i64,
    pub limit: i64,
}

#[tauri::command]
pub(crate) fn list_study_guide_page(
    database: State<'_, Database>,
    input: ListStudyGuidesInput,
) -> Result<StudyGuideLibraryPage, CommandError> {
    list_study_guides(database.inner(), input)
}

const GUIDE_ROWS_SQL: &str = r#"
WITH guide_rows AS (
    SELECT
        guide.id AS guide_id,
        'document' AS target_kind,
        document.id AS target_id,
        document.title AS title,
        document.original_filename AS original_filename,
        document.source_format AS source_format,
        document.source_unit_kind AS source_unit_kind,
        document.source_unit_count AS source_unit_count,
        1 AS source_count,
        guide.revision AS guide_revision,
        (
            SELECT COUNT(*)
            FROM document_study_guide_sections section
            WHERE section.guide_id = guide.id
        ) AS section_count,
        (
            SELECT COUNT(*)
            FROM document_study_guide_overlays overlay
            WHERE overlay.document_id = document.id
              AND overlay.state <> 'discarded'
              AND (overlay.guide_id IS NULL OR overlay.guide_id = guide.id)
        ) AS note_count,
        guide.published_at AS published_at
    FROM document_study_guides guide
    JOIN documents document ON document.id = guide.document_id
    WHERE guide.state = 'active'

    UNION ALL

    SELECT
        guide.id AS guide_id,
        'collection' AS target_kind,
        collection.id AS target_id,
        collection.name AS title,
        NULL AS original_filename,
        NULL AS source_format,
        NULL AS source_unit_kind,
        NULL AS source_unit_count,
        (
            SELECT COUNT(*)
            FROM document_collection_memberships membership
            WHERE membership.collection_id = collection.id
        ) AS source_count,
        guide.revision AS guide_revision,
        COALESCE(json_array_length(json_extract(guide.content_json, '$.sections')), 0)
            AS section_count,
        (
            SELECT COUNT(*)
            FROM study_target_guide_overlays overlay
            WHERE overlay.target_id = target.id
              AND overlay.guide_id = guide.id
              AND overlay.state <> 'discarded'
        ) AS note_count,
        guide.published_at AS published_at
    FROM study_target_guides guide
    JOIN study_targets target ON target.id = guide.target_id
    JOIN document_collections collection ON collection.id = target.collection_id
    WHERE guide.state = 'active'
      AND target.target_kind = 'collection'
)
"#;

pub(crate) fn list_study_guides(
    database: &Database,
    input: ListStudyGuidesInput,
) -> Result<StudyGuideLibraryPage, CommandError> {
    let query = input.query.trim();
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(CommandError::new(
            "study_guide_search_too_long",
            "Study-guide search text is too long.",
        ));
    }
    if input.offset < 0 || input.offset > MAX_OFFSET {
        return Err(CommandError::new(
            "invalid_study_guide_page",
            "The requested Study Guides page was invalid.",
        ));
    }
    if input.limit < 1 || input.limit > MAX_PAGE_SIZE {
        return Err(CommandError::new(
            "invalid_study_guide_page",
            "The requested Study Guides page size was invalid.",
        ));
    }

    let normalized_query = query.to_ascii_lowercase();
    let pattern = format!("%{}%", escape_like(&normalized_query));
    let filter = r#"
WHERE (
    ?1 = ''
    OR lower(title) LIKE ?2 ESCAPE '!'
    OR lower(COALESCE(original_filename, '')) LIKE ?2 ESCAPE '!'
    OR lower(COALESCE(source_format, '')) LIKE ?2 ESCAPE '!'
    OR (target_kind = 'collection' AND 'document collection' LIKE ?2 ESCAPE '!')
)
"#;
    let order = match input.sort {
        StudyGuideLibrarySort::Recent => {
            "published_at DESC, title COLLATE NOCASE, target_kind, target_id"
        }
        StudyGuideLibrarySort::Oldest => {
            "published_at ASC, title COLLATE NOCASE, target_kind, target_id"
        }
        StudyGuideLibrarySort::TitleAsc => {
            "title COLLATE NOCASE ASC, published_at DESC, target_kind, target_id"
        }
        StudyGuideLibrarySort::TitleDesc => {
            "title COLLATE NOCASE DESC, published_at DESC, target_kind, target_id"
        }
    };

    database.with_connection(|connection| {
        let count_sql = format!("{GUIDE_ROWS_SQL}\nSELECT COUNT(*) FROM guide_rows\n{filter}");
        let total_count = connection
            .query_row(&count_sql, params![normalized_query, pattern], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(CommandError::database)?;

        let sql = format!(
            "{GUIDE_ROWS_SQL}
SELECT guide_id,target_kind,target_id,title,original_filename,source_format,
       source_unit_kind,source_unit_count,source_count,guide_revision,
       section_count,note_count,published_at
FROM guide_rows
{filter}
ORDER BY {order}
LIMIT ?3 OFFSET ?4"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(
                params![normalized_query, pattern, input.limit, input.offset],
                |row| {
                    Ok(StudyGuideLibraryItem {
                        guide_id: row.get(0)?,
                        target_kind: row.get(1)?,
                        target_id: row.get(2)?,
                        title: row.get(3)?,
                        original_filename: row.get(4)?,
                        source_format: row.get(5)?,
                        source_unit_kind: row.get(6)?,
                        source_unit_count: row.get(7)?,
                        source_count: row.get(8)?,
                        guide_revision: row.get(9)?,
                        section_count: row.get(10)?,
                        note_count: row.get(11)?,
                        published_at: row.get(12)?,
                    })
                },
            )
            .map_err(CommandError::database)?;
        let items = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(StudyGuideLibraryPage {
            items,
            total_count,
            offset: input.offset,
            limit: input.limit,
        })
    })
}

fn default_page_size() -> i64 {
    DEFAULT_PAGE_SIZE
}

fn escape_like(value: &str) -> String {
    value
        .replace('!', "!!")
        .replace('%', "!%")
        .replace('_', "!_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn insert_guide(
        database: &Database,
        seed: char,
        title: &str,
        filename: &str,
        published_at: i64,
    ) {
        let document_id = seed.to_string().repeat(32);
        let plan_id = char::from_u32(seed as u32 + 1)
            .expect("next seed")
            .to_string()
            .repeat(32);
        let guide_id = char::from_u32(seed as u32 + 2)
            .expect("guide seed")
            .to_string()
            .repeat(32);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                             id, title, original_filename, mime_type, byte_size, sha256,
                             source_relative_path, lifecycle_state, source_format,
                             source_unit_kind, source_unit_count, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, 'application/pdf', 1, ?4,
                                   ?5, 'ready', 'pdf', 'page', 10, 1, 1)",
                        params![
                            document_id,
                            title,
                            filename,
                            seed.to_string().repeat(64),
                            format!("documents/{seed}/source.pdf"),
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_learning_plans (
                             id, document_id, revision, compiler_version,
                             outline_fingerprint, plan_fingerprint, summary_text,
                             learning_objectives_json, key_concepts_json, state,
                             created_at, updated_at
                         ) VALUES (?1, ?2, 1, 1, ?3, ?4, 'Summary', '[]', '[]',
                                   'active', 1, 1)",
                        params![
                            plan_id,
                            document_id,
                            seed.to_string().repeat(64),
                            char::from_u32(seed as u32 + 3)
                                .expect("fingerprint seed")
                                .to_string()
                                .repeat(64),
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_study_guides (
                             id, document_id, plan_id, revision, schema_version,
                             prompt_version, assembly_version, overview_json,
                             overview_text, state, created_at, updated_at, published_at
                         ) VALUES (?1, ?2, ?3, 1, 1, 1, 1, ?4, 'Overview',
                                   'active', 1, 1, ?5)",
                        params![
                            guide_id,
                            document_id,
                            plan_id,
                            r#"{"summary":"Overview","learningObjectives":[],"keyConcepts":[]}"#,
                            published_at,
                        ],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("guide fixture should insert");
    }

    #[test]
    fn list_is_bounded_searchable_and_sorted_without_hydrating_sections() {
        let database = Database::open_in_memory().expect("database should open");
        insert_guide(&database, '1', "Zebra physiology", "zebra.pdf", 20);
        insert_guide(&database, '5', "Cardiac output", "heart.pdf", 10);

        let page = list_study_guides(
            &database,
            ListStudyGuidesInput {
                query: "heart".to_string(),
                sort: StudyGuideLibrarySort::TitleAsc,
                offset: 0,
                limit: 40,
            },
        )
        .expect("page should load");
        assert_eq!(page.total_count, 1);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].title, "Cardiac output");
        assert_eq!(page.items[0].target_kind, "document");
        assert_eq!(page.items[0].section_count, 0);

        let sorted = list_study_guides(
            &database,
            ListStudyGuidesInput {
                query: String::new(),
                sort: StudyGuideLibrarySort::TitleAsc,
                offset: 0,
                limit: 1,
            },
        )
        .expect("sorted page should load");
        assert_eq!(sorted.total_count, 2);
        assert_eq!(sorted.items[0].title, "Cardiac output");
    }

    #[test]
    fn unified_library_includes_collection_guides_in_global_search_and_sort() {
        let database = Database::open_in_memory().expect("database should open");
        insert_guide(&database, '1', "Zebra physiology", "zebra.pdf", 20);
        database
            .with_connection(|connection| {
                let collection_id = "c".repeat(32);
                let target_id = "d".repeat(32);
                let guide_id = "e".repeat(32);
                connection
                    .execute(
                        "INSERT INTO document_collections(
                             id,parent_id,name,sort_order,created_at,updated_at,revision
                         ) VALUES(?1,NULL,'Week 1 physiology',1000,1,1,1)",
                        [&collection_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO study_targets(
                             id,target_kind,collection_id,created_at,updated_at
                         ) VALUES(?1,'collection',?2,1,1)",
                        params![target_id, collection_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO study_target_guides(
                             id,target_id,batch_id,revision,source_snapshot_id,
                             synthesis_analysis_hash,content_json,content_hash,state,
                             created_at,updated_at,published_at
                         ) VALUES(?1,?2,NULL,2,?3,NULL,?4,?5,'active',1,1,30)",
                        params![
                            guide_id,
                            target_id,
                            "f".repeat(64),
                            r#"{"schemaVersion":2,"collectionId":"cccccccccccccccccccccccccccccccc","title":"Week 1 physiology","overview":{"summary":"Overview","learningObjectives":[],"keyConcepts":[]},"sections":[{},{}],"coverageNotes":[]}"#,
                            "a".repeat(64),
                        ],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("collection guide fixture should insert");

        let page = list_study_guides(
            &database,
            ListStudyGuidesInput {
                query: "week 1".to_string(),
                sort: StudyGuideLibrarySort::Recent,
                offset: 0,
                limit: 40,
            },
        )
        .expect("unified page should load");
        assert_eq!(page.total_count, 1);
        assert_eq!(page.items[0].target_kind, "collection");
        assert_eq!(page.items[0].title, "Week 1 physiology");
        assert_eq!(page.items[0].section_count, 2);

        let all = list_study_guides(
            &database,
            ListStudyGuidesInput {
                query: String::new(),
                sort: StudyGuideLibrarySort::Recent,
                offset: 0,
                limit: 40,
            },
        )
        .expect("all guides should load");
        assert_eq!(all.total_count, 2);
        assert_eq!(all.items[0].target_kind, "collection");
        assert_eq!(all.items[1].target_kind, "document");

        let second_page = list_study_guides(
            &database,
            ListStudyGuidesInput {
                offset: 1,
                limit: 1,
                ..Default::default()
            },
        )
        .expect("global paging should load");
        assert_eq!(second_page.total_count, 2);
        assert_eq!(second_page.items.len(), 1);
        assert_eq!(second_page.items[0].guide_id, all.items[1].guide_id);
    }

    #[test]
    fn wildcard_characters_are_literal_search_text() {
        assert_eq!(escape_like(r"100%_ready!path"), r"100!%!_ready!!path");
    }

    #[test]
    fn page_and_query_limits_are_explicit() {
        let database = Database::open_in_memory().expect("database should open");
        let oversized = list_study_guides(
            &database,
            ListStudyGuidesInput {
                query: "x".repeat(MAX_QUERY_CHARS + 1),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(oversized.code, "study_guide_search_too_long");

        let invalid_page = list_study_guides(
            &database,
            ListStudyGuidesInput {
                limit: MAX_PAGE_SIZE + 1,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(invalid_page.code, "invalid_study_guide_page");
    }
}
