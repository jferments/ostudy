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

/// A library whose voice is `model` through a keyless OpenRouter connection,
/// with one written lesson. Nothing here reaches the network.
fn library(model: &str) -> (Database, String) {
    let database = Database::open_in_memory().unwrap();
    let lesson_id = database
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
                    model_id: model.to_string(),
                    voice: if model == KOKORO { "af_heart" } else { "" }.to_string(),
                    expected_revision: 0,
                },
                NOW,
            )?;
            Ok(create_authored_lesson(connection, SUBJECT, loop_of_henle(&[]), NOW)?.id)
        })
        .unwrap();
    (database, lesson_id)
}

fn script(lesson_id: &str) -> PodcastScript {
    PodcastScript {
        schema_version: PODCAST_SCRIPT_VERSION,
        title: "From filtration to urine".into(),
        chapters: vec![PodcastChapter {
            lesson_id: lesson_id.to_string(),
            revision_number: 1,
            title: "The loop of Henle".into(),
            turns: vec![
                PodcastTurn {
                    host: 0,
                    text: "Today we follow water through the kidney.".into(),
                },
                PodcastTurn {
                    host: 1,
                    text: "So where does it start?".into(),
                },
            ],
        }],
    }
}

fn keep_podcast(database: &Database, lesson_id: &str, voices: [String; 2]) -> String {
    database
        .with_connection(|connection| {
            let model = resolved(connection)?.model;
            keep(
                connection,
                SUBJECT,
                15,
                model,
                &voices,
                &script(lesson_id),
                &serde_json::json!({}),
                NOW,
            )
        })
        .unwrap()
}

fn count(database: &Database, sql: &str) -> i64 {
    database
        .with_connection(|connection| {
            connection
                .query_row(sql, [], |row| row.get(0))
                .map_err(CommandError::database)
        })
        .unwrap()
}

