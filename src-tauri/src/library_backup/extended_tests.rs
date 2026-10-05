include!("history_tests.rs");
include!("study_guide_backup_tests.rs");

#[path = "question_backup_tests.rs"]
mod question_backup_tests;

#[path = "performance_fixture_tests.rs"]
pub(super) mod performance_fixture_tests;

#[path = "current_format_questions.rs"]
mod current_format_questions;
#[path = "current_format_sources.rs"]
mod current_format_sources;
#[path = "current_format_tests.rs"]
mod current_format_tests;

#[path = "subjects_restore_tests.rs"]
mod subjects_restore_tests;
