#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)


MIGRATION = r'''CREATE TABLE ai_prompt_presets (
    id                            TEXT PRIMARY KEY CHECK(length(id) = 32),
    name                          TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100),
    outline_instructions          TEXT NOT NULL DEFAULT '' CHECK(length(outline_instructions) <= 12000),
    card_generation_instructions TEXT NOT NULL DEFAULT '' CHECK(length(card_generation_instructions) <= 20000),
    revision                      INTEGER NOT NULL CHECK(revision >= 1),
    created_at                    INTEGER NOT NULL,
    updated_at                    INTEGER NOT NULL CHECK(updated_at >= created_at)
) STRICT;

CREATE TABLE ai_task_policies (
    task_kind                      TEXT PRIMARY KEY CHECK(task_kind = 'flashcard_generation'),
    reasoning_effort               TEXT NOT NULL CHECK(reasoning_effort IN (
                                        'automatic', 'none', 'minimal', 'low', 'medium',
                                        'high', 'xhigh', 'max'
                                    )),
    context_limit_tokens           INTEGER CHECK(context_limit_tokens IS NULL OR context_limit_tokens BETWEEN 2048 AND 16777216),
    image_input_policy             TEXT NOT NULL CHECK(image_input_policy IN ('automatic', 'never')),
    coverage_mode                  TEXT NOT NULL CHECK(coverage_mode IN ('concise', 'balanced', 'thorough')),
    openrouter_routing_mode        TEXT NOT NULL CHECK(openrouter_routing_mode IN ('automatic', 'prefer', 'only')),
    openrouter_provider_slugs_json TEXT NOT NULL DEFAULT '[]',
    prompt_preset_id               TEXT REFERENCES ai_prompt_presets(id) ON DELETE SET NULL,
    revision                       INTEGER NOT NULL CHECK(revision >= 1),
    updated_at                     INTEGER NOT NULL
) STRICT;

INSERT INTO ai_task_policies (
    task_kind, reasoning_effort, context_limit_tokens, image_input_policy,
    coverage_mode, openrouter_routing_mode, openrouter_provider_slugs_json,
    prompt_preset_id, revision, updated_at
) VALUES (
    'flashcard_generation',
    COALESCE(
        (SELECT json_extract(value_json, '$') FROM app_settings WHERE key = 'aiReasoningEffort'),
        'low'
    ),
    NULL, 'automatic', 'balanced', 'automatic', '[]', NULL, 1,
    COALESCE((SELECT MAX(updated_at) FROM app_settings), 0)
);

DELETE FROM app_settings WHERE key IN (
    'aiReasoningEffort', 'aiContextLimitTokens', 'aiImageInputPolicy',
    'aiCoverageMode', 'aiOpenRouterRoutingMode', 'aiOpenRouterProviderSlugs',
    'aiPromptPresetId', 'aiPromptPresets'
);

CREATE TABLE document_learning_targets (
    document_id          TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    target_id            TEXT NOT NULL CHECK(length(target_id) BETWEEN 8 AND 128),
    target_kind          TEXT NOT NULL CHECK(target_kind IN (
                             'objective', 'concept', 'definition', 'mechanism', 'process',
                             'comparison', 'equation', 'application', 'misconception', 'prerequisite'
                         )),
    statement            TEXT NOT NULL CHECK(length(trim(statement)) BETWEEN 1 AND 2000),
    importance           INTEGER NOT NULL CHECK(importance BETWEEN 1 AND 100),
    evidence_confidence  INTEGER NOT NULL DEFAULT 100 CHECK(evidence_confidence BETWEEN 0 AND 100),
    source_spans_json    TEXT NOT NULL DEFAULT '[]',
    prompt_policy_hash   TEXT NOT NULL CHECK(length(prompt_policy_hash) = 64),
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL,
    PRIMARY KEY(document_id, target_id)
) STRICT;

CREATE TABLE document_learning_target_coverage (
    document_id TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    note_id     TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    created_at  INTEGER NOT NULL,
    PRIMARY KEY(document_id, target_id, note_id),
    FOREIGN KEY(document_id, target_id)
        REFERENCES document_learning_targets(document_id, target_id)
        ON DELETE CASCADE
) STRICT;

CREATE INDEX document_learning_target_coverage_note_idx
    ON document_learning_target_coverage(note_id);
'''

migration_path = "src-tauri/migrations/0002_ai_task_policy_learning_coverage.sql"
write(migration_path, MIGRATION)
checksum = hashlib.sha256(MIGRATION.encode()).hexdigest()

