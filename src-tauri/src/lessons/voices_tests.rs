use super::*;
use crate::{
    ai::speech::{DRAMA, KOKORO},
    lessons::{
        authoring::create_authored_lesson,
        fixture::loop_of_henle,
        narration::{save_settings, SaveSpeechSettingsInput},
    },
};

const SUBJECT: &str = "55555555555555555555555555555555";
const OPENROUTER: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const NOW: i64 = 1_700_000_000_000;

/// Three MPEG-1 layer III frames: enough for the media sniffer to call it mp3.
fn mp3_bytes() -> Vec<u8> {
    let mut frame = vec![0xff, 0xfb, 0x90, 0x00];
    frame.resize(417, 0);
    frame.repeat(3)
}

/// A library whose voice is Drama 3 through a keyless OpenRouter connection,
/// with one written lesson. Nothing here reaches the network.
fn library() -> (Database, String) {
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
            connection
                .execute(
                    "INSERT INTO ai_provider_profiles (
                         id, display_name, normalized_name, provider_kind, base_url, model_id,
                         auth_mode, enabled, allow_insecure_http, request_timeout_ms,
                         credential_storage, credential_epoch, revision, created_at, updated_at,
                         reasoning_effort
                     ) VALUES (?1, ?1, ?1, 'openrouter', 'https://openrouter.ai/api/v1/',
                               'deepseek/deepseek-chat', 'bearer', 1, 0, 5000, 'none', 0, 1, 1, 1,
                               'none')",
                    params![OPENROUTER],
                )
                .map_err(CommandError::database)?;
            save_settings(
                connection,
                SaveSpeechSettingsInput {
                    profile_id: Some(OPENROUTER.to_string()),
                    model_id: DRAMA.to_string(),
                    voice: String::new(),
                    expected_revision: 0,
                },
                NOW,
            )?;
            Ok(())
        })
        .unwrap();
    let lesson_id = new_lesson(&database);
    (database, lesson_id)
}

fn new_lesson(database: &Database) -> String {
    database
        .with_connection(|connection| {
            create_authored_lesson(connection, SUBJECT, loop_of_henle(&[]), NOW)
        })
        .unwrap()
        .id
}

fn temporary_paths(name: &str) -> AppPaths {
    let root = std::env::temp_dir().join(format!(
        "ostudy-voices-{name}-{}-{}",
        std::process::id(),
        unix_time_millis().unwrap()
    ));
    AppPaths::new(root).unwrap()
}

fn runtime() -> AiRuntimeState {
    AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap()
}

fn settings(database: &Database) -> SpeechSettingsDto {
    database
        .with_connection(|connection| speech_settings(connection))
        .unwrap()
}

/// A voice as trying one leaves it, without asking a model.
fn seed_voice(database: &Database, paths: &AppPaths) -> SpeechVoiceDto {
    seed_named_voice(database, paths, "New voice")
}

fn seed_named_voice(database: &Database, paths: &AppPaths, name: &str) -> SpeechVoiceDto {
    database
        .with_connection(|connection| {
            let transaction = connection.transaction().map_err(CommandError::database)?;
            let audio = mp3_bytes();
            let (asset_id, _) = write_audio_asset(
                &transaction,
                paths,
                &checked_audio(&audio)?,
                &audio,
                "voice",
                "voice sample kept",
                NOW,
            )?;
            let id = generate_id(&transaction)?;
            transaction
                .execute("DELETE FROM speech_voices WHERE state = 'tried'", [])
                .map_err(CommandError::database)?;
            transaction
                .execute(
                    "INSERT INTO speech_voices(
                         id, model_id, name, state, sample_text, asset_id, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 'tried', ?4, ?5, ?6, ?6)",
                    params![id, DRAMA, name, SAMPLE, asset_id, NOW],
                )
                .map_err(CommandError::database)?;
            transaction.commit().map_err(CommandError::database)?;
            Ok(listed(connection)?
                .into_iter()
                .find(|voice| voice.id == id)
                .unwrap())
        })
        .unwrap()
}

