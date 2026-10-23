mod extended_tests {
    use super::*;
    use serde_json::json;

    fn settings() -> AppSettingsDto {
        AppSettingsDto {
            theme: "dark".to_string(),
            ui_font: "system".to_string(),
            card_font: "system".to_string(),
            ui_scale: 100,
            card_base_size: 18,
            motion_preference: "system".to_string(),
            material_preference: "automatic".to_string(),
            decorative_artwork: true,
            celebrations_enabled: true,
            tutorials_enabled: true,
            confirm_deletions: true,
            practice_shuffle: true,
            display_question_on_answer: false,
            desired_retention: 0.90,
            daily_new_limit: 20,
            daily_review_limit: 200,
            new_day_start_hour: 4,
            guide_pdf_theme: "sage".to_string(),
            guide_pdf_include_notes: true,
            lecture_audio_with_new_lessons: true,
        }
    }

    #[test]
    fn every_setting_round_trips_through_json_storage() {
        let database = Database::open_in_memory().expect("database should open");
        let expected = AppSettingsDto {
            theme: "synthwave".to_string(),
            ui_font: "custom:Atkinson Hyperlegible".to_string(),
            card_font: "serif".to_string(),
            ui_scale: 135,
            card_base_size: 30,
            motion_preference: "none".to_string(),
            material_preference: "simple".to_string(),
            decorative_artwork: false,
            celebrations_enabled: false,
            tutorials_enabled: false,
            confirm_deletions: false,
            practice_shuffle: false,
            display_question_on_answer: true,
            desired_retention: 0.96,
            daily_new_limit: 12,
            daily_review_limit: 80,
            new_day_start_hour: 23,
            guide_pdf_theme: "sage".to_string(),
            guide_pdf_include_notes: true,
            lecture_audio_with_new_lessons: true,
        };
        validate_settings(&expected).expect("fixture should validate");
        database
            .with_connection(|connection| save_settings_in_connection(connection, &expected))
            .expect("settings should save");
        let loaded = database
            .with_connection(load_settings)
            .expect("settings should load");

        assert_eq!(loaded.theme, expected.theme);
        assert_eq!(loaded.ui_font, expected.ui_font);
        assert_eq!(loaded.card_font, expected.card_font);
        assert_eq!(loaded.ui_scale, expected.ui_scale);
        assert_eq!(loaded.card_base_size, expected.card_base_size);
        assert_eq!(loaded.motion_preference, expected.motion_preference);
        assert_eq!(loaded.material_preference, expected.material_preference);
        assert_eq!(loaded.decorative_artwork, expected.decorative_artwork);
        assert_eq!(loaded.celebrations_enabled, expected.celebrations_enabled);
        assert_eq!(loaded.tutorials_enabled, expected.tutorials_enabled);
        assert_eq!(loaded.confirm_deletions, expected.confirm_deletions);
        assert_eq!(loaded.practice_shuffle, expected.practice_shuffle);
        assert_eq!(
            loaded.display_question_on_answer,
            expected.display_question_on_answer
        );
        assert_eq!(loaded.desired_retention, expected.desired_retention);
        assert_eq!(loaded.daily_new_limit, expected.daily_new_limit);
        assert_eq!(loaded.daily_review_limit, expected.daily_review_limit);
        assert_eq!(loaded.new_day_start_hour, expected.new_day_start_hour);
    }

    #[test]
    fn validation_reports_the_specific_invalid_setting() {
        let mut value = settings();
        value.theme = "remote-theme".to_string();
        assert_eq!(validate_settings(&value).unwrap_err().code, "invalid_theme");

        let mut value = settings();
        value.ui_font = "custom:Bad/font".to_string();
        assert_eq!(validate_settings(&value).unwrap_err().code, "invalid_font");

        let mut value = settings();
        value.motion_preference = "constant-spin".to_string();
        assert_eq!(
            validate_settings(&value).unwrap_err().code,
            "invalid_motion_preference"
        );

        let mut value = settings();
        value.material_preference = "transparent-everything".to_string();
        assert_eq!(
            validate_settings(&value).unwrap_err().code,
            "invalid_material_preference"
        );

        let mut value = settings();
        value.ui_scale = 79;
        assert_eq!(validate_settings(&value).unwrap_err().code, "invalid_ui_scale");

        let mut value = settings();
        value.card_base_size = 33;
        assert_eq!(validate_settings(&value).unwrap_err().code, "invalid_card_size");

        let mut value = settings();
        value.daily_review_limit = MAX_DAILY_LIMIT + 1;
        assert_eq!(
            validate_settings(&value).unwrap_err().code,
            "invalid_daily_limit"
        );

        for hour in [-1, 24] {
            let mut value = settings();
            value.new_day_start_hour = hour;
            assert_eq!(
                validate_settings(&value).unwrap_err().code,
                "invalid_new_day_start_hour",
                "{hour}"
            );
        }
        for hour in [0, 23] {
            let mut value = settings();
            value.new_day_start_hour = hour;
            validate_settings(&value).expect("whole hours of the day are valid");
        }

        let mut value = settings();
        value.desired_retention = 0.79;
        assert_eq!(
            validate_settings(&value).unwrap_err().code,
            "invalid_desired_retention"
        );
    }

    #[test]
    fn strict_settings_input_rejects_removed_or_misspelled_fields() {
        let base = json!({
            "theme": "dark",
            "uiFont": "system",
            "cardFont": "system",
            "uiScale": 100,
            "cardBaseSize": 18,
            "motionPreference": "system",
            "materialPreference": "automatic",
            "decorativeArtwork": true,
            "celebrationsEnabled": true,
            "tutorialsEnabled": true,
            "confirmDeletions": true,
            "practiceShuffle": true,
            "displayQuestionOnAnswer": false,
            "desiredRetention": 0.9,
            "dailyNewLimit": 20,
            "dailyReviewLimit": 200,
            "newDayStartHour": 4
        });
        serde_json::from_value::<AppSettingsDto>(base.clone())
            .expect("the complete settings shape should parse");

        for hour in [json!(4.5), json!("4"), json!(true), json!(null)] {
            let mut not_an_integer = base.clone();
            not_an_integer["newDayStartHour"] = hour.clone();
            assert!(
                serde_json::from_value::<AppSettingsDto>(not_an_integer).is_err(),
                "{hour} is not a whole hour"
            );
        }
        let mut missing = base.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove("newDayStartHour");
        let error = serde_json::from_value::<AppSettingsDto>(missing)
            .expect_err("the hour must be sent explicitly");
        assert!(error.to_string().contains("newDayStartHour"));

        let mut removed_density = base.clone();
        removed_density["deckViewDensity"] = json!("comfortable");
        let error = serde_json::from_value::<AppSettingsDto>(removed_density)
            .expect_err("removed deck density must not be ignored");
        assert!(error.to_string().contains("deckViewDensity"));

        let mut removed_reasoning = base.clone();
        removed_reasoning["aiReasoningEffort"] = json!("high");
        let error = serde_json::from_value::<AppSettingsDto>(removed_reasoning)
            .expect_err("global AI reasoning must be rejected by application settings");
        assert!(error.to_string().contains("aiReasoningEffort"));

        let mut misspelled = base;
        misspelled["shuffleDecks"] = json!(true);
        let error = serde_json::from_value::<AppSettingsDto>(misspelled)
            .expect_err("misspelled settings must not be ignored");
        assert!(error.to_string().contains("shuffleDecks"));
    }

    #[test]
    fn corrupted_stored_json_fails_visibly_instead_of_using_a_default() {
        let database = Database::open_in_memory().expect("database should open");
        let error = database
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE app_settings SET value_json = 'not-json' WHERE key = 'theme'",
                        [],
                    )
                    .map_err(CommandError::database)?;
                load_settings(connection)
            })
            .expect_err("corrupt settings should fail visibly");
        assert_eq!(error.code, "serialization_error");
    }

    #[test]
    fn a_stored_hour_outside_the_day_fails_study_time_capture_but_not_settings() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                for (stored, code) in [
                    ("24", "invalid_new_day_start_hour"),
                    ("-1", "invalid_new_day_start_hour"),
                    ("4.5", "serialization_error"),
                ] {
                    connection
                        .execute(
                            "INSERT INTO app_settings (key, value_json, updated_at)
                             VALUES ('newDayStartHour', ?1, 1)
                             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                            [stored],
                        )
                        .map_err(CommandError::database)?;
                    assert_eq!(
                        load_new_day_start_hour(connection).unwrap_err().code,
                        code,
                        "{stored}"
                    );
                    assert_eq!(
                        crate::study_calendar::StudyTimeContext::capture(connection)
                            .unwrap_err()
                            .code,
                        code,
                        "{stored}"
                    );
                }
                connection
                    .execute(
                        "UPDATE app_settings SET value_json = '24' WHERE key = 'newDayStartHour'",
                        [],
                    )
                    .map_err(CommandError::database)?;
                // Settings still load, so the learner can choose a valid hour.
                assert_eq!(load_settings(connection)?.new_day_start_hour, 24);
                Ok(())
            })
            .expect("stored hours should be checked");
    }
}
