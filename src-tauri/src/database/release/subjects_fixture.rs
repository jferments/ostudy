//! A schema-5 library that exercises every filing and copy-merging choice
//! release migration 6 makes. Shared by the migration, preservation, analytics
//! and sync tests, so each checks the same learner history.
//!
//! The rows use only tables every release schema from 1 to 5 has, so the same
//! library can be seeded at an older schema and upgraded through several
//! releases.

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::cards::questions::{
    ChoiceOption, DisplayOrder, InstanceIdentity, QuestionAttempt, QuestionBody,
    QuestionDefinition, QuestionInstance, ValidatedQuestion, QUESTION_SCHEMA_VERSION,
};

use super::{write_release_ledger, BASELINE_SQL, RELEASE_MIGRATIONS};

pub(crate) const T0: i64 = 1_760_000_000_000;
pub(crate) const DAY: i64 = 86_400_000;
/// A folder created before folder ids were checked; it merges by name.
pub(crate) const LEGACY_ALGEBRA_FOLDER: &str = "legacy-algebra";
/// A folder with a short id and no matching collection.
pub(crate) const LEGACY_GEOGRAPHY_FOLDER: &str = "geo";
/// The subject id migration 6 derives from [`LEGACY_GEOGRAPHY_FOLDER`].
pub(crate) const GEOGRAPHY_SUBJECT: &str = "67656f00000000000000000000000000";

/// A release library at `version` with its ledger, built the way each release
/// left it. The caller seeds rows and then upgrades it.
pub(crate) fn release_library(version: i64) -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    let transaction = connection.transaction().unwrap();
    transaction.execute_batch(BASELINE_SQL).unwrap();
    write_release_ledger(&transaction).unwrap();
    for migration in RELEASE_MIGRATIONS.iter().filter(|m| m.version <= version) {
        transaction.execute_batch(migration.sql).unwrap();
        transaction
            .execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, 1)",
                params![migration.version, migration.name, migration.checksum],
            )
            .unwrap();
    }
    transaction
        .pragma_update(None, "user_version", version)
        .unwrap();
    transaction.commit().unwrap();
    connection
}

/// Apply the bundled release migrations after `from`, up to `to`, to a
/// file-backed library exactly as an older build did, without running the
/// current startup path.
pub(crate) fn advance_release_library(connection: &Connection, from: i64, to: i64) {
    for migration in RELEASE_MIGRATIONS
        .iter()
        .filter(|m| m.version > from && m.version <= to)
    {
        connection.execute_batch(migration.sql).unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, 1)",
                params![migration.version, migration.name, migration.checksum],
            )
            .unwrap();
    }
    connection.pragma_update(None, "user_version", to).unwrap();
}

/// One learner note and its cards, in template order (forward, then reverse).
#[derive(Clone, Debug)]
pub(crate) struct Note {
    pub(crate) id: String,
    pub(crate) cards: Vec<String>,
}

impl Note {
    pub(crate) fn card(&self) -> &str {
        &self.cards[0]
    }
}