fn keep_voice(database: &Database, id: &str) -> SpeechSettingsDto {
    database
        .with_connection(|connection| keep(connection, id, NOW))
        .unwrap()
}

/// The voice `lesson_id` is spoken in, as narration asks for it.
fn voice_for(database: &Database, lesson_id: &str) -> (String, String) {
    let settings = database
        .with_connection(|connection| resolved(connection))
        .unwrap();
    let (model, voice) = lesson_voice(database, &settings, lesson_id).unwrap();
    (model.id.to_string(), voice)
}

/// The learner's own voices, without those that come with ostudy.
fn own(settings: SpeechSettingsDto) -> Vec<SpeechVoiceDto> {
    settings
        .voices
        .into_iter()
        .filter(|voice| voice.state != "builtIn")
        .collect()
}

fn query<T: rusqlite::types::FromSql>(database: &Database, sql: &str) -> T {
    database
        .with_connection(|connection| {
            connection
                .query_row(sql, [], |row| row.get(0))
                .map_err(CommandError::database)
        })
        .unwrap()
}

fn lesson_voice_of(database: &Database, lesson_id: &str) -> String {
    database
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT voice FROM lesson_voices WHERE lesson_id = ?1",
                    [lesson_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)
        })
        .unwrap()
}

#[test]
fn a_tried_voice_can_be_kept_named_chosen_and_removed() {
    let (database, _) = library();
    let paths = temporary_paths("keep");

    let first = seed_voice(&database, &paths);
    assert_eq!(first.name, "New voice");
    assert_eq!(own(settings(&database)), vec![first.clone()]);
    // Trying again replaces the voice being tried.
    let second = seed_voice(&database, &paths);
    assert_eq!(own(settings(&database)), vec![second.clone()]);

    let kept = keep_voice(&database, &second.id);
    assert_eq!(kept.voice, voice_key(&second.id));
    let revision = kept.revision;
    let kept = own(kept);
    assert_eq!(kept.len(), 1);
    assert_eq!(
        (kept[0].name.as_str(), kept[0].state.as_str()),
        ("Voice 1", "saved")
    );

    // Only a saved voice of the model can be chosen.
    let choose = |model: &str, voice: String, expected_revision: i64| {
        database.with_connection(|connection| {
            save_settings(
                connection,
                SaveSpeechSettingsInput {
                    profile_id: Some(OPENROUTER.to_string()),
                    model_id: model.to_string(),
                    voice,
                    expected_revision,
                },
                NOW,
            )
        })
    };
    let gone = choose(DRAMA, voice_key(&first.id), revision).unwrap_err();
    assert_eq!(gone.code, "speech_voice_unknown");
    let preset = choose(KOKORO, voice_key(&second.id), revision).unwrap_err();
    assert_eq!(preset.code, "speech_voice_unknown");
    let again = choose(DRAMA, voice_key(&second.id), revision).unwrap();
    assert_eq!(again.voice, voice_key(&second.id));

    let rename_to = |name: &str| {
        database.with_connection(|connection| {
            rename(
                connection,
                RenameSpeechVoiceInput {
                    voice_id: second.id.clone(),
                    name: name.to_string(),
                },
                NOW,
            )
        })
    };
    assert_eq!(
        rename_to("  ").unwrap_err().code,
        "speech_voice_name_invalid"
    );
    assert_eq!(
        rename_to(&"x".repeat(61)).unwrap_err().code,
        "speech_voice_name_invalid"
    );
    assert_eq!(
        own(rename_to(" Ms. Rivera ").unwrap())[0].name,
        "Ms. Rivera"
    );

    // The voice in use stays until another is chosen; one no lesson speaks in
    // is deleted, its sample left for asset cleanup.
    let in_use = database
        .with_connection(|connection| remove(connection, &second.id, NOW))
        .unwrap_err();
    assert_eq!(in_use.code, "speech_voice_in_use");
    let third = seed_voice(&database, &paths);
    keep_voice(&database, &third.id);
    let removed = database
        .with_connection(|connection| remove(connection, &second.id, NOW))
        .unwrap();
    let removed = own(removed);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].name, "Voice 2");
    assert_eq!(
        query::<i64>(&database, "SELECT COUNT(*) FROM speech_voices"),
        1
    );

    // A described voice keeps its description as its name.
    let described = seed_named_voice(&database, &paths, "a calm older woman");
    let kept = keep_voice(&database, &described.id);
    let named = own(kept)
        .into_iter()
        .find(|voice| voice.id == described.id)
        .unwrap();
    assert_eq!(named.name, "a calm older woman");
}

