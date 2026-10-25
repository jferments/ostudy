//! The question of the day: which cards it may pick, that a study day keeps
//! its pick, and that the pick leans toward new and struggled cards.

use rusqlite::params;

use super::*;
use crate::database::Database;

const RICH_TEXT_Q: &str =
    r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"q"}]}]}"#;
const RICH_TEXT_A: &str =
    r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]}"#;
const NOW: &str = "2026-06-10T12:00:00-07:00";

fn study_time(now: &str) -> StudyTimeContext {
    let millis = chrono::DateTime::parse_from_rfc3339(now)
        .expect("test time should parse")
        .timestamp_millis();
    StudyTimeContext::pacific_2026_at(millis, crate::study_calendar::DEFAULT_NEW_DAY_START_HOUR)
        .expect("study time should resolve")
}

fn test_paths() -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-question-of-the-day-{}-{}",
        std::process::id(),
        std::thread::current()
            .name()
            .unwrap_or("main")
            .replace("::", "-")
    ));
    AppPaths::new(root).unwrap()
}

struct Library<'a> {
    connection: &'a Connection,
    next: u32,
}

impl<'a> Library<'a> {
    fn id(&mut self, kind: u8) -> String {
        self.next += 1;
        format!("{kind:02x}{:030x}", self.next)
    }

    fn exec(&self, sql: &str, values: impl rusqlite::Params) {
        self.connection
            .execute(sql, values)
            .unwrap_or_else(|error| panic!("{error}: {sql}"));
    }

    fn subject(&mut self) -> String {
        let id = self.id(0xc0);
        self.exec(
            "INSERT INTO document_collections(id, name, sort_order, created_at, updated_at,
                                              revision)
             VALUES (?1, 'Biology', 1000, 1, 1, 1)",
            params![id],
        );
        id
    }

    fn deck(&mut self, subject: Option<&str>) -> String {
        let id = self.id(0xde);
        self.exec(
            "INSERT INTO decks(id, name, created_at, updated_at, subject_id)
             VALUES (?1, 'Cells', 1, 1, ?2)",
            params![id, subject],
        );
        id
    }

    /// A Basic card in `deck` with a schedule in `state`, `reps` and `lapses`.
    fn card(&mut self, deck: &str, direction: &str, state: i64, reps: i64, lapses: i64) -> String {
        let note = self.id(0x40);
        self.exec(
            "INSERT INTO notes(id, deck_id, front_doc, back_doc, front_text, back_text,
                               created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'q', 'a', 1, 1)",
            params![note, deck, RICH_TEXT_Q, RICH_TEXT_A],
        );
        let id = self.id(0xca);
        self.exec(
            "INSERT INTO cards(id, note_id, deck_id, direction, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 1, 1)",
            params![id, note, deck, direction],
        );
        self.exec(
            "INSERT INTO card_schedules(card_id, state, due_at, reps, lapses, last_review_at,
                                        scheduler_name, scheduler_version, updated_at)
             VALUES (?1, ?2, 1, ?3, ?4, 1, 'fsrs', 'rs-fsrs-1.2.1', 1)",
            params![id, state, reps, lapses],
        );
        id
    }
}

fn candidate(card_id: &str, state: i64, reps: i64, lapses: i64) -> Candidate {
    Candidate {
        card_id: card_id.to_string(),
        state,
        reps,
        lapses,
        difficulty: 5.0,
    }
}

#[test]
fn an_empty_library_has_no_question() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let question = load_question_of_the_day(connection, &test_paths(), &study_time(NOW))?;
            assert!(question.is_none());
            Ok(())
        })
        .unwrap();
}

