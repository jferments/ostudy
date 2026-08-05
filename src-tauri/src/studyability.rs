//! Canonical current-card, studyable, and studyable-due semantics.
//!
//! Query builders in workload, deck summaries, Search/Filter, custom practice,
//! and Analytics call these helpers rather than copying subtly different burial,
//! suspension, archive, and deletion predicates.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StudyabilityState {
    pub card_deleted: bool,
    pub note_deleted: bool,
    pub deck_deleted: bool,
    pub deck_archived: bool,
    pub suspended: bool,
    pub buried_until: Option<i64>,
    pub due_at: i64,
}

impl StudyabilityState {
    pub fn is_current(self) -> bool {
        !self.card_deleted && !self.note_deleted && !self.deck_deleted && !self.deck_archived
    }

    pub fn is_studyable(self, now: i64) -> bool {
        self.is_current() && !self.suspended && self.buried_until.is_none_or(|value| value <= now)
    }

    pub fn is_studyable_due(self, now: i64) -> bool {
        self.is_studyable(now) && self.due_at <= now
    }
}

pub fn current_card_conditions(card: &str, note: &str, deck: &str) -> Vec<String> {
    vec![
        format!("{card}.deleted_at IS NULL"),
        format!("{note}.deleted_at IS NULL"),
        format!("{deck}.deleted_at IS NULL"),
        format!("{deck}.archived_at IS NULL"),
    ]
}

pub fn studyable_card_conditions(
    card: &str,
    note: &str,
    deck: &str,
    now_parameter: &str,
) -> Vec<String> {
    let mut conditions = current_card_conditions(card, note, deck);
    conditions.push(format!("{card}.suspended_at IS NULL"));
    conditions.push(format!(
        "({card}.buried_until IS NULL OR {card}.buried_until <= {now_parameter})"
    ));
    conditions
}

pub fn studyable_due_conditions(
    card: &str,
    note: &str,
    deck: &str,
    schedule: &str,
    now_parameter: &str,
) -> Vec<String> {
    let mut conditions = studyable_card_conditions(card, note, deck, now_parameter);
    conditions.push(format!("{schedule}.due_at <= {now_parameter}"));
    conditions
}

/// Shared CTE for the current learning epoch.
///
/// A reset begins a new epoch without deleting immutable review evidence. The
/// projection returns one boolean-like integer per card using exactly the rule
/// described by the scheduling ADR.
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

    fn state() -> StudyabilityState {
        StudyabilityState {
            card_deleted: false,
            note_deleted: false,
            deck_deleted: false,
            deck_archived: false,
            suspended: false,
            buried_until: None,
            due_at: 100,
        }
    }

    #[test]
    fn due_requires_current_unburied_unsuspended_card() {
        assert!(state().is_studyable_due(100));
        assert!(!StudyabilityState {
            suspended: true,
            ..state()
        }
        .is_studyable_due(100));
        assert!(!StudyabilityState {
            buried_until: Some(101),
            ..state()
        }
        .is_studyable_due(100));
        assert!(!StudyabilityState {
            deck_archived: true,
            ..state()
        }
        .is_studyable_due(100));
        assert!(!StudyabilityState {
            deck_deleted: true,
            ..state()
        }
        .is_studyable_due(100));
        assert!(!StudyabilityState {
            due_at: 101,
            ..state()
        }
        .is_studyable_due(100));
    }

    #[test]
    fn burial_expires_at_the_exact_boundary() {
        assert!(StudyabilityState {
            buried_until: Some(100),
            ..state()
        }
        .is_studyable(100));
    }
}
