//! Tutor replies read aloud in the learner's voice.
//!
//! Only a finished reply the library saved is spoken, never text still being
//! written, and only its public text: a quiz's private answer lives elsewhere
//! and never reaches a voice. The reply's Markdown is read as a person would
//! say it: marks are dropped, a link says its words, a table says its cells,
//! and code is left out. Long replies are spoken piece by piece through the
//! lesson narration, which keeps each piece's audio, so listening again costs
//! nothing.

use rusqlite::{params, OptionalExtension};
use serde::Deserialize;

use crate::{
    ai::AiRuntimeState,
    database::Database,
    error::CommandError,
    lessons::narration::{speak_piece, SpokenPieceDto},
    paths::AppPaths,
    util::validate_entity_id,
};

/// The operation a reply's speech is accounted under.
const PURPOSE: &str = "tutor_speech";
/// More pieces than a saved reply can hold; it bounds a bad request.
const MAX_PIECE: usize = 64;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpeakTutorReplyInput {
    pub turn_id: String,
    /// Which piece of the reply to speak, counted from 0.
    pub piece: usize,
}

pub(crate) async fn speak_tutor_reply(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    input: SpeakTutorReplyInput,
) -> Result<SpokenPieceDto, CommandError> {
    validate_entity_id(&input.turn_id, "Tutor reply")?;
    if input.piece > MAX_PIECE {
        return Err(CommandError::new(
            "speech_piece_missing",
            "There is no more to listen to.",
        ));
    }
    let text = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT assistant_text FROM tutor_turns WHERE id = ?1 AND state = 'complete'",
                params![input.turn_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)
    })?;
    let text = text.map(|text| spoken_reply(&text)).unwrap_or_default();
    if text.trim().is_empty() {
        return Err(CommandError::new(
            "tutor_reply_unspoken",
            "This reply has nothing to read aloud.",
        ));
    }
    speak_piece(
        database,
        paths,
        runtime,
        &text,
        input.piece,
        format!("tutor:{}", input.turn_id),
        PURPOSE,
    )
    .await
}

/// A reply's Markdown as it is said aloud: one line of plain sentences.
pub(crate) fn spoken_reply(markdown: &str) -> String {
    let mut spoken = Vec::new();
    let mut fence: Option<&str> = None;
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(&trimmed[..3]);
            continue;
        }
        if trimmed.starts_with('|') {
            if let Some(row) = table_row(trimmed) {
                spoken.push(sentence(&row));
            }
            continue;
        }
        let (block, ends_sentence) = block_text(trimmed);
        let text = inline_text(block);
        if text.is_empty() {
            continue;
        }
        spoken.push(if ends_sentence { sentence(&text) } else { text });
    }
    spoken.join(" ")
}

/// A line without its block marks, and whether it stands alone (a heading or
/// a list item), so it ends as a sentence even without a full stop.
fn block_text(line: &str) -> (&str, bool) {
    let mut line = line;
    while let Some(rest) = line.strip_prefix('>') {
        line = rest.trim_start();
    }
    let hashes = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        return (line[hashes..].trim(), true);
    }
    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(bullet) {
            return (rest.trim(), true);
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        let rest = &line[digits..];
        if let Some(rest) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return (rest.trim(), true);
        }
    }
    if line
        .chars()
        .all(|character| matches!(character, '-' | '*' | '_' | ' '))
    {
        return ("", false);
    }
    (line, false)
}

/// A table row's cells, or none for the line under its header.
fn table_row(line: &str) -> Option<String> {
    let cells: Vec<String> = line
        .trim_matches('|')
        .split('|')
        .map(|cell| inline_text(cell.trim()))
        .filter(|cell| !cell.is_empty())
        .collect();
    if cells.iter().all(|cell| {
        cell.chars()
            .all(|character| matches!(character, '-' | ':' | ' '))
    }) {
        return None;
    }
    Some(cells.join(", "))
}