path = "src-tauri/src/database.rs"
text = read(path)
old = '''const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "public_baseline",
    checksum: "2d978f8be026ecb820e71ba77a8585e1de51cb08fc2820714526a33284167f85",
    sql: include_str!("../migrations/0001_public_baseline.sql"),
}];

pub(crate) const LATEST_SCHEMA_VERSION: i64 = 1;'''
new = f'''const MIGRATIONS: &[Migration] = &[
    Migration {{
        version: 1,
        name: "public_baseline",
        checksum: "2d978f8be026ecb820e71ba77a8585e1de51cb08fc2820714526a33284167f85",
        sql: include_str!("../migrations/0001_public_baseline.sql"),
    }},
    Migration {{
        version: 2,
        name: "ai_task_policy_learning_coverage",
        checksum: "{checksum}",
        sql: include_str!("../migrations/0002_ai_task_policy_learning_coverage.sql"),
    }},
];

pub(crate) const LATEST_SCHEMA_VERSION: i64 = 2;'''
if old not in text:
    if 'name: "ai_task_policy_learning_coverage"' not in text:
        raise RuntimeError(f"{path}: migration registry anchor missing")
else:
    text = text.replace(old, new, 1)
text = text.replace(
    '''                "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (2, 'future', 'future', 0)",''',
    '''                "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (3, 'future', 'future', 0)",''',
)
write(path, text)

path = "src-tauri/src/settings.rs"
text = read(path)

# Load the versioned policy once and project it into the existing settings DTO.
old_load = '''    let ai_reasoning_effort = load_ai_reasoning_effort(connection)?;
    publish_ai_reasoning_effort(&ai_reasoning_effort);'''
new_load = '''    let ai_policy = load_ai_task_policy(connection)?;
    let ai_reasoning_effort = ai_policy.reasoning_effort.clone();
    let ai_prompt_presets = load_ai_prompt_presets(connection)?;
    publish_ai_reasoning_effort(&ai_reasoning_effort);'''
if old_load in text:
    text = text.replace(old_load, new_load, 1)
elif "let ai_policy = load_ai_task_policy" not in text:
    raise RuntimeError(f"{path}: AI policy load anchor missing")

replacements = {
'''        ai_context_limit_tokens: get_setting(connection, "aiContextLimitTokens", None::<i64>)?,''':
'''        ai_context_limit_tokens: ai_policy.context_limit_tokens,''',
'''        ai_image_input_policy: get_setting(
            connection,
            "aiImageInputPolicy",
            default_ai_image_input_policy(),
        )?,''':
'''        ai_image_input_policy: ai_policy.image_input_policy.clone(),''',
'''        ai_coverage_mode: get_setting(
            connection,
            "aiCoverageMode",
            default_ai_coverage_mode(),
        )?,''':
'''        ai_coverage_mode: ai_policy.coverage_mode.clone(),''',
'''        ai_openrouter_routing_mode: get_setting(
            connection,
            "aiOpenRouterRoutingMode",
            default_ai_openrouter_routing_mode(),
        )?,''':
'''        ai_openrouter_routing_mode: ai_policy.openrouter_routing_mode.clone(),''',
'''        ai_openrouter_provider_slugs: get_setting(
            connection,
            "aiOpenRouterProviderSlugs",
            Vec::<String>::new(),
        )?,''':
'''        ai_openrouter_provider_slugs: ai_policy.openrouter_provider_slugs.clone(),''',
'''        ai_prompt_preset_id: get_setting(connection, "aiPromptPresetId", None::<String>)?,''':
'''        ai_prompt_preset_id: ai_policy.prompt_preset_id.clone(),''',
'''        ai_prompt_presets: get_setting(
            connection,
            "aiPromptPresets",
            Vec::<AiPromptPresetDto>::new(),
        )?,''':
'''        ai_prompt_presets,''',
}
for old_block, new_block in replacements.items():
    if old_block in text:
        text = text.replace(old_block, new_block, 1)

# Replace the legacy app-settings reasoning read with the native task-policy row.
pattern = re.compile(
    r'''pub\(crate\) fn load_ai_reasoning_effort\(connection: &Connection\) -> Result<String, CommandError> \{.*?\n\}''',
    re.S,
)
reasoning_fn = r'''pub(crate) fn load_ai_reasoning_effort(connection: &Connection) -> Result<String, CommandError> {
    Ok(load_ai_task_policy(connection)?.reasoning_effort)
}'''
text, count = pattern.subn(reasoning_fn, text, count=1)
if count != 1:
    raise RuntimeError(f"{path}: load_ai_reasoning_effort function not found")

