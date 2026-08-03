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
            reduce_motion: false,
            confirm_deletions: true,
            practice_shuffle: true,
            desired_retention: 0.90,
            daily_new_limit: 20,
            daily_review_limit: 200,
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
            reduce_motion: true,
            confirm_deletions: false,
            practice_shuffle: false,
            desired_retention: 0.96,
            daily_new_limit: 12,
            daily_review_limit: 80,
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
        assert_eq!(loaded.reduce_motion, expected.reduce_motion);
        assert_eq!(loaded.confirm_deletions, expected.confirm_deletions);
        assert_eq!(loaded.practice_shuffle, expected.practice_shuffle);
        assert_eq!(loaded.desired_retention, expected.desired_retention);
        assert_eq!(loaded.daily_new_limit, expected.daily_new_limit);
        assert_eq!(loaded.daily_review_limit, expected.daily_review_limit);
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
        value.ui_scale = 79;
        assert_eq!(validate_settings(&value).unwrap_err().code, "invalid_ui_scale");

        let mut value = settings();
        value.card_base_size = 33;
        assert_eq!(validate_settings(&value).unwrap_err().code, "invalid_card_size");

        let mut value = settings();
        value.daily_review_limit = MAX_DAILY_LIMIT + 1;
        assert_eq!(validate_settings(&value).unwrap_err().code, "invalid_daily_limit");

        let mut value = settings();
        value.desired_retention = 0.79;
        assert_eq!(
            validate_settings(&value).unwrap_err().code,
            "invalid_desired_retention"
        );
    }

    #[test]
    fn strict_settings_input_rejects_removed_or_misspelled_fields() {
        let error = serde_json::from_value::<AppSettingsDto>(json!({
            "theme": "dark",
            "uiFont": "system",
            "cardFont": "system",
            "uiScale": 100,
            "cardBaseSize": 18,
            "reduceMotion": false,
            "confirmDeletions": true,
            "practiceShuffle": true,
            "desiredRetention": 0.9,
            "dailyNewLimit": 20,
            "dailyReviewLimit": 200,
            "shuffleDecks": true
        }))
        .expect_err("obsolete settings must not be ignored");
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
}
