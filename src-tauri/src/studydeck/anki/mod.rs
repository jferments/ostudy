//! One-way Anki input adapter. The normal ostudy importer remains the only writer.

mod collection;
mod html;
mod package;
mod template;

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::OpenOptions,
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{
    duplicate_fingerprint, normalize_tag_name, read, PackageCard, PackageData, PackageDeck,
    PackageNote, PackageTag, TemporaryFileGuard, CURRENT_PACKAGE_VERSION,
};
use crate::{cards::validate_content, error::CommandError, util::unix_time_millis};

const MAX_ISSUES: usize = 100;
const MAX_OUTPUT_BYTES: usize = 128 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportIssue {
    pub source_card_id: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ImportReport {
    pub source_card_count: usize,
    pub imported_card_count: usize,
    pub skipped_card_count: usize,
    pub simplified_card_count: usize,
    pub native_style_card_count: usize,
    pub typed_answer_card_count: usize,
    pub native_cloze_card_count: usize,
    pub cloze_fallback_card_count: usize,
    pub issues: Vec<ImportIssue>,
    pub omitted_issue_count: usize,
}

impl ImportReport {
    fn issue(&mut self, card: i64, message: impl AsRef<str>) {
        if self.issues.len() < MAX_ISSUES {
            self.issues.push(ImportIssue {
                source_card_id: card.to_string(),
                message: message
                    .as_ref()
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(300)
                    .collect(),
            });
        } else {
            self.omitted_issue_count += 1;
        }
    }
}

pub(super) fn read_package(
    path: &Path,
    display_name: &str,
) -> Result<(PackageData, ImportReport), CommandError> {
    let mut archive = package::Archive::open(path)?;
    let bytes = archive.collection()?;
    // Create our own sibling filename in managed transfer storage. A donor ZIP
    // name or media-map name is never used for filesystem access.
    let mut created = None;
    for _ in 0..32 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary =
            path.with_extension(format!("anki-{}-{sequence}.sqlite3", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => {
                created = Some((file, TemporaryFileGuard::new(temporary)));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(CommandError::io(error)),
        }
    }
    let (mut file, guard) =
        created.ok_or_else(|| failure("Could not allocate Anki temporary storage."))?;
    file.write_all(&bytes).map_err(CommandError::io)?;
    file.flush().map_err(CommandError::io)?;
    drop(file);
    drop(bytes);
    let collection = collection::read(&guard.path)?;
    let result = convert(collection, &mut archive, display_name)?;
    let (mut package, report, used_assets) = result;
    package.assets = archive.take_assets(&used_assets);
    read::validate_package_data(&package, true)?;
    Ok((package, report))
}

fn convert(
    collection: collection::Collection,
    archive: &mut package::Archive,
    display_name: &str,
) -> Result<(PackageData, ImportReport, BTreeSet<String>), CommandError> {
    let now = unix_time_millis()?;
    let title = display_name
        .strip_suffix(".apkg")
        .unwrap_or(display_name)
        .trim();
    let title = title.chars().take(80).collect::<String>();
    let title =
        crate::decks::validate_name(title).unwrap_or_else(|_| "Imported Anki deck".to_string());
    let deck_id = source_id("deck", &title);
    let mut package = PackageData {
        version: CURRENT_PACKAGE_VERSION,
        deck: PackageDeck {
            source_id: deck_id.clone(),
            name: title,
            description: "Imported from Anki. Compatible cards become independent editable oStudy Basic or Fill in the blank notes with fresh schedules. Original deck paths are retained as tags; Anki review history and exact styling are not imported.".to_string(),
            sort_order: 0,
            created_at: now,
            updated_at: now,
            revision: 1,
        },
        notes: Vec::new(),
        cards: Vec::new(),
        tags: Vec::new(),
        note_tags: Vec::new(),
        assets: Vec::new(),
        note_assets: Vec::new(),
    };
    let mut report = ImportReport {
        source_card_count: collection.cards.len(),
        ..ImportReport::default()
    };
    let mut tags = BTreeMap::<String, PackageTag>::new();
    let mut used_assets = BTreeSet::new();
    let mut output_bytes = 0_usize;
    for card in &collection.cards {
        let converted = match convert_card(&collection, card, archive) {
            Ok(converted) => converted,
            Err(message) => {
                report.skipped_card_count += 1;
                report.issue(card.id, format!("Skipped: {message}"));
                continue;
            }
        };
        output_bytes = output_bytes
            .saturating_add(converted.validated.front_doc_json.len())
            .saturating_add(converted.validated.back_doc_json.len());
        if output_bytes > MAX_OUTPUT_BYTES {
            return Err(failure(
                "Converted Anki content exceeds the 128 MiB output limit.",
            ));
        }
        let note_id = source_id("note", &card.id.to_string());
        let mut warnings = converted.warnings;
        let mut note_tag_ids = BTreeSet::new();
        let all_tags = std::iter::once(format!("Anki deck::{}", converted.source_deck))
            .chain(converted.source_tags.iter().cloned());
        for source_name in all_tags {
            let normalized = normalize_tag_name(&source_name);
            if normalized.is_empty() {
                continue;
            }
            if note_tag_ids.len() >= 32 {
                warnings.insert("Tags beyond ostudy's 32-tag limit were omitted.".into());
                break;
            }
            let name = source_name
                .chars()
                .filter(|c| !c.is_control())
                .take(64)
                .collect::<String>();
            let normalized = normalize_tag_name(&name);
            if normalized.is_empty() {
                continue;
            }
            if source_name.chars().count() > 64 {
                warnings.insert(
                    "Long tags or source deck paths were shortened to 64 characters.".into(),
                );
            }
            let tag_id = source_id("tag", &normalized);
            if !tags.contains_key(&normalized) && tags.len() >= 100_000 {
                return Err(failure(
                    "Converted Anki deck contains too many distinct tags.",
                ));
            }
            tags.entry(normalized.clone())
                .or_insert_with(|| PackageTag {
                    source_id: tag_id.clone(),
                    name,
                    normalized_name: normalized,
                    created_at: now,
                    updated_at: now,
                });
            note_tag_ids.insert(tag_id);
        }
        for tag_id in note_tag_ids {
            package.note_tags.push((note_id.clone(), tag_id));
        }
        for (side, assets) in [
            ("front", &converted.validated.front_asset_ids),
            ("back", &converted.validated.back_asset_ids),
        ] {
            for asset in assets {
                used_assets.insert(asset.clone());
                package
                    .note_assets
                    .push((note_id.clone(), asset.clone(), side.to_string()));
            }
        }
        let duplicate_hash = duplicate_fingerprint(
            &converted.validated.front_text,
            &converted.validated.back_text,
        );
        package.notes.push(PackageNote {
            source_id: note_id.clone(),
            source_deck_id: deck_id.clone(),
            note_type: converted.note_type.into(),
            content_format: "tiptap-json".into(),
            content_schema_version: 1,
            front_doc: converted.front,
            back_doc: converted.back,
            front_text: converted.validated.front_text,
            back_text: converted.validated.back_text,
            duplicate_hash,
            reverse_enabled: false,
            cloze_source: converted.cloze_source,
            cloze_mode: "one_at_a_time".into(),
            created_at: now,
            updated_at: now,
            revision: 1,
        });
        package.cards.push(PackageCard {
            source_id: source_id("card", &card.id.to_string()),
            source_note_id: note_id,
            direction: "forward".into(),
            template_ordinal: 0,
            created_at: now,
            updated_at: now,
        });
        if converted.style_normalized {
            report.native_style_card_count += 1;
        }
        if converted.typed_answer {
            report.typed_answer_card_count += 1;
        }
        if converted.native_cloze {
            report.native_cloze_card_count += 1;
        }
        if converted.cloze_fallback {
            report.cloze_fallback_card_count += 1;
        }
        if !warnings.is_empty() {
            report.simplified_card_count += 1;
            for warning in warnings {
                report.issue(card.id, warning);
            }
        }
        report.imported_card_count += 1;
    }
    if package.cards.is_empty() {
        let reasons = report
            .issues
            .iter()
            .take(3)
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        return Err(failure(format!(
            "No compatible Anki cards were found; no deck was created. {reasons}"
        )));
    }
    package.tags = tags.into_values().collect();
    Ok((package, report, used_assets))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourcePresentationKind {
    Basic,
    Cloze,
}

impl SourcePresentationKind {
    fn from_model_kind(kind: i64) -> Result<Self, String> {
        match kind {
            0 => Ok(Self::Basic),
            1 => Ok(Self::Cloze),
            _ => Err("Unsupported Anki note type.".into()),
        }
    }
}

struct ConvertedCard<'a> {
    front: serde_json::Value,
    back: serde_json::Value,
    validated: crate::cards::ValidatedContent,
    warnings: BTreeSet<String>,
    source_tags: &'a [String],
    source_deck: &'a str,
    note_type: &'static str,
    cloze_source: String,
    style_normalized: bool,
    typed_answer: bool,
    native_cloze: bool,
    cloze_fallback: bool,
}

fn convert_card<'a>(
    collection: &'a collection::Collection,
    card: &collection::Card,
    archive: &mut package::Archive,
) -> Result<ConvertedCard<'a>, String> {
    let note = collection
        .notes
        .get(&card.note_id)
        .ok_or("Card refers to a missing note.")?;
    let model = collection
        .models
        .get(&note.model_id)
        .ok_or("Card refers to a missing note type.")?;
    let deck = collection
        .decks
        .get(&card.deck_id)
        .ok_or("Card refers to a missing deck.")?;
    let source_kind = SourcePresentationKind::from_model_kind(model.kind)?;
    if note.fields.len() != model.flds.len() {
        return Err("Mismatched Anki field count.".into());
    }
    let template_index = if source_kind == SourcePresentationKind::Cloze {
        0
    } else {
        card.ordinal as usize
    };
    let template = model
        .tmpls
        .get(template_index)
        .ok_or("Card refers to a missing template.")?;
    let mut fields: BTreeMap<String, String> = model
        .flds
        .iter()
        .zip(&note.fields)
        .map(|(field, value)| (field.name.clone(), value.clone()))
        .collect();
    for (name, value) in [
        ("Tags", note.tags.join(" ")),
        ("Type", model.name.clone()),
        ("Deck", deck.name.clone()),
        (
            "Subdeck",
            deck.name
                .rsplit("::")
                .next()
                .unwrap_or(&deck.name)
                .to_string(),
        ),
        ("Card", template.name.clone()),
    ] {
        fields
            .entry(name.to_string())
            .or_insert_with(|| template::escape(&value));
    }
    let question = template::render(&template.qfmt, &fields, card.ordinal, false, "")?;
    if source_kind == SourcePresentationKind::Cloze && !question.matched_cloze {
        return Err("Cloze card has no matching deletion on its question side.".into());
    }
    // A normal oStudy Basic note owns question/answer presentation. Suppress
    // Anki's common {{FrontSide}} duplication on the stored answer; the practice
    // setting decides whether the question is shown again after reveal. Complex
    // cloze fallbacks keep FrontSide because their context is part of the source
    // presentation and may not fit the native cloze representation.
    let front_side = if source_kind == SourcePresentationKind::Basic {
        ""
    } else {
        question.html.as_str()
    };
    let answer = template::render(&template.afmt, &fields, card.ordinal, true, front_side)?;
    let front = html::document(&question.html, &mut |name| archive.image(name))?;
    let back = html::document(&answer.html, &mut |name| archive.image(name))?;
    let style_normalized =
        !model.css.trim().is_empty() || front.style_omitted || back.style_omitted;
    let typed_answer = question.used_type_filter || answer.used_type_filter;
    let mut warnings = front.warnings;
    warnings.extend(back.warnings);
    let fallback_validated = validate_content(
        front.value.clone(),
        back.value.clone(),
        String::new(),
        String::new(),
    )
    .map_err(|error| error.message)?;

    if source_kind == SourcePresentationKind::Cloze {
        if let Some(field_name) = template::simple_cloze_field(&template.qfmt, &template.afmt) {
            if let Some(field) = fields.get(&field_name) {
                if let Some(native) = template::native_cloze(field, card.ordinal + 1)? {
                    let native_front = plain_text_document(&native.question);
                    let native_back = plain_text_document(&native.answer);
                    let validated = validate_content(
                        native_front.clone(),
                        native_back.clone(),
                        String::new(),
                        String::new(),
                    )
                    .map_err(|error| error.message)?;
                    return Ok(ConvertedCard {
                        front: native_front,
                        back: native_back,
                        validated,
                        warnings,
                        source_tags: &note.tags,
                        source_deck: &deck.name,
                        note_type: "cloze",
                        cloze_source: native.source,
                        style_normalized,
                        typed_answer,
                        native_cloze: true,
                        cloze_fallback: false,
                    });
                }
            }
        }
        return Ok(ConvertedCard {
            front: front.value,
            back: back.value,
            validated: fallback_validated,
            warnings,
            source_tags: &note.tags,
            source_deck: &deck.name,
            note_type: "basic",
            cloze_source: String::new(),
            style_normalized,
            typed_answer,
            native_cloze: false,
            cloze_fallback: true,
        });
    }

    Ok(ConvertedCard {
        front: front.value,
        back: back.value,
        validated: fallback_validated,
        warnings,
        source_tags: &note.tags,
        source_deck: &deck.name,
        note_type: "basic",
        cloze_source: String::new(),
        style_normalized,
        typed_answer,
        native_cloze: false,
        cloze_fallback: false,
    })
}

fn plain_text_document(text: &str) -> serde_json::Value {
    let content = text
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                json!({"type": "paragraph", "content": []})
            } else {
                json!({"type": "paragraph", "content": [{"type": "text", "text": line}]})
            }
        })
        .collect::<Vec<_>>();
    json!({"type": "doc", "content": content})
}

fn failure(message: impl Into<String>) -> CommandError {
    CommandError::new("invalid_anki_package", message)
}

fn source_id(kind: &str, source: &str) -> String {
    let hash = Sha256::digest(format!("ostudy-anki-v1\0{kind}\0{source}").as_bytes());
    format!("{hash:x}")[..32].to_string()
}
