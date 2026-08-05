#!/usr/bin/env python3
"""Migrate native tests to the scoped local-calendar APIs."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

path = ROOT / "src-tauri/src/regression_tests.rs"
content = path.read_text()
content = content.replace(
    "    settings::load_settings,\n"
    "    util::{generate_id, unix_time_millis},\n"
    "    workload::{planned_scheduled_cards, workload_overview},\n",
    "    settings::load_settings,\n"
    "    study_calendar::StudyTimeContext,\n"
    "    util::{generate_id, unix_time_millis},\n"
    "    workload::{planned_scheduled_cards_scoped, workload_overview_scoped},\n",
)
content = content.replace(
    "workload_overview(connection, std::slice::from_ref(&deck_id), &settings)?",
    "workload_overview_scoped(\n"
    "                connection,\n"
    "                std::slice::from_ref(&deck_id),\n"
    "                &[],\n"
    "                &settings,\n"
    "                &StudyTimeContext::capture()?,\n"
    "            )?",
)
content = content.replace(
    "planned_scheduled_cards(connection, std::slice::from_ref(&deck_id), &settings)?",
    "planned_scheduled_cards_scoped(\n"
    "                connection,\n"
    "                std::slice::from_ref(&deck_id),\n"
    "                &[],\n"
    "                &settings,\n"
    "                &StudyTimeContext::capture()?,\n"
    "            )?",
)
content = content.replace(
    "workload_overview(connection, &[deck_id], &settings)?",
    "workload_overview_scoped(\n"
    "                connection,\n"
    "                &[deck_id],\n"
    "                &[],\n"
    "                &settings,\n"
    "                &StudyTimeContext::capture()?,\n"
    "            )?",
)
path.write_text(content)

path = ROOT / "src-tauri/src/authoring/tests.rs"
content = path.read_text().replace(
    'due_filter: "all".to_string(),',
    'availability_filter: "all".to_string(),',
)
path.write_text(content)

path = ROOT / "src-tauri/src/schedule_controls/extended_tests.rs"
content = path.read_text()
old = '''        assert_eq!(
            validate_input(action_input(&[], 19, "suspend"))
                .expect_err("an empty selection must fail")
                .code,
            "invalid_schedule_selection"
        );

        let card_ids = vec![test_id(20)];
        let mut invalid = action_input(&card_ids, 21, "reschedule");
        invalid.due_date = Some(-1);
        assert_eq!(
            validate_input(invalid)
                .expect_err("negative dates must fail")
                .code,
            "invalid_reschedule_date"
        );

        let mut unexpected = action_input(&card_ids, 22, "suspend");
        unexpected.due_date = Some(1);
        assert_eq!(
            validate_input(unexpected)
                .expect_err("non-reschedule actions cannot carry dates")
                .code,
            "unexpected_reschedule_date"
        );'''
new = '''        let context = StudyTimeContext::capture().expect("calendar context");
        assert_eq!(
            validate_input(action_input(&[], 19, "suspend"), &context)
                .expect_err("an empty selection must fail")
                .code,
            "invalid_schedule_selection"
        );

        let card_ids = vec![test_id(20)];
        let mut invalid = action_input(&card_ids, 21, "reschedule");
        invalid.due_date = Some("not-a-date".to_string());
        assert_eq!(
            validate_input(invalid, &context)
                .expect_err("invalid local dates must fail")
                .code,
            "invalid_calendar_date"
        );

        let mut unexpected = action_input(&card_ids, 22, "suspend");
        unexpected.due_date = Some(context.local_date_string());
        assert_eq!(
            validate_input(unexpected, &context)
                .expect_err("non-reschedule actions cannot carry dates")
                .code,
            "unexpected_reschedule_date"
        );'''
if old not in content:
    raise SystemExit("schedule strict-input test marker missing")
path.write_text(content.replace(old, new, 1))

print("Migrated native tests to Stage 3.6-3.8 APIs.")
