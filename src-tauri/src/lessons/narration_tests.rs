use rusqlite::params;

use super::*;
use crate::{
    ai::speech::{DRAMA, KOKORO},
    lessons::{authoring::create_authored_lesson, fixture::loop_of_henle},
};

const SUBJECT: &str = "55555555555555555555555555555555";
const OPENROUTER: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const SECOND_OPENROUTER: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
const LOCAL: &str = "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3";
const NOW: i64 = 1_700_000_000_000;

fn library() -> Database {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO document_collections(
                         id, name, sort_order, created_at, updated_at, revision
                     ) VALUES (?1, ?1, 1000, 1, 1, 1)",
                    params![SUBJECT],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    database
}

fn add_profile(database: &Database, id: &str, kind: &str, base_url: &str, updated_at: i64) {
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO ai_provider_profiles (
                         id, display_name, normalized_name, provider_kind, base_url, model_id,
                         auth_mode, enabled, created_at, updated_at
                     ) VALUES (?1, ?1, ?1, ?2, ?3, 'deepseek/deepseek-chat', 'bearer', 1, 1, ?4)",
                    params![id, kind, base_url, updated_at],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
}

fn add_openrouter(database: &Database, id: &str, updated_at: i64) {
    add_profile(
        database,
        id,
        "openrouter",
        "https://openrouter.ai/api/v1/",
        updated_at,
    );
}

fn settings(database: &Database) -> SpeechSettingsDto {
    database
        .with_connection(|connection| speech_settings(connection))
        .unwrap()
}

fn save(
    database: &Database,
    profile_id: Option<&str>,
    model_id: &str,
    voice: &str,
    expected_revision: i64,
) -> Result<SpeechSettingsDto, CommandError> {
    database.with_connection(|connection| {
        save_settings(
            connection,
            SaveSpeechSettingsInput {
                profile_id: profile_id.map(str::to_string),
                model_id: model_id.to_string(),
                voice: voice.to_string(),
                expected_revision,
            },
            NOW,
        )
    })
}

fn temporary_paths(name: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-narration-{name}-{}-{}",
        std::process::id(),
        unix_time_millis().unwrap()
    ));
    AppPaths::new(root).unwrap()
}

/// Three MPEG-1 layer III frames: enough for the media sniffer to call it mp3.
fn mp3_bytes() -> Vec<u8> {
    let mut frame = vec![0xff, 0xfb, 0x90, 0x00];
    frame.resize(417, 0);
    frame.repeat(3)
}

fn lesson(database: &Database) -> String {
    database
        .with_connection(|connection| {
            create_authored_lesson(connection, SUBJECT, loop_of_henle(&[]), NOW)
        })
        .unwrap()
        .id
}

fn step_input(lesson_id: &str, step_key: &str) -> NarrateLessonStepInput {
    NarrateLessonStepInput {
        lesson_id: lesson_id.to_string(),
        revision_number: 1,
        step_key: step_key.to_string(),
    }
}

fn narrate(
    database: &Database,
    paths: &AppPaths,
    input: NarrateLessonStepInput,
) -> Result<LessonStepNarrationDto, CommandError> {
    let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
    tauri::async_runtime::block_on(narrate_lesson_step(database, paths, &runtime, input))
}

fn narration_of(step_key: &str) -> String {
    loop_of_henle(&[])
        .steps
        .into_iter()
        .find(|step| step.key == step_key)
        .unwrap()
        .narration
}

#[test]
fn narration_is_cut_into_whole_sentences_that_fit_one_request() {
    assert!(segments("  ", 100).is_empty());
    assert_eq!(
        segments("One. Two! Three? Four", 100),
        vec!["One. Two! Three? Four"]
    );
    assert_eq!(
        segments("First sentence here. Second one here. Third.", 25),
        vec!["First sentence here.", "Second one here. Third."]
    );
    // A decimal point is not the end of a sentence.
    assert_eq!(
        segments("It is 3.5 mm. Next.", 14),
        vec!["It is 3.5 mm.", "Next."]
    );

    let long = "word ".repeat(30);
    let pieces = segments(&long, 22);
    assert!(pieces.len() > 1);
    assert!(pieces.iter().all(|piece| piece.chars().count() <= 22));
    assert!(pieces.iter().all(|piece| !piece.starts_with(' ')));
    assert_eq!(pieces.join(" ").split_whitespace().count(), 30);

    let word = "é".repeat(50);
    let pieces = segments(&word, 20);
    assert_eq!(pieces.concat(), word);
    assert!(pieces.iter().all(|piece| piece.chars().count() <= 20));
}