/// Every id the tests check. Field names follow the fixture's story: `o_*`
/// are original notes, `c*` are collection copies, `k_*` decks, `b_*`
/// study-bundle decks, `d_*` documents, `f_*` folders and `c_*` collections.
#[derive(Clone, Debug)]
pub(crate) struct SubjectsLibrary {
    pub(crate) f_bio: String,
    pub(crate) f_chem: String,
    pub(crate) f_hist_upper: String,
    pub(crate) f_hist_lower: String,
    pub(crate) c_bio: String,
    pub(crate) c_phys: String,
    pub(crate) c_hist: String,
    pub(crate) c_unused: String,
    pub(crate) c_algebra: String,
    pub(crate) k_bio: String,
    pub(crate) k_mech: String,
    pub(crate) k_acids: String,
    pub(crate) k_optics: String,
    pub(crate) k_dates: String,
    pub(crate) k_misc: String,
    pub(crate) k_merged: String,
    pub(crate) k_geo: String,
    pub(crate) k_algebra: String,
    pub(crate) k_deleted: String,
    pub(crate) k_arch: String,
    pub(crate) b_bio: String,
    pub(crate) b_phys: String,
    pub(crate) b_unused: String,
    pub(crate) b_shared: String,
    pub(crate) d_cells: String,
    pub(crate) d_mech: String,
    pub(crate) d_acids: String,
    pub(crate) d_optics: String,
    pub(crate) d_loose: String,
    pub(crate) d_unfiled: String,
    /// The two page ranges set on Mechanics in Physics, which it stays in.
    pub(crate) kept_ranges: Vec<String>,
    /// The page range set on Mechanics in Biology, which loses it.
    pub(crate) dropped_range: String,
    /// The page range set on Cells in Biology.
    pub(crate) cells_range: String,
    pub(crate) o_bio: Vec<Note>,
    pub(crate) o_mech: Vec<Note>,
    pub(crate) o_opt: Vec<Note>,
    pub(crate) o_arch: Note,
    pub(crate) o_dates: Note,
    pub(crate) o_misc: Note,
    pub(crate) q_orig: Note,
    /// Copies in the Biology bundle of `o_bio[i]`.
    pub(crate) cb: Vec<Note>,
    /// Copies of `o_mech[i]` in the Biology bundle.
    pub(crate) cm_bio: Vec<Note>,
    /// Copies of `o_mech[i]` in the Physics bundle.
    pub(crate) cm_phys: Vec<Note>,
    /// Copies of `o_opt[i]` in the Physics bundle.
    pub(crate) co: Vec<Note>,
    pub(crate) cq: Note,
    pub(crate) ghost: Note,
    pub(crate) carch: Note,
    pub(crate) amb: Note,
    pub(crate) exam_tag: String,
    pub(crate) session: String,
    pub(crate) attempt: String,
    /// Every review event, keyed by the card reviewed, in insertion order.
    pub(crate) reviews: Vec<(String, String)>,
    pub(crate) copy_source: String,
    pub(crate) original_source: String,
    pub(crate) copy_checkpoint: String,
    pub(crate) original_checkpoint: String,
}

impl SubjectsLibrary {
    /// Copy notes that migration 6 retires into their originals.
    pub(crate) fn retired_notes(&self) -> Vec<&Note> {
        let mut notes = vec![&self.cb[0], &self.cb[1], &self.cq, &self.co[0]];
        notes.extend(self.cm_bio.iter());
        notes.extend(self.cm_phys.iter());
        notes
    }

    /// (retired card, surviving card) for every merge, sorted by retired card.
    pub(crate) fn expected_merges(&self) -> Vec<(String, String)> {
        let mut merges = vec![
            (
                self.cb[0].card().to_string(),
                self.o_bio[0].card().to_string(),
            ),
            (
                self.cb[1].card().to_string(),
                self.o_bio[1].card().to_string(),
            ),
            (self.cq.card().to_string(), self.q_orig.card().to_string()),
            (
                self.co[0].card().to_string(),
                self.o_opt[0].card().to_string(),
            ),
        ];
        for (copies, originals) in [(&self.cm_bio, &self.o_mech), (&self.cm_phys, &self.o_mech)] {
            for (copy, original) in copies.iter().zip(originals) {
                merges.push((copy.card().to_string(), original.card().to_string()));
            }
        }
        merges.sort();
        merges
    }
}

struct Builder<'a> {
    connection: &'a Connection,
    next: u32,
    reviews: Vec<(String, String)>,
}

fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

