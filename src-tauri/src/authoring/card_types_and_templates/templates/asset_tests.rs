use super::super::finalize_assets_after_committed_write;
use super::*;

#[test]
fn post_commit_asset_failure_does_not_erase_durable_template() {
    let database = Database::open_in_memory().expect("database should open");
    let temporary = std::env::temp_dir().join(format!(
        "ostudy-template-post-commit-{}-{}",
        std::process::id(),
        unix_time_millis().expect("clock")
    ));
    let paths = AppPaths::new(temporary.clone()).expect("paths should initialize");
    let template_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let missing_draft_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO card_templates (
                         id, name, normalized_name, description,
                         created_at, updated_at, revision
                     ) VALUES (?1, 'Durable', 'durable', '', 1, 1, 1)",
                    [template_id],
                )
                .map_err(CommandError::database)?;

            finalize_assets_after_committed_write(
                connection,
                &paths,
                &[],
                Some(missing_draft_id),
                template_id,
                "test_post_commit_template",
            );

            let persisted: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM card_templates WHERE id = ?1",
                    [template_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(persisted, 1);
            Ok(())
        })
        .expect("post-commit finalization failure should remain non-fatal");

    let _ = std::fs::remove_dir_all(temporary);
}
