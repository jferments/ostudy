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
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub source_card_count: usize,
    pub imported_card_count: usize,
    pub skipped_card_count: usize,
    pub simplified_card_count: usize,
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
        deck: PackageDeck { source_id: deck_id.clone(), name: title,
            description: "Imported from Anki. Cards are independent editable Basic notes with fresh schedules. Original deck paths are retained as tags; Anki review history and styling are not imported.".to_string(),
            sort_order: 0, created_at: now, updated_at: now, revision: 1 },
        notes: Vec::new(), cards: Vec::new(), tags: Vec::new(), note_tags: Vec::new(),
        assets: Vec::new(), note_assets: Vec::new(),
    };
    let mut report = ImportReport {
        source_card_count: collection.cards.len(),
        ..ImportReport::default()
    };
    let mut tags = BTreeMap::<String, PackageTag>::new();
    let mut used_assets = BTreeSet::new();
    let mut output_bytes = 0_usize;
    for card in &collection.cards {
        let converted = convert_card(&collection, card, archive);
        let (front, back, validated, mut warnings, source_tags, source_deck) = match converted {
            Ok(converted) => converted,
            Err(message) => {
                report.skipped_card_count += 1;
                report.issue(card.id, format!("Skipped: {message}"));
                continue;
            }
        };
        output_bytes = output_bytes
            .saturating_add(validated.front_doc_json.len())
            .saturating_add(validated.back_doc_json.len());
        if output_bytes > MAX_OUTPUT_BYTES {
            return Err(failure(
                "Converted Anki content exceeds the 128 MiB output limit.",
            ));
        }
        let note_id = source_id("note", &card.id.to_string());
        let mut note_tag_ids = BTreeSet::new();
        let all_tags =
            std::iter::once(format!("Anki deck::{source_deck}")).chain(source_tags.iter().cloned());
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
            ("front", &validated.front_asset_ids),
            ("back", &validated.back_asset_ids),
        ] {
            for asset in assets {
                used_assets.insert(asset.clone());
                package
                    .note_assets
                    .push((note_id.clone(), asset.clone(), side.to_string()));
            }
        }
        let duplicate_hash = duplicate_fingerprint(&validated.front_text, &validated.back_text);
        package.notes.push(PackageNote {
            source_id: note_id.clone(),
            source_deck_id: deck_id.clone(),
            note_type: "basic".into(),
            content_format: "tiptap-json".into(),
            content_schema_version: 1,
            front_doc: front,
            back_doc: back,
            front_text: validated.front_text,
            back_text: validated.back_text,
            duplicate_hash,
            reverse_enabled: false,
            cloze_source: String::new(),
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

type ConvertedCard<'a> = (
    serde_json::Value,
    serde_json::Value,
    crate::cards::ValidatedContent,
    BTreeSet<String>,
    &'a [String],
    &'a str,
);

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
    if !matches!(model.kind, 0 | 1) || note.fields.len() != model.flds.len() {
        return Err("Unsupported note type or mismatched field count.".into());
    }
    let template_index = if model.kind == 1 {
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
    if model.kind == 1 && !question.matched_cloze {
        return Err("Cloze card has no matching deletion on its question side.".into());
    }
    let answer = template::render(&template.afmt, &fields, card.ordinal, true, &question.html)?;
    let front = html::document(&question.html, &mut |name| archive.image(name))?;
    let back = html::document(&answer.html, &mut |name| archive.image(name))?;
    let validated = validate_content(
        front.value.clone(),
        back.value.clone(),
        String::new(),
        String::new(),
    )
    .map_err(|error| error.message)?;
    let mut warnings = front.warnings;
    warnings.extend(back.warnings);
    if !model.css.trim().is_empty() {
        warnings.insert("Anki stylesheet omitted; cards use ostudy's native appearance.".into());
    }
    Ok((
        front.value,
        back.value,
        validated,
        warnings,
        &note.tags,
        &deck.name,
    ))
}

fn failure(message: impl Into<String>) -> CommandError {
    CommandError::new("invalid_anki_package", message)
}

fn source_id(kind: &str, source: &str) -> String {
    let hash = Sha256::digest(format!("ostudy-anki-v1\0{kind}\0{source}").as_bytes());
    format!("{hash:x}")[..32].to_string()
}
