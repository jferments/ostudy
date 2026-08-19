#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


# Generated-import internals retain a source-index -> created-note mapping only
# inside Rust. #[serde(skip)] keeps it out of external JSON/API responses.
path = "src-tauri/src/authoring/card_types_and_templates/generated_import/mod.rs"
text = read(path)
text = text.replace(
    '''pub struct FlashcardImportResultDto {
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
}''',
    '''pub struct FlashcardImportResultDto {
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
    #[serde(skip)]
    pub(crate) created_note_ids_by_source_index: Vec<(usize, String)>,
}''',
    1,
)
text = text.replace(
    '''struct PreparedImportNote {
    prepared: PreparedGeneratedNote,''',
    '''struct PreparedImportNote {
    source_index: usize,
    prepared: PreparedGeneratedNote,''',
    1,
)
write(path, text)

path = "src-tauri/src/authoring/card_types_and_templates/generated_import/prepare.rs"
text = read(path)
text = text.replace(
    '''    Ok(PreparedImportNote {
        prepared,''',
    '''    Ok(PreparedImportNote {
        source_index: index,
        prepared,''',
    1,
)
write(path, text)

path = "src-tauri/src/authoring/card_types_and_templates/generated_import/apply.rs"
text = read(path)
text = text.replace(
    "    let mut skipped = 0_i64;\n",
    "    let mut skipped = 0_i64;\n    let mut created_note_ids_by_source_index = Vec::new();\n",
    1,
)
text = text.replace(
    '''        let PreparedImportNote {
            prepared,''',
    '''        let PreparedImportNote {
            source_index,
            prepared,''',
    1,
)
text = text.replace(
    "        created_notes += 1;\n        created_cards += i64::try_from(cards.len()).unwrap_or(i64::MAX);",
    "        created_note_ids_by_source_index.push((source_index, note_id));\n        created_notes += 1;\n        created_cards += i64::try_from(cards.len()).unwrap_or(i64::MAX);",
    1,
)
text = text.replace(
    '''        skipped_duplicate_count: skipped,
    };''',
    '''        skipped_duplicate_count: skipped,
        created_note_ids_by_source_index,
    };''',
    1,
)
write(path, text)

path = "src-tauri/src/ai/document_intelligence.rs"
text = read(path)
# Public within the native crate so the document executor can persist and query it.
text = text.replace("struct DocumentLearningTarget {", "pub(crate) struct DocumentLearningTarget {", 1)
text = text.replace("fn learning_targets_for_window(\n", "pub(crate) fn learning_targets_for_window(\n", 1)
# Durable coverage exclusions are a task snapshot, used by Generate More.
text = text.replace(
    "    pub openrouter_provider_slugs: Vec<String>,\n    profile: ProfileSnapshot,",
    "    pub openrouter_provider_slugs: Vec<String>,\n    pub learning_target_exclusions: HashSet<String>,\n    profile: ProfileSnapshot,",
    1,
)
text = text.replace(
    "            openrouter_provider_slugs,\n            profile,",
    "            openrouter_provider_slugs,\n            learning_target_exclusions: HashSet::new(),\n            profile,",
    1,
)
# Filter persisted-covered targets from subsequent passes without changing the
# ordinary initial-generation target plan.
old_targets = '''    let targets = learning_targets_for_window(outline, window, &task.coverage_mode);'''
new_targets = '''    let targets = learning_targets_for_window(outline, window, &task.coverage_mode)
        .into_iter()
        .filter(|target| !task.learning_target_exclusions.contains(&target.id))
        .collect::<Vec<_>>();'''
if old_targets in text:
    text = text.replace(old_targets, new_targets, 1)

