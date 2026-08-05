//! Canonical current-card, studyable, and studyable-due semantics.
//!
//! Every list, workload, practice, Search/Filter, and Analytics projection uses
//! these predicates. “Ready now” always means a current card that is unsuspended,
//! unburied at the captured instant, and whose exact FSRS due instant has arrived.

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

pub fn current_card_sql(card: &str, note: &str, deck: &str) -> String {
    current_card_conditions(card, note, deck).join(" AND ")
}

pub fn studyable_card_sql(card: &str, note: &str, deck: &str, now_parameter: &str) -> String {
    studyable_card_conditions(card, note, deck, now_parameter).join(" AND ")
}

pub fn studyable_due_sql(
    card: &str,
    note: &str,
    deck: &str,
    schedule: &str,
    now_parameter: &str,
) -> String {
    studyable_due_conditions(card, note, deck, schedule, now_parameter).join(" AND ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active() -> StudyabilityState {
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
    fn due_requires_current_unsuspended_and_unburied() {
        assert!(active().is_studyable_due(100));
        assert!(!StudyabilityState {
            suspended: true,
            ..active()
        }
        .is_studyable_due(100));
        assert!(!StudyabilityState {
            buried_until: Some(101),
            ..active()
        }
        .is_studyable_due(100));
        assert!(!StudyabilityState {
            deck_archived: true,
            ..active()
        }
        .is_studyable_due(100));
        assert!(!StudyabilityState {
            deck_deleted: true,
            ..active()
        }
        .is_studyable_due(100));
        assert!(!StudyabilityState {
            due_at: 101,
            ..active()
        }
        .is_studyable_due(100));
    }

    #[test]
    fn burial_expires_at_the_exact_boundary() {
        assert!(StudyabilityState {
            buried_until: Some(100),
            ..active()
        }
        .is_studyable(100));
    }
}
