#!/usr/bin/env python3
"""Tighten and validate the final Stage 6/7 implementation.

The script is idempotent. It can run after the earlier one-shot patch succeeded,
or it can finish those patches itself when their helper files are still present.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run_optional(path: str) -> None:
    candidate = ROOT / path
    if candidate.is_file():
        subprocess.run(["python3", str(candidate)], cwd=ROOT, check=True)


def sql_clean(expression: str) -> str:
    value = expression
    for _ in range(4):
        value = f"replace({value}, '  ', ' ')"
    return (
        "trim(replace(replace(replace(replace("
        f"{value}, char(9), ' '), char(10), ' '), char(13), ' '), char(0), ''))"
    )


def provider_value(path: str, limit: int) -> str:
    cleaned = sql_clean(f"json_extract(plan.artifact_metadata_json, '{path}')")
    return f"substr({cleaned}, 1, {limit})"


# Complete either interrupted predecessor before polishing its output.
run_optional("scripts/agent-stage7-finalize.py")
if not (ROOT / "src-tauri/migrations/0007_document_artifact_metadata.sql").is_file():
    run_optional("scripts/agent-stage6-complete.py")
    run_optional("scripts/agent-stage7-complete.py")

migration_path = ROOT / "src-tauri/migrations/0007_document_artifact_metadata.sql"
if not migration_path.is_file():
    raise RuntimeError("Stage 7 migration was not generated")
migration = migration_path.read_text(encoding="utf-8")

contract_sql = r'''-- Optional provider-authored metadata is part of the persisted learning-plan
-- contract. Null means the provider omitted it or the generation path used an
-- older checkpoint; the candidate view then supplies the deterministic fallback.
ALTER TABLE document_learning_plans ADD COLUMN artifact_metadata_json TEXT
    CHECK(
        artifact_metadata_json IS NULL OR COALESCE(
            json_valid(artifact_metadata_json)
            AND json_type(artifact_metadata_json) = 'object'
            AND json_type(artifact_metadata_json, '$.documentTitle') = 'text'
            AND length(trim(json_extract(artifact_metadata_json, '$.documentTitle'))) BETWEEN 1 AND 120
            AND json_type(artifact_metadata_json, '$.deckTitle') = 'text'
            AND length(trim(json_extract(artifact_metadata_json, '$.deckTitle'))) BETWEEN 1 AND 120
            AND json_type(artifact_metadata_json, '$.deckDescription') = 'text'
            AND length(trim(json_extract(artifact_metadata_json, '$.deckDescription'))) BETWEEN 1 AND 800,
            0
        )
    );
ALTER TABLE document_learning_plans ADD COLUMN artifact_metadata_origin TEXT NOT NULL
    DEFAULT 'deterministic_fallback'
    CHECK(artifact_metadata_origin IN ('provider', 'deterministic_fallback'));
ALTER TABLE document_learning_plans ADD COLUMN artifact_metadata_generator_version INTEGER NOT NULL
    DEFAULT 1 CHECK(artifact_metadata_generator_version >= 1);
ALTER TABLE document_learning_plans ADD COLUMN artifact_metadata_prompt_version INTEGER NOT NULL
    DEFAULT 1 CHECK(artifact_metadata_prompt_version >= 1);

'''
if "ALTER TABLE document_learning_plans ADD COLUMN artifact_metadata_json" not in migration:
    marker = "CREATE TABLE document_artifact_metadata ("
    index = migration.find(marker)
    if index < 0:
        raise RuntimeError("document_artifact_metadata table marker missing")
    migration = migration[:index] + contract_sql + migration[index:]

# Locate the generated fallback expressions and layer the validated provider
# payload over them. Boilerplate and filename echoes are ignored rather than
# becoming user-facing titles/descriptions.
block = re.compile(
    r"\s+'deterministic_fallback' AS metadata_origin,\n"
    r"\s+1 AS generator_version,\n"
    r"\s+(?P<prompt>.+?) AS prompt_version,\n"
    r"\s+(?P<title>.+?) AS document_title,\n"
    r"\s+(?P=title) AS deck_title,\n"
    r"\s+(?P<description>.+?) AS deck_description,\n",
)
match = block.search(migration)
if match:
    fallback_prompt = match.group("prompt")
    fallback_title = match.group("title")
    fallback_description = match.group("description")
    provider_document = provider_value("$.documentTitle", 120)
    provider_deck = provider_value("$.deckTitle", 120)
    provider_description = provider_value("$.deckDescription", 800)

    def accepted_title(value: str) -> str:
        return (
            "CASE WHEN plan.artifact_metadata_json IS NULL "
            f"OR lower({value}) LIKE '%ai-generated%' "
            f"OR lower({value}) LIKE 'generated from%' "
            f"OR lower({value}) = lower(document.original_filename) "
            f"THEN NULL ELSE {value} END"
        )

    accepted_description = (
        "CASE WHEN plan.artifact_metadata_json IS NULL "
        f"OR lower({provider_description}) LIKE '%ai-generated%' "
        f"OR lower({provider_description}) LIKE '%generated from%' "
        f"OR instr(lower({provider_description}), lower(document.original_filename)) > 0 "
        f"THEN NULL ELSE {provider_description} END"
    )
    replacement = (
        "\n    CASE WHEN plan.artifact_metadata_json IS NOT NULL "
        "THEN 'provider' ELSE 'deterministic_fallback' END AS metadata_origin,\n"
        "    COALESCE(plan.artifact_metadata_generator_version, 1) AS generator_version,\n"
        "    COALESCE(plan.artifact_metadata_prompt_version, "
        + fallback_prompt
        + ") AS prompt_version,\n"
        "    COALESCE(NULLIF("
        + accepted_title(provider_document)
        + ", ''), "
        + fallback_title
        + ") AS document_title,\n"
        "    COALESCE(NULLIF("
        + accepted_title(provider_deck)
        + ", ''), "
        + fallback_title
        + ") AS deck_title,\n"
        "    COALESCE(NULLIF("
        + accepted_description
        + ", ''), "
        + fallback_description
        + ") AS deck_description,\n"
    )
    migration = migration[: match.start()] + replacement + migration[match.end() :]
elif "plan.artifact_metadata_json" not in migration:
    raise RuntimeError("Could not locate artifact metadata candidate projection")

# Initial migration repair must recognize only exact known filename-derived
# templates. A user name such as “My flashcards” is never sufficient evidence.
trigger_marker = "CREATE TRIGGER document_artifact_metadata_after_plan_insert"
trigger_index = migration.find(trigger_marker)
if trigger_index < 0:
    raise RuntimeError("artifact metadata trigger marker missing")
backfill = migration[:trigger_index]
triggers = migration[trigger_index:]

source_stem_for_deck = """trim(replace(COALESCE((
                SELECT citation.original_filename
                FROM document_artifact_metadata AS metadata
                JOIN source_citation_snapshots AS citation
                  ON citation.source_document_id = metadata.document_id
                WHERE metadata.deck_id = decks.id
            ), ''), '.' || COALESCE((
                SELECT citation.source_format
                FROM document_artifact_metadata AS metadata
                JOIN source_citation_snapshots AS citation
                  ON citation.source_document_id = metadata.document_id
                WHERE metadata.deck_id = decks.id
            ), ''), ''))"""
exact_repair = (
    "OR lower(trim(name)) = lower('flashcards from ' || "
    + source_stem_for_deck
    + ")\n          OR lower(trim(name)) = lower("
    + source_stem_for_deck
    + " || ' flashcards')"
)
backfill = backfill.replace(
    "OR lower(trim(name)) LIKE 'flashcards from %'\n          OR lower(trim(name)) LIKE '% flashcards'",
    exact_repair,
)
# A generated trigger's main name CASE already has exact source-based checks;
# broad LIKE clauses were only used to calculate its revision increment. Remove
# them so a user-authored suffix cannot be classified as generated metadata.
triggers = triggers.replace(
    "\n                      OR lower(trim(name)) LIKE 'flashcards from %'\n                      OR lower(trim(name)) LIKE '% flashcards'",
    "",
)
migration = backfill + triggers
migration_path.write_text(migration, encoding="utf-8")

# Extend the migration behavior test with a validated provider payload. This
# proves the provider contract and deterministic fallback share the same
# user-edit-protection ledger while the immutable source citation remains intact.
test_path = ROOT / "src-tauri/tests/document_artifact_metadata_migration7.rs"
if not test_path.is_file():
    raise RuntimeError("Stage 7 migration test was not generated")
test = test_path.read_text(encoding="utf-8")
if "provider_metadata_uses_the_validated_optional_plan_contract" not in test:
    addition = r'''

#[test]
fn provider_metadata_uses_the_validated_optional_plan_contract() {
    let connection = base_connection();
    connection.execute_batch(MIGRATION).expect("apply migration");
    let document_id = "1".repeat(32);
    let deck_id = "2".repeat(32);
    let fingerprint = "3".repeat(64);
    connection
        .execute(
            "INSERT INTO decks (id, name, description, created_at, updated_at, revision)
             VALUES (?1, 'provider-source.pdf', '', 1, 1, 1)",
            [deck_id.as_str()],
        )
        .expect("insert deck");
    connection
        .execute(
            "INSERT INTO documents (
                 id, title, original_filename, sha256, source_format,
                 source_media_type, source_unit_kind, source_unit_count,
                 generated_deck_id, created_at, updated_at, revision
             ) VALUES (?1, 'provider-source', 'provider-source.pdf', ?2, 'pdf',
                       'application/pdf', 'page', 10, ?3, 1, 1, 1)",
            params![document_id, fingerprint, deck_id],
        )
        .expect("insert document");
    let columns = connection
        .prepare("PRAGMA table_info(document_learning_plans)")
        .expect("prepare columns")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("query columns")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect columns");
    let mut names = Vec::new();
    let mut values = Vec::new();
    for name in columns {
        if matches!(
            name.as_str(),
            "artifact_metadata_json"
                | "artifact_metadata_origin"
                | "artifact_metadata_generator_version"
                | "artifact_metadata_prompt_version"
        ) {
            continue;
        }
        names.push(name.clone());
        let value = match name.as_str() {
            "document_id" => format!("'{}'", document_id),
            "revision" | "compiler_version" | "prompt_version" | "schema_version" => "1".into(),
            "updated_at" | "created_at" => "10".into(),
            name if name.contains("topics") => r#"'[{"title":"Fallback topic","sortIndex":0}]'"#.into(),
            name if name.contains("objectives") || name.contains("concepts") => r#"'["Fallback concept"]'"#.into(),
            name if name.contains("summary") || name.contains("overview") => "'Fallback summary'".into(),
            name if name.ends_with("_json") => "'{}'".into(),
            name if name.ends_with("_at") => "10".into(),
            _ => "''".into(),
        };
        values.push(value);
    }
    connection
        .execute_batch(&format!(
            "INSERT INTO document_learning_plans ({}) VALUES ({})",
            names.join(", "),
            values.join(", ")
        ))
        .expect("insert learning plan");
    connection
        .execute(
            "UPDATE document_learning_plans
             SET artifact_metadata_json = ?2,
                 artifact_metadata_origin = 'provider',
                 artifact_metadata_generator_version = 2,
                 artifact_metadata_prompt_version = 9
             WHERE document_id = ?1",
            params![
                document_id,
                r#"{"documentTitle":"Distributive shock essentials","deckTitle":"Shock response","deckDescription":"Recognize vasodilatory shock and connect hemodynamics to bedside interventions."}"#,
            ],
        )
        .expect("apply provider metadata");

    let title: String = connection
        .query_row("SELECT title FROM documents WHERE id = ?1", [document_id.as_str()], |row| row.get(0))
        .expect("read document title");
    let (name, description): (String, String) = connection
        .query_row(
            "SELECT name, description FROM decks WHERE id = ?1",
            [deck_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read deck metadata");
    let (origin, generator_version, prompt_version): (String, i64, i64) = connection
        .query_row(
            "SELECT metadata_origin, generator_version, prompt_version
             FROM document_artifact_metadata WHERE document_id = ?1",
            [document_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read metadata provenance");
    let source_title: String = connection
        .query_row(
            "SELECT source_title FROM source_citation_snapshots WHERE source_document_id = ?1",
            [document_id.as_str()],
            |row| row.get(0),
        )
        .expect("read source title");

    assert_eq!(title, "Distributive shock essentials");
    assert_eq!(name, "Shock response");
    assert_eq!(
        description,
        "Recognize vasodilatory shock and connect hemodynamics to bedside interventions."
    );
    assert_eq!(origin, "provider");
    assert_eq!(generator_version, 2);
    assert_eq!(prompt_version, 9);
    assert_eq!(source_title, "provider-source");
}
'''
    test += addition
    test_path.write_text(test, encoding="utf-8")

architecture_path = ROOT / "docs/architecture/document-artifact-metadata.md"
architecture = architecture_path.read_text(encoding="utf-8")
provider_paragraph = """

The learning-plan persistence contract also accepts a strictly validated optional `artifact_metadata_json` object containing `documentTitle`, `deckTitle`, and `deckDescription`, together with provider/fallback origin and generator/prompt versions. Provider values are normalized and rejected when they echo the filename or contain generator boilerplate. Missing or rejected values fall back field-by-field to the validated plan projection.
"""
if "artifact_metadata_json" not in architecture:
    architecture += provider_paragraph
architecture_path.write_text(architecture, encoding="utf-8")

print("Stage 6/7 polish applied.")
