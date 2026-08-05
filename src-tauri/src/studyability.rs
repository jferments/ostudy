//! Canonical current-card eligibility vocabulary.
//!
//! All user-facing “ready now” counts use [`studyable_due_sql`]. Raw schedule
//! overdue state is intentionally not exposed as an interchangeable concept.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StudyabilityFacts {
    pub card_deleted: bool,
    pub note_deleted: bool,
    pub deck_deleted: bool,
    pub deck_archived: bool,
    pub suspended: bool,
    pub buried_until: Option<i64>,
    pub due_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StudyabilityState {
    pub is_current_generated: bool,
    pub is_studyable: bool,
    pub is_schedule_due: bool,
    pub is_studyable_due: bool,
}

pub fn classify(facts: StudyabilityFacts, now_utc_millis: i64) -> StudyabilityState {
    let is_current_generated = !facts.card_deleted
        && !facts.note_deleted
        && !facts.deck_deleted
        && !facts.deck_archived;
    let burial_expired = facts
        .buried_until
        .is_none_or(|buried_until| now_utc_millis >= buried_until);
    let is_studyable = is_current_generated && !facts.suspended && burial_expired;
    let is_schedule_due = facts.due_at <= now_utc_millis;
    StudyabilityState {
        is_current_generated,
        is_studyable,
        is_schedule_due,
        is_studyable_due: is_studyable && is_schedule_due,
    }
}

/// SQL condition for a generated card that exists in the active current library.
pub fn current_card_sql(card: &str, note: &str, deck: &str) -> String {
    format!(
        "{card}.deleted_at IS NULL \
         AND {note}.deleted_at IS NULL \
         AND {deck}.deleted_at IS NULL \
         AND {deck}.archived_at IS NULL"
    )
}

/// SQL condition for a card that may enter practice at the captured instant.
pub fn studyable_sql(card: &str, note: &str, deck: &str, now_parameter: &str) -> String {
    format!(
        "{} AND {card}.suspended_at IS NULL \
         AND ({card}.buried_until IS NULL OR {card}.buried_until <= {now_parameter})",
        current_card_sql(card, note, deck)
    )
}

/// SQL condition for the user-facing “ready now” concept.
pub fn studyable_due_sql(
    card: &str,
    note: &str,
    deck: &str,
    schedule: &str,
    now_parameter: &str,
) -> String {
    format!(
        "{} AND {schedule}.due_at <= {now_parameter}",
        studyable_sql(card, note, deck, now_parameter)
    )
}

/// Projection columns used by summary and page queries.
pub fn projection_sql(
    card: &str,
    note: &str,
    deck: &str,
    schedule: &str,
    now_parameter: &str,
) -> String {
    let current = current_card_sql(card, note, deck);
    let studyable = studyable_sql(card, note, deck, now_parameter);
    let due = studyable_due_sql(card, note, deck, schedule, now_parameter);
    format!(
        "CASE WHEN {current} THEN 1 ELSE 0 END AS is_current_generated, \
         CASE WHEN {studyable} THEN 1 ELSE 0 END AS is_studyable, \
         CASE WHEN {schedule}.due_at <= {now_parameter} THEN 1 ELSE 0 END AS is_schedule_due, \
         CASE WHEN {due} THEN 1 ELSE 0 END AS is_studyable_due"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> StudyabilityFacts {
        StudyabilityFacts {
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
    fn burial_boundary_is_studyable_at_exact_deadline() {
        let mut facts = base();
        facts.buried_until = Some(100);
        let state = classify(facts, 100);
        assert!(state.is_studyable);
        assert!(state.is_studyable_due);
    }

    #[test]
    fn schedule_due_does_not_override_suspension_or_burial() {
        let mut suspended = base();
        suspended.suspended = true;
        let suspended_state = classify(suspended, 100);
        assert!(suspended_state.is_schedule_due);
        assert!(!suspended_state.is_studyable_due);

        let mut buried = base();
        buried.buried_until = Some(101);
        let buried_state = classify(buried, 100);
        assert!(buried_state.is_schedule_due);
        assert!(!buried_state.is_studyable_due);
    }

    #[test]
    fn archived_or_deleted_ancestors_remove_current_inventory() {
        for mutation in 0..4 {
            let mut facts = base();
            match mutation {
                0 => facts.card_deleted = true,
                1 => facts.note_deleted = true,
                2 => facts.deck_deleted = true,
                _ => facts.deck_archived = true,
            }
            let state = classify(facts, 100);
            assert!(!state.is_current_generated);
            assert!(!state.is_studyable);
            assert!(!state.is_studyable_due);
        }
    }
}
