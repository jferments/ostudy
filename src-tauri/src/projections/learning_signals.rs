//! One SQL projection for the current learning epoch.
//!
//! A reset starts a new epoch without deleting older review evidence. Queries
//! join `learning_signal` by card ID instead of issuing review queries per card.
//! A collection copy merged into its original (release schema 6) is the same
//! fact: its reviews and resets count under the surviving card.

pub const LEARNING_SIGNAL_CTES: &str = r#"
latest_reset AS (
    SELECT COALESCE(merged.surviving_card_id, a.card_id) AS card_id,
           MAX(a.occurred_at) AS reset_at
    FROM schedule_action_events a
    LEFT JOIN card_merges merged ON merged.retired_card_id = a.card_id
    WHERE a.action = 'reset'
    GROUP BY COALESCE(merged.surviving_card_id, a.card_id)
),
learning_reviews AS (
    SELECT
        COALESCE(merged.surviving_card_id, r.card_id) AS card_id,
        r.answer_correct,
        r.reviewed_at,
        r.created_at,
        r.id
    FROM review_events r
    LEFT JOIN card_merges merged ON merged.retired_card_id = r.card_id
),
ranked_learning_reviews AS (
    SELECT
        r.card_id,
        r.answer_correct,
        ROW_NUMBER() OVER (
            PARTITION BY r.card_id
            ORDER BY r.reviewed_at DESC, r.created_at DESC, r.id DESC
        ) AS review_rank
    FROM learning_reviews r
    LEFT JOIN latest_reset lr ON lr.card_id = r.card_id
    WHERE r.reviewed_at > COALESCE(lr.reset_at, -1)
),
learning_signal AS (
    SELECT
        card_id,
        CASE
            WHEN MAX(CASE WHEN review_rank = 1 THEN answer_correct END) = 1
             AND MAX(CASE WHEN review_rank = 2 THEN answer_correct END) = 1
                THEN 0
            WHEN MAX(CASE WHEN review_rank = 1 THEN answer_correct END) = 0
             AND MAX(CASE WHEN review_rank = 2 THEN answer_correct END) = 0
                THEN 1
            WHEN SUM(CASE WHEN review_rank <= 5 AND answer_correct = 0 THEN 1 ELSE 0 END) >= 3
                THEN 1
            ELSE 0
        END AS needs_more_practice
    FROM ranked_learning_reviews
    WHERE review_rank <= 5
    GROUP BY card_id
)
"#;

pub const fn join_sql() -> &'static str {
    "LEFT JOIN learning_signal ls ON ls.card_id = c.id"
}

pub const fn value_sql() -> &'static str {
    "COALESCE(ls.needs_more_practice, 0)"
}

#[cfg(test)]
fn needs_more_practice_from_recent(recent_descending: &[i64]) -> bool {
    if recent_descending.len() >= 2 && recent_descending[0] == 1 && recent_descending[1] == 1 {
        return false;
    }
    if recent_descending.len() >= 2 && recent_descending[0] == 0 && recent_descending[1] == 0 {
        return true;
    }
    recent_descending
        .iter()
        .take(5)
        .filter(|value| **value == 0)
        .count()
        >= 3
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use super::{join_sql, needs_more_practice_from_recent, value_sql, LEARNING_SIGNAL_CTES};
    use crate::database::subjects_fixture::{
        advance_release_library, release_library, seed, DAY, T0,
    };

    #[test]
    fn repeated_wrong_answers_require_more_practice() {
        assert!(needs_more_practice_from_recent(&[0, 0]));
        assert!(needs_more_practice_from_recent(&[0, 1, 0, 1, 0]));
    }

    #[test]
    fn two_remembered_answers_clear_the_signal() {
        assert!(!needs_more_practice_from_recent(&[1, 1, 0, 0]));
        assert!(!needs_more_practice_from_recent(&[1, 0]));
    }

    fn signal(connection: &Connection, card: &str) -> i64 {
        connection
            .query_row(
                &format!(
                    "WITH {LEARNING_SIGNAL_CTES}
                     SELECT {value} FROM cards c {join} WHERE c.id = ?1",
                    value = value_sql(),
                    join = join_sql(),
                ),
                [card],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn a_merged_copy_s_reviews_and_resets_count_for_its_original() {
        let connection = release_library(5);
        let library = seed(&connection);
        let review_of = |card: &str| {
            library
                .reviews
                .iter()
                .find(|(reviewed, _)| reviewed == card)
                .map(|(_, review)| review.clone())
                .unwrap()
        };
        for card in [
            library.o_mech[0].card(),
            library.cm_phys[0].card(),
            library.o_bio[0].card(),
            library.cb[0].card(),
            library.o_mech[2].card(),
            library.cm_phys[2].card(),
        ] {
            connection
                .execute(
                    "UPDATE review_events SET answer_correct = 0, rating = 1, confidence = 'wrong'
                     WHERE id = ?1",
                    [review_of(card)],
                )
                .unwrap();
        }
        // The learner reset the Biology copy after both wrong answers.
        let operation = format!("5a{}", "e".repeat(30));
        connection
            .execute(
                "INSERT INTO schedule_action_operations(id, action, requested_count,
                     changed_count, unchanged_count, rejected_count, occurred_at, created_at)
                 VALUES (?1, 'reset', 1, 1, 0, 0, ?2, ?2)",
                params![operation, T0 + 3 * DAY],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO schedule_action_events(id, operation_id, card_id, action,
                     due_before, due_after, state_before, state_after,
                     schedule_revision_before, schedule_revision_after, occurred_at, created_at)
                 SELECT ?1, ?2, card_id, 'reset', due_at, ?4, state, 0, revision,
                        revision + 1, ?4, ?4
                 FROM card_schedules WHERE card_id = ?3",
                params![
                    format!("5e{}", "e".repeat(30)),
                    operation,
                    library.cb[0].card(),
                    T0 + 3 * DAY
                ],
            )
            .unwrap();
        advance_release_library(&connection, 5, 6);

        // The two latest answers were wrong: one direct, one through a copy.
        assert_eq!(signal(&connection, library.o_mech[0].card()), 1);
        // The copy's reset started a new epoch for the fact.
        assert_eq!(signal(&connection, library.o_bio[0].card()), 0);
        // The original's reset after both wrong answers still ends the epoch
        // that holds its copy's review.
        assert_eq!(signal(&connection, library.o_mech[2].card()), 0);
    }
}
