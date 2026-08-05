//! Shared current-learning-epoch projection.
//!
//! The rule remains: two consecutive wrong answers, or at least three wrong
//! answers in the last five unless the two newest answers are both correct.

pub const LEARNING_SIGNALS_CTE: &str = r#"
latest_schedule_reset AS (
    SELECT card_id, MAX(occurred_at) AS reset_at
    FROM schedule_action_events
    WHERE action = 'reset'
    GROUP BY card_id
),
ranked_epoch_reviews AS (
    SELECT
        review.card_id,
        review.answer_correct,
        ROW_NUMBER() OVER (
            PARTITION BY review.card_id
            ORDER BY review.reviewed_at DESC, review.created_at DESC, review.id DESC
        ) AS review_rank
    FROM review_events review
    LEFT JOIN latest_schedule_reset reset ON reset.card_id = review.card_id
    WHERE review.reviewed_at > COALESCE(reset.reset_at, -1)
),
current_learning_signals AS (
    SELECT
        card_id,
        CASE
            WHEN SUM(CASE WHEN review_rank <= 2 AND answer_correct = 0 THEN 1 ELSE 0 END) = 2
            THEN 1
            WHEN SUM(CASE WHEN review_rank <= 5 AND answer_correct = 0 THEN 1 ELSE 0 END) >= 3
             AND SUM(CASE WHEN review_rank <= 2 AND answer_correct = 1 THEN 1 ELSE 0 END) <> 2
            THEN 1
            ELSE 0
        END AS needs_more_practice
    FROM ranked_epoch_reviews
    WHERE review_rank <= 5
    GROUP BY card_id
)
"#;

pub fn value_sql(card_id_expression: &str) -> String {
    format!(
        "COALESCE((SELECT signal.needs_more_practice \
         FROM current_learning_signals signal \
         WHERE signal.card_id = {card_id_expression}), 0)"
    )
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    #[test]
    fn cte_produces_the_expected_epoch_signal() {
        let connection = Connection::open_in_memory().expect("database should open");
        connection
            .execute_batch(
                "CREATE TABLE review_events (
                     id TEXT PRIMARY KEY,
                     card_id TEXT NOT NULL,
                     answer_correct INTEGER NOT NULL,
                     reviewed_at INTEGER NOT NULL,
                     created_at INTEGER NOT NULL
                 );
                 CREATE TABLE schedule_action_events (
                     card_id TEXT NOT NULL,
                     action TEXT NOT NULL,
                     occurred_at INTEGER NOT NULL
                 );
                 INSERT INTO review_events VALUES
                     ('1', 'weak', 0, 1, 1),
                     ('2', 'weak', 0, 2, 2),
                     ('3', 'recovered', 0, 1, 1),
                     ('4', 'recovered', 1, 2, 2),
                     ('5', 'recovered', 1, 3, 3);",
            )
            .expect("fixture should load");
        let sql = format!(
            "WITH {LEARNING_SIGNALS_CTE}
             SELECT card_id, needs_more_practice
             FROM current_learning_signals ORDER BY card_id"
        );
        let mut statement = connection.prepare(&sql).expect("query should prepare");
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
            .expect("query should run")
            .collect::<Result<Vec<_>, _>>()
            .expect("rows should collect");
        assert_eq!(rows, vec![("recovered".to_string(), 0), ("weak".to_string(), 1)]);
    }
}