#[test]
fn the_voice_defaults_to_drama_through_the_document_writing_connection() {
    let database = library();
    let unset = settings(&database);
    assert_eq!(unset.profile_id, None);
    assert_eq!(unset.model_id, DRAMA);
    assert_eq!(unset.voice, "builtin:relaxed-man");
    assert_eq!(unset.revision, 0);
    assert_eq!(unset.models.len(), 3);
    assert_eq!(
        unset.models.iter().filter(|model| model.on_device).count(),
        1
    );

    add_profile(
        &database,
        LOCAL,
        "llama_cpp",
        "http://127.0.0.1:8080/v1/",
        9,
    );
    add_openrouter(&database, OPENROUTER, 1);
    add_openrouter(&database, SECOND_OPENROUTER, 5);
    let defaults = settings(&database);
    let listed: Vec<_> = defaults
        .connections
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(listed, vec![SECOND_OPENROUTER, OPENROUTER]);
    assert_eq!(defaults.profile_id.as_deref(), Some(SECOND_OPENROUTER));

    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO ai_task_defaults(task_kind, profile_id, model_id, updated_at)
                     VALUES ('flashcard_generation', ?1, 'deepseek/deepseek-chat', 1)",
                    params![OPENROUTER],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(settings(&database).profile_id.as_deref(), Some(OPENROUTER));
}

#[test]
fn saving_the_voice_checks_the_model_voice_connection_and_revision() {
    let database = library();
    add_openrouter(&database, OPENROUTER, 1);
    add_profile(
        &database,
        LOCAL,
        "llama_cpp",
        "http://127.0.0.1:8080/v1/",
        2,
    );

    let saved = save(&database, Some(OPENROUTER), DRAMA, "", 0).unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.model_id, DRAMA);
    assert_eq!(settings(&database).voice, "builtin:relaxed-man");

    let stale = save(&database, Some(OPENROUTER), KOKORO, "am_adam", 0).unwrap_err();
    assert_eq!(stale.code, "speech_settings_changed");
    let unknown_voice = save(&database, Some(OPENROUTER), KOKORO, "nobody", 1).unwrap_err();
    assert_eq!(unknown_voice.code, "speech_voice_unknown");
    let voiced_drama = save(&database, Some(OPENROUTER), DRAMA, "af_heart", 1).unwrap_err();
    assert_eq!(voiced_drama.code, "speech_voice_unknown");
    let unknown_model = save(&database, Some(OPENROUTER), "someone/else", "", 1).unwrap_err();
    assert_eq!(unknown_model.code, "speech_model_unknown");
    let local = save(&database, Some(LOCAL), KOKORO, "af_heart", 1).unwrap_err();
    assert_eq!(local.code, "speech_connection_unavailable");

    let changed = save(&database, Some(OPENROUTER), KOKORO, "bm_george", 1).unwrap();
    assert_eq!((changed.revision, changed.voice.as_str()), (2, "bm_george"));

    // A connection that is switched off stops being the voice's connection.
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE ai_provider_profiles SET enabled = 0 WHERE id = ?1",
                    params![OPENROUTER],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    let current = settings(&database);
    assert_eq!(current.profile_id, None);
    assert_eq!(current.revision, 2);
}

#[test]
fn a_saved_voice_keeps_speaking_after_its_service_is_replaced() {
    let database = library();
    add_openrouter(&database, OPENROUTER, 1);
    let saved = save(&database, Some(OPENROUTER), KOKORO, "bm_george", 0).unwrap();
    assert_eq!(saved.profile_id.as_deref(), Some(OPENROUTER));

    // Removing the service clears the saved connection; a new one takes over
    // with the chosen model and voice instead of leaving lessons silent.
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "DELETE FROM ai_provider_profiles WHERE id = ?1",
                    params![OPENROUTER],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(settings(&database).profile_id, None);
    add_openrouter(&database, SECOND_OPENROUTER, 2);
    let current = settings(&database);
    assert_eq!(current.profile_id.as_deref(), Some(SECOND_OPENROUTER));
    assert_eq!(
        (current.model_id.as_str(), current.voice.as_str()),
        (KOKORO, "bm_george")
    );
}

#[test]
fn a_step_is_narrated_only_through_a_configured_voice_and_from_its_saved_text() {
    let database = library();
    let paths = temporary_paths("refusals");
    let lesson_id = lesson(&database);

    let missing = narrate(&database, &paths, step_input(&lesson_id, "no-such-step")).unwrap_err();
    assert_eq!(missing.code, "lesson_step_missing");
    let unconfigured = narrate(&database, &paths, step_input(&lesson_id, "two-limbs")).unwrap_err();
    assert_eq!(unconfigured.code, "speech_not_configured");
    let other_revision = narrate(
        &database,
        &paths,
        NarrateLessonStepInput {
            revision_number: 2,
            ..step_input(&lesson_id, "two-limbs")
        },
    )
    .unwrap_err();
    assert_ne!(other_revision.code, "speech_not_configured");
}

