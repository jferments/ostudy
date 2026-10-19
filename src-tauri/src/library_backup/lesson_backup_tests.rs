use super::*;
use crate::lessons::{
    authoring::create_authored_lesson, fixture::loop_of_henle, list_lessons, save_progress,
    SaveLessonProgressInput,
};

#[test]
fn full_backup_restores_a_lesson_its_revisions_and_where_the_learner_stopped() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-lesson-backup-{}-{}",
        std::process::id(),
        unix_time_millis().unwrap()
    ));
    let subject = "5".repeat(32);
    let source_paths = AppPaths::new(root.join("source")).unwrap();
    let source = Database::open(source_paths.database_path()).unwrap();
    let lesson_id = source
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO document_collections(id, name, sort_order, created_at,
                         updated_at, revision)
                     VALUES (?1, 'Renal physiology', 1000, 1, 1, 1)",
                    [&subject],
                )
                .map_err(CommandError::database)?;
            let lesson = create_authored_lesson(connection, &subject, loop_of_henle(&[]), 10)?;
            save_progress(
                connection,
                SaveLessonProgressInput {
                    lesson_id: lesson.id.clone(),
                    revision_number: 1,
                    current_step: 1,
                    finished: false,
                },
                20,
            )?;
            Ok(lesson.id)
        })
        .unwrap();
    let before = source
        .with_connection(|connection| list_lessons(connection, &subject))
        .unwrap();

    let backup_path = root.join("full.ostudybackup");
    export_backup(&source, &source_paths, &backup_path).unwrap();
    let target_paths = AppPaths::new(root.join("target")).unwrap();
    copy_and_sync(&backup_path, &pending_restore_path(&target_paths)).unwrap();
    let guard = apply_pending_restore(&target_paths).unwrap().unwrap();
    let restored = Database::open(target_paths.database_path()).unwrap();
    let after = restored
        .with_connection(|connection| list_lessons(connection, &subject))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&after).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    let lesson = &after.lessons[0];
    assert_eq!(lesson.id, lesson_id);
    let progress = lesson.progress.as_ref().unwrap();
    assert_eq!((progress.current_step, progress.started_at), (1, 20));
    drop(restored);
    guard.commit();
    drop(source);
    fs::remove_dir_all(root).unwrap();
}