#[test]
fn a_kept_podcast_lists_reads_with_turn_keys_and_outlives_its_lessons() {
    let (database, lesson_id) = library(KOKORO);
    let id = keep_podcast(
        &database,
        &lesson_id,
        default_voices(speech::speech_model(KOKORO).unwrap()),
    );

    let listed = database
        .with_connection(|connection| list(connection, SUBJECT))
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, "From filtration to urine");
    assert_eq!(listed[0].lesson_titles, ["The loop of Henle"]);
    assert_eq!(listed[0].minutes, 1);

    let podcast: PodcastDto = database
        .with_connection(|connection| Ok(kept(connection, &id)?.into()))
        .unwrap();
    assert_eq!(podcast.hosts[0].name, "Ada");
    assert_eq!(podcast.hosts[0].voice, "af_heart");
    assert_eq!(podcast.hosts[1].voice, "am_michael");
    let turns = &podcast.chapters[0].turns;
    assert_eq!(turns[1].key, "0.1");
    assert_eq!(turns[1].host, 1);

    // The podcast is its own text: rewriting or deleting a lesson keeps it,
    // and deleting the subject deletes it.
    database
        .with_connection(|connection| {
            connection
                .execute("DELETE FROM lessons WHERE id = ?1", [&lesson_id])
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(count(&database, "SELECT COUNT(*) FROM podcasts"), 1);
    database
        .with_connection(|connection| {
            connection
                .execute("DELETE FROM document_collections WHERE id = ?1", [SUBJECT])
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(count(&database, "SELECT COUNT(*) FROM podcasts"), 0);
}

#[test]
fn a_damaged_podcast_does_not_play_or_hide_the_others() {
    let (database, lesson_id) = library(KOKORO);
    let voices = default_voices(speech::speech_model(KOKORO).unwrap());
    let good = keep_podcast(&database, &lesson_id, voices.clone());
    let damaged = keep_podcast(&database, &lesson_id, voices);
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE podcasts SET script_json = '{\"schemaVersion\":1}' WHERE id = ?1",
                    [&damaged],
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    let listed = database
        .with_connection(|connection| list(connection, SUBJECT))
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, good);
    let error = database
        .with_connection(|connection| kept(connection, &damaged).map(|_| ()))
        .unwrap_err();
    assert_eq!(error.code, "podcast_script_invalid");

    database
        .with_connection(|connection| delete(connection, &good))
        .unwrap();
    assert_eq!(count(&database, "SELECT COUNT(*) FROM podcasts"), 1);
}

#[test]
fn scripts_keep_to_their_hosts_length_and_spoken_text() {
    let lesson_id = "a".repeat(32);
    let valid = script(&lesson_id);
    validate(&valid, 5).unwrap();

    let mut stranger = valid.clone();
    stranger.chapters[0].turns[0].host = 2;
    let mut directed = valid.clone();
    directed.chapters[0].turns[0].text = "[laughs] Right.".into();
    let mut lonely = valid.clone();
    lonely.chapters[0].turns.pop();
    let mut repeated = valid.clone();
    repeated.chapters.push(repeated.chapters[0].clone());
    let mut long = valid.clone();
    long.chapters[0].turns = (0..20)
        .map(|_| PodcastTurn {
            host: 0,
            text: "word ".repeat(150).trim().to_string(),
        })
        .collect();
    for script in [stranger, directed, lonely, repeated, long] {
        assert_eq!(
            validate(&script, 5).unwrap_err().code,
            "podcast_script_invalid"
        );
    }
}

#[test]
fn a_podcast_is_written_only_from_this_subjects_lessons_and_allowed_voices() {
    let (database, lesson_id) = library(DRAMA);
    let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
    let write_with = |lesson_ids: Vec<String>, minutes: u32, voices: Option<[String; 2]>| {
        tauri::async_runtime::block_on(write(
            &database,
            &runtime,
            WritePodcastInput {
                subject_id: SUBJECT.to_string(),
                lesson_ids,
                target_minutes: minutes,
                voices,
                operation_id: "d".repeat(32),
            },
        ))
        .unwrap_err()
        .code
    };
    assert_eq!(write_with(vec![], 15, None), "podcast_lessons_invalid");
    assert_eq!(
        write_with(vec![lesson_id.clone(), lesson_id.clone()], 15, None),
        "podcast_lessons_invalid"
    );
    assert_eq!(
        write_with(vec!["b".repeat(32)], 15, None),
        "podcast_lessons_invalid"
    );
    assert_eq!(
        write_with(vec![lesson_id.clone()], 45, None),
        "podcast_length_invalid"
    );
    // Drama 3 speaks only the learner's voices, not a preset.
    assert_eq!(
        write_with(
            vec![lesson_id.clone()],
            15,
            Some(["af_heart".into(), voices::default_voice()])
        ),
        "podcast_voice_invalid"
    );
    assert_eq!(count(&database, "SELECT COUNT(*) FROM podcasts"), 0);
}

#[test]
fn hosts_keep_their_voices_and_fall_back_to_their_own_default() {
    let (database, lesson_id) = library(DRAMA);
    let drama = speech::speech_model(DRAMA).unwrap();
    let defaults = default_voices(drama);
    assert_eq!(
        defaults,
        ["builtin:calm-older-woman", "builtin:relaxed-man"]
    );
    let settings = database
        .with_connection(|connection| resolved(connection))
        .unwrap();
    for (host, voice) in defaults.iter().enumerate() {
        let (model, chosen) = host_voice(&database, &settings, DRAMA, voice, host).unwrap();
        assert_eq!((model.id, chosen.as_str()), (DRAMA, voice.as_str()));
    }
    // A saved voice that is gone gives way to that host's default, so the two
    // hosts still sound different.
    let gone = voices::voice_key(&"e".repeat(32));
    assert_eq!(
        host_voice(&database, &settings, DRAMA, &gone, 1).unwrap().1,
        defaults[1]
    );
    assert_eq!(
        host_voice(&database, &settings, DRAMA, &gone, 0).unwrap().1,
        defaults[0]
    );
    let id = keep_podcast(&database, &lesson_id, defaults);
    assert!(!id.is_empty());
}