#[test]
fn kept_audio_is_reused_without_asking_the_voice_again() {
    let database = library();
    let paths = temporary_paths("reuse");
    add_openrouter(&database, OPENROUTER, 1);
    save(&database, Some(OPENROUTER), KOKORO, "bm_george", 0).unwrap();
    let lesson_id = lesson(&database);
    let narration = narration_of("two-limbs");
    let model = speech::speech_model(KOKORO).unwrap();
    let voice = Voice {
        engine: "openrouter",
        model,
        voice: "bm_george".to_string(),
        reference: None,
    };
    let pieces = segments(&narration, model.max_characters);
    let mut kept = Vec::new();
    for piece in &pieces {
        let segment = keep_segment(&database, &paths, &voice, piece, &mp3_bytes()).unwrap();
        // Keeping the same piece twice keeps one copy.
        let again = keep_segment(&database, &paths, &voice, piece, &mp3_bytes()).unwrap();
        assert_eq!(segment.asset_id, again.asset_id);
        kept.push(segment.asset_id);
    }

    // The profile has no saved key and the host is never reached: every
    // piece comes from what was kept.
    let narrated = narrate(&database, &paths, step_input(&lesson_id, "two-limbs")).unwrap();
    assert_eq!(narrated.made, 0);
    let assets: Vec<_> = narrated
        .segments
        .iter()
        .map(|segment| segment.asset_id.clone())
        .collect();
    assert_eq!(assets, kept);

    database
        .with_connection(|connection| {
            for asset_id in &kept {
                let (state, relative_path): (String, String) = connection
                    .query_row(
                        "SELECT lifecycle_state, relative_path FROM assets WHERE id = ?1",
                        params![asset_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(state, "ready");
                assert_eq!(
                    fs::read(paths.resolve_relative(&relative_path)).unwrap(),
                    mp3_bytes()
                );
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn text_is_spoken_piece_by_piece_from_kept_audio() {
    let database = library();
    let paths = temporary_paths("pieces");
    add_openrouter(&database, OPENROUTER, 1);
    save(&database, Some(OPENROUTER), KOKORO, "bm_george", 0).unwrap();
    let model = speech::speech_model(KOKORO).unwrap();
    let sentence = "The loop of Henle concentrates urine by moving salt out of the tubule. ";
    let text = sentence.repeat(40);
    // Pieces as short as the caller asks, not as long as the model takes.
    let longest = 400;
    let pieces = segments(&text, longest);
    assert!(pieces.len() > segments(&text, model.max_characters).len());
    let voice = Voice {
        engine: "openrouter",
        model,
        voice: "bm_george".to_string(),
        reference: None,
    };
    let kept: Vec<_> = pieces
        .iter()
        .map(|piece| keep_segment(&database, &paths, &voice, piece, &mp3_bytes()).unwrap())
        .collect();
    let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
    let speak = |index| {
        tauri::async_runtime::block_on(speak_piece(
            &database,
            &paths,
            &runtime,
            &text,
            index,
            longest,
            "reply".into(),
            "tutor_speech",
        ))
    };
    // The host is never reached: each piece is the one kept for it.
    for (index, segment) in kept.iter().enumerate() {
        let spoken = speak(index).unwrap();
        assert_eq!(spoken.segment, *segment);
        assert_eq!(spoken.pieces, pieces.len());
        assert!(!spoken.made);
    }
    assert_eq!(
        speak(pieces.len()).unwrap_err().code,
        "speech_piece_missing"
    );
}

#[test]
fn something_other_than_audio_is_not_kept() {
    let database = library();
    let paths = temporary_paths("not-audio");
    let voice = Voice {
        engine: "openrouter",
        model: speech::speech_model(KOKORO).unwrap(),
        voice: "af_heart".to_string(),
        reference: None,
    };
    let refused = keep_segment(&database, &paths, &voice, "Hello.", b"{\"error\":1}").unwrap_err();
    assert_eq!(refused.code, "speech_audio_invalid");
    let segments: i64 = database
        .with_connection(|connection| {
            connection
                .query_row("SELECT COUNT(*) FROM speech_segments", [], |row| row.get(0))
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(segments, 0);
}

#[test]
fn a_step_cannot_be_narrated_twice_at_once() {
    let first = Narrating::begin("lesson:1:step".to_string()).unwrap();
    let second = Narrating::begin("lesson:1:step".to_string());
    assert_eq!(
        second.err().map(|error| error.code).as_deref(),
        Some("narration_in_progress")
    );
    drop(first);
    assert!(Narrating::begin("lesson:1:step".to_string()).is_ok());
}

#[test]
fn each_podcast_host_speaks_in_their_own_kept_voice() {
    use crate::lessons::podcasts::{
        self, NarratePodcastTurnInput, PodcastChapter, PodcastScript, PodcastTurn,
    };

    let database = library();
    let paths = temporary_paths("podcast");
    add_openrouter(&database, OPENROUTER, 1);
    save(&database, Some(OPENROUTER), KOKORO, "af_heart", 0).unwrap();
    let lesson_id = lesson(&database);
    let model = speech::speech_model(KOKORO).unwrap();
    let voices = podcasts::default_voices(model);
    let turns = [
        "Water leaves the descending limb.",
        "And the ascending limb?",
    ];
    let script = PodcastScript {
        schema_version: podcasts::PODCAST_SCRIPT_VERSION,
        title: "The loop".into(),
        chapters: vec![PodcastChapter {
            lesson_id: lesson_id.clone(),
            revision_number: 1,
            title: "The loop of Henle".into(),
            turns: turns
                .iter()
                .enumerate()
                .map(|(host, text)| PodcastTurn {
                    host,
                    text: text.to_string(),
                })
                .collect(),
        }],
    };
    let id = database
        .with_connection(|connection| {
            podcasts::keep(
                connection,
                SUBJECT,
                5,
                model,
                &voices,
                &script,
                &serde_json::json!({}),
                NOW,
            )
        })
        .unwrap();
    let keep_in = |voice: &str, text: &str| {
        keep_segment(
            &database,
            &paths,
            &Voice {
                engine: "openrouter",
                model,
                voice: voice.to_string(),
                reference: None,
            },
            text,
            &mp3_bytes(),
        )
        .unwrap()
        .asset_id
    };
    let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
    let narrate_turn = |key: &str| {
        tauri::async_runtime::block_on(podcasts::narrate_turn(
            &database,
            &paths,
            &runtime,
            NarratePodcastTurnInput {
                podcast_id: id.clone(),
                turn_key: key.to_string(),
            },
        ))
    };

    // Ada's turn plays the audio kept in Ada's voice; the host is never asked.
    let ada = keep_in(&voices[0], turns[0]);
    let spoken = narrate_turn("0.0").unwrap();
    assert_eq!(spoken.made, 0);
    assert_eq!(spoken.segments[0].asset_id, ada);

    // Ben's words kept in Ada's voice are not Ben's: they would be made anew,
    // and this keyless connection cannot.
    keep_in(&voices[0], turns[1]);
    assert!(narrate_turn("0.1").is_err());
    let ben = keep_in(&voices[1], turns[1]);
    assert_eq!(narrate_turn("0.1").unwrap().segments[0].asset_id, ben);

    // Choosing another lesson voice does not change the hosts'.
    save(&database, Some(OPENROUTER), KOKORO, "bm_george", 1).unwrap();
    assert_eq!(narrate_turn("0.0").unwrap().segments[0].asset_id, ada);
    assert_eq!(
        narrate_turn("0.2").unwrap_err().code,
        "podcast_turn_missing"
    );
}

#[test]
fn a_voice_on_the_device_needs_no_connection_and_is_kept_as_its_own_audio() {
    let database = library();
    let paths = temporary_paths("device");
    let kokoro = crate::device_voices::KOKORO.id;
    // No OpenRouter connection at all: the voice on the device can be chosen.
    let saved = save(&database, None, kokoro, "bm_george", 0).unwrap();
    assert_eq!(saved.model_id, kokoro);
    assert_eq!(saved.profile_id, None);
    assert!(save(&database, None, kokoro, "expr-voice-4-m", saved.revision).is_err());

    let lesson_id = lesson(&database);
    let refused = narrate(&database, &paths, step_input(&lesson_id, "two-limbs")).unwrap_err();
    let expected = if crate::device_voices::SUPPORTED {
        "device_voice_missing"
    } else {
        "device_voice_unsupported"
    };
    assert_eq!(refused.code, expected);
    assert!(refused.message.contains("Kokoro"), "{}", refused.message);

    // Kept audio from the device is WAV under its own engine, never taken for
    // a provider's.
    let model = speech::speech_model(kokoro).unwrap();
    let voice = Voice {
        engine: DEVICE_ENGINE,
        model,
        voice: "bm_george".to_string(),
        reference: None,
    };
    let wav = crate::platform::speech_synthesis::wav(&[0.0; 2400], 24_000);
    let segment = keep_segment(&database, &paths, &voice, "Hello.", &wav).unwrap();
    database
        .with_connection(|connection| {
            let (engine, format): (String, String) = connection
                .query_row(
                    "SELECT engine, format FROM speech_segments WHERE asset_id = ?1",
                    params![segment.asset_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!((engine.as_str(), format.as_str()), ("device", "wav"));
            let openrouter = Voice {
                engine: "openrouter",
                ..voice
            };
            assert!(kept_segment(connection, &openrouter, "Hello.")?.is_none());
            Ok(())
        })
        .unwrap();
}
