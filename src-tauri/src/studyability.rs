//! One definition of current, studyable, and studyable-due cards.
//!
//! Every list, workload, search, and current-inventory query uses these helpers
//! instead of recreating subtly different eligibility predicates.

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardEligibility {
    pub card_deleted: bool,
    pub note_deleted: bool,
    pub deck_deleted: bool,
    pub deck_archived: bool,
    pub manually_suspended: bool,
    pub archive_suspended: bool,
    pub buried_until: Option<i64>,
    pub due_at: i64,
}

#[cfg(test)]
impl CardEligibility {
    pub fn is_current_generated(self) -> bool {
        !self.card_deleted && !self.note_deleted && !self.deck_deleted && !self.deck_archived
    }

    pub fn is_studyable(self, now_utc_millis: i64) -> bool {
        self.is_current_generated()
            && !self.manually_suspended
            && !self.archive_suspended
            && self
                .buried_until
                .is_none_or(|buried_until| now_utc_millis >= buried_until)
    }

    pub fn is_schedule_due(self, now_utc_millis: i64) -> bool {
        self.due_at <= now_utc_millis
    }

    pub fn is_studyable_due(self, now_utc_millis: i64) -> bool {
        self.is_studyable(now_utc_millis) && self.is_schedule_due(now_utc_millis)
    }
}

pub fn current_card_sql(card: &str, note: &str, deck: &str) -> &'static str {
    match (card, note, deck) {
        ("c", "n", "d") => {
            "c.deleted_at IS NULL AND n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL"
        }
        _ => panic!("studyability aliases must be c, n, and d"),
    }
}

pub fn studyable_card_sql(now_placeholder: &str) -> String {
    format!(
        "{} AND c.suspended_at IS NULL AND c.archive_suspended_at IS NULL AND (c.buried_until IS NULL OR c.buried_until <= {now_placeholder})",
        current_card_sql("c", "n", "d")
    )
}

pub fn studyable_due_sql(now_placeholder: &str) -> String {
    format!(
        "{} AND s.due_at <= {now_placeholder}",
        studyable_card_sql(now_placeholder)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> CardEligibility {
        CardEligibility {
            card_deleted: false,
            note_deleted: false,
            deck_deleted: false,
            deck_archived: false,
            manually_suspended: false,
            archive_suspended: false,
            buried_until: None,
            due_at: 100,
        }
    }

    #[test]
    fn burial_expires_at_the_exact_boundary() {
        let mut card = base();
        card.buried_until = Some(100);
        assert!(!card.is_studyable(99));
        assert!(card.is_studyable(100));
    }

    #[test]
    fn due_does_not_override_suspension_or_archival() {
        let mut card = base();
        card.due_at = 0;
        card.manually_suspended = true;
        assert!(card.is_schedule_due(100));
        assert!(!card.is_studyable_due(100));
        card.manually_suspended = false;
        card.deck_archived = true;
        assert!(!card.is_studyable_due(100));
    }

    #[test]
    fn future_schedule_is_studyable_but_not_ready_now() {
        let mut card = base();
        card.due_at = 101;
        assert!(card.is_studyable(100));
        assert!(!card.is_studyable_due(100));
    }
}