# Strip AI policy/preset keys from the generic app_settings entry array. They are
# written below through dedicated tables in the same SQLite transaction.
for key in [
    "aiReasoningEffort",
    "aiContextLimitTokens",
    "aiImageInputPolicy",
    "aiCoverageMode",
    "aiOpenRouterRoutingMode",
    "aiOpenRouterProviderSlugs",
    "aiPromptPresetId",
    "aiPromptPresets",
]:
    text = re.sub(
        r'''\n\s*\(\n\s*"''' + re.escape(key) + r'''",\n\s*serde_json::to_string\(&settings\.[a-zA-Z0-9_]+\),\n\s*\),''',
        "",
        text,
        count=1,
    )

# Persist AI task policy immediately before the settings transaction commits.
# This is deliberately anchored to the save_settings_in_connection commit rather
# than the legacy app_settings cleanup SQL, because earlier UI migrations may
# legitimately rewrite that cleanup list (for example removing deck density).
if "save_ai_policy_in_transaction(&transaction, settings, now)?;" not in text:
    function_start = text.find("fn save_settings_in_connection(")
    if function_start < 0:
        raise RuntimeError(f"{path}: save_settings_in_connection not found")
    commit_marker = "    transaction.commit().map_err(CommandError::database)"
    commit_index = text.find(commit_marker, function_start)
    if commit_index < 0:
        raise RuntimeError(f"{path}: settings transaction commit boundary missing")
    text = (
        text[:commit_index]
        + "    save_ai_policy_in_transaction(&transaction, settings, now)?;\n"
        + text[commit_index:]
    )

# Add native storage helpers before the generic validation functions.
anchor = "fn default_ai_image_input_policy() -> String {"
helpers = r'''#[derive(Clone, Debug)]
struct StoredAiTaskPolicy {
    reasoning_effort: String,
    context_limit_tokens: Option<i64>,
    image_input_policy: String,
    coverage_mode: String,
    openrouter_routing_mode: String,
    openrouter_provider_slugs: Vec<String>,
    prompt_preset_id: Option<String>,
}

fn load_ai_task_policy(connection: &Connection) -> Result<StoredAiTaskPolicy, CommandError> {
    connection
        .query_row(
            "SELECT reasoning_effort, context_limit_tokens, image_input_policy,
                    coverage_mode, openrouter_routing_mode,
                    openrouter_provider_slugs_json, prompt_preset_id
             FROM ai_task_policies WHERE task_kind = 'flashcard_generation'",
            [],
            |row| {
                let provider_json = row.get::<_, String>(5)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    provider_json,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .map_err(CommandError::database)
        .and_then(|row| {
            Ok(StoredAiTaskPolicy {
                reasoning_effort: row.0,
                context_limit_tokens: row.1,
                image_input_policy: row.2,
                coverage_mode: row.3,
                openrouter_routing_mode: row.4,
                openrouter_provider_slugs: serde_json::from_str(&row.5)
                    .map_err(CommandError::serialization)?,
                prompt_preset_id: row.6,
            })
        })
}

fn load_ai_prompt_presets(connection: &Connection) -> Result<Vec<AiPromptPresetDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, outline_instructions, card_generation_instructions,
                    revision, created_at, updated_at
             FROM ai_prompt_presets ORDER BY created_at, id",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map([], |row| {
            Ok(AiPromptPresetDto {
                id: row.get(0)?,
                name: row.get(1)?,
                outline_instructions: row.get(2)?,
                card_generation_instructions: row.get(3)?,
                revision: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn save_ai_policy_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    settings: &AppSettingsDto,
    now: i64,
) -> Result<(), CommandError> {
    // Deleting inside the same transaction is safe: the policy FK temporarily
    // becomes NULL, presets are recreated, then the selected preset is restored.
    transaction
        .execute("DELETE FROM ai_prompt_presets", [])
        .map_err(CommandError::database)?;
    for preset in &settings.ai_prompt_presets {
        transaction
            .execute(
                "INSERT INTO ai_prompt_presets (
                     id, name, outline_instructions, card_generation_instructions,
                     revision, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    preset.id,
                    preset.name.trim(),
                    preset.outline_instructions,
                    preset.card_generation_instructions,
                    preset.revision,
                    preset.created_at,
                    preset.updated_at,
                ],
            )
            .map_err(CommandError::database)?;
    }
    let providers = serde_json::to_string(&settings.ai_openrouter_provider_slugs)
        .map_err(CommandError::serialization)?;
    transaction
        .execute(
            "INSERT INTO ai_task_policies (
                 task_kind, reasoning_effort, context_limit_tokens, image_input_policy,
                 coverage_mode, openrouter_routing_mode, openrouter_provider_slugs_json,
                 prompt_preset_id, revision, updated_at
             ) VALUES ('flashcard_generation', ?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8)
             ON CONFLICT(task_kind) DO UPDATE SET
                 reasoning_effort = excluded.reasoning_effort,
                 context_limit_tokens = excluded.context_limit_tokens,
                 image_input_policy = excluded.image_input_policy,
                 coverage_mode = excluded.coverage_mode,
                 openrouter_routing_mode = excluded.openrouter_routing_mode,
                 openrouter_provider_slugs_json = excluded.openrouter_provider_slugs_json,
                 prompt_preset_id = excluded.prompt_preset_id,
                 revision = ai_task_policies.revision + 1,
                 updated_at = excluded.updated_at",
            params![
                settings.ai_reasoning_effort,
                settings.ai_context_limit_tokens,
                settings.ai_image_input_policy,
                settings.ai_coverage_mode,
                settings.ai_openrouter_routing_mode,
                providers,
                settings.ai_prompt_preset_id,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "DELETE FROM app_settings WHERE key IN (
                 'aiReasoningEffort', 'aiContextLimitTokens', 'aiImageInputPolicy',
                 'aiCoverageMode', 'aiOpenRouterRoutingMode', 'aiOpenRouterProviderSlugs',
                 'aiPromptPresetId', 'aiPromptPresets'
             )",
            [],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

'''
if "struct StoredAiTaskPolicy" not in text:
    if anchor not in text:
        raise RuntimeError(f"{path}: AI helper anchor missing")
    text = text.replace(anchor, helpers + anchor, 1)

