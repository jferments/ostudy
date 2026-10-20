//! End-of-chapter material (key terms, chapter reviews, review questions,
//! references) restates or tests what the chapter already taught. Extraction
//! records it as having nothing to learn instead of sending it to the model,
//! so guides do not gain a glossary section or a summary that repeats the
//! chapter.

use std::collections::HashSet;

use rusqlite::Connection;

use crate::error::CommandError;

use super::contracts::{SourceKind, SourceRecord};

const BACK_MATTER_HEADINGS: &[&str] = &[
    "key terms",
    "key words",
    "glossary",
    "chapter review",
    "chapter summary",
    "review questions",
    "critical thinking questions",
    "interactive link questions",
    "references",
    "bibliography",
    "works cited",
    "further reading",
    "suggested reading",
    "answer key",
    "index",
];

pub(crate) struct ChunkHeading {
    pub index: i64,
    pub heading: Option<String>,
    pub chars: usize,
}

fn normalized(heading: &str) -> String {
    heading
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c.is_whitespace())
        .trim_end_matches(|c: char| c == ':' || c == '.' || c.is_whitespace())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// A run starts at a back-matter heading and continues through chunks with no
/// heading and through headings that repeat an earlier section title (a
/// chapter review is organised by the chapter's own sections). Any new
/// heading ends it, so the next chapter of a longer document is kept. A
/// document that would lose half its text or more keeps everything: it is a
/// question bank or glossary in its own right.
pub(crate) fn back_matter_chunks(chunks: &[ChunkHeading]) -> HashSet<i64> {
    let mut earlier = HashSet::new();
    let mut in_run = false;
    let mut marked = HashSet::new();
    let mut marked_chars = 0_usize;
    for chunk in chunks {
        if let Some(heading) = chunk.heading.as_deref().map(normalized) {
            if BACK_MATTER_HEADINGS.contains(&heading.as_str()) {
                in_run = true;
            } else if !(in_run && earlier.contains(&heading)) {
                in_run = false;
                earlier.insert(heading);
            }
        }
        if in_run {
            marked.insert(chunk.index);
            marked_chars = marked_chars.saturating_add(chunk.chars);
        }
    }
    let total = chunks
        .iter()
        .fold(0_usize, |sum, chunk| sum.saturating_add(chunk.chars));
    if marked_chars.saturating_mul(2) >= total {
        return HashSet::new();
    }
    marked
}