impl Builder<'_> {
    fn id(&mut self, kind: u8) -> String {
        self.next += 1;
        format!("{kind:02x}{:030x}", self.next)
    }

    fn exec(&self, sql: &str, values: impl rusqlite::Params) {
        self.connection
            .execute(sql, values)
            .unwrap_or_else(|error| panic!("{error}: {sql}"));
    }

    fn folder(&mut self, id: Option<&str>, name: &str, order: i64) -> String {
        let id = id.map(str::to_string).unwrap_or_else(|| self.id(0x0f));
        self.exec(
            "INSERT INTO deck_folders(id, name, sort_order, created_at, updated_at, revision)
             VALUES (?1, ?2, ?3, ?4, ?4, 2)",
            params![id, name, order, T0],
        );
        id
    }

    fn collection(&mut self, name: &str, order: i64) -> String {
        let id = self.id(0xc0);
        self.exec(
            "INSERT INTO document_collections(id, parent_id, name, sort_order, created_at,
                                              updated_at, revision)
             VALUES (?1, NULL, ?2, ?3, ?4, ?4, 3)",
            params![id, name, order, T0 + 1],
        );
        id
    }

    fn deck(&mut self, name: &str, folder: Option<&str>) -> String {
        let id = self.id(0xde);
        self.exec(
            "INSERT INTO decks(id, name, description, sort_order, created_at, updated_at,
                               revision, folder_id)
             VALUES (?1, ?2, '', 0, ?3, ?3, 1, ?4)",
            params![id, name, T0, folder],
        );
        id
    }

    fn document(&mut self, title: &str, deck: Option<&str>, at: i64) -> String {
        let id = self.id(0xd0);
        self.exec(
            "INSERT INTO documents(id, title, original_filename, mime_type, byte_size, sha256,
                                   source_relative_path, lifecycle_state, created_at,
                                   updated_at, generated_deck_id)
             VALUES (?1, ?2, ?3, 'application/pdf', 10, ?4, ?5, 'ready', ?6, ?6, ?7)",
            params![
                id,
                title,
                format!("{title}.pdf"),
                format!("{id}{id}"),
                format!("documents/{id}.pdf"),
                at,
                deck
            ],
        );
        if let Some(deck) = deck {
            self.link(&id, deck, at);
        }
        id
    }

    /// A generation job that put this document's cards in `deck`.
    fn link(&mut self, document: &str, deck: &str, at: i64) {
        let job = self.id(0x10);
        self.exec(
            "INSERT INTO document_generation_jobs(id, document_id, provider_kind, model_id,
                 profile_revision, credential_epoch, network_scope, state, page_count,
                 created_at, updated_at, generated_deck_id)
             VALUES (?1, ?2, 'local', 'model', 1, 0, 'local', 'succeeded', 1, ?3, ?3, ?4)",
            params![job, document, at, deck],
        );
        self.exec(
            "INSERT INTO document_decks(document_id, deck_id, job_id, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![document, deck, job, at],
        );
    }

    fn member(&mut self, collection: &str, document: &str, at: i64) -> String {
        let id = self.id(0x3e);
        self.exec(
            "INSERT INTO document_collection_memberships(id, collection_id, document_id,
                 sort_order, created_at, updated_at, revision)
             VALUES (?1, ?2, ?3, 0, ?4, ?4, 1)",
            params![id, collection, document, at],
        );
        id
    }

    fn range(&mut self, membership: &str, start: i64, end: i64) -> String {
        let id = self.id(0x7a);
        self.exec(
            "INSERT INTO document_collection_ranges(id, membership_id, unit_kind, start_unit,
                 end_unit, source_sha256, created_at)
             VALUES (?1, ?2, 'page', ?3, ?4, ?5, ?6)",
            params![id, membership, start, end, "c".repeat(64), T0 + 20],
        );
        id
    }

    fn note(&mut self, deck: &str, text: &str, at: i64, reverse: bool) -> Note {
        let id = self.id(0x40);
        let document = rich(text).to_string();
        let answer = rich("answer").to_string();
        self.exec(
            "INSERT INTO notes(id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text, created_at, updated_at,
                 revision, reverse_enabled)
             VALUES (?1, ?2, 'basic', 'tiptap-json', 1, ?3, ?4, ?5, 'answer', ?6, ?6, 1, ?7)",
            params![id, deck, document, answer, text, at, reverse],
        );
        let mut cards = vec![self.card(&id, deck, "forward", at)];
        if reverse {
            cards.push(self.card(&id, deck, "reverse", at));
        }
        Note { id, cards }
    }

    fn interactive(&mut self, deck: &str, definition: &str, at: i64) -> Note {
        let id = self.id(0x41);
        let question = ValidatedQuestion::parse(definition).unwrap();
        let search = question.search_text().unwrap();
        self.exec(
            "INSERT INTO notes(id, deck_id, note_type, content_format, content_schema_version,
                 question_definition, front_text, back_text, created_at, updated_at, revision)
             VALUES (?1, ?2, 'interactive', 'question-json', 1, ?3, ?4, ?5, ?6, ?6, 1)",
            params![id, deck, definition, search.prompt, search.answer, at],
        );
        let card = self.card(&id, deck, "forward", at);
        self.exec(
            "INSERT INTO card_question_presentations(card_id, note_id, question_key)
             VALUES (?1, ?2, 'main')",
            params![card, id],
        );
        Note {
            id,
            cards: vec![card],
        }
    }

    /// A card with the schedule `scheduler.rs` gives a new card, whose
    /// last_review_at is the creation time.
    fn card(&mut self, note: &str, deck: &str, direction: &str, at: i64) -> String {
        let id = self.id(0xca);
        self.exec(
            "INSERT INTO cards(id, note_id, deck_id, direction, template_ordinal, created_at,
                               updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
            params![id, note, deck, direction, at],
        );
        self.exec(
            "INSERT INTO card_schedules(card_id, state, due_at, stability, difficulty,
                 elapsed_days, scheduled_days, reps, lapses, last_review_at, scheduler_name,
                 scheduler_version, updated_at, revision)
             VALUES (?1, 0, ?2, 0.0, 0.0, 0, 0, 0, 0, ?2, 'fsrs', 'rs-fsrs-1.2.1', ?2, 1)",
            params![id, at],
        );
        id
    }

    /// A review with its attribution, and the schedule the review produced.
    fn review(&mut self, card: &str, at: i64, session: Option<&str>) -> String {
        let id = self.id(0xee);
        let submission = self.id(0x5b);
        let n = self.reviews.len() as f64;
        let (note, deck, deck_name, direction): (String, String, String, String) = self
            .connection
            .query_row(
                "SELECT c.note_id, c.deck_id, d.name, c.direction
                 FROM cards c JOIN decks d ON d.id = c.deck_id WHERE c.id = ?1",
                [card],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        let due = at + 3 * DAY;
        let stability = 3.0 + n / 10.0;
        self.exec(
            "INSERT INTO review_events(id, submission_id, session_id, card_id, reviewed_at,
                 rating, answer_correct, confidence, response_time_ms, state_before,
                 state_after, due_before, due_after, stability_before, stability_after,
                 difficulty_before, difficulty_after, scheduled_days_before,
                 scheduled_days_after, scheduler_name, scheduler_version, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 3, 1, 'somewhat_confident', 1000, 0, 2, ?5, ?6,
                     0.0, ?7, 0.0, 5.0, 0, 3, 'fsrs', 'rs-fsrs-1.2.1', ?5)",
            params![id, submission, session, card, at, due, stability],
        );
        self.exec(
            "INSERT INTO review_event_attributions(review_event_id, reviewed_at,
                 review_local_date, time_zone_id, utc_offset_minutes, card_id, note_id,
                 note_revision, owner_deck_id_at_review, captured_deck_name, direction,
                 card_type, card_label, presentation_variant, question_excerpt,
                 question_text_hash, question_was_truncated, attribution_source)
             VALUES (?1, ?2, date(?2 / 1000, 'unixepoch'), 'UTC', 0, ?3, ?4, 1, ?5, ?6, ?7,
                     'basic', 'Basic', ?7, 'question', ?8, 0, 'captured')",
            params![
                id,
                at,
                card,
                note,
                deck,
                deck_name,
                direction,
                "0".repeat(64)
            ],
        );
        self.exec(
            "UPDATE card_schedules
             SET state = 2, due_at = ?2, stability = ?3, difficulty = 5.0, scheduled_days = 3,
                 reps = reps + 1, last_review_at = ?4, updated_at = ?4, revision = revision + 1
             WHERE card_id = ?1",
            params![card, due, stability, at],
        );
        self.reviews.push((card.to_string(), id.clone()));
        id
    }

    /// A reset recorded the way schedule_controls.rs records one.
    fn reset(&mut self, card: &str, at: i64) {
        let operation = self.id(0x5a);
        let event = self.id(0x5e);
        self.exec(
            "INSERT INTO schedule_action_operations(id, action, requested_count, changed_count,
                 unchanged_count, rejected_count, occurred_at, created_at)
             VALUES (?1, 'reset', 1, 1, 0, 0, ?2, ?2)",
            params![operation, at],
        );
        let (state, due, revision): (i64, i64, i64) = self
            .connection
            .query_row(
                "SELECT state, due_at, revision FROM card_schedules WHERE card_id = ?1",
                [card],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        self.exec(
            "INSERT INTO schedule_action_events(id, operation_id, card_id, action, due_before,
                 due_after, state_before, state_after, schedule_revision_before,
                 schedule_revision_after, occurred_at, created_at)
             VALUES (?1, ?2, ?3, 'reset', ?4, ?5, ?6, 0, ?7, ?8, ?5, ?5)",
            params![
                event,
                operation,
                card,
                due,
                at,
                state,
                revision,
                revision + 1
            ],
        );
        self.exec(
            "UPDATE card_schedules
             SET state = 0, due_at = ?2, stability = 0.0, difficulty = 0.0, scheduled_days = 0,
                 reps = 0, lapses = 0, last_review_at = ?2, updated_at = ?2,
                 revision = revision + 1
             WHERE card_id = ?1",
            params![card, at],
        );
    }

    fn target(&mut self, collection: &str, deck: &str, at: i64) -> String {
        let id = self.id(0x7e);
        self.exec(
            "INSERT INTO study_targets(id, target_kind, collection_id, deck_id, created_at,
                                       updated_at)
             VALUES (?1, 'collection', ?2, ?3, ?4, ?4)",
            params![id, collection, deck, at],
        );
        id
    }

    fn origin(&mut self, target: &str, copy: &Note, document: &str, source: &str, at: i64) {
        self.exec(
            "INSERT INTO study_target_card_origins(target_id, target_note_id,
                 source_document_id, source_note_id, source_note_revision,
                 source_identity_sha256, created_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
            params![target, copy.id, document, source, "f".repeat(64), at],
        );
    }

    /// The bundle copy composition.rs made: a new note at revision 1 with fresh
    /// cards and schedules, and its origin row.
    fn copy(&mut self, target: &str, bundle: &str, source: &Note, document: &str, at: i64) -> Note {
        let (text, reverse): (String, bool) = self
            .connection
            .query_row(
                "SELECT front_text, reverse_enabled FROM notes WHERE id = ?1",
                [&source.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let copy = self.note(bundle, &text, at, reverse);
        self.origin(target, &copy, document, &source.id, at);
        copy
    }
}

fn question_definition() -> String {
    let option_a = "0a".repeat(16);
    let option_b = "0b".repeat(16);
    ValidatedQuestion::validate(QuestionDefinition {
        schema_version: QUESTION_SCHEMA_VERSION,
        question: QuestionBody::SingleChoice {
            prompt: rich("Which organelle makes ATP?"),
            options: vec![
                ChoiceOption {
                    id: option_a.clone(),
                    content: rich("Mitochondrion"),
                    rationale: None,
                },
                ChoiceOption {
                    id: option_b,
                    content: rich("Ribosome"),
                    rationale: None,
                },
            ],
            correct_option_id: option_a,
            explanation: None,
        },
    })
    .unwrap()
    .to_json()
    .unwrap()
}

/// Seed the subjects story into a release library at schema 1 to 5.
pub(crate) fn seed(connection: &Connection) -> SubjectsLibrary {
    let mut b = Builder {
        connection,
        next: 0,
        reviews: Vec::new(),
    };
    b.exec(
        "INSERT OR REPLACE INTO library_organization(singleton_id, revision, updated_at)
         VALUES (1, 7, ?1)",
        [T0],
    );
    b.exec(
        "INSERT OR REPLACE INTO document_collection_organization(singleton_id, revision,
                                                                 updated_at)
         VALUES (1, 3, ?1)",
        [T0],
    );

    // Folders. "History" and "history" share a name, so neither merges with
    // the History collection. Two legacy folders have ids that are not 32
    // characters long.
    let f_bio = b.folder(None, "Biology", 1000);
    let f_chem = b.folder(None, "Chemistry", 2000);
    let f_hist_upper = b.folder(None, "History", 3000);
    let f_hist_lower = b.folder(None, "history", 4000);
    b.folder(Some(LEGACY_ALGEBRA_FOLDER), "  Algebra ", 5000);
    b.folder(Some(LEGACY_GEOGRAPHY_FOLDER), "Geography", 6000);

    let c_bio = b.collection(" biology", 1000);
    let c_phys = b.collection("Physics", 2000);
    let c_hist = b.collection("History", 3000);
    let c_unused = b.collection("Unused", 4000);
    let c_algebra = b.collection("algebra", 5000);

    let k_bio = b.deck("Cell biology", Some(&f_bio));
    let k_mech = b.deck("Mechanics notes", Some(&f_chem));
    let k_acids = b.deck("Acids", Some(&f_chem));
    let k_optics = b.deck("Optics", None);
    let k_dates = b.deck("Dates", Some(&f_hist_upper));
    let k_misc = b.deck("Misc", None);
    let k_merged = b.deck("Merged deck", Some(&f_bio));
    let k_geo = b.deck("Maps", Some(LEGACY_GEOGRAPHY_FOLDER));
    let k_algebra = b.deck("Equations", Some(LEGACY_ALGEBRA_FOLDER));
    let k_deleted = b.deck("Old deck", Some(&f_chem));
    let k_arch = b.deck("Archived originals", Some(&f_bio));
    let b_bio = b.deck("biology Study Bundle", None);
    let b_phys = b.deck("Physics Study Bundle", Some(&f_chem));
    let b_unused = b.deck("Unused Study Bundle", None);
    let b_shared = b.deck("Shared Study Bundle", None);

    let d_cells = b.document("Cells", Some(&k_bio), T0 + 1);
    let d_mech = b.document("Mechanics", Some(&k_mech), T0 + 2);
    let d_acids = b.document("Acids", Some(&k_acids), T0 + 3);
    let d_optics = b.document("Optics", Some(&k_optics), T0 + 4);
    let d_loose = b.document("Loose", None, T0 + 5);
    let d_unfiled = b.document("Unfiled", None, T0 + 6);
    // Deck merging moved two documents' links onto one deck.
    b.link(&d_cells, &k_merged, T0 + 7);
    b.link(&d_optics, &k_merged, T0 + 8);
    b.link(&d_loose, &k_deleted, T0 + 9);

    let cells_member = b.member(&c_bio, &d_cells, T0 + 10);
    // Mechanics joined Physics first, then Biology.
    let mech_physics = b.member(&c_phys, &d_mech, T0 + 11);
    let mech_biology = b.member(&c_bio, &d_mech, T0 + 12);
    b.member(&c_phys, &d_optics, T0 + 13);
    b.member(&c_hist, &d_loose, T0 + 14);
    let cells_range = b.range(&cells_member, 1, 4);
    let kept_ranges = vec![b.range(&mech_physics, 1, 3), b.range(&mech_physics, 5, 6)];
    let dropped_range = b.range(&mech_biology, 2, 4);

    let o_bio: Vec<Note> = (0..5)
        .map(|i| b.note(&k_bio, &format!("bio {i}"), T0, i == 4))
        .collect();
    let o_mech: Vec<Note> = (0..3)
        .map(|i| b.note(&k_mech, &format!("mech {i}"), T0, false))
        .collect();
    let o_opt: Vec<Note> = (0..2)
        .map(|i| b.note(&k_optics, &format!("optics {i}"), T0, false))
        .collect();
    let o_arch = b.note(&k_arch, "archived original", T0, false);
    let o_dates = b.note(&k_dates, "1066", T0, false);
    let o_misc = b.note(&k_misc, "misc", T0, false);
    let definition = question_definition();
    let q_orig = b.interactive(&k_bio, &definition, T0);

    let t_bio = b.target(&c_bio, &b_bio, T0 + DAY - 10);
    let t_phys = b.target(&c_phys, &b_phys, T0 + DAY - 9);
    b.target(&c_unused, &b_unused, T0 + DAY - 8);
    let t_hist = b.target(&c_hist, &b_shared, T0 + DAY - 7);
    let t_algebra = b.target(&c_algebra, &b_shared, T0 + DAY - 6);

    let at = T0 + DAY;
    let cb: Vec<Note> = o_bio
        .iter()
        .map(|original| b.copy(&t_bio, &b_bio, original, &d_cells, at))
        .collect();
    let cm_bio: Vec<Note> = o_mech
        .iter()
        .map(|original| b.copy(&t_bio, &b_bio, original, &d_mech, at))
        .collect();
    let cm_phys: Vec<Note> = o_mech
        .iter()
        .map(|original| b.copy(&t_phys, &b_phys, original, &d_mech, at))
        .collect();
    let co: Vec<Note> = o_opt
        .iter()
        .map(|original| b.copy(&t_phys, &b_phys, original, &d_optics, at))
        .collect();
    let cq = b.interactive(&b_bio, &definition, at);
    b.origin(&t_bio, &cq, &d_cells, &q_orig.id, at);
    // A copy whose original was purged; its origin names a missing note.
    let ghost = b.note(&b_bio, "copy of a purged note", at, false);
    b.origin(&t_bio, &ghost, &d_cells, &"9".repeat(32), at);
    let carch = b.copy(&t_bio, &b_bio, &o_arch, &d_cells, at);
    // One copy note that two collections both claim, from different originals.
    let amb = b.note(&b_shared, "1066", at, false);
    b.origin(&t_hist, &amb, &d_loose, &o_dates.id, at);
    b.origin(&t_algebra, &amb, &d_loose, &o_misc.id, at);

    // Learner changes after the copies were made.
    let exam_tag = b.id(0x7a);
    b.exec(
        "INSERT INTO tags(id, name, normalized_name, created_at, updated_at)
         VALUES (?1, 'exam', 'exam', ?2, ?2)",
        params![exam_tag, T0],
    );
    // Tagging does not change a note's revision, so this copy is unedited.
    b.exec(
        "INSERT INTO note_tags(note_id, tag_id) VALUES (?1, ?2)",
        params![cb[1].id, exam_tag],
    );
    b.exec(
        "UPDATE notes SET revision = 2, front_text = 'edited copy', updated_at = ?2
         WHERE id = ?1",
        params![cb[2].id, at + 5],
    );
    b.exec(
        "UPDATE notes SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![o_bio[3].id, at + 6],
    );
    b.exec(
        "UPDATE cards SET deleted_at = ?2 WHERE id = ?1",
        params![o_bio[4].cards[1], at + 7],
    );
    b.exec(
        "UPDATE notes SET deck_id = ?2 WHERE id = ?1",
        params![co[0].id, k_misc],
    );
    b.exec(
        "UPDATE cards SET deck_id = ?2 WHERE note_id = ?1",
        params![co[0].id, k_misc],
    );
    b.exec(
        "UPDATE notes SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![co[1].id, at + 8],
    );
    b.exec(
        "UPDATE cards SET deleted_at = ?2, updated_at = ?2 WHERE note_id = ?1",
        params![co[1].id, at + 8],
    );
    b.exec(
        "UPDATE decks SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![k_deleted, at + 9],
    );
    b.exec(
        "UPDATE decks SET archived_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![k_arch, at + 10],
    );

    // One session studied an original, its copy and an interactive copy.
    let session = b.id(0x5e);
    b.exec(
        "INSERT INTO practice_sessions(id, selected_deck_ids_json, shuffle, card_count,
                                       started_at)
         VALUES (?1, ?2, 0, 3, ?3)",
        params![session, json!([k_bio, b_bio]).to_string(), T0 + 2 * DAY],
    );
    for (position, card) in [o_bio[0].card(), cb[0].card(), cq.card()]
        .into_iter()
        .enumerate()
    {
        b.exec(
            "INSERT INTO practice_session_cards(session_id, card_id, position)
             VALUES (?1, ?2, ?3)",
            params![session, card, position as i64],
        );
    }

    // Reviews. The copy studied last decides the survivor's schedule.
    b.review(o_bio[0].card(), T0 + 2 * DAY, Some(&session));
    b.review(cb[0].card(), T0 + 2 * DAY + 60_000, Some(&session));
    b.review(o_bio[1].card(), T0 + 5 * DAY, None);
    b.review(o_mech[0].card(), T0 + 5 * DAY, None);
    b.review(cm_bio[0].card(), T0 + 2 * DAY, None);
    b.review(cm_phys[0].card(), T0 + 7 * DAY, None);
    // Studied before the copies existed: a copy's new schedule stores its
    // creation time as last_review_at, which must not make it look newer.
    b.review(o_mech[1].card(), T0 + DAY - 3_600_000, None);
    // A reset after the copy's last review keeps the original's schedule.
    b.review(o_mech[2].card(), T0 + 3 * DAY, None);
    b.review(cm_phys[2].card(), T0 + 4 * DAY, None);
    b.reset(o_mech[2].card(), T0 + 6 * DAY);
    b.review(co[0].card(), T0 + 3 * DAY, None);
    let question_review = b.review(cq.card(), T0 + 2 * DAY + 120_000, Some(&session));
    b.review(cb[2].card(), T0 + 4 * DAY, None);

    let attempt = b.id(0xa7);
    let instance = QuestionInstance::capture(
        InstanceIdentity {
            instance_id: attempt.clone(),
            note_id: cq.id.clone(),
            card_id: cq.card().to_string(),
            note_revision: 1,
        },
        ValidatedQuestion::parse(&definition).unwrap(),
        "main".to_string(),
        DisplayOrder {
            item_ids: vec!["0a".repeat(16), "0b".repeat(16)],
            target_ids: Vec::new(),
        },
    )
    .unwrap();
    let started = QuestionAttempt::start(instance).unwrap();
    b.exec(
        "INSERT INTO question_attempts(instance_id, session_id, card_id, note_id,
             question_kind, instance_hash, revision, phase, assisted, checkpoint_json,
             created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'single_choice', ?5, 1, 'answering', 0, ?6, ?7, ?7)",
        params![
            attempt,
            session,
            cq.card(),
            cq.id,
            started.instance().fingerprint(),
            started.to_json().unwrap(),
            T0 + 2 * DAY
        ],
    );
    b.exec(
        "INSERT INTO question_review_links(review_event_id, instance_id)
         VALUES (?1, ?2)",
        params![question_review, attempt],
    );

    // The Tutor indexed one copy card and one original card. Retiring the
    // copy deactivates its source and drops the guide checkpoint built on it.
    let copy_source = b.id(0x50);
    let original_source = b.id(0x50);
    for (source, card) in [(&copy_source, &cb[0]), (&original_source, &o_bio[1])] {
        b.exec(
            "INSERT INTO knowledge_sources(id, kind, owner_id, segment_key, parent_key,
                 owner_revision, content_hash, record_json, active, note_id, card_id,
                 extraction_contract, extracted_hash, created_at, updated_at)
             VALUES (?1, 'card', ?2, 'card', 'card', '1', ?3, '{}', 1, ?4, ?2, ?5, ?6, ?7, ?7)",
            params![
                source,
                card.card(),
                "1".repeat(64),
                card.id,
                "2".repeat(64),
                "3".repeat(64),
                T0 + 3 * DAY
            ],
        );
    }
    let copy_checkpoint = "4".repeat(64);
    let original_checkpoint = "5".repeat(64);
    for (checkpoint, source) in [
        (&copy_checkpoint, &copy_source),
        (&original_checkpoint, &original_source),
    ] {
        b.exec(
            "INSERT INTO collection_guide_checkpoints(cache_key, collection_id, contract_hash,
                 result_json, result_hash, created_at)
             VALUES (?1, ?2, ?3, '{}', ?3, ?4)",
            params![checkpoint, c_bio, "6".repeat(64), T0 + 3 * DAY],
        );
        b.exec(
            "INSERT INTO collection_guide_checkpoint_sources(cache_key, source_id)
             VALUES (?1, ?2)",
            params![checkpoint, source],
        );
    }
    for (collection, stale) in [(&c_bio, 0), (&c_phys, 1)] {
        b.exec(
            "INSERT INTO knowledge_collection_views(collection_id, snapshot_id, analysis_hash,
                 view_version, synthesis_json, reduced, stale, created_at, updated_at)
             VALUES (?1, ?2, ?2, 1, '{}', 0, ?3, ?4, ?4)",
            params![collection, "7".repeat(64), stale, T0 + 3 * DAY],
        );
    }

    let reviews = b.reviews;
    SubjectsLibrary {
        f_bio,
        f_chem,
        f_hist_upper,
        f_hist_lower,
        c_bio,
        c_phys,
        c_hist,
        c_unused,
        c_algebra,
        k_bio,
        k_mech,
        k_acids,
        k_optics,
        k_dates,
        k_misc,
        k_merged,
        k_geo,
        k_algebra,
        k_deleted,
        k_arch,
        b_bio,
        b_phys,
        b_unused,
        b_shared,
        d_cells,
        d_mech,
        d_acids,
        d_optics,
        d_loose,
        d_unfiled,
        kept_ranges,
        dropped_range,
        cells_range,
        o_bio,
        o_mech,
        o_opt,
        o_arch,
        o_dates,
        o_misc,
        q_orig,
        cb,
        cm_bio,
        cm_phys,
        co,
        cq,
        ghost,
        carch,
        amb,
        exam_tag,
        session,
        attempt,
        reviews,
        copy_source,
        original_source,
        copy_checkpoint,
        original_checkpoint,
    }
}
