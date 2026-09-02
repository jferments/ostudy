//! Bounded projections for the first-class Study Guides workspace.
//!
//! The list command returns only guide/document metadata and aggregate counts.
//! Full guide bodies are hydrated only after a learner opens one guide.

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
pub(crate) struct ListDocumentStudyGuidesInput {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub sort: StudyGuideLibrarySort,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_page_size")]
    pub limit: i64,
}

impl Default for ListDocumentStudyGuidesInput {
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
    pub document_id: String,
    pub document_title: String,
    pub original_filename: String,
    pub source_format: String,
    pub source_unit_kind: String,
    pub source_unit_count: i64,
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
pub(crate) fn list_document_study_guide_page(
    database: State<'_, Database>,
    input: ListDocumentStudyGuidesInput,
) -> Result<StudyGuideLibraryPage, CommandError> {
    list_study_guides(database.inner(), input)
}

pub(crate) fn list_study_guides(
    database: &Database,
    input: ListDocumentStudyGuidesInput,
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
    let order = match input.sort {
        StudyGuideLibrarySort::Recent => {
            "guide.published_at DESC, document.title COLLATE NOCASE, document.id"
        }
        StudyGuideLibrarySort::Oldest => {
            "guide.published_at ASC, document.title COLLATE NOCASE, document.id"
        }
        StudyGuideLibrarySort::TitleAsc => {
            "document.title COLLATE NOCASE ASC, guide.published_at DESC, document.id"
        }
        StudyGuideLibrarySort::TitleDesc => {
            "document.title COLLATE NOCASE DESC, guide.published_at DESC, document.id"
        }
    };

    database.with_connection(|connection| {
        let total_count = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM document_study_guides guide
                 JOIN documents document ON document.id = guide.document_id
                 WHERE guide.state = 'active'
                   AND (
                       ?1 = ''
                       OR lower(document.title) LIKE ?2 ESCAPE '!'
                       OR lower(document.original_filename) LIKE ?2 ESCAPE '!'
                       OR lower(document.source_format) LIKE ?2 ESCAPE '!'
                   )",
                params![normalized_query, pattern],
                |row| row.get::<_, i64>(0),
            )
            .map_err(CommandError::database)?;

        let sql = format!(
            "SELECT guide.id, document.id, document.title, document.original_filename,
                    document.source_format, document.source_unit_kind,
                    document.source_unit_count, guide.revision,
                    (
                        SELECT COUNT(*)
                        FROM document_study_guide_sections section
                        WHERE section.guide_id = guide.id
                    ),
                    (
                        SELECT COUNT(*)
                        FROM document_study_guide_overlays overlay
                        WHERE overlay.document_id = document.id
                          AND overlay.state <> 'discarded'
                          AND (overlay.guide_id IS NULL OR overlay.guide_id = guide.id)
                    ),
                    guide.published_at
             FROM document_study_guides guide
             JOIN documents document ON document.id = guide.document_id
             WHERE guide.state = 'active'
               AND (
                   ?1 = ''
                   OR lower(document.title) LIKE ?2 ESCAPE '!'
                   OR lower(document.original_filename) LIKE ?2 ESCAPE '!'
                   OR lower(document.source_format) LIKE ?2 ESCAPE '!'
               )
             ORDER BY {order}
             LIMIT ?3 OFFSET ?4"
        );
        let mut statement = connection
            .prepare(&sql)
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map(
                params![normalized_query, pattern, input.limit, input.offset],
                |row| {
                    Ok(StudyGuideLibraryItem {
                        guide_id: row.get(0)?,
                        document_id: row.get(1)?,
                        document_title: row.get(2)?,
                        original_filename: row.get(3)?,
                        source_format: row.get(4)?,
                        source_unit_kind: row.get(5)?,
                        source_unit_count: row.get(6)?,
                        guide_revision: row.get(7)?,
                        section_count: row.get(8)?,
                        note_count: row.get(9)?,
                        published_at: row.get(10)?,
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
            ListDocumentStudyGuidesInput {
                query: "heart".to_string(),
                sort: StudyGuideLibrarySort::TitleAsc,
                offset: 0,
                limit: 40,
            },
        )
        .expect("page should load");
        assert_eq!(page.total_count, 1);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].document_title, "Cardiac output");
        assert_eq!(page.items[0].section_count, 0);

        let sorted = list_study_guides(
            &database,
            ListDocumentStudyGuidesInput {
                query: String::new(),
                sort: StudyGuideLibrarySort::TitleAsc,
                offset: 0,
                limit: 1,
            },
        )
        .expect("sorted page should load");
        assert_eq!(sorted.total_count, 2);
        assert_eq!(sorted.items[0].document_title, "Cardiac output");
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
            ListDocumentStudyGuidesInput {
                query: "x".repeat(MAX_QUERY_CHARS + 1),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(oversized.code, "study_guide_search_too_long");

        let invalid_page = list_study_guides(
            &database,
            ListDocumentStudyGuidesInput {
                limit: MAX_PAGE_SIZE + 1,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert_eq!(invalid_page.code, "invalid_study_guide_page");
    }
}
