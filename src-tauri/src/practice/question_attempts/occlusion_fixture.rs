use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use image::{Rgba, RgbaImage};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::super::{render, reveal, OcclusionImageInput, RevealPracticeQuestionInput};
use crate::{
    cards::questions::{AttemptView, RevealAnswerRequest},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    practice::{
        presentation::PracticeContentDto,
        question_attempts::{read, tests::populate, AttemptIdentityInput},
        PracticeSessionDto, SubmitReviewInput,
    },
    practice_selection::start_card_practice_in_connection,
    scheduler::{insert_new_schedule, load_schedule},
    util::unix_time_millis,
};

pub(super) use crate::practice::question_attempts::tests::{id, rich};
pub(super) const SOURCE_PIXEL: [u8; 4] = [200, 20, 30, 255];
static NEXT: AtomicU64 = AtomicU64::new(1);

struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) struct Fixture {
    pub database: Database,
    pub paths: AppPaths,
    pub source_path: PathBuf,
    _cleanup: Cleanup,
}

pub(super) fn definition() -> Value {
    json!({"schemaVersion":1,"question":{
        "kind":"image_occlusion","imageAssetId":id(8),
        "masks":[
            {"id":id(20),"rect":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}},
            {"id":id(21),"rect":{"x":0.6,"y":0.6,"width":0.2,"height":0.2}}
        ],
        "questions":[
            {"id":id(30),"prompt":rich("Identify the highlighted region."),
             "answer":rich("FIRST PRIVATE ANSWER"),"explanation":null,
             "maskIds":[id(20)],"mode":"hide_all"},
            {"id":id(31),"prompt":rich("Name the second region."),
             "answer":rich("SECOND PRIVATE ANSWER"),"explanation":null,
             "maskIds":[id(21)],"mode":"hide_target"}
        ]
    }})
}

impl Fixture {
    pub fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-image-practice-{}-{}-{}",
            std::process::id(),
            unix_time_millis().unwrap(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let paths = AppPaths::new(root.clone()).unwrap();
        let database = Database::open(paths.database_path()).unwrap();
        let relative = format!("assets/images/{}.png", id(8));
        let source_path = paths.resolve_relative(&relative);
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        RgbaImage::from_pixel(20, 20, Rgba(SOURCE_PIXEL))
            .save(&source_path)
            .unwrap();
        let bytes = std::fs::metadata(&source_path).unwrap().len() as i64;
        let fixture = Self {
            database,
            paths,
            source_path,
            _cleanup: Cleanup(root),
        };
        fixture.run(|connection| {
            populate(
                connection,
                &AttemptIdentityInput {
                    session_id: id(40),
                    card_id: id(3),
                    instance_id: id(6),
                },
                false,
            )?;
            connection
                .execute_batch(
                    "DELETE FROM practice_sessions; DELETE FROM question_attempts;
                     DELETE FROM card_question_presentations;",
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE notes SET question_definition = ?1, front_text = 'Image question'
                     WHERE id = ?2",
                    params![definition().to_string(), id(2)],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO assets (
                         id, original_filename, mime_type, byte_size, relative_path, created_at
                     ) VALUES (?1, 'PRIVATE-FILENAME.png', 'image/png', ?2, ?3, 1)",
                    params![id(8), bytes, relative],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO note_assets (note_id, asset_id, side) VALUES (?1, ?2, 'content')",
                    params![id(2), id(8)],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO cards (id, note_id, deck_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 1, 1)",
                    params![id(4), id(2), id(1)],
                )
                .map_err(CommandError::database)?;
            insert_new_schedule(connection, &id(4), 1)?;
            for (card, key) in [(3, 30), (4, 31)] {
                connection
                    .execute(
                        "INSERT INTO card_question_presentations (card_id, note_id, question_key)
                         VALUES (?1, ?2, ?3)",
                        params![id(card), id(2), id(key)],
                    )
                    .map_err(CommandError::database)?;
            }
            Ok(())
        });
        fixture
    }

    pub fn run<T>(&self, operation: impl FnOnce(&mut Connection) -> Result<T, CommandError>) -> T {
        self.database.with_connection(operation).unwrap()
    }

    pub fn start(&self, both: bool) -> PracticeSessionDto {
        self.run(|connection| {
            start_card_practice_in_connection(
                connection,
                &self.paths,
                id(90),
                if both { vec![id(3), id(4)] } else { vec![id(3)] },
                false,
            )
        })
    }

    pub fn view(&self, target: &AttemptIdentityInput) -> Value {
        self.run(|connection| {
            serde_json::to_value(read(connection, target)?).map_err(CommandError::serialization)
        })
    }

    pub fn reveal_input(&self, target: &AttemptIdentityInput) -> RevealPracticeQuestionInput {
        RevealPracticeQuestionInput {
            target: target.clone(),
            request: RevealAnswerRequest {
                reveal_id: id(80),
                expected_revision: 1,
                response_time_ms: 234,
            },
        }
    }

    pub fn reveal(&self, target: &AttemptIdentityInput) -> AttemptView {
        self.run(|connection| reveal(connection, self.reveal_input(target)))
    }

    pub fn image_input(&self, target: &AttemptIdentityInput, revision: i64) -> OcclusionImageInput {
        OcclusionImageInput {
            target: target.clone(),
            expected_revision: revision,
            maximum_edge: 20,
        }
    }

    pub fn raster(
        &self,
        target: &AttemptIdentityInput,
        revision: i64,
    ) -> Result<Vec<u8>, CommandError> {
        render(
            &self.database,
            &self.paths,
            &self.image_input(target, revision),
        )
    }

    pub fn image(&self, target: &AttemptIdentityInput, revision: i64) -> RgbaImage {
        image::load_from_memory(&self.raster(target, revision).unwrap())
            .unwrap()
            .into_rgba8()
    }

    pub fn review(&self, target: &AttemptIdentityInput, outcome: &str) -> SubmitReviewInput {
        self.run(|connection| {
            Ok(SubmitReviewInput {
                session_id: target.session_id.clone(),
                card_id: target.card_id.clone(),
                submission_id: id(81),
                outcome: outcome.to_string(),
                response_time_ms: 234,
                schedule_revision: load_schedule(connection, &target.card_id)?.revision,
            })
        })
    }

    pub fn counts(&self) -> (i64, i64, i64) {
        self.run(|connection| {
            connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM review_events),
                            (SELECT COUNT(*) FROM question_review_links),
                            (SELECT reps FROM card_schedules WHERE card_id = ?1)",
                    [id(3)],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)
        })
    }
}

pub(super) fn target(session: &PracticeSessionDto, index: usize) -> AttemptIdentityInput {
    let presentation = &session.first_window.cards[index].presentation;
    let PracticeContentDto::Interactive { question } = &presentation.content else {
        panic!("Occlusion must use the captured question route");
    };
    AttemptIdentityInput {
        session_id: session.id.clone(),
        card_id: presentation.id.clone(),
        instance_id: question.attempt.instance_id.clone(),
    }
}
