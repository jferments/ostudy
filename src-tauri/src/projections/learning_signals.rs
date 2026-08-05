//! Shared current-learning-epoch projection.
//!
//! A reset starts a new epoch without deleting immutable review evidence. This
//! CTE computes the user-facing weak-card signal once for aggregate/list queries.

pub const LEARNING_SIGNAL_CTE: &str = "latest_reset AS (
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
)";
