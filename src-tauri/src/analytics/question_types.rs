//! One row per historically captured question kind. These are reviewed first
//! attempts, not tile counts, current authored keys, or inferred mastery.

use rusqlite::{params_from_iter, Connection};
use serde::Serialize;

use super::{
    history::rate,
    scope::{AnalyticsContext, HistoryPeriod},
};
use crate::error::CommandError;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionTypeAnalyticsDto {
    pub question_type: String,
    pub review_count: i64,
    pub self_assessed_count: i64,
    pub objective_count: i64,
    pub correct_count: i64,
    pub incorrect_count: i64,
    pub incomplete_count: i64,
    pub objective_accuracy: Option<f64>,
    pub unassisted_objective_count: i64,
    pub unassisted_correct_count: i64,
    pub unassisted_accuracy: Option<f64>,
    pub assistance_tracked_count: i64,
    pub assisted_count: i64,
    pub override_count: i64,
    pub remediation_count: i64,
    pub unavailable_evidence_count: i64,
}

pub(super) fn load(
    connection: &Connection,
    context: &AnalyticsContext,
) -> Result<Vec<QuestionTypeAnalyticsDto>, CommandError> {
    let mut values = Vec::new();
    let conditions = context.historical_conditions(&mut values, HistoryPeriod::Current);
    // Review/instance links are unique in both directions. Correlating retries
    // avoids multiplying first checks when a learner practices the board again.
    // Deliberately never read checkpoint JSON, authored answers or raw responses.
    let sql = format!(
        "SELECT a.card_type, COUNT(*),
            SUM(CASE WHEN a.card_type IN ('basic', 'cloze')
                OR (a.card_type = 'image_occlusion' AND q.phase = 'revealed')
                THEN 1 ELSE 0 END),
            SUM(CASE WHEN q.phase = 'checked' THEN 1 ELSE 0 END),
            SUM(CASE WHEN q.phase = 'checked' AND q.verdict = 'correct' THEN 1 ELSE 0 END),
            SUM(CASE WHEN q.phase = 'checked' AND q.verdict = 'incorrect' THEN 1 ELSE 0 END),
            SUM(CASE WHEN q.phase = 'checked' AND q.verdict = 'incomplete' THEN 1 ELSE 0 END),
            SUM(CASE WHEN q.phase = 'checked' AND q.assisted = 0 THEN 1 ELSE 0 END),
            SUM(CASE WHEN q.phase = 'checked' AND q.assisted = 0
                AND q.verdict = 'correct' THEN 1 ELSE 0 END),
            COUNT(q.instance_id),
            SUM(CASE WHEN q.assisted = 1 THEN 1 ELSE 0 END),
            SUM(CASE WHEN link.override_reason IS NOT NULL THEN 1 ELSE 0 END),
            SUM((SELECT COUNT(*) FROM question_remediation_attempts remediation
                WHERE remediation.instance_id = q.instance_id))
         FROM review_events r
         JOIN review_event_attributions a ON a.review_event_id = r.id
         JOIN cards c ON c.id = r.card_id
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         LEFT JOIN question_review_links link ON link.review_event_id = r.id
         LEFT JOIN question_attempts q ON q.instance_id = link.instance_id
         WHERE {}
         GROUP BY a.card_type ORDER BY a.card_type",
        conditions.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            let review_count: i64 = row.get(1)?;
            let self_assessed_count: i64 = row.get(2)?;
            let objective_count: i64 = row.get(3)?;
            let correct_count: i64 = row.get(4)?;
            let unassisted_objective_count: i64 = row.get(7)?;
            let unassisted_correct_count: i64 = row.get(8)?;
            Ok(QuestionTypeAnalyticsDto {
                question_type: row.get(0)?,
                review_count,
                self_assessed_count,
                objective_count,
                correct_count,
                incorrect_count: row.get(5)?,
                incomplete_count: row.get(6)?,
                objective_accuracy: rate(correct_count, objective_count),
                unassisted_objective_count,
                unassisted_correct_count,
                unassisted_accuracy: rate(unassisted_correct_count, unassisted_objective_count),
                assistance_tracked_count: row.get(9)?,
                assisted_count: row.get(10)?,
                override_count: row.get(11)?,
                remediation_count: row.get(12)?,
                unavailable_evidence_count: review_count - self_assessed_count - objective_count,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::{analytics::AnalyticsQueryDto, database::Database};

    fn context(decks: Vec<String>, tags: Vec<String>) -> AnalyticsContext {
        AnalyticsContext::from_query(AnalyticsQueryDto {
            range: "all".to_string(),
            deck_ids: decks,
            tag_ids: tags,
            custom_start_local_date: None,
            custom_end_local_date: None,
            highlighted_session_id: None,
        })
        .unwrap()
    }

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        // Minimal relational fixture for the aggregation, including deliberately
        // missing evidence. The real migrated-schema check is separate below.
        connection
            .execute_batch(
                "CREATE TABLE decks(id TEXT PRIMARY KEY, deleted_at INTEGER);
                 CREATE TABLE notes(id TEXT PRIMARY KEY, deleted_at INTEGER, note_type TEXT);
                 CREATE TABLE cards(id TEXT PRIMARY KEY, note_id TEXT, deck_id TEXT, deleted_at INTEGER);
                 CREATE TABLE review_events(id TEXT PRIMARY KEY, card_id TEXT, answer_correct INTEGER);
                 CREATE TABLE review_event_attributions(review_event_id TEXT PRIMARY KEY,
                     card_type TEXT, review_local_date TEXT);
                 CREATE TABLE review_event_tag_attributions(review_event_id TEXT, tag_id TEXT);
                 CREATE TABLE question_attempts(instance_id TEXT PRIMARY KEY, phase TEXT,
                     verdict TEXT, assisted INTEGER);
                 CREATE TABLE question_review_links(review_event_id TEXT PRIMARY KEY,
                     instance_id TEXT UNIQUE, override_reason TEXT);
                 CREATE TABLE question_remediation_attempts(operation_id TEXT PRIMARY KEY,
                     instance_id TEXT);
                 INSERT INTO decks VALUES('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', NULL);
                 INSERT INTO decks VALUES('bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', NULL);
                 INSERT INTO notes VALUES('note', NULL, 'basic');
                 INSERT INTO cards VALUES('card', 'note', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', NULL);",
            )
            .unwrap();
        connection
    }

    fn review(
        connection: &Connection,
        id: &str,
        kind: &str,
        verdict: Option<&str>,
        assisted: bool,
    ) {
        connection
            .execute("INSERT INTO review_events VALUES(?1, 'card', 1)", [id])
            .unwrap();
        connection
            .execute(
                "INSERT INTO review_event_attributions VALUES(?1, ?2, '2001-01-02')",
                params![id, kind],
            )
            .unwrap();
        if verdict.is_some() || kind == "image_occlusion" {
            let phase = if verdict.is_some() {
                "checked"
            } else {
                "revealed"
            };
            connection
                .execute(
                    "INSERT INTO question_attempts VALUES(?1, ?2, ?3, ?4)",
                    params![id, phase, verdict, assisted],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO question_review_links VALUES(?1, ?1, NULL)",
                    [id],
                )
                .unwrap();
        }
    }

    #[test]
    fn real_migrated_empty_library_has_no_invented_accuracy() {
        Database::open_in_memory()
            .unwrap()
            .with_connection(|connection| {
                assert!(load(connection, &context(Vec::new(), Vec::new()))?.is_empty());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn first_results_remain_separate_from_confidence_hints_overrides_and_retries() {
        let connection = fixture();
        review(&connection, "one", "matching", Some("correct"), false);
        review(&connection, "two", "matching", Some("incorrect"), true);
        review(&connection, "three", "matching", Some("incomplete"), false);
        connection
            .execute_batch(
                "UPDATE question_review_links SET override_reason = 'Deliberate override'
                     WHERE instance_id = 'two';
                 INSERT INTO question_remediation_attempts VALUES('retry1', 'two');
                 INSERT INTO question_remediation_attempts VALUES('retry2', 'two');",
            )
            .unwrap();
        let rows = load(&connection, &context(Vec::new(), Vec::new())).unwrap();
        let row = &rows[0];
        assert_eq!(row.review_count, 3);
        assert_eq!(
            (row.correct_count, row.incorrect_count, row.incomplete_count),
            (1, 1, 1)
        );
        assert_eq!(row.objective_accuracy, Some(1.0 / 3.0));
        assert_eq!(row.unassisted_accuracy, Some(0.5));
        assert_eq!(row.assisted_count, 1);
        assert_eq!(row.override_count, 1);
        assert_eq!(row.remediation_count, 2);
        assert_eq!(row.unavailable_evidence_count, 0);
    }

    #[test]
    fn self_rated_types_and_missing_objective_evidence_are_not_zero_percent_accuracy() {
        let connection = fixture();
        for kind in ["basic", "cloze", "image_occlusion", "ordering"] {
            review(&connection, kind, kind, None, false);
        }
        for row in load(&connection, &context(Vec::new(), Vec::new())).unwrap() {
            assert_eq!(row.review_count, 1);
            assert_eq!(row.objective_accuracy, None);
            if row.question_type == "ordering" {
                assert_eq!(row.unavailable_evidence_count, 1);
                assert_eq!(row.self_assessed_count, 0);
            } else {
                assert_eq!(row.self_assessed_count, 1);
                assert_eq!(row.unavailable_evidence_count, 0);
            }
        }
    }

    #[test]
    fn historical_kind_tags_and_calendar_survive_moves_and_deleted_owners_disappear() {
        let connection = fixture();
        review(&connection, "one", "select_all", Some("correct"), false);
        connection
            .execute_batch(
                "INSERT INTO review_event_tag_attributions
                     VALUES('one', 'cccccccccccccccccccccccccccccccc');
                 UPDATE cards SET deck_id = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';",
            )
            .unwrap();
        let mut selected = context(vec!["b".repeat(32)], vec!["c".repeat(32)]);
        let rows = load(&connection, &selected).unwrap();
        assert_eq!(rows[0].question_type, "select_all");
        assert!(
            load(&connection, &context(vec!["a".repeat(32)], Vec::new()))
                .unwrap()
                .is_empty()
        );
        selected.tag_ids = vec!["d".repeat(32)];
        assert!(load(&connection, &selected).unwrap().is_empty());
        selected.tag_ids.clear();
        selected.window.end_local_date = "2001-01-01".parse().unwrap();
        assert!(load(&connection, &selected).unwrap().is_empty());
        connection
            .execute("UPDATE notes SET deleted_at = 1", [])
            .unwrap();
        assert!(load(&connection, &context(Vec::new(), Vec::new()))
            .unwrap()
            .is_empty());
    }
}
