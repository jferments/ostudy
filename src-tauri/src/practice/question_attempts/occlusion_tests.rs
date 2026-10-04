use serde_json::json;

use super::*;
use crate::practice::{record_practice_event, submit_review_with_question};

#[path = "occlusion_fixture.rs"]
mod fixture;
use fixture::{definition, id, rich, target, Fixture, SOURCE_PIXEL};

fn error_code<T>(result: Result<T, CommandError>) -> String {
    result.err().expect("operation should fail").code
}

fn review_context(view: &AttemptView) -> QuestionReviewContext {
    QuestionReviewContext {
        instance_id: view.instance_id.clone(),
        attempt_revision: view.revision,
        override_reason: None,
    }
}

#[test]
fn real_session_capture_retains_independent_targets_without_original_image_metadata() {
    let fixture = Fixture::new();
    let session = fixture.start(true);
    assert_eq!(session.card_count, 2);
    let first = target(&session, 0);
    let second = target(&session, 1);
    assert_ne!(first.instance_id, second.instance_id);
    assert_ne!(first.card_id, second.card_id);
    let public = serde_json::to_string(&session).unwrap();
    for forbidden in ["PRIVATE", "imageAssetId", "frontDoc", "backDoc"] {
        assert!(!public.contains(forbidden));
    }
    assert!(!public.contains(&id(8)));
    fixture.run(|connection| {
        let retained: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM question_attempt_assets WHERE asset_id = ?1",
                [id(8)],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        assert_eq!(retained, 2);
        Ok(())
    });
    let first_pixels = fixture.image(&first, 1);
    let second_pixels = fixture.image(&second, 1);
    assert_ne!(first_pixels.get_pixel(4, 4).0, SOURCE_PIXEL);
    assert_ne!(first_pixels.get_pixel(14, 14).0, SOURCE_PIXEL);
    assert_eq!(second_pixels.get_pixel(4, 4).0, SOURCE_PIXEL);
    assert_ne!(second_pixels.get_pixel(14, 14).0, SOURCE_PIXEL);
    assert_eq!(fixture.counts(), (0, 0, 0));
}

#[test]
fn native_reveal_uncovers_the_image_and_replays_without_a_second_mutation() {
    let fixture = Fixture::new();
    let session = fixture.start(false);
    let target = target(&session, 0);
    let hidden = fixture.image(&target, 1);
    assert_ne!(hidden.get_pixel(4, 4).0, SOURCE_PIXEL);
    assert_ne!(hidden.get_pixel(14, 14).0, SOURCE_PIXEL);
    let revealed = fixture.reveal(&target);
    let visible = fixture.image(&target, revealed.revision);
    assert!(visible.pixels().all(|pixel| pixel.0 == SOURCE_PIXEL));
    let view = fixture.view(&target);
    assert_eq!(view["attempt"]["progress"]["phase"], "revealed");
    assert!(view["attempt"]["progress"].get("evaluation").is_none());
    assert!(view.to_string().contains("FIRST PRIVATE ANSWER"));
    assert!(!view.to_string().contains("SECOND PRIVATE ANSWER"));
    fixture.run(|connection| {
        let before = connection.total_changes();
        let replay = reveal(connection, fixture.reveal_input(&target))?;
        assert_eq!(connection.total_changes(), before);
        assert_eq!(
            serde_json::to_value(replay).unwrap(),
            serde_json::to_value(&revealed).unwrap()
        );
        Ok(())
    });
    assert_eq!(fixture.counts(), (0, 0, 0));
}