#[test]
fn a_voice_description_is_short_and_has_no_brackets() {
    let (database, _) = library();
    let paths = temporary_paths("describe");
    for description in ["x".repeat(121), "[whispering] a spy".to_string()] {
        let refused = tauri::async_runtime::block_on(try_speech_voice(
            &database,
            &paths,
            &runtime(),
            TrySpeechVoiceInput {
                description: Some(description),
            },
        ))
        .unwrap_err();
        assert_eq!(refused.code, "speech_voice_description_invalid");
    }
}

#[test]
fn a_lesson_keeps_the_voice_it_was_first_spoken_in() {
    let (database, lesson_id) = library();
    let paths = temporary_paths("lesson");
    let first = seed_voice(&database, &paths);
    keep_voice(&database, &first.id);

    assert_eq!(
        voice_for(&database, &lesson_id),
        (DRAMA.to_string(), first.voice.clone())
    );
    assert_eq!(lesson_voice_of(&database, &lesson_id), first.voice);
    // The request carries the voice's sample and the words it says.
    let carried = reference(&database, &paths, &first.voice).unwrap().unwrap();
    assert_eq!(carried.audio, mp3_bytes());
    assert_eq!(carried.transcript, SAMPLE);
    assert!(reference(&database, &paths, "bm_george").unwrap().is_none());

    // Choosing another voice leaves the lesson in its own; a lesson spoken
    // for the first time now takes the new one.
    let second = seed_voice(&database, &paths);
    keep_voice(&database, &second.id);
    assert_eq!(voice_for(&database, &lesson_id).1, first.voice);
    let other = new_lesson(&database);
    assert_eq!(voice_for(&database, &other).1, second.voice);

    // Removing the first voice hides it but keeps it for its lesson.
    let removed = database
        .with_connection(|connection| remove(connection, &first.id, NOW))
        .unwrap();
    assert!(removed.voices.iter().all(|voice| voice.id != first.id));
    assert_eq!(voice_for(&database, &lesson_id).1, first.voice);
    assert_eq!(
        query::<String>(
            &database,
            &format!("SELECT state FROM speech_voices WHERE id = '{}'", first.id)
        ),
        "removed"
    );

    // A lesson whose voice lost its recording takes the voice in use.
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE assets SET lifecycle_state = 'missing'
                     WHERE id = (SELECT asset_id FROM speech_voices WHERE id = ?1)",
                    [&first.id],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(
        reference(&database, &paths, &first.voice).unwrap_err().code,
        "speech_voice_missing"
    );
    assert_eq!(voice_for(&database, &lesson_id).1, second.voice);
    assert_eq!(lesson_voice_of(&database, &lesson_id), second.voice);

    // Deleting a lesson forgets its voice.
    database
        .with_connection(|connection| {
            connection
                .execute("DELETE FROM lessons WHERE id = ?1", [&lesson_id])
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(
        query::<i64>(&database, "SELECT COUNT(*) FROM lesson_voices"),
        1
    );
}

#[test]
fn a_model_with_preset_voices_speaks_its_preset_and_offers_none_to_try() {
    let (database, lesson_id) = library();
    let paths = temporary_paths("presets");
    database
        .with_connection(|connection| {
            save_settings(
                connection,
                SaveSpeechSettingsInput {
                    profile_id: Some(OPENROUTER.to_string()),
                    model_id: KOKORO.to_string(),
                    voice: "bm_george".to_string(),
                    expected_revision: 1,
                },
                NOW,
            )
        })
        .unwrap();
    let refused = tauri::async_runtime::block_on(try_speech_voice(
        &database,
        &paths,
        &runtime(),
        TrySpeechVoiceInput::default(),
    ))
    .unwrap_err();
    assert_eq!(refused.code, "speech_voice_presets");
    assert_eq!(
        voice_for(&database, &lesson_id),
        (KOKORO.to_string(), "bm_george".to_string())
    );
}

#[test]
fn two_voices_come_with_ostudy_and_the_relaxed_man_is_the_default() {
    let (database, lesson_id) = library();
    let paths = temporary_paths("built-in");
    let current = settings(&database);
    assert_eq!(current.voice, "builtin:relaxed-man");
    let built_in: Vec<_> = current
        .voices
        .iter()
        .map(|voice| {
            (
                voice.name.as_str(),
                voice.state.as_str(),
                voice.asset_id.is_none(),
            )
        })
        .collect();
    assert_eq!(
        built_in,
        vec![
            ("Relaxed man", "builtIn", true),
            ("Calm older woman", "builtIn", true)
        ]
    );

    // A lesson read with no voice chosen is read in the default, carrying its
    // sample and the words it says.
    assert_eq!(voice_for(&database, &lesson_id).1, "builtin:relaxed-man");
    let carried = reference(&database, &paths, "builtin:relaxed-man")
        .unwrap()
        .unwrap();
    assert_eq!(carried.audio, BUILT_IN_VOICES[0].audio);
    assert_eq!(carried.transcript, SAMPLE);
    assert!(carried.audio.len() > 10_000);
    for voice in BUILT_IN_VOICES {
        assert!(matches!(
            crate::media::inspect_media_bytes(voice.audio).unwrap().kind,
            crate::media::MediaKind::Audio
        ));
    }

    // The other can be chosen, but neither renamed nor removed.
    let chosen = keep_voice(&database, "calm-older-woman");
    assert_eq!(chosen.voice, "builtin:calm-older-woman");
    let other = new_lesson(&database);
    assert_eq!(voice_for(&database, &other).1, "builtin:calm-older-woman");
    assert_eq!(voice_for(&database, &lesson_id).1, "builtin:relaxed-man");
    assert!(database
        .with_connection(|connection| remove(connection, "calm-older-woman", NOW))
        .is_err());
    let unknown = database
        .with_connection(|connection| {
            save_settings(
                connection,
                SaveSpeechSettingsInput {
                    profile_id: Some(OPENROUTER.to_string()),
                    model_id: DRAMA.to_string(),
                    voice: "builtin:nobody".to_string(),
                    expected_revision: chosen.revision,
                },
                NOW,
            )
        })
        .unwrap_err();
    assert_eq!(unknown.code, "speech_voice_unknown");
}

#[test]
fn a_voice_a_podcast_host_speaks_in_is_kept_when_removed() {
    let (database, _) = library();
    let paths = temporary_paths("podcast-host");
    let host = seed_voice(&database, &paths);
    keep_voice(&database, &host.id);
    let spare = seed_voice(&database, &paths);
    keep_voice(&database, &spare.id);
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO podcasts(id, subject_id, title, target_minutes, model_id,
                         first_voice, second_voice, script_json, provenance_json, created_at)
                     VALUES (?1, ?2, 'Renal', 15, ?3, ?4, 'builtin:relaxed-man', '{}', '{}', 1)",
                    params!["d".repeat(32), SUBJECT, DRAMA, host.voice],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    database
        .with_connection(|connection| remove(connection, &host.id, NOW))
        .unwrap();
    assert_eq!(
        query::<String>(
            &database,
            &format!("SELECT state FROM speech_voices WHERE id = '{}'", host.id)
        ),
        "removed"
    );
    assert!(speaks_for_test(&database, &host.voice));
}

fn speaks_for_test(database: &Database, voice: &str) -> bool {
    database
        .with_connection(|connection| speaks(connection, voice))
        .unwrap()
}