#[test]
fn only_studyable_cards_can_be_the_question() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let mut library = Library {
                connection,
                next: 0,
            };
            let deck = library.deck(None);
            let suspended = library.card(&deck, "forward", 2, 4, 0);
            library.exec(
                "UPDATE cards SET suspended_at = 1 WHERE id = ?1",
                [&suspended],
            );
            let archived_deck = library.deck(None);
            library.card(&archived_deck, "forward", 0, 0, 0);
            library.exec(
                "UPDATE decks SET archived_at = 1 WHERE id = ?1",
                [&archived_deck],
            );
            let time = study_time(NOW);
            assert!(load_question_of_the_day(connection, &test_paths(), &time)?.is_none());

            let studyable = library.card(&deck, "forward", 2, 4, 0);
            let question = load_question_of_the_day(connection, &test_paths(), &time)?
                .expect("the studyable card should be chosen");
            assert_eq!(question.card.id, studyable);
            assert_eq!(question.study_date, "2026-06-10");
            assert_eq!(question.reason, QuestionReason::Review);
            Ok(())
        })
        .unwrap();
}

#[test]
fn shows_the_card_as_practice_would_and_names_its_subject() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let mut library = Library {
                connection,
                next: 0,
            };
            let subject = library.subject();
            let deck = library.deck(Some(&subject));
            let reverse = library.card(&deck, "reverse", 2, 6, 2);
            let question =
                load_question_of_the_day(connection, &test_paths(), &study_time(NOW))?.unwrap();
            assert_eq!(question.card.id, reverse);
            // A reverse card asks with the note's answer side.
            assert_eq!(question.card.front_text, "a");
            assert_eq!(question.card.back_text, "q");
            assert_eq!(question.subject_id.as_deref(), Some(subject.as_str()));
            assert_eq!(question.deck_name, "Cells");
            assert_eq!(question.reason, QuestionReason::Struggled);
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_study_day_keeps_its_question_and_the_next_day_draws_again() {
    let cards: Vec<Candidate> = (0..40)
        .map(|index| candidate(&format!("card-{index:02}"), 2, 5, 0))
        .collect();
    let today = choose("2026-06-10", &cards).unwrap();
    assert_eq!(choose("2026-06-10", &cards), Some(today));
    // The order cards are read in does not change the pick.
    let reversed: Vec<Candidate> = cards.iter().rev().cloned().collect();
    assert_eq!(
        choose("2026-06-10", &reversed).map(|card| &card.card_id),
        Some(&today.card_id)
    );
    let days = (1..=9)
        .map(|day| {
            choose(&format!("2026-06-{day:02}"), &cards)
                .unwrap()
                .card_id
                .clone()
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        days.len() > 1,
        "different days should not all draw the same card"
    );
}

#[test]
fn leans_toward_new_and_struggled_cards() {
    let mut cards: Vec<Candidate> = (0..20)
        .map(|index| candidate(&format!("review-{index:02}"), 2, 5, 0))
        .collect();
    cards.push(candidate("new", 0, 0, 0));
    cards.push(candidate("forgotten", 2, 8, 3));
    cards.push(Candidate {
        difficulty: 8.5,
        ..candidate("hard", 2, 4, 0)
    });
    cards.push(candidate("relearning", RELEARNING, 3, 0));

    let mut picks = std::collections::BTreeMap::<&str, u32>::new();
    let days = 2_000;
    for day in 0..days {
        let card = choose(&format!("day-{day}"), &cards).unwrap();
        let kind = if card.card_id.starts_with("review-") {
            "review"
        } else {
            card.card_id.as_str()
        };
        *picks.entry(kind).or_default() += 1;
    }
    // Weights: 20 reviews at 1, new 3, forgotten 6, hard 3, relearning 3, of 35.
    let share = |kind: &str| f64::from(*picks.get(kind).unwrap_or(&0)) / f64::from(days);
    let review_each = share("review") / 20.0;
    assert!(share("new") > 2.0 * review_each, "{picks:?}");
    assert!(share("forgotten") > share("new"), "{picks:?}");
    assert!(share("hard") > 2.0 * review_each, "{picks:?}");
    assert!(share("relearning") > 2.0 * review_each, "{picks:?}");
    assert!(
        share("review") > 0.4,
        "ordinary reviews still come up: {picks:?}"
    );
}
