//! Canonical studyability projection shared by workload, Search/Filter,
//! practice, deck summaries, and Analytics.
//!
//! Current/studyable/due predicates live directly in the bounded SQL queries;
//! this module supplies the one reusable learning-signal CTE so those queries do
//! not fall back to per-card database calls.

/// Shared CTE for the current learning epoch.
///
/// A reset begins a new epoch without deleting immutable review evidence. The
/// projection returns one boolean-like integer per card.
pub fn learning_signal_cte() -> &'static str {
    "latest_reset AS (
         SELECT card_id, MAX(occurred_at) AS reset_at
         FROM schedule_action_events
         WHERE action = 'reset'
         GROUP BY card_id
     ),
     ranked_learning_reviews AS (
         SELECT r.card_id, r.answer_correct,
                ROW_NUMBER() OVER (
                    PARTITION BY r.card_id
                    ORDER BY r.reviewed_at DESC, r.created_at DESC, r.id DESC
                ) AS review_rank
         FROM review_events r
         LEFT JOIN latest_reset lr ON lr.card_id = r.card_id
         WHERE r.reviewed_at > COALESCE(lr.reset_at, -1)
     ),
     learning_signals AS (
         SELECT card_id,
                CASE
                    WHEN MAX(CASE WHEN review_rank = 1 THEN answer_correct END) = 1
                     AND MAX(CASE WHEN review_rank = 2 THEN answer_correct END) = 1
                    THEN 0
                    WHEN MAX(CASE WHEN review_rank = 1 THEN answer_correct END) = 0
                     AND MAX(CASE WHEN review_rank = 2 THEN answer_correct END) = 0
                    THEN 1
                    WHEN SUM(CASE
                        WHEN review_rank <= 5 AND answer_correct = 0 THEN 1 ELSE 0
                    END) >= 3
                    THEN 1
                    ELSE 0
                END AS needs_more_practice
         FROM ranked_learning_reviews
         WHERE review_rank <= 5
         GROUP BY card_id
     )"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning_signal_projection_is_bounded_to_recent_reviews() {
        let sql = learning_signal_cte();
        assert!(sql.contains("review_rank <= 5"));
        assert!(sql.contains("latest_reset"));
    }
}