# Replace the old strip-only helper with an extractor that preserves the exact
# final curated target mapping until the canonical import transaction.
pattern = re.compile(
    r'''pub\(crate\) fn strip_internal_generation_metadata\(cards: &mut \[Value\]\) \{.*?\n\}''',
    re.S,
)
metadata_helpers = r'''pub(crate) fn extract_internal_generation_metadata(cards: &mut [Value]) -> Vec<Vec<String>> {
    cards
        .iter_mut()
        .map(|card| {
            let Some(object) = card.as_object_mut() else {
                return Vec::new();
            };
            object
                .remove("_ostudyLearningTargetIds")
                .and_then(|value| value.as_array().cloned())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub(crate) fn strip_internal_generation_metadata(cards: &mut [Value]) {
    let _ = extract_internal_generation_metadata(cards);
}'''
text, count = pattern.subn(metadata_helpers, text, count=1)
if count != 1:
    raise RuntimeError(f"{path}: metadata stripping helper not found")

# Durable target plan and coverage ledger helpers.
anchor = "pub(crate) async fn curate_document_cards("
helpers = r'''pub(crate) fn persist_document_learning_targets(
    database: &Database,
    document_id: &str,
    task: &DocumentTaskContext,
    outline: &DocumentOutline,
    windows: &[SemanticWindow],
) -> Result<(), CommandError> {
    let now = crate::util::unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction()
            .map_err(CommandError::database)?;
        for window in windows {
            let spans = serde_json::to_string(&json!([{
                "pageStart": window.page_start,
                "pageEnd": window.page_end
            }]))
            .map_err(CommandError::serialization)?;
            for target in learning_targets_for_window(outline, window, &task.coverage_mode) {
                transaction
                    .execute(
                        "INSERT INTO document_learning_targets (
                             document_id, target_id, target_kind, statement, importance,
                             evidence_confidence, source_spans_json, prompt_policy_hash,
                             created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, 100, ?6, ?7, ?8, ?8)
                         ON CONFLICT(document_id, target_id) DO UPDATE SET
                             target_kind = excluded.target_kind,
                             statement = excluded.statement,
                             importance = excluded.importance,
                             evidence_confidence = excluded.evidence_confidence,
                             source_spans_json = excluded.source_spans_json,
                             prompt_policy_hash = excluded.prompt_policy_hash,
                             updated_at = excluded.updated_at",
                        rusqlite::params![
                            document_id,
                            target.id,
                            target.kind,
                            target.statement,
                            i64::from(target.importance),
                            spans,
                            task.prompt_policy_hash,
                            now,
                        ],
                    )
                    .map_err(CommandError::database)?;
            }
        }
        transaction.commit().map_err(CommandError::database)
    })
}

pub(crate) fn load_covered_learning_target_ids(
    database: &Database,
    document_id: &str,
    prompt_policy_hash: &str,
) -> Result<HashSet<String>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT DISTINCT coverage.target_id
                 FROM document_learning_target_coverage coverage
                 JOIN document_learning_targets target
                   ON target.document_id = coverage.document_id
                  AND target.target_id = coverage.target_id
                 JOIN notes note ON note.id = coverage.note_id
                 WHERE coverage.document_id = ?1
                   AND target.prompt_policy_hash = ?2
                   AND note.deleted_at IS NULL",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map(rusqlite::params![document_id, prompt_policy_hash], |row| row.get(0))
            .map_err(CommandError::database)?
            .collect::<Result<HashSet<_>, _>>()
            .map_err(CommandError::database)
    })
}

pub(crate) fn persist_learning_target_coverage(
    connection: &rusqlite::Connection,
    document_id: &str,
    coverage_by_source_index: &[Vec<String>],
    created_notes_by_source_index: &[(usize, String)],
    now: i64,
) -> Result<(), CommandError> {
    for (source_index, note_id) in created_notes_by_source_index {
        let Some(target_ids) = coverage_by_source_index.get(*source_index) else {
            continue;
        };
        for target_id in target_ids {
            connection
                .execute(
                    "INSERT OR IGNORE INTO document_learning_target_coverage (
                         document_id, target_id, note_id, created_at
                     ) VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![document_id, target_id, note_id, now],
                )
                .map_err(CommandError::database)?;
        }
    }
    Ok(())
}

'''
if "persist_document_learning_targets" not in text:
    if anchor not in text:
        raise RuntimeError(f"{path}: curation anchor missing")
    text = text.replace(anchor, helpers + anchor, 1)