pub(crate) fn document_back_matter(
    connection: &Connection,
    document_id: &str,
) -> Result<HashSet<i64>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT chunk_index, heading_context, length(full_text)
             FROM document_semantic_chunk_sources WHERE document_id = ?1
             ORDER BY chunk_index LIMIT 100001",
        )
        .map_err(CommandError::database)?;
    let chunks = statement
        .query_map([document_id], |row| {
            Ok(ChunkHeading {
                index: row.get(0)?,
                heading: row.get(1)?,
                chars: usize::try_from(row.get::<_, i64>(2)?).unwrap_or(0),
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(back_matter_chunks(&chunks))
}

/// Page renders attached to back-matter chunks. A chunk that shares one (the
/// last table of a chapter on the page where Key Terms begin) would otherwise
/// show the model the skipped material in the image.
pub(crate) fn back_matter_assets(
    connection: &Connection,
    document_id: &str,
) -> Result<HashSet<String>, CommandError> {
    let chunks = document_back_matter(connection, document_id)?;
    if chunks.is_empty() {
        return Ok(HashSet::new());
    }
    let mut statement = connection
        .prepare(
            "SELECT chunk_index, visual_asset_ids_json FROM document_semantic_chunk_sources
             WHERE document_id = ?1 AND visual_asset_ids_json <> '[]'
             ORDER BY chunk_index LIMIT 100001",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([document_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut assets = HashSet::new();
    for (index, json) in rows {
        if chunks.contains(&index) {
            assets.extend(
                serde_json::from_str::<Vec<String>>(&json).map_err(CommandError::serialization)?,
            );
        }
    }
    Ok(assets)
}

/// The canonical chunk a document source was cut from; image-set sources have
/// no chunk headings and never count as back matter.
pub(crate) fn source_chunk(source: &SourceRecord) -> Option<(&str, i64)> {
    if source.kind != SourceKind::Document {
        return None;
    }
    let document = source.document_id.as_deref()?;
    let index = source.segment_key.split(':').next()?.parse().ok()?;
    Some((document, index))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunks(headings: &[Option<&str>]) -> Vec<ChunkHeading> {
        headings
            .iter()
            .enumerate()
            .map(|(index, heading)| ChunkHeading {
                index: i64::try_from(index).unwrap(),
                heading: heading.map(str::to_string),
                chars: 100,
            })
            .collect()
    }

    fn marked(headings: &[Option<&str>]) -> Vec<i64> {
        let mut found = back_matter_chunks(&chunks(headings))
            .into_iter()
            .collect::<Vec<_>>();
        found.sort_unstable();
        found
    }

    #[test]
    fn a_chapter_review_organised_by_the_chapter_sections_is_all_back_matter() {
        let headings = [
            Some("The Heart"),
            Some("19.2 Cardiac Muscle"),
            None,
            Some("19.3 Cardiac Cycle"),
            None,
            Some("Phases of the Cardiac Cycle"),
            None,
            Some("19.4 Cardiac Physiology"),
            None,
            None,
            None,
            Some("Key Terms"),
            None,
            Some("Chapter Review"),
            Some("19.2 Cardiac Muscle"),
            None,
            Some("19.3 cardiac cycle"),
            Some("Review Questions:"),
            Some("19.4 Cardiac Physiology"),
        ];
        assert_eq!(marked(&headings), (11..=18).collect::<Vec<_>>());
    }

    #[test]
    fn a_new_heading_after_the_review_starts_learning_again() {
        let headings = [
            Some("Chapter 19 The Heart"),
            None,
            None,
            None,
            Some("Review Questions"),
            Some("Chapter 19 The Heart"),
            Some("Chapter 20 Blood Vessels"),
            None,
            None,
            None,
        ];
        assert_eq!(marked(&headings), vec![4, 5]);
    }

    #[test]
    fn a_document_that_is_mostly_questions_or_terms_keeps_everything() {
        assert!(marked(&[Some("Intro"), Some("Key Terms"), None]).is_empty());
        assert!(marked(&[Some("Glossary"), None, None, None]).is_empty());
    }

    #[test]
    fn stored_chunks_mark_the_sources_cut_from_them() {
        let (db, mut source, _, _) = crate::knowledge::authoring::tests::fixture();
        let document = source.owner_id.clone();
        let locator = serde_json::to_string(
            &crate::document_domain::SourceLocator::new(
                crate::document_domain::SourceUnitKind::Page,
                vec![1],
                None,
            )
            .unwrap(),
        )
        .unwrap();
        db.with_connection(|c| {
            for (index, heading, text, assets) in [
                (0, Some("Cardiac Cycle"), "Systole and diastole alternate. ".repeat(20), r#"["unit-render:000001"]"#),
                (1, None, "Valves close in sequence. ".repeat(20), r#"["unit-render:000002"]"#),
                (2, Some("Review Questions"), "1. Which valve closes first?".to_string(), r#"["unit-render:000002"]"#),
                (3, None, "2. What causes the second heart sound?".to_string(), "[]"),
            ] {
                c.execute(
                    "INSERT INTO document_semantic_chunk_sources(document_id,chunk_index,text,full_text,
                        source_locator_json,visual_asset_ids_json,element_types_json,heading_context,
                        token_estimate,is_oversized,content_sha256,canonical_schema_version,created_at,updated_at)
                     VALUES(?1,?2,?3,?3,?7,?6,'[]',?4,1,0,?5,1,1,1)",
                    rusqlite::params![document, index, text, heading, "a".repeat(64), assets, locator],
                )
                .map_err(CommandError::database)?;
            }
            let found = document_back_matter(c, &document)?;
            assert_eq!(found, HashSet::from([2, 3]));
            assert_eq!(
                back_matter_assets(c, &document)?,
                HashSet::from(["unit-render:000002".to_string()])
            );
            Ok(())
        })
        .unwrap();
        // The chunk before Review Questions shares that page's render; the
        // model must not read the questions from it.
        let images = |index: &str| {
            crate::knowledge::adapters::materialize_source(
                &db,
                &crate::knowledge::adapters::SourceSeed {
                    kind: SourceKind::Document,
                    owner_id: document.clone(),
                    parent_key: index.to_string(),
                },
            )
            .unwrap()
            .into_iter()
            .map(|source| source.images.len())
            .sum::<usize>()
        };
        assert_eq!((images("0"), images("1")), (1, 0));
        assert_eq!(source_chunk(&source), Some((document.as_str(), 0)));
        source.segment_key = "3:1:0".into();
        assert_eq!(source_chunk(&source), Some((document.as_str(), 3)));
        source.segment_key = "image:abc".into();
        assert_eq!(source_chunk(&source), None);
        source.kind = SourceKind::Card;
        source.segment_key = "3".into();
        assert_eq!(source_chunk(&source), None);
    }

    #[test]
    fn headings_that_only_mention_review_are_not_back_matter() {
        let headings = [
            Some("Review of cardiac anatomy"),
            None,
            Some("Summary"),
            None,
            Some("Exercises"),
        ];
        assert!(marked(&headings).is_empty());
        assert_eq!(normalized(" 19.6  Key   Terms: "), "key terms");
    }
}
