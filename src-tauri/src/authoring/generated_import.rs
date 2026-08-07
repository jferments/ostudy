use super::*;

// Strict, direct import for LLM-generated `.ostudy.json` flashcard files. The
// importer reuses canonical authoring validation and persistence rather than
// maintaining a parallel card-storage path.

const GENERATED_IMPORT_FORMAT: &str = "ostudy-flashcards";
const GENERATED_IMPORT_VERSION: i64 = 1;
const MAX_GENERATED_IMPORT_BYTES: usize = 16 * 1024 * 1024;
const MAX_GENERATED_IMPORT_CARDS: usize = 10_000;
const MAX_GENERATED_IMPORT_ERROR_EXAMPLES: usize = 20;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum GeneratedImportDeckMode {
    Create,
    Append,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratedImportFile {
    format: String,
    format_version: i64,
    deck: GeneratedImportDeck,
    #[serde(default)]
    default_tags: Vec<String>,
    cards: Vec<GeneratedImportCard>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedImportDeck {
    mode: GeneratedImportDeckMode,
    name: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum GeneratedImportCard {
    Basic {
        front: String,
        back: String,
        #[serde(default, rename = "reverseEnabled")]
        reverse_enabled: bool,
        #[serde(default)]
        tags: Vec<String>,
    },
    Cloze {
        text: String,
        #[serde(default = "default_stage3_cloze_mode")]
        mode: String,
        #[serde(default)]
        tags: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardImportResultDto {
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
}

struct PreparedImportNote {
    prepared: PreparedGeneratedNote,
    tags: Vec<ValidatedTag>,
    normalized: (String, String),
    duplicate_hash: String,
}

struct PreparedImport {
    mode: GeneratedImportDeckMode,
    name: String,
    description: String,
    notes: Vec<PreparedImportNote>,
}

#[tauri::command]
pub fn import_generated_flashcards(
    database: State<'_, Database>,
    source_path: String,
) -> Result<FlashcardImportResultDto, CommandError> {
    let prepared = prepare_flashcard_import(&source_path)?;
    database.with_connection(|connection| import_prepared_flashcards(connection, prepared))
}

fn prepare_flashcard_import(source_path: &str) -> Result<PreparedImport, CommandError> {
    let source = std::fs::canonicalize(source_path).map_err(CommandError::io)?;
    if !source.is_file() {
        return Err(CommandError::new(
            "invalid_flashcard_import",
            "Select an ostudy `.studydeck` or generated `.ostudy.json` file.",
        ));
    }
    let bytes = std::fs::read(&source).map_err(CommandError::io)?;
    prepare_flashcard_import_bytes(&bytes)
}

fn prepare_flashcard_import_bytes(bytes: &[u8]) -> Result<PreparedImport, CommandError> {
    if bytes.is_empty() || bytes.len() > MAX_GENERATED_IMPORT_BYTES {
        return Err(CommandError::new(
            "invalid_flashcard_import_size",
            "The generated flashcard file is empty or exceeds the 16 MB limit.",
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        CommandError::new(
            "invalid_flashcard_import_utf8",
            "The generated flashcard file must be valid UTF-8 JSON.",
        )
    })?;
    let parsed: GeneratedImportFile = serde_json::from_str(text).map_err(|error| {
        CommandError::new(
            "invalid_flashcard_import_json",
            format!(
                "Could not parse generated flashcards at line {}, column {}: {error}",
                error.line(),
                error.column()
            ),
        )
    })?;

    if parsed.format != GENERATED_IMPORT_FORMAT || parsed.format_version != GENERATED_IMPORT_VERSION {
        return Err(CommandError::new(
            "unsupported_flashcard_import",
            "This is not a supported ostudy generated-flashcard file (expected format `ostudy-flashcards`, version 1).",
        ));
    }

    let name = crate::decks::validate_name(parsed.deck.name)?;
    let description = crate::decks::validate_description(parsed.deck.description)?;
    if parsed.deck.mode == GeneratedImportDeckMode::Append && !description.is_empty() {
        return Err(CommandError::new(
            "invalid_flashcard_import_deck",
            "deck.description must be omitted or empty when deck.mode is `append`.",
        ));
    }
    if parsed.cards.is_empty() {
        return Err(CommandError::new(
            "empty_flashcard_import",
            "The generated flashcard file does not contain any cards.",
        ));
    }
    if parsed.cards.len() > MAX_GENERATED_IMPORT_CARDS {
        return Err(CommandError::new(
            "too_many_flashcard_import_cards",
            format!(
                "A generated flashcard file may contain at most {MAX_GENERATED_IMPORT_CARDS} source notes."
            ),
        ));
    }

    validate_import_tag_syntax(&parsed.default_tags, "defaultTags")?;
    let mut notes = Vec::with_capacity(parsed.cards.len());
    let mut error_count = 0_usize;
    let mut error_examples = Vec::new();
    for (index, card) in parsed.cards.into_iter().enumerate() {
        match prepare_import_card(index, &parsed.default_tags, card) {
            Ok(note) => notes.push(note),
            Err(error) => {
                error_count += 1;
                if error_examples.len() < MAX_GENERATED_IMPORT_ERROR_EXAMPLES {
                    error_examples.push(error.message);
                }
            }
        }
    }
    if error_count > 0 {
        return Err(CommandError::new(
            "invalid_flashcard_import_cards",
            format_import_card_errors(error_count, &error_examples),
        ));
    }

    Ok(PreparedImport {
        mode: parsed.deck.mode,
        name,
        description,
        notes,
    })
}

fn format_import_card_errors(total: usize, examples: &[String]) -> String {
    let mut message = format!(
        "{total} {} could not be imported:\n{}",
        if total == 1 { "card" } else { "cards" },
        examples.join("\n")
    );
    let omitted = total.saturating_sub(examples.len());
    if omitted > 0 {
        message.push_str(&format!(
            "\n… {omitted} additional {} omitted from this diagnostic.",
            if omitted == 1 { "error was" } else { "errors were" }
        ));
    }
    message
}

fn import_prepared_flashcards(
    connection: &mut Connection,
    prepared: PreparedImport,
) -> Result<FlashcardImportResultDto, CommandError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let now = unix_time_millis()?;
    let append_mode = prepared.mode == GeneratedImportDeckMode::Append;
    let deck_id = resolve_generated_import_deck(
        &transaction,
        prepared.mode,
        &prepared.name,
        &prepared.description,
        now,
    )?;

    let mut seen = HashSet::new();
    let mut created_notes = 0_i64;
    let mut created_cards = 0_i64;
    let mut skipped = 0_i64;

    for note in prepared.notes {
        let PreparedImportNote {
            prepared,
            tags,
            normalized,
            duplicate_hash,
        } = note;
        let duplicate_in_batch = !seen.insert(normalized.clone());
        let duplicate_in_deck = has_exact_duplicate(
            &transaction,
            &deck_id,
            &duplicate_hash,
            &normalized.0,
            &normalized.1,
            None,
        )?;
        if duplicate_in_batch || duplicate_in_deck {
            skipped += 1;
            continue;
        }

        let PreparedGeneratedNote {
            note_type,
            reverse_enabled,
            cloze_source,
            cloze_mode,
            content,
            cards,
        } = prepared;
        let note_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, duplicate_hash,
                     reverse_enabled, cloze_source, cloze_mode,
                     created_at, updated_at, revision
                 ) VALUES (
                     ?1, ?2, ?3, 'tiptap-json', 1,
                     ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, 1
                 )",
                params![
                    note_id,
                    deck_id,
                    note_type,
                    content.front_doc_json,
                    content.back_doc_json,
                    content.front_text,
                    content.back_text,
                    duplicate_hash,
                    crate::util::bool_to_integer(reverse_enabled),
                    cloze_source,
                    cloze_mode,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        sync_generated_cards(&transaction, &note_id, &deck_id, &cards, now)?;
        replace_note_assets(&transaction, &note_id, &content)?;
        replace_note_tags(&transaction, &note_id, &tags, now)?;
        created_notes += 1;
        created_cards += i64::try_from(cards.len()).unwrap_or(i64::MAX);
    }

    if append_mode && created_notes > 0 {
        transaction
            .execute(
                "UPDATE decks
                 SET updated_at = ?2, revision = revision + 1
                 WHERE id = ?1",
                params![deck_id, now],
            )
            .map_err(CommandError::database)?;
    }

    transaction.commit().map_err(CommandError::database)?;
    Ok(FlashcardImportResultDto {
        deck_id,
        created_note_count: created_notes,
        created_card_count: created_cards,
        skipped_duplicate_count: skipped,
    })
}

fn resolve_generated_import_deck(
    connection: &Connection,
    mode: GeneratedImportDeckMode,
    name: &str,
    description: &str,
    now: i64,
) -> Result<String, CommandError> {
    match mode {
        GeneratedImportDeckMode::Create => {
            insert_generated_import_deck(connection, name, description, now)
        }
        GeneratedImportDeckMode::Append => {
            let mut statement = connection
                .prepare(
                    "SELECT id
                     FROM decks
                     WHERE name = ?1
                       AND deleted_at IS NULL
                       AND archived_at IS NULL
                     ORDER BY created_at ASC, id ASC",
                )
                .map_err(CommandError::database)?;
            let rows = statement
                .query_map([name], |row| row.get::<_, String>(0))
                .map_err(CommandError::database)?;
            let mut ids = Vec::new();
            for row in rows {
                ids.push(row.map_err(CommandError::database)?);
            }
            match ids.as_slice() {
                [deck_id] => Ok(deck_id.clone()),
                [] => Err(CommandError::new(
                    "generated_import_deck_not_found",
                    format!(
                        "The JSON file requested append mode, but no active deck named “{name}” exists."
                    ),
                )),
                _ => Err(CommandError::new(
                    "generated_import_deck_ambiguous",
                    format!(
                        "The JSON file requested append mode, but more than one active deck is named “{name}”. Rename the destination deck so the match is unique."
                    ),
                )),
            }
        }
    }
}

fn prepare_import_card(
    index: usize,
    default_tags: &[String],
    card: GeneratedImportCard,
) -> Result<PreparedImportNote, CommandError> {
    let path = format!("cards[{index}]");
    let (draft, card_tags) = match card {
        GeneratedImportCard::Basic {
            front,
            back,
            reverse_enabled,
            tags,
        } => {
            let front_doc = markdown_document(&front, &format!("{path}.front"))?;
            let back_doc = markdown_document(&back, &format!("{path}.back"))?;
            (
                GeneratedNoteDraft {
                    card_type: "basic".to_string(),
                    reverse_enabled,
                    cloze_source: String::new(),
                    cloze_mode: "one_at_a_time".to_string(),
                    front_doc,
                    back_doc,
                    front_text: String::new(),
                    back_text: String::new(),
                },
                tags,
            )
        }
        GeneratedImportCard::Cloze { text, mode, tags } => (
            GeneratedNoteDraft {
                card_type: "cloze".to_string(),
                reverse_enabled: false,
                cloze_source: text,
                cloze_mode: mode,
                front_doc: json!({"type": "doc", "content": []}),
                back_doc: json!({"type": "doc", "content": []}),
                front_text: String::new(),
                back_text: String::new(),
            },
            tags,
        ),
    };

    validate_import_tag_syntax(&card_tags, &format!("{path}.tags"))?;
    let mut combined = default_tags.to_vec();
    combined.extend(card_tags);
    let tags = validate_tags(combined).map_err(|error| {
        CommandError::new(error.code, format!("{path}.tags: {}", error.message))
    })?;
    let prepared = prepare_generated_note(draft)
        .map_err(|error| CommandError::new(error.code, format!("{path}: {}", error.message)))?;
    let normalized = normalized_pair(
        &prepared.content.front_text,
        &prepared.content.back_text,
    );
    let duplicate_hash = duplicate_hash(&normalized.0, &normalized.1);

    Ok(PreparedImportNote {
        prepared,
        tags,
        normalized,
        duplicate_hash,
    })
}

fn validate_import_tag_syntax(tags: &[String], path: &str) -> Result<(), CommandError> {
    for (index, tag) in tags.iter().enumerate() {
        if tag.starts_with('#') {
            return Err(CommandError::new(
                "invalid_import_tag",
                format!("{path}[{index}]: omit the leading `#` from tag names."),
            ));
        }
    }
    Ok(())
}

fn insert_generated_import_deck(
    connection: &Connection,
    name: &str,
    description: &str,
    now: i64,
) -> Result<String, CommandError> {
    let deck_id = generate_id(connection)?;
    let sort_order: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM decks WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO decks (
                 id, name, description, sort_order, created_at, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1)",
            params![deck_id, name, description, sort_order, now],
        )
        .map_err(CommandError::database)?;
    Ok(deck_id)
}

fn markdown_document(source: &str, path: &str) -> Result<Value, CommandError> {
    if source.trim().is_empty() {
        return Err(CommandError::new(
            "empty_import_card_side",
            format!("{path}: add text to this card side."),
        ));
    }
    if source.contains("![") {
        return Err(unsupported_markdown(
            path,
            "images are not supported in version 1",
        ));
    }
    if source.contains('<') || source.contains('>') && source.contains("</") {
        return Err(unsupported_markdown(path, "raw HTML is not supported"));
    }
    if source.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with('|') && trimmed.ends_with('|')
    }) {
        return Err(unsupported_markdown(
            path,
            "Markdown tables are not supported in version 1",
        ));
    }

    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    let lines = normalized.lines().collect::<Vec<_>>();
    let mut nodes = Vec::new();
    let mut index = 0_usize;
    while index < lines.len() {
        let line = lines[index];
        if line.trim().is_empty() {
            index += 1;
            continue;
        }
        if line.trim_start().starts_with("```") {
            let language = line.trim_start().trim_start_matches("```").trim();
            index += 1;
            let mut code = Vec::new();
            while index < lines.len() && !lines[index].trim_start().starts_with("```") {
                code.push(lines[index]);
                index += 1;
            }
            if index == lines.len() {
                return Err(unsupported_markdown(
                    path,
                    "a fenced code block is not closed",
                ));
            }
            index += 1;
            nodes.push(json!({
                "type": "codeBlock",
                "attrs": {"language": if language.is_empty() { Value::Null } else { Value::String(language.to_string()) }},
                "content": [{"type": "text", "text": code.join("\n")}]
            }));
            continue;
        }
        if let Some(level) = heading_level(line) {
            let text = line.trim_start()[level + 1..].trim();
            nodes.push(json!({
                "type": "heading",
                "attrs": {"level": level},
                "content": inline_nodes(text, path)?
            }));
            index += 1;
            continue;
        }
        if matches!(line.trim(), "---" | "***" | "___") {
            nodes.push(json!({"type": "horizontalRule"}));
            index += 1;
            continue;
        }
        if line.trim_start().starts_with("> ") {
            let mut quoted = Vec::new();
            while index < lines.len() && lines[index].trim_start().starts_with("> ") {
                quoted.push(lines[index].trim_start()[2..].to_string());
                index += 1;
            }
            nodes.push(json!({
                "type": "blockquote",
                "content": [{"type": "paragraph", "content": inline_nodes(&quoted.join("\n"), path)?}]
            }));
            continue;
        }
        if bullet_item(line).is_some() {
            let mut items = Vec::new();
            while index < lines.len() {
                let Some(text) = bullet_item(lines[index]) else {
                    break;
                };
                items.push(json!({
                    "type": "listItem",
                    "content": [{"type": "paragraph", "content": inline_nodes(text, path)?}]
                }));
                index += 1;
            }
            nodes.push(json!({"type": "bulletList", "content": items}));
            continue;
        }
        if let Some((start, _)) = ordered_item(line) {
            let mut items = Vec::new();
            while index < lines.len() {
                let Some((_, text)) = ordered_item(lines[index]) else {
                    break;
                };
                items.push(json!({
                    "type": "listItem",
                    "content": [{"type": "paragraph", "content": inline_nodes(text, path)?}]
                }));
                index += 1;
            }
            nodes.push(json!({"type": "orderedList", "attrs": {"start": start}, "content": items}));
            continue;
        }

        let mut paragraph = vec![line.to_string()];
        index += 1;
        while index < lines.len() && !lines[index].trim().is_empty() && !is_block_start(lines[index])
        {
            paragraph.push(lines[index].to_string());
            index += 1;
        }
        nodes.push(json!({
            "type": "paragraph",
            "content": inline_nodes(&paragraph.join("\n"), path)?
        }));
    }
    Ok(json!({"type": "doc", "content": nodes}))
}

fn unsupported_markdown(path: &str, detail: &str) -> CommandError {
    CommandError::new(
        "unsupported_import_markdown",
        format!("{path}: {detail}."),
    )
}

fn heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let count = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    if (1..=4).contains(&count) && trimmed.as_bytes().get(count) == Some(&b' ') {
        Some(count)
    } else {
        None
    }
}

fn bullet_item(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    for prefix in ["- ", "* ", "+ "] {
        if let Some(value) = trimmed.strip_prefix(prefix) {
            return Some(value);
        }
    }
    None
}

fn ordered_item(line: &str) -> Option<(i64, &str)> {
    let trimmed = line.trim_start();
    let separator = trimmed.find(". ")?;
    let number = trimmed[..separator].parse::<i64>().ok()?;
    if number < 1 {
        return None;
    }
    Some((number, &trimmed[separator + 2..]))
}

fn is_block_start(line: &str) -> bool {
    let trimmed = line.trim_start();
    heading_level(line).is_some()
        || trimmed.starts_with("```")
        || trimmed.starts_with("> ")
        || bullet_item(line).is_some()
        || ordered_item(line).is_some()
        || matches!(trimmed, "---" | "***" | "___")
}

fn inline_nodes(text: &str, path: &str) -> Result<Vec<Value>, CommandError> {
    let mut nodes = Vec::new();
    let mut cursor = 0_usize;
    while cursor < text.len() {
        let remainder = &text[cursor..];
        if remainder.starts_with('\n') {
            nodes.push(json!({"type": "hardBreak"}));
            cursor += 1;
            continue;
        }
        if let Some((consumed, label, href)) = parse_link(remainder) {
            nodes.push(json!({
                "type": "text",
                "text": label,
                "marks": [{"type": "link", "attrs": {"href": href, "target": null, "rel": null, "class": null}}]
            }));
            cursor += consumed;
            continue;
        }
        let markers = [
            ("**", "bold"),
            ("__", "bold"),
            ("~~", "strike"),
            ("*", "italic"),
            ("_", "italic"),
            ("`", "code"),
        ];
        let mut matched = false;
        for (marker, mark_type) in markers {
            if let Some(after) = remainder.strip_prefix(marker) {
                if let Some(close) = after.find(marker) {
                    let value = &after[..close];
                    if value.is_empty() {
                        continue;
                    }
                    nodes.push(json!({
                        "type": "text",
                        "text": value,
                        "marks": [{"type": mark_type}]
                    }));
                    cursor += marker.len() + close + marker.len();
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }
        let next = next_inline_marker(remainder).unwrap_or(remainder.len());
        let take = next.max(1);
        let value = &remainder[..take];
        if value.contains('[') && value.contains("](") {
            return Err(unsupported_markdown(
                path,
                "a Markdown link is malformed",
            ));
        }
        nodes.push(json!({"type": "text", "text": value}));
        cursor += take;
    }
    Ok(nodes)
}

fn parse_link(value: &str) -> Option<(usize, &str, &str)> {
    if !value.starts_with('[') {
        return None;
    }
    let label_end = value.find("](")?;
    let href_start = label_end + 2;
    let href_end = value[href_start..].find(')')? + href_start;
    let label = &value[1..label_end];
    let href = &value[href_start..href_end];
    if label.is_empty() || href.is_empty() {
        return None;
    }
    Some((href_end + 1, label, href))
}

fn next_inline_marker(value: &str) -> Option<usize> {
    ["\n", "[", "**", "__", "~~", "*", "_", "`"]
        .iter()
        .filter_map(|marker| value.find(marker))
        .filter(|position| *position > 0)
        .min()
}

#[cfg(test)]
mod generated_import_tests {
    use super::*;

    fn generated_file(mode: &str, deck_name: &str, front: &str, back: &str) -> Vec<u8> {
        format!(
            r#"{{
              "format":"ostudy-flashcards",
              "formatVersion":1,
              "deck":{{"mode":"{mode}","name":"{deck_name}"}},
              "cards":[{{"type":"basic","front":"{front}","back":"{back}"}}]
            }}"#
        )
        .into_bytes()
    }

    #[test]
    fn markdown_builds_supported_rich_document() {
        let document = markdown_document(
            "# Heading\n\nA **bold** answer.\n\n- one\n- two",
            "cards[0].front",
        )
        .expect("Markdown should parse");
        crate::cards::validate_document(&document)
            .expect("document should pass canonical validation");
    }

    #[test]
    fn markdown_rejects_html_images_and_tables() {
        assert!(markdown_document("<script>x</script>", "front").is_err());
        assert!(markdown_document("![x](https://example.com/x.png)", "front").is_err());
        assert!(markdown_document("| A | B |", "front").is_err());
    }

    #[test]
    fn strict_json_requires_a_deck_mode_and_rejects_unknown_card_fields() {
        let missing_mode = br#"{
          "format":"ostudy-flashcards",
          "formatVersion":1,
          "deck":{"name":"Test"},
          "cards":[{"type":"basic","front":"Q","back":"A"}]
        }"#;
        assert!(prepare_flashcard_import_bytes(missing_mode).is_err());

        let unknown_field = br#"{
          "format":"ostudy-flashcards",
          "formatVersion":1,
          "deck":{"mode":"create","name":"Test"},
          "cards":[{"type":"basic","front":"Q","back":"A","overwrite":true}]
        }"#;
        assert!(prepare_flashcard_import_bytes(unknown_field).is_err());
    }

    #[test]
    fn create_then_append_uses_the_named_deck_and_skips_exact_duplicates() {
        let database = Database::open_in_memory().expect("database should open");
        let created = database
            .with_connection(|connection| {
                import_prepared_flashcards(
                    connection,
                    prepare_flashcard_import_bytes(&generated_file(
                        "create",
                        "Pharmacology",
                        "What reverses warfarin?",
                        "Vitamin K",
                    ))?,
                )
            })
            .expect("new deck import should succeed");
        assert_eq!(created.created_note_count, 1);
        assert_eq!(created.created_card_count, 1);

        let duplicate = database
            .with_connection(|connection| {
                import_prepared_flashcards(
                    connection,
                    prepare_flashcard_import_bytes(&generated_file(
                        "append",
                        "Pharmacology",
                        "What reverses warfarin?",
                        "Vitamin K",
                    ))?,
                )
            })
            .expect("duplicate append should succeed");
        assert_eq!(duplicate.deck_id, created.deck_id);
        assert_eq!(duplicate.created_note_count, 0);
        assert_eq!(duplicate.skipped_duplicate_count, 1);

        let appended = database
            .with_connection(|connection| {
                import_prepared_flashcards(
                    connection,
                    prepare_flashcard_import_bytes(&generated_file(
                        "append",
                        "Pharmacology",
                        "What monitors warfarin?",
                        "PT/INR",
                    ))?,
                )
            })
            .expect("unique append should succeed");
        assert_eq!(appended.deck_id, created.deck_id);
        assert_eq!(appended.created_note_count, 1);

        database
            .with_connection(|connection| {
                let count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM notes WHERE deck_id = ?1 AND deleted_at IS NULL",
                        [&created.deck_id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(count, 2);
                Ok(())
            })
            .expect("note count should be readable");
    }

    #[test]
    fn append_requires_one_exact_active_deck_name() {
        let database = Database::open_in_memory().expect("database should open");
        let missing = database
            .with_connection(|connection| {
                import_prepared_flashcards(
                    connection,
                    prepare_flashcard_import_bytes(&generated_file(
                        "append",
                        "Missing",
                        "Question",
                        "Answer",
                    ))?,
                )
            })
            .expect_err("missing append destination should fail");
        assert_eq!(missing.code, "generated_import_deck_not_found");

        database
            .with_connection(|connection| {
                let first = prepare_flashcard_import_bytes(&generated_file(
                    "create",
                    "Duplicate name",
                    "Question 1",
                    "Answer 1",
                ))?;
                import_prepared_flashcards(connection, first)?;
                let second = prepare_flashcard_import_bytes(&generated_file(
                    "create",
                    "Duplicate name",
                    "Question 2",
                    "Answer 2",
                ))?;
                import_prepared_flashcards(connection, second)?;
                Ok(())
            })
            .expect("duplicate deck names may be created explicitly");

        let ambiguous = database
            .with_connection(|connection| {
                import_prepared_flashcards(
                    connection,
                    prepare_flashcard_import_bytes(&generated_file(
                        "append",
                        "Duplicate name",
                        "Question 3",
                        "Answer 3",
                    ))?,
                )
            })
            .expect_err("ambiguous append destination should fail");
        assert_eq!(ambiguous.code, "generated_import_deck_ambiguous");
    }

    #[test]
    fn card_validation_reports_every_small_batch_error_before_writing() {
        let invalid = br#"{
          "format":"ostudy-flashcards",
          "formatVersion":1,
          "deck":{"mode":"create","name":"Invalid cards"},
          "cards":[
            {"type":"basic","front":"","back":"Answer"},
            {"type":"basic","front":"Question","back":"![image](https://example.com/image.png)"}
          ]
        }"#;
        let error = prepare_flashcard_import_bytes(invalid)
            .expect_err("invalid cards should be reported together");
        assert_eq!(error.code, "invalid_flashcard_import_cards");
        assert!(error.message.contains("cards[0].front"));
        assert!(error.message.contains("cards[1].back"));
    }

    #[test]
    fn large_invalid_batches_report_a_bounded_example_set() {
        let cards = (0..50)
            .map(|_| r#"{"type":"basic","front":"","back":"Answer"}"#)
            .collect::<Vec<_>>()
            .join(",");
        let invalid = format!(
            r#"{{"format":"ostudy-flashcards","formatVersion":1,"deck":{{"mode":"create","name":"Invalid cards"}},"cards":[{cards}]}}"#
        );
        let error = prepare_flashcard_import_bytes(invalid.as_bytes())
            .expect_err("invalid cards should be summarized");
        assert!(error.message.starts_with("50 cards could not be imported:"));
        assert!(error.message.contains("30 additional errors were omitted"));
        assert!(!error.message.contains("cards[20].front"));
    }
}