# Extend metadata tests to prove target IDs survive until extraction and are then
# absent from canonical card JSON.
test_anchor = "    #[test]\n    fn internal_learning_target_metadata_never_reaches_canonical_import() {"
if test_anchor in text and "internal_learning_target_metadata_returns_coverage_map" not in text:
    extra_test = r'''    #[test]
    fn internal_learning_target_metadata_returns_coverage_map() {
        let mut cards = vec![json!({
            "type": "basic",
            "front": "Q",
            "back": "A",
            "tags": [],
            "_ostudyLearningTargetIds": ["target-one", "target-two"]
        })];
        let coverage = extract_internal_generation_metadata(&mut cards);
        assert_eq!(coverage, vec![vec!["target-one".to_string(), "target-two".to_string()]]);
        assert!(cards[0].get("_ostudyLearningTargetIds").is_none());
    }

'''
    text = text.replace(test_anchor, extra_test + test_anchor, 1)
write(path, text)

# Test literals created before this finalizer need the new exclusion field.
for file in (ROOT / "src-tauri").rglob("*.rs"):
    source = file.read_text()
    if "DocumentTaskContext {" not in source:
        continue
    # Add after prompt policy hash in literal initializers only; struct definition
    # contains `pub` and is therefore unaffected by this pattern.
    source = re.sub(
        r'(\n\s*prompt_policy_hash:\s*[^\n]+,\n)(?!\s*learning_target_exclusions:)',
        r'\1            learning_target_exclusions: HashSet::new(),\n',
        source,
    )
    file.write_text(source)

# Initial generation persists the target plan and records final curated coverage
# in the same authoring transaction that creates notes/deck provenance.
path = "src-tauri/src/documents.rs"
text = read(path)
execute_anchor = '''    let job_id = document_store::create_generation_job('''
if "persist_document_learning_targets" not in text:
    if execute_anchor not in text:
        raise RuntimeError(f"{path}: initial generation job anchor missing")
    text = text.replace(
        execute_anchor,
        '''    document_intelligence::persist_document_learning_targets(
        database.inner(),
        document_id,
        &task,
        &outline,
        &generation_windows,
    )?;

''' + execute_anchor,
        1,
    )
# Semantic applicator currently strips immediately after curation; extract instead.
text = text.replace(
    "    document_intelligence::strip_internal_generation_metadata(&mut generated_cards);",
    "    let coverage_by_source_index =\n        document_intelligence::extract_internal_generation_metadata(&mut generated_cards);",
    1,
)
coverage_anchor = '''            connection
                .execute(
                    "INSERT INTO document_decks'''
if "persist_learning_target_coverage(\n                connection,\n                document_id" not in text:
    if coverage_anchor not in text:
        raise RuntimeError(f"{path}: document commit hook anchor missing")
    text = text.replace(
        coverage_anchor,
        '''            document_intelligence::persist_learning_target_coverage(
                connection,
                document_id,
                &coverage_by_source_index,
                &import_result.created_note_ids_by_source_index,
                now,
            )?;
''' + coverage_anchor,
        1,
    )
write(path, text)

# Generate More keeps the legacy Q/A constraint for pre-v2 documents, but its
# primary exclusion source is the durable target coverage ledger.
path = "src-tauri/src/document_more.rs"
text = read(path)
# Remove artificial objective insertion; it should not become a durable target.
text = re.sub(
    r'''\n\s*outline\.learning_objectives\.insert\(\n\s*0,\n\s*"Second-pass objective:.*?\.to_string\(\),\n\s*\);''',
    "",
    text,
    count=1,
    flags=re.S,
)
# After windows are available, persist current targets and exclude covered ones.
windows_anchor = '''    validate_semantic_windows(&windows, &source)?;
'''
if "load_covered_learning_target_ids" not in text:
    if windows_anchor not in text:
        raise RuntimeError(f"{path}: additional window validation anchor missing")
    text = text.replace(
        windows_anchor,
        windows_anchor + '''    document_intelligence::persist_document_learning_targets(
        database.inner(),
        &input.document_id,
        &task,
        &outline,
        &windows,
    )?;
    task.learning_target_exclusions = document_intelligence::load_covered_learning_target_ids(
        database.inner(),
        &input.document_id,
        &task.prompt_policy_hash,
    )?;
    crate::logging::debug_target(
        "ai.document.coverage",
        "document_additional_coverage_loaded",
        "Loaded durable target coverage without logging card or source content.",
        serde_json::json!({
            "document": input.document_id.as_str(),
            "coveredTargets": task.learning_target_exclusions.len(),
            "policyHash": task.prompt_policy_hash.as_str(),
        }),
    );
''',
        1,
    )