#[test]
fn self_rating_updates_fsrs_once_and_does_not_invent_an_objective_verdict() {
    for (outcome, correct) in [("wrong", 0), ("somewhat_confident", 1)] {
        let fixture = Fixture::new();
        let session = fixture.start(false);
        let target = target(&session, 0);
        let revealed = fixture.reveal(&target);
        let input = fixture.review(&target, outcome);
        fixture.run(|connection| {
            let result = submit_review_with_question(
                connection,
                input.clone(),
                Some(review_context(&revealed)),
            )?;
            assert_eq!(result.correct_count, correct);
            let before = connection.total_changes();
            let replay =
                submit_review_with_question(connection, input, Some(review_context(&revealed)))?;
            assert_eq!(connection.total_changes(), before);
            assert_eq!(
                serde_json::to_value(result).unwrap(),
                serde_json::to_value(replay).unwrap()
            );
            let evidence: (Option<String>, String, i64) = connection
                .query_row(
                    "SELECT qa.verdict, qa.phase, r.answer_correct FROM question_attempts qa
                     JOIN question_review_links link ON link.instance_id = qa.instance_id
                     JOIN review_events r ON r.id = link.review_event_id WHERE qa.instance_id = ?1",
                    [&target.instance_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(evidence, (None, "revealed".to_string(), correct));
            let replay = reveal(connection, fixture.reveal_input(&target))?;
            assert_eq!(replay.revision, revealed.revision);
            Ok(())
        });
        assert_eq!(fixture.counts(), (1, 1, 1));
    }
}

#[test]
fn exposure_failure_rolls_back_attempt_receipt_and_session_counters() {
    let fixture = Fixture::new();
    let session = fixture.start(false);
    let target = target(&session, 0);
    let before = fixture.view(&target);
    fixture.run(|connection| {
        connection
            .execute_batch(
                "CREATE TRIGGER reject_image_exposure BEFORE UPDATE OF revealed_at ON practice_session_cards
                 BEGIN SELECT RAISE(ABORT, 'injected exposure failure'); END;",
            )
            .map_err(CommandError::database)?;
        assert!(reveal(connection, fixture.reveal_input(&target)).is_err());
        let count: i64 = connection
            .query_row("SELECT revealed_count FROM practice_sessions", [], |row| row.get(0))
            .map_err(CommandError::database)?;
        assert_eq!(count, 0);
        connection
            .execute_batch("DROP TRIGGER reject_image_exposure;")
            .map_err(CommandError::database)?;
        Ok(())
    });
    assert_eq!(fixture.view(&target), before);
    fixture.reveal(&target);
    assert_eq!(fixture.counts(), (0, 0, 0));
}

#[test]
fn image_review_link_failure_rolls_back_schedule_but_keeps_the_committed_reveal() {
    let fixture = Fixture::new();
    let session = fixture.start(false);
    let target = target(&session, 0);
    let revealed = fixture.reveal(&target);
    let input = fixture.review(&target, "somewhat_confident");
    fixture.run(|connection| {
        connection
            .execute_batch(
                "CREATE TRIGGER reject_image_review_link BEFORE INSERT ON question_review_links
                 BEGIN SELECT RAISE(ABORT, 'injected link failure'); END;",
            )
            .map_err(CommandError::database)?;
        let failed =
            submit_review_with_question(connection, input.clone(), Some(review_context(&revealed)));
        assert!(failed.is_err());
        Ok(())
    });
    assert_eq!(fixture.counts(), (0, 0, 0));
    assert_eq!(
        fixture.view(&target)["attempt"]["progress"]["phase"],
        "revealed"
    );
    fixture.run(|connection| {
        connection
            .execute_batch("DROP TRIGGER reject_image_review_link;")
            .map_err(CommandError::database)?;
        submit_review_with_question(connection, input, Some(review_context(&revealed)))?;
        Ok(())
    });
    assert_eq!(fixture.counts(), (1, 1, 1));
}

#[test]
fn source_edits_and_database_reopen_preserve_captured_geometry_and_reveal() {
    let fixture = Fixture::new();
    let session = fixture.start(false);
    let target = target(&session, 0);
    fixture.reveal(&target);
    let before = fixture.view(&target);
    let pixels = fixture.image(&target, 2);
    fixture.run(|connection| {
        let mut edited = definition();
        edited["question"]["masks"][0]["rect"]["x"] = json!(0.5);
        edited["question"]["questions"][0]["answer"] = rich("A later answer");
        connection
            .execute(
                "UPDATE notes SET revision = revision + 1, question_definition = ?1 WHERE id = ?2",
                params![edited.to_string(), id(2)],
            )
            .map_err(CommandError::database)?;
        connection
            .execute("DELETE FROM note_assets WHERE note_id = ?1", [id(2)])
            .map_err(CommandError::database)?;
        // The captured owner must prevent deleting its source even after note edits.
        assert!(connection
            .execute("DELETE FROM assets WHERE id = ?1", [id(8)])
            .is_err());
        Ok(())
    });
    let reopened = Database::open(fixture.paths.database_path()).unwrap();
    let restored = reopened
        .with_connection(|connection| {
            serde_json::to_value(read(connection, &target)?).map_err(CommandError::serialization)
        })
        .unwrap();
    assert_eq!(restored, before);
    let bytes = render(&reopened, &fixture.paths, &fixture.image_input(&target, 2)).unwrap();
    assert_eq!(
        image::load_from_memory(&bytes).unwrap().into_rgba8(),
        pixels
    );
}

#[test]
fn cached_pixels_cannot_bypass_material_availability_source_ownership_or_missing_files() {
    let fixture = Fixture::new();
    let session = fixture.start(false);
    let target = target(&session, 0);
    fixture.image(&target, 1);
    fixture.run(|connection| {
        connection
            .execute(
                "UPDATE cards SET deleted_at = 100 WHERE id = ?1",
                [&target.card_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    });
    assert_eq!(
        error_code(fixture.raster(&target, 1)),
        "practice_question_unavailable"
    );
    fixture.run(|connection| {
        connection
            .execute(
                "UPDATE cards SET deleted_at = NULL WHERE id = ?1",
                [&target.card_id],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "DELETE FROM question_attempt_assets WHERE instance_id = ?1",
                [&target.instance_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    });
    assert_eq!(
        error_code(fixture.raster(&target, 1)),
        "invalid_question_checkpoint"
    );
    fixture.run(|connection| {
        connection
            .execute(
                "INSERT INTO question_attempt_assets (instance_id, asset_id) VALUES (?1, ?2)",
                params![target.instance_id, id(8)],
            )
            .map_err(CommandError::database)?;
        Ok(())
    });
    std::fs::remove_file(&fixture.source_path).unwrap();
    assert!(fixture.raster(&target, 1).is_err());
    assert_eq!(fixture.counts(), (0, 0, 0));
}

#[test]
fn callers_cannot_request_revealed_pixels_or_use_a_foreign_or_stale_instance() {
    let fixture = Fixture::new();
    let session = fixture.start(false);
    let target = target(&session, 0);
    let forged = json!({"target":{"sessionId":target.session_id,"cardId":target.card_id,"instanceId":target.instance_id},
        "expectedRevision":1,"maximumEdge":20,"revealed":true});
    assert!(serde_json::from_value::<OcclusionImageInput>(forged).is_err());
    let mut other = fixture.image_input(&target, 1);
    other.target.instance_id = id(999);
    assert!(render(&fixture.database, &fixture.paths, &other).is_err());
    fixture.run(|connection| {
        assert!(
            record_practice_event(connection, &target.session_id, &target.card_id, true).is_err()
        );
        Ok(())
    });
    fixture.reveal(&target);
    assert_eq!(
        error_code(fixture.raster(&target, 1)),
        "question_attempt_conflict"
    );
    let mut invalid = fixture.image_input(&target, 2);
    invalid.maximum_edge = MAX_RENDER_EDGE + 1;
    assert_eq!(
        error_code(render(&fixture.database, &fixture.paths, &invalid)),
        "invalid_occlusion_request"
    );
}
