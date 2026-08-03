//! Strict content-only import for LLM-generated `.ostudy.json` files.
//!
//! This file is included inside the `authoring` module so imports reuse the
//! ordinary card validation, duplicate, tag, generated-card, and scheduling
//! implementation instead of maintaining a parallel write path.

const GENERATED_IMPORT_FORMAT: &str = "ostudy-flashcards";
const GENERATED_IMPORT_VERSION: i64 = 1;
const MAX_GENERATED_IMPORT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_GENERATED_IMPORT_CARDS: usize = 10_000;

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
    name: String,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum GeneratedImportCard {
    #[serde(deny_unknown_fields)]
    Basic {
        front: String,
        back: String,
        #[serde(default, rename = "reverseEnabled")]
        reverse_enabled: bool,
        #[serde(default)]
        tags: Vec<String>,
    },
    #[serde(deny_unknown_fields)]
    Cloze {
        text: String,
        #[serde(default = "default_stage3_cloze_mode")]
        mode: String,
        #[serde(default)]
        tags: Vec<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectFlashcardImportInput {
    pub source_path: String,
    #[serde(default)]
    pub destination_deck_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommitFlashcardImportInput {
    pub source_path: String,
    pub expected_fingerprint: String,
    #[serde(default)]
    pub destination_deck_id: Option<String>,
    #[serde(default)]
    pub allow_duplicates: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardImportSampleDto {
    pub card_type: String,
    pub question: String,
    pub answer: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardImportPreviewDto {
    pub fingerprint: String,
    pub deck_name: String,
    pub deck_description: String,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub basic_count: i64,
    pub reverse_enabled_count: i64,
    pub cloze_count: i64,
    pub duplicate_count: i64,
    pub unique_tags: Vec<String>,
    pub samples: Vec<FlashcardImportSampleDto>,
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
    tags: Vec<String>,
    normalized: (String, String),
    duplicate_hash: String,
    sample_type: String,
}

struct PreparedImport {
    fingerprint: String,
    name: String,
    description: String,
    notes: Vec<PreparedImportNote>,
}

#[tauri::command]
pub fn inspect_flashcard_import(
    database: State<'_, Database>,
    input: InspectFlashcardImportInput,
) -> Result<FlashcardImportPreviewDto, CommandError> {
    if let Some(deck_id) = input.destination_deck_id.as_deref() {
        validate_entity_id(deck_id, "destination deck")?;
    }
    let prepared = prepare_flashcard_import(&input.source_path)?;
    database.with_connection(|connection| {
        preview_prepared_import(connection, prepared, input.destination_deck_id.as_deref())
    })
}

#[tauri::command]
pub fn commit_flashcard_import(
    database: State<'_, Database>,
    input: CommitFlashcardImportInput,
) -> Result<FlashcardImportResultDto, CommandError> {
    if input.expected_fingerprint.len() != 64
        || !input
            .expected_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(CommandError::new(
            "invalid_import_fingerprint",
            "The import preview fingerprint is invalid. Preview the file again.",
        ));
    }
    if let Some(deck_id) = input.destination_deck_id.as_deref() {
        validate_entity_id(deck_id, "destination deck")?;
    }
    let prepared = prepare_flashcard_import(&input.source_path)?;
    if !prepared
        .fingerprint
        .eq_ignore_ascii_case(&input.expected_fingerprint)
    {
        return Err(CommandError::new(
            "import_file_changed",
            "The selected JSON file changed after preview. Preview it again before importing.",
        ));
    }

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let now = unix_time_millis()?;
        let deck_id = match input.destination_deck_id {
            Some(deck_id) => {
                ensure_active_deck(&transaction, &deck_id)?;
                deck_id
            }
            None => insert_generated_import_deck(
                &transaction,
                &prepared.name,
                &prepared.description,
                now,
            )?,
        };

        let mut seen = HashSet::new();
        let mut created_notes = 0_i64;
        let mut created_cards = 0_i64;
        let mut skipped = 0_i64;

        for note in prepared.notes {
            let duplicate_in_batch = !seen.insert(note.normalized.clone());
            let duplicate_in_deck = has_exact_duplicate(
                &transaction,
                &deck_id,
                &note.duplicate_hash,
                &note.normalized.0,
                &note.normalized.1,
                None,
            )?;
            if !input.allow_duplicates && (duplicate_in_batch || duplicate_in_deck) {
                skipped += 1;
                continue;
            }

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
                        note.prepared.note_type,
                        note.prepared.content.front_doc_json,
                        note.prepared.content.back_doc_json,
                        note.prepared.content.front_text,
                        note.prepared.content.back_text,
                        note.duplicate_hash,
                        crate::util::bool_to_integer(note.prepared.reverse_enabled),
                        note.prepared.cloze_source,
                        note.prepared.cloze_mode,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
            sync_generated_cards(
                &transaction,
                &note_id,
                &deck_id,
                &note.prepared.cards,
                now,
            )?;
            replace_note_assets(&transaction, &note_id, &note.prepared.content)?;
            replace_note_tags(&transaction, &note_id, &note.tags, now)?;
            created_notes += 1;
            created_cards += i64::try_from(note.prepared.cards.len()).unwrap_or(i64::MAX);
        }

        transaction.commit().map_err(CommandError::database)?;
        Ok(FlashcardImportResultDto {
            deck_id,
            created_note_count: created_notes,
            created_card_count: created_cards,
            skipped_duplicate_count: skipped,
        })
    })
}

fn prepare_flashcard_import(source_path: &str) -> Result<PreparedImport, CommandError> {
    let source = std::fs::canonicalize(source_path).map_err(CommandError::io)?;
    if !source.is_file() {
        return Err(CommandError::new(
            "invalid_flashcard_import",
            "Select a generated `.ostudy.json` file.",
        ));
    }
    let metadata = std::fs::metadata(&source).map_err(CommandError::io)?;
    if metadata.len() == 0 || metadata.len() > MAX_GENERATED_IMPORT_BYTES {
        return Err(CommandError::new(
            "invalid_flashcard_import_size",
            "The generated flashcard file is empty or exceeds the 16 MB limit.",
        ));
    }
    let bytes = std::fs::read(&source).map_err(CommandError::io)?;
    let fingerprint = sha256_hex(&bytes);
    let text = std::str::from_utf8(&bytes).map_err(|_| {
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
    let default_tags = validate_import_tags(parsed.default_tags, "defaultTags")?;
    let mut notes = Vec::with_capacity(parsed.cards.len());
    for (index, card) in parsed.cards.into_iter().enumerate() {
        notes.push(prepare_import_card(index, &default_tags, card)?);
    }
    Ok(PreparedImport {
        fingerprint,
        name,
        description,
        notes,
    })
}

fn prepare_import_card(
    index: usize,
    default_tags: &[String],
    card: GeneratedImportCard,
) -> Result<PreparedImportNote, CommandError> {
    let path = format!("cards[{index}]");
    let (draft, card_tags, sample_type) = match card {
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
                if reverse_enabled { "basic_reverse" } else { "basic" }.to_string(),
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
            "cloze".to_string(),
        ),
    };
    let mut combined = default_tags.to_vec();
    combined.extend(validate_import_tags(card_tags, &format!("{path}.tags"))?);
    let tags = validate_tags(combined).map_err(|error| {
        CommandError::new(
            error.code,
            format!("{path}.tags: {}", error.message),
        )
    })?;
    let prepared = prepare_generated_note(draft).map_err(|error| {
        CommandError::new(error.code, format!("{path}: {}", error.message))
    })?;
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
        sample_type,
    })
}

fn validate_import_tags(tags: Vec<String>, path: &str) -> Result<Vec<String>, CommandError> {
    for (index, tag) in tags.iter().enumerate() {
        if tag.starts_with('#') {
            return Err(CommandError::new(
                "invalid_import_tag",
                format!("{path}[{index}]: omit the leading `#` from tag names."),
            ));
        }
    }
    validate_tags(tags).map_err(|error| {
        CommandError::new(error.code, format!("{path}: {}", error.message))
    })
}

fn preview_prepared_import(
    connection: &Connection,
    prepared: PreparedImport,
    destination_deck_id: Option<&str>,
) -> Result<FlashcardImportPreviewDto, CommandError> {
    if let Some(deck_id) = destination_deck_id {
        ensure_active_deck(connection, deck_id)?;
    }
    let mut seen = HashSet::new();
    let mut duplicates = 0_i64;
    let mut generated_cards = 0_i64;
    let mut basic = 0_i64;
    let mut reverse = 0_i64;
    let mut cloze = 0_i64;
    let mut tags = BTreeSet::new();
    let mut samples = Vec::new();

    for note in &prepared.notes {
        let duplicate_in_batch = !seen.insert(note.normalized.clone());
        let duplicate_in_deck = if let Some(deck_id) = destination_deck_id {
            has_exact_duplicate(
                connection,
                deck_id,
                &note.duplicate_hash,
                &note.normalized.0,
                &note.normalized.1,
                None,
            )?
        } else {
            false
        };
        if duplicate_in_batch || duplicate_in_deck {
            duplicates += 1;
        }
        generated_cards += i64::try_from(note.prepared.cards.len()).unwrap_or(i64::MAX);
        match note.sample_type.as_str() {
            "cloze" => cloze += 1,
            "basic_reverse" => {
                basic += 1;
                reverse += 1;
            }
            _ => basic += 1,
        }
        tags.extend(note.tags.iter().cloned());
        if samples.len() < 5 {
            samples.push(FlashcardImportSampleDto {
                card_type: note.sample_type.clone(),
                question: note.prepared.content.front_text.clone(),
                answer: note.prepared.content.back_text.clone(),
                tags: note.tags.clone(),
            });
        }
    }

    Ok(FlashcardImportPreviewDto {
        fingerprint: prepared.fingerprint,
        deck_name: prepared.name,
        deck_description: prepared.description,
        source_note_count: i64::try_from(prepared.notes.len()).unwrap_or(i64::MAX),
        generated_card_count: generated_cards,
        basic_count: basic,
        reverse_enabled_count: reverse,
        cloze_count: cloze,
        duplicate_count: duplicates,
        unique_tags: tags.into_iter().collect(),
        samples,
    })
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
        return Err(unsupported_markdown(path, "images are not supported in version 1"));
    }
    if source.lines().any(|line| line.trim_start().starts_with('<')) {
        return Err(unsupported_markdown(path, "raw HTML is not supported"));
    }
    if source.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with('|') && trimmed.ends_with('|')
    }) {
        return Err(unsupported_markdown(path, "Markdown tables are not supported in version 1"));
    }

    let lines = source.replace("\r\n", "\n").replace('\r', "\n");
    let lines = lines.lines().collect::<Vec<_>>();
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
                return Err(unsupported_markdown(path, "a fenced code block is not closed"));
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
                "content": [{
                    "type": "paragraph",
                    "content": inline_nodes(&quoted.join("\n"), path)?
                }]
            }));
            continue;
        }
        if bullet_item(line).is_some() {
            let mut items = Vec::new();
            while index < lines.len() {
                let Some(text) = bullet_item(lines[index]) else { break; };
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
                let Some((_, text)) = ordered_item(lines[index]) else { break; };
                items.push(json!({
                    "type": "listItem",
                    "content": [{"type": "paragraph", "content": inline_nodes(text, path)?}]
                }));
                index += 1;
            }
            nodes.push(json!({
                "type": "orderedList",
                "attrs": {"start": start},
                "content": items
            }));
            continue;
        }

        let mut paragraph = vec![line.to_string()];
        index += 1;
        while index < lines.len()
            && !lines[index].trim().is_empty()
            && !is_block_start(lines[index])
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
    if (1..=6).contains(&count) && trimmed.as_bytes().get(count) == Some(&b' ') {
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
                "marks": [{"type": "link", "attrs": {"href": href, "target": null, "rel": "noopener noreferrer nofollow", "class": null}}]
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
            if remainder.starts_with(marker) {
                let after = &remainder[marker.len()..];
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
            return Err(unsupported_markdown(path, "a Markdown link is malformed"));
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

fn sha256_hex(bytes: &[u8]) -> String {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
        0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
        0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
        0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
        0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
    ];
    let bit_len = (bytes.len() as u64).wrapping_mul(8);
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut state = INITIAL;
    for chunk in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, word) in words.iter_mut().take(16).enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes(chunk[offset..offset + 4].try_into().expect("word"));
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(sigma1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = sigma0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }
    state.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
mod generated_import_tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
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
    fn strict_json_rejects_unknown_card_fields() {
        let value = r#"{
          "format":"ostudy-flashcards",
          "formatVersion":1,
          "deck":{"name":"Test"},
          "cards":[{"type":"basic","front":"Q","back":"A","overwrite":true}]
        }"#;
        assert!(serde_json::from_str::<GeneratedImportFile>(value).is_err());
    }
}