text = text.replace(
    "    document_intelligence::strip_internal_generation_metadata(&mut generated);",
    "    let coverage_by_source_index =\n        document_intelligence::extract_internal_generation_metadata(&mut generated);",
    1,
)
# Persist added-note coverage before document/job finalization in the same commit.
commit_anchor = '''        |connection, import_result, now| {
            if import_result.deck_id != context.deck_id {'''
if "persist_learning_target_coverage(\n                connection,\n                &input.document_id" not in text:
    if commit_anchor not in text:
        raise RuntimeError(f"{path}: additional commit hook anchor missing")
    replacement = '''        |connection, import_result, now| {
            if import_result.deck_id != context.deck_id {'''
    replacement += '''
                return Err(CommandError::new(
                    "document_generated_deck_changed",
                    "The generated deck changed before additional cards could be committed. No cards were added.",
                ));
            }
            document_intelligence::persist_learning_target_coverage(
                connection,
                &input.document_id,
                &coverage_by_source_index,
                &import_result.created_note_ids_by_source_index,
                now,
            )?;
'''
    # Replace the whole existing initial guard through its closing block to avoid
    # duplicating the return statement.
    guard_pattern = re.compile(
        r'''        \|connection, import_result, now\| \{\n            if import_result\.deck_id != context\.deck_id \{\n                return Err\(CommandError::new\(\n                    "document_generated_deck_changed",\n                    "The generated deck changed before additional cards could be committed\. No cards were added\.",\n                \)\);\n            \}\n'''
    )
    text, count = guard_pattern.subn(replacement, text, count=1)
    if count != 1:
        raise RuntimeError(f"{path}: additional deck guard not found")
write(path, text)

# Hidden mapping must never become part of an external generated-import result.
path = "src-tauri/src/authoring/card_types_and_templates/generated_import/mod.rs"
text = read(path)
test_anchor = "    #[test]\n    fn create_then_append_uses_the_named_deck_and_skips_exact_duplicates() {"
serialization_test = r'''    #[test]
    fn internal_created_note_mapping_is_not_serialized() {
        let result = FlashcardImportResultDto {
            deck_id: "a".repeat(32),
            created_note_count: 1,
            created_card_count: 1,
            skipped_duplicate_count: 0,
            created_note_ids_by_source_index: vec![(0, "b".repeat(32))],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("createdNoteIds"));
        assert!(!json.contains(&"b".repeat(32)));
    }

'''
if "internal_created_note_mapping_is_not_serialized" not in text:
    if test_anchor not in text:
        raise RuntimeError(f"{path}: generated import test anchor missing")
    text = text.replace(test_anchor, serialization_test + test_anchor, 1)
write(path, text)

# Docs spell out card-move/delete semantics for the coverage ledger.
path = "docs/architecture/ai-and-document-processing.md"
text = read(path)
addition = '''
### Durable semantic coverage ledger

The current prompt-policy target plan is persisted in `document_learning_targets`. Final curated candidates retain private target IDs until canonical authoring creates their note IDs; the same transaction then writes `document_learning_target_coverage`. Coverage belongs to the generated note, not its current deck. Moving a note therefore preserves document coverage, while permanent note deletion cascades the coverage row and makes that target eligible again. `Generate more cards` filters current targets against this ledger and keeps the older question/answer summary only as a compatibility constraint for documents generated before the ledger existed.
'''
if "### Durable semantic coverage ledger" not in text:
    text += addition
write(path, text)