# Regression: the AI policy no longer leaks back into general app_settings.
test_anchor = "    #[test]\n    fn loads_and_persists_workload_defaults() {"
policy_test = r'''    #[test]
    fn ai_generation_policy_uses_dedicated_native_tables() {
        let database = Database::open_in_memory().expect("database should open");
        let mut settings = database.with_connection(load_settings).unwrap();
        settings.ai_reasoning_effort = "minimal".to_string();
        settings.ai_context_limit_tokens = Some(65_536);
        settings.ai_coverage_mode = "thorough".to_string();
        settings.ai_prompt_presets = vec![AiPromptPresetDto {
            id: "a".repeat(32),
            name: "Clinical".to_string(),
            outline_instructions: "Emphasize mechanisms.".to_string(),
            card_generation_instructions: "Prefer clinical application.".to_string(),
            revision: 1,
            created_at: 1,
            updated_at: 1,
        }];
        settings.ai_prompt_preset_id = Some("a".repeat(32));
        database
            .with_connection(|connection| save_settings_in_connection(connection, &settings))
            .unwrap();
        database
            .with_connection(|connection| {
                let count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM app_settings WHERE key LIKE 'ai%'",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(count, 0);
                let policy = load_ai_task_policy(connection)?;
                assert_eq!(policy.reasoning_effort, "minimal");
                assert_eq!(policy.context_limit_tokens, Some(65_536));
                assert_eq!(policy.coverage_mode, "thorough");
                assert_eq!(policy.prompt_preset_id.as_deref(), Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
                assert_eq!(load_ai_prompt_presets(connection)?.len(), 1);
                Ok(())
            })
            .unwrap();
    }

'''
if "ai_generation_policy_uses_dedicated_native_tables" not in text:
    if test_anchor not in text:
        raise RuntimeError(f"{path}: settings test anchor missing")
    text = text.replace(test_anchor, policy_test + test_anchor, 1)
write(path, text)

# Documentation distinguishes UI projection from persistence ownership.
path = "docs/architecture/ai-and-document-processing.md"
text = read(path)
addition = '''
### Versioned task policy and prompt presets

Document-generation behavior is stored in dedicated `ai_task_policies` and `ai_prompt_presets` tables, not as generic appearance/application settings. The Settings DTO projects those values into the existing React form for compatibility, but native task-policy rows own reasoning level, context cap, image policy, semantic coverage, OpenRouter host routing, and prompt-preset selection. Prompt preset bodies are ordinary non-secret learner configuration and are eligible for backup/sync; credentials remain excluded.
'''
if "### Versioned task policy and prompt presets" not in text:
    text += addition
write(path, text)