/// Text without inline marks: links say their words, images and addresses
/// are left out, and emphasis, code and math marks are dropped.
fn inline_text(text: &str) -> String {
    let mut out = String::new();
    let characters: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        let image = character == '!' && characters.get(index + 1) == Some(&'[');
        if character == '[' || image {
            let open = if image { index + 1 } else { index };
            if let Some((words, after)) = link(&characters, open) {
                if !image {
                    out.push_str(&inline_text(&words));
                }
                index = after;
                continue;
            }
        }
        if character == 'h' && starts_address(&characters[index..]) {
            while index < characters.len() && !characters[index].is_whitespace() {
                index += 1;
            }
            continue;
        }
        // Math delimiters such as \( and \] are not said.
        if character == '\\' && matches!(characters.get(index + 1), Some('(' | ')' | '[' | ']')) {
            index += 2;
            continue;
        }
        let word_inside = |offset: isize| {
            index
                .checked_add_signed(offset)
                .and_then(|at| characters.get(at))
                .is_some_and(|neighbor| neighbor.is_alphanumeric())
        };
        match character {
            '*' | '`' | '$' => {}
            // Snake_case keeps its underscores; emphasis loses them.
            '_' if !(word_inside(-1) && word_inside(1)) => {}
            _ => out.push(character),
        }
        index += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `[words](address)` starting at `open`: the words and the index after it.
fn link(characters: &[char], open: usize) -> Option<(String, usize)> {
    let close = open
        + characters[open..]
            .iter()
            .position(|character| *character == ']')?;
    if characters.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = close
        + 1
        + characters[close + 1..]
            .iter()
            .position(|character| *character == ')')?;
    Some((characters[open + 1..close].iter().collect(), end + 1))
}

fn starts_address(characters: &[char]) -> bool {
    let start: String = characters.iter().take(8).collect();
    start.starts_with("http://") || start.starts_with("https://")
}

/// Ends with a full stop unless it already ends a sentence, so the voice
/// pauses after a heading, an item or a row.
fn sentence(text: &str) -> String {
    let text = text.trim_end_matches(':');
    if text.ends_with(['.', '!', '?']) {
        text.to_string()
    } else {
        format!("{text}.")
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;

    const CONVERSATION: &str = "c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0";
    const FINISHED: &str = "d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1";
    const RUNNING: &str = "e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2";

    fn library() -> Database {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO tutor_conversations(
                             id, operation_id, title, scope_json, created_at, updated_at
                         ) VALUES (?1, ?1, 'Chat', '{}', 1, 1)",
                        params![CONVERSATION],
                    )
                    .map_err(CommandError::database)?;
                for (sequence, (id, state, text)) in [
                    (FINISHED, "complete", "The **heart** pumps."),
                    (RUNNING, "running", "Half a"),
                ]
                .into_iter()
                .enumerate()
                {
                    connection
                        .execute(
                            "INSERT INTO tutor_turns(
                                 id, conversation_id, operation_id, sequence, request_hash,
                                 request_json, mode, user_text, assistant_text, state,
                                 profile_revision, provider_kind, model_id, created_at, updated_at
                             ) VALUES (?1, ?2, ?1, ?3, ?4, '{}', 'explain', 'Why?', ?5, ?6,
                                       1, 'openrouter', 'model', 1, 1)",
                            params![
                                id,
                                CONVERSATION,
                                sequence as i64 + 1,
                                "0".repeat(64),
                                text,
                                state
                            ],
                        )
                        .map_err(CommandError::database)?;
                }
                Ok(())
            })
            .unwrap();
        database
    }

    fn speak(
        database: &Database,
        turn_id: &str,
        piece: usize,
    ) -> Result<SpokenPieceDto, CommandError> {
        let paths = AppPaths::new(
            std::env::temp_dir().join(format!("ostudy-tutor-speech-{}", std::process::id())),
        )
        .unwrap();
        let runtime = AiRuntimeState::new(crate::platform::unavailable_secret_store()).unwrap();
        tauri::async_runtime::block_on(speak_tutor_reply(
            database,
            &paths,
            &runtime,
            SpeakTutorReplyInput {
                turn_id: turn_id.to_string(),
                piece,
            },
        ))
    }

    #[test]
    fn only_a_finished_reply_is_read_and_only_through_a_configured_voice() {
        let database = library();
        // Text still being written, or a reply that does not exist, is never spoken.
        assert_eq!(
            speak(&database, RUNNING, 0).unwrap_err().code,
            "tutor_reply_unspoken"
        );
        assert_eq!(
            speak(&database, &"f".repeat(32), 0).unwrap_err().code,
            "tutor_reply_unspoken"
        );
        // A finished reply needs a voice connection before anything is sent.
        assert_eq!(
            speak(&database, FINISHED, 0).unwrap_err().code,
            "speech_not_configured"
        );
        assert_eq!(
            speak(&database, FINISHED, 65).unwrap_err().code,
            "speech_piece_missing"
        );
        assert_eq!(
            speak(&database, "bad", 0).unwrap_err().code,
            "invalid_identifier"
        );
    }

    #[test]
    fn marks_are_dropped_and_blocks_end_as_sentences() {
        let reply = "## Why it works\n\nThe **heart** pumps _blood_ through `arteries`.\n\n\
                     - Left side: body\n- Right side: lungs\n\n1. First\n2) Second\n\n> A quote\n\n---";
        assert_eq!(
            spoken_reply(reply),
            "Why it works. The heart pumps blood through arteries. Left side: body. \
             Right side: lungs. First. Second. A quote"
        );
    }

    #[test]
    fn links_say_their_words_and_images_code_and_addresses_are_left_out() {
        let reply = "See [the guide](https://example.com/a) and https://example.com/b now.\n\
                     ![diagram](image.png)\n\n```python\nprint('hi')\n```\nAfter $x^2$, \\(y\\) and snake_case.";
        assert_eq!(
            spoken_reply(reply),
            "See the guide and now. After x^2, y and snake_case."
        );
    }

    #[test]
    fn a_table_says_its_cells_row_by_row() {
        let reply = "| Ion | Charge |\n| --- | :---: |\n| Na⁺ | +1 |\n| Cl⁻ | −1 |";
        assert_eq!(spoken_reply(reply), "Ion, Charge. Na⁺, +1. Cl⁻, −1.");
    }

    #[test]
    fn a_reply_of_only_code_says_nothing() {
        assert_eq!(spoken_reply("```\nlet x = 1;\n```"), "");
    }
}
