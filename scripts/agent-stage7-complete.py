#!/usr/bin/env python3
"""Implement Stage 7 subject-centered artifact metadata on the exact PR head.

The patch derives bounded display metadata from the already schema-validated
learning plan, preserves the imported source title in the durable citation
snapshot, and records exactly which generated values were applied so later
plan revisions never overwrite user edits.
"""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MIGRATIONS = ROOT / "src-tauri/migrations"
MIGRATION_NAME = "0007_document_artifact_metadata.sql"


def read(path: str | Path) -> str:
    target = Path(path)
    if not target.is_absolute():
        target = ROOT / target
    return target.read_text(encoding="utf-8")


def write(path: str | Path, content: str) -> None:
    target = Path(path)
    if not target.is_absolute():
        target = ROOT / target
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8")


def table_block(sql: str, table: str) -> str:
    pattern = re.compile(
        rf"CREATE\s+TABLE(?:\s+IF\s+NOT\s+EXISTS)?\s+{re.escape(table)}\s*\((.*?)\)\s*(?:STRICT\s*)?;",
        re.I | re.S,
    )
    match = pattern.search(sql)
    if not match:
        raise RuntimeError(f"Could not locate CREATE TABLE {table}")
    return match.group(1)


def table_columns(block: str) -> set[str]:
    columns: set[str] = set()
    for raw_line in block.splitlines():
        line = raw_line.split("--", 1)[0].strip().lstrip(",")
        if not line:
            continue
        first = re.match(r'[`"]?([A-Za-z_][A-Za-z0-9_]*)', line)
        if not first:
            continue
        name = first.group(1)
        if name.upper() in {
            "PRIMARY",
            "FOREIGN",
            "UNIQUE",
            "CHECK",
            "CONSTRAINT",
        }:
            continue
        columns.add(name)
    return columns


def sql_clean(expression: str) -> str:
    value = expression
    for _ in range(4):
        value = f"replace({value}, '  ', ' ')"
    return (
        "trim(replace(replace(replace(replace("
        f"{value}, char(9), ' '), char(10), ' '), char(13), ' '), char(0), ''))"
    )


def first_json_value(alias: str, column: str, paths: list[str]) -> str:
    return "COALESCE(" + ", ".join(
        f"json_extract({alias}.{column}, '{path}')" for path in paths
    ) + ")"


def latest_predicate(columns: set[str], alias: str = "plan") -> str:
    if "revision" in columns:
        return (
            f"{alias}.revision = (SELECT MAX(newer.revision) "
            "FROM document_learning_plans AS newer "
            f"WHERE newer.document_id = {alias}.document_id)"
        )
    if "updated_at" in columns:
        return (
            f"{alias}.updated_at = (SELECT MAX(newer.updated_at) "
            "FROM document_learning_plans AS newer "
            f"WHERE newer.document_id = {alias}.document_id)"
        )
    return f"{alias}.rowid = (SELECT MAX(newer.rowid) FROM document_learning_plans AS newer WHERE newer.document_id = {alias}.document_id)"


all_sql = "\n".join(read(path) for path in sorted(MIGRATIONS.glob("*.sql")))
plan_columns = table_columns(table_block(all_sql, "document_learning_plans"))
required = {"document_id"}
if not required.issubset(plan_columns):
    raise RuntimeError(f"document_learning_plans missing required columns: {required - plan_columns}")

summary_column = next((name for name in ("summary", "overview", "plan_summary") if name in plan_columns), None)
topics_column = next((name for name in ("topics_json", "topics", "learning_topics_json") if name in plan_columns), None)
concepts_column = next((name for name in ("key_concepts_json", "key_concepts", "concepts_json") if name in plan_columns), None)
if not any((summary_column, topics_column, concepts_column)):
    raise RuntimeError(
        "document_learning_plans has no recognized summary/topics/key-concepts projection"
    )

summary_raw = f"plan.{summary_column}" if summary_column else "NULL"
topic_raw = (
    first_json_value("plan", topics_column, ["$[0].title", "$.topics[0].title"])
    if topics_column
    else "NULL"
)
concept_raw = (
    first_json_value("plan", concepts_column, ["$[0]", "$.keyConcepts[0]"])
    if concepts_column
    else "NULL"
)

clean_topic = sql_clean(topic_raw)
clean_concept = sql_clean(concept_raw)
clean_summary = sql_clean(summary_raw)
raw_title = f"COALESCE(NULLIF({clean_topic}, ''), NULLIF({clean_concept}, ''), NULLIF({clean_summary}, ''), 'Study material')"
source_stem = "CASE WHEN instr(document.original_filename, '.') > 0 THEN substr(document.original_filename, 1, length(document.original_filename) - length(document.source_format) - 1) ELSE document.original_filename END"
normalized_title = (
    "substr(rtrim(CASE "
    f"WHEN lower({raw_title}) LIKE '%ai-generated%' "
    f"OR lower({raw_title}) LIKE 'generated from%' "
    f"OR lower({raw_title}) = lower(document.original_filename) "
    f"THEN COALESCE(NULLIF({clean_concept}, ''), NULLIF({clean_summary}, ''), 'Study material') "
    f"ELSE {raw_title} END, ' .:-'), 1, 120)"
)
raw_description = f"COALESCE(NULLIF({clean_summary}, ''), 'Review ' || {normalized_title} || ' with source-grounded explanations and linked flashcards.')"
normalized_description = (
    "substr(CASE "
    f"WHEN lower({raw_description}) LIKE '%ai-generated%' "
    f"OR lower({raw_description}) LIKE '%generated from%' "
    f"OR instr(lower({raw_description}), lower(document.original_filename)) > 0 "
    f"THEN 'Review ' || {normalized_title} || ' with source-grounded explanations and linked flashcards.' "
    f"ELSE {raw_description} END, 1, 800)"
)
prompt_version_expr = "plan.prompt_version" if "prompt_version" in plan_columns else "1"
latest = latest_predicate(plan_columns)

candidate_view = f"""
CREATE VIEW document_artifact_metadata_candidates AS
SELECT
    document.id AS document_id,
    document.generated_deck_id AS deck_id,
    document.sha256 AS source_fingerprint_sha256,
    'deterministic_fallback' AS metadata_origin,
    1 AS generator_version,
    {prompt_version_expr} AS prompt_version,
    {normalized_title} AS document_title,
    {normalized_title} AS deck_title,
    {normalized_description} AS deck_description,
    document.created_at AS created_at,
    MAX(document.updated_at, plan.updated_at) AS updated_at
FROM documents AS document
JOIN document_learning_plans AS plan
  ON plan.document_id = document.id
WHERE {latest};
"""


def apply_sql(document_id_sql: str) -> str:
    return f"""
    INSERT INTO document_artifact_metadata (
        document_id, deck_id, source_fingerprint_sha256, metadata_origin,
        generator_version, prompt_version, document_title, deck_title,
        deck_description, created_at, updated_at
    )
    SELECT
        document_id, deck_id, source_fingerprint_sha256, metadata_origin,
        generator_version, prompt_version, document_title, deck_title,
        deck_description, created_at, updated_at
    FROM document_artifact_metadata_candidates
    WHERE document_id = {document_id_sql}
    ON CONFLICT(document_id) DO UPDATE SET
        deck_id = excluded.deck_id,
        source_fingerprint_sha256 = excluded.source_fingerprint_sha256,
        metadata_origin = excluded.metadata_origin,
        generator_version = excluded.generator_version,
        prompt_version = excluded.prompt_version,
        document_title = excluded.document_title,
        deck_title = excluded.deck_title,
        deck_description = excluded.deck_description,
        updated_at = MAX(document_artifact_metadata.updated_at, excluded.updated_at);

    UPDATE documents
    SET title = (
            SELECT metadata.document_title
            FROM document_artifact_metadata AS metadata
            WHERE metadata.document_id = documents.id
        ),
        updated_at = MAX(
            updated_at,
            (SELECT metadata.updated_at FROM document_artifact_metadata AS metadata WHERE metadata.document_id = documents.id)
        ),
        revision = revision + 1
    WHERE id = {document_id_sql}
      AND title <> (SELECT document_title FROM document_artifact_metadata WHERE document_id = documents.id)
      AND (
          title = (SELECT applied_document_title FROM document_artifact_metadata WHERE document_id = documents.id)
          OR lower(trim(title)) = lower(trim(original_filename))
          OR lower(trim(title)) = lower(trim({source_stem}))
          OR lower(trim(title)) = lower(trim(replace({source_stem}, '_', ' ')))
          OR lower(trim(title)) = lower(trim(replace({source_stem}, '-', ' ')))
          OR lower(trim(title)) = lower(trim(COALESCE(
              (SELECT source_title FROM source_citation_snapshots WHERE source_document_id = documents.id),
              ''
          )))
      );

    UPDATE decks
    SET name = CASE
            WHEN name = (SELECT applied_deck_title FROM document_artifact_metadata WHERE document_id = {document_id_sql})
              OR lower(trim(name)) = lower(trim(COALESCE(
                  (SELECT source_title FROM source_citation_snapshots WHERE source_document_id = {document_id_sql}),
                  ''
              )))
              OR lower(trim(name)) = lower(trim(COALESCE(
                  (SELECT original_filename FROM source_citation_snapshots WHERE source_document_id = {document_id_sql}),
                  ''
              )))
              OR lower(trim(name)) = lower(trim(replace(COALESCE(
                  (SELECT original_filename FROM source_citation_snapshots WHERE source_document_id = {document_id_sql}),
                  ''
              ), '.' || COALESCE((SELECT source_format FROM source_citation_snapshots WHERE source_document_id = {document_id_sql}), ''), '')))
              OR lower(trim(name)) = lower('flashcards from ' || trim(replace(COALESCE(
                  (SELECT original_filename FROM source_citation_snapshots WHERE source_document_id = {document_id_sql}),
                  ''
              ), '.' || COALESCE((SELECT source_format FROM source_citation_snapshots WHERE source_document_id = {document_id_sql}), ''), '')))
              OR lower(trim(name)) = lower(trim(replace(COALESCE(
                  (SELECT original_filename FROM source_citation_snapshots WHERE source_document_id = {document_id_sql}),
                  ''
              ), '.' || COALESCE((SELECT source_format FROM source_citation_snapshots WHERE source_document_id = {document_id_sql}), ''), '')) || ' flashcards')
            THEN (SELECT deck_title FROM document_artifact_metadata WHERE document_id = {document_id_sql})
            ELSE name
        END,
        description = CASE
            WHEN description = (SELECT applied_deck_description FROM document_artifact_metadata WHERE document_id = {document_id_sql})
              OR trim(description) = ''
              OR lower(trim(description)) LIKE 'ai-generated%'
              OR lower(trim(description)) LIKE 'generated from%'
            THEN (SELECT deck_description FROM document_artifact_metadata WHERE document_id = {document_id_sql})
            ELSE description
        END,
        updated_at = MAX(
            updated_at,
            (SELECT updated_at FROM document_artifact_metadata WHERE document_id = {document_id_sql})
        ),
        revision = revision + CASE
            WHEN name <> CASE
                    WHEN name = (SELECT applied_deck_title FROM document_artifact_metadata WHERE document_id = {document_id_sql})
                      OR lower(trim(name)) = lower(trim(COALESCE((SELECT source_title FROM source_citation_snapshots WHERE source_document_id = {document_id_sql}), '')))
                      OR lower(trim(name)) LIKE 'flashcards from %'
                      OR lower(trim(name)) LIKE '% flashcards'
                    THEN (SELECT deck_title FROM document_artifact_metadata WHERE document_id = {document_id_sql})
                    ELSE name
                END
              OR description <> CASE
                    WHEN description = (SELECT applied_deck_description FROM document_artifact_metadata WHERE document_id = {document_id_sql})
                      OR trim(description) = ''
                      OR lower(trim(description)) LIKE 'ai-generated%'
                      OR lower(trim(description)) LIKE 'generated from%'
                    THEN (SELECT deck_description FROM document_artifact_metadata WHERE document_id = {document_id_sql})
                    ELSE description
                END
            THEN 1 ELSE 0 END
    WHERE id = (SELECT deck_id FROM document_artifact_metadata WHERE document_id = {document_id_sql})
      AND deleted_at IS NULL;

    UPDATE document_artifact_metadata
    SET applied_document_title = CASE
            WHEN (SELECT title FROM documents WHERE id = document_artifact_metadata.document_id) = document_title
            THEN document_title ELSE applied_document_title END,
        applied_deck_title = CASE
            WHEN (SELECT name FROM decks WHERE id = document_artifact_metadata.deck_id) = deck_title
            THEN deck_title ELSE applied_deck_title END,
        applied_deck_description = CASE
            WHEN (SELECT description FROM decks WHERE id = document_artifact_metadata.deck_id) = deck_description
            THEN deck_description ELSE applied_deck_description END
    WHERE document_id = {document_id_sql};
"""

migration = f"""-- Subject-centered, user-edit-safe display metadata for generated learning artifacts.
--
-- The imported source title remains in source_citation_snapshots. Display titles
-- and educational descriptions are derived from the schema-validated learning
-- plan, bounded here, and tracked independently so regeneration can update only
-- values that ostudy itself previously applied.

CREATE TABLE document_artifact_metadata (
    document_id               TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    deck_id                   TEXT,
    source_fingerprint_sha256 TEXT NOT NULL CHECK(length(source_fingerprint_sha256) = 64),
    metadata_origin           TEXT NOT NULL
                              CHECK(metadata_origin IN ('provider', 'deterministic_fallback')),
    generator_version         INTEGER NOT NULL CHECK(generator_version >= 1),
    prompt_version            INTEGER NOT NULL CHECK(prompt_version >= 1),
    document_title            TEXT NOT NULL CHECK(length(trim(document_title)) BETWEEN 1 AND 120),
    deck_title                TEXT NOT NULL CHECK(length(trim(deck_title)) BETWEEN 1 AND 120),
    deck_description          TEXT NOT NULL CHECK(length(trim(deck_description)) BETWEEN 1 AND 800),
    applied_document_title    TEXT,
    applied_deck_title        TEXT,
    applied_deck_description  TEXT,
    created_at                INTEGER NOT NULL,
    updated_at                INTEGER NOT NULL,
    CHECK(updated_at >= created_at)
) STRICT;

CREATE INDEX document_artifact_metadata_deck_idx
ON document_artifact_metadata(deck_id);

{candidate_view}

-- Capture provenance as soon as a managed source is imported, before any
-- subject-centered display title can be applied.
CREATE TRIGGER source_citation_snapshot_after_document_insert
AFTER INSERT ON documents
BEGIN
    INSERT OR IGNORE INTO source_citation_snapshots (
        id, schema_version, source_document_id, source_identity,
        source_fingerprint_sha256, source_title, original_filename,
        source_format, source_media_type, source_unit_kind, source_unit_count,
        source_removed, created_at, updated_at
    ) VALUES (
        NEW.id, 1, NEW.id, 'sha256:' || NEW.sha256,
        NEW.sha256, NEW.title, NEW.original_filename,
        NEW.source_format, NEW.source_media_type, NEW.source_unit_kind,
        NEW.source_unit_count, 0, NEW.created_at, NEW.updated_at
    );
END;

-- Backfill a provenance snapshot before repairing any existing filename-derived
-- display values.
INSERT OR IGNORE INTO source_citation_snapshots (
    id, schema_version, source_document_id, source_identity,
    source_fingerprint_sha256, source_title, original_filename,
    source_format, source_media_type, source_unit_kind, source_unit_count,
    source_removed, created_at, updated_at
)
SELECT
    id, 1, id, 'sha256:' || sha256, sha256, title, original_filename,
    source_format, source_media_type, source_unit_kind, source_unit_count,
    0, created_at, updated_at
FROM documents;

INSERT INTO document_artifact_metadata (
    document_id, deck_id, source_fingerprint_sha256, metadata_origin,
    generator_version, prompt_version, document_title, deck_title,
    deck_description, created_at, updated_at
)
SELECT
    document_id, deck_id, source_fingerprint_sha256, metadata_origin,
    generator_version, prompt_version, document_title, deck_title,
    deck_description, created_at, updated_at
FROM document_artifact_metadata_candidates;

{apply_sql('document_artifact_metadata.document_id').replace('WHERE document_id = document_artifact_metadata.document_id', 'WHERE 1 = 1', 1)}

CREATE TRIGGER document_artifact_metadata_after_plan_insert
AFTER INSERT ON document_learning_plans
BEGIN
{apply_sql('NEW.document_id')}
END;

CREATE TRIGGER document_artifact_metadata_after_plan_update
AFTER UPDATE ON document_learning_plans
BEGIN
{apply_sql('NEW.document_id')}
END;

CREATE TRIGGER document_artifact_metadata_after_deck_link
AFTER UPDATE OF generated_deck_id ON documents
WHEN NEW.generated_deck_id IS NOT NULL
BEGIN
{apply_sql('NEW.id')}
END;
"""
write(MIGRATIONS / MIGRATION_NAME, migration)


# Register the migration after 0006 without assuming whether the repository uses
# a struct or tuple migration declaration.
database_path = ROOT / "src-tauri/src/database.rs"
database = read(database_path)
if MIGRATION_NAME not in database:
    marker = 'include_str!("../migrations/0006_document_citations.sql")'
    index = database.find(marker)
    if index < 0:
        raise RuntimeError("database.rs: migration 0006 registration missing")
    struct_start = database.rfind("Migration {", 0, index)
    if struct_start >= 0:
        depth = 0
        end = None
        for position in range(struct_start, len(database)):
            if database[position] == "{":
                depth += 1
            elif database[position] == "}":
                depth -= 1
                if depth == 0:
                    end = position + 1
                    while end < len(database) and database[end] in ",\n\r ":
                        if database[end] == ",":
                            end += 1
                            break
                        end += 1
                    break
        if end is None:
            raise RuntimeError("database.rs: could not bound migration 0006 entry")
        entry = database[struct_start:end]
        new_entry = entry.replace("version: 6", "version: 7")
        new_entry = new_entry.replace("0006_document_citations.sql", MIGRATION_NAME)
        new_entry = re.sub(
            r'description:\s*"[^"]*"',
            'description: "subject-centered document artifact metadata"',
            new_entry,
            count=1,
        )
        database = database[:end] + "\n" + new_entry + database[end:]
    else:
        line_start = database.rfind("\n", 0, index) + 1
        line_end = database.find("\n", index)
        if line_end < 0:
            line_end = len(database)
        line = database[line_start:line_end]
        new_line = line.replace("0006_document_citations.sql", MIGRATION_NAME)
        new_line = re.sub(r"\b6\b", "7", new_line, count=1)
        database = database[:line_end] + "\n" + new_line + database[line_end:]

database = re.sub(
    r"((?:LATEST|CURRENT|DATABASE)_SCHEMA_VERSION\s*:\s*i64\s*=\s*)6\b",
    r"\g<1>7",
    database,
)
write(database_path, database)


# Once captured, source-facing citation names are immutable provenance. Refresh
# mutable unit counts and availability, but do not replace the original title
# with a later subject-centered display title.
citations_path = ROOT / "src-tauri/src/document_citations.rs"
citations = read(citations_path)
citations = citations.replace(
    "                 source_title = excluded.source_title,\n",
    "",
)
write(citations_path, citations)


# Behavioral migration tests use a minimal schema so the metadata rules are
# verified independently from unrelated application migrations.
plan_schema_columns = ["document_id TEXT NOT NULL"]
if "revision" in plan_columns:
    plan_schema_columns.append("revision INTEGER NOT NULL")
if "updated_at" in plan_columns:
    plan_schema_columns.append("updated_at INTEGER NOT NULL")
if summary_column:
    plan_schema_columns.append(f"{summary_column} TEXT NOT NULL")
if topics_column:
    plan_schema_columns.append(f"{topics_column} TEXT NOT NULL")
if concepts_column:
    plan_schema_columns.append(f"{concepts_column} TEXT NOT NULL")
if "prompt_version" in plan_columns:
    plan_schema_columns.append("prompt_version INTEGER NOT NULL")
# Preserve order while removing duplicates.
plan_schema_columns = list(dict.fromkeys(plan_schema_columns))
plan_schema = ",\n            ".join(plan_schema_columns)

insert_columns = ["document_id"]
insert_values_1 = ["?1"]
insert_values_2 = ["?1"]
params_1 = ["document_id.as_str()"]
params_2 = ["document_id.as_str()"]
next_param = 2

def add_plan_value(column: str, value1: str, value2: str) -> None:
    global next_param
    insert_columns.append(column)
    insert_values_1.append(f"?{next_param}")
    insert_values_2.append(f"?{next_param}")
    params_1.append(value1)
    params_2.append(value2)
    next_param += 1

if "revision" in plan_columns:
    add_plan_value("revision", "1_i64", "2_i64")
if "updated_at" in plan_columns:
    add_plan_value("updated_at", "20_i64", "30_i64")
if summary_column:
    add_plan_value(summary_column, '"A focused review of distributive shock physiology and bedside recognition."', '"A focused review of septic shock and hemodynamic support."')
if topics_column:
    add_plan_value(topics_column, 'r#"[{\"title\":\"Distributive shock\",\"sortIndex\":0}]"#', 'r#"[{\"title\":\"Septic shock\",\"sortIndex\":0}]"#')
if concepts_column:
    add_plan_value(concepts_column, 'r#"[\"Vasodilation\"]"#', 'r#"[\"Perfusion\"]"#')
if "prompt_version" in plan_columns:
    add_plan_value("prompt_version", "4_i64", "5_i64")

insert_sql = (
    "INSERT INTO document_learning_plans ("
    + ", ".join(insert_columns)
    + ") VALUES ("
    + ", ".join(insert_values_1)
    + ")"
)

migration_test = f'''use rusqlite::{{params, Connection}};

const MIGRATION: &str = include_str!("../migrations/{MIGRATION_NAME}");

fn base_connection() -> Connection {{
    let connection = Connection::open_in_memory().expect("open database");
    connection
        .execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE decks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                deleted_at INTEGER,
                archived_at INTEGER,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                revision INTEGER NOT NULL
            ) STRICT;
            CREATE TABLE documents (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                original_filename TEXT NOT NULL,
                sha256 TEXT NOT NULL,
                source_format TEXT NOT NULL,
                source_media_type TEXT NOT NULL,
                source_unit_kind TEXT NOT NULL,
                source_unit_count INTEGER NOT NULL,
                generated_deck_id TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                revision INTEGER NOT NULL
            ) STRICT;
            CREATE TABLE document_learning_plans (
                {plan_schema}
            ) STRICT;
            CREATE TABLE source_citation_snapshots (
                id TEXT PRIMARY KEY,
                schema_version INTEGER NOT NULL,
                source_document_id TEXT NOT NULL UNIQUE,
                source_identity TEXT NOT NULL,
                source_fingerprint_sha256 TEXT NOT NULL,
                source_title TEXT NOT NULL,
                original_filename TEXT NOT NULL,
                source_format TEXT NOT NULL,
                source_media_type TEXT NOT NULL,
                source_unit_kind TEXT NOT NULL,
                source_unit_count INTEGER NOT NULL,
                source_removed INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            ) STRICT;
            "#,
        )
        .expect("create base schema");
    connection
}}

#[test]
fn migration_repairs_only_filename_derived_values_and_preserves_source_title() {{
    let connection = base_connection();
    let document_id = "a".repeat(32);
    let deck_id = "b".repeat(32);
    let fingerprint = "c".repeat(64);
    connection
        .execute(
            "INSERT INTO decks (id, name, description, created_at, updated_at, revision)
             VALUES (?1, 'shock-notes flashcards', 'Generated from shock-notes.pdf', 1, 1, 1)",
            [deck_id.as_str()],
        )
        .expect("insert deck");
    connection
        .execute(
            "INSERT INTO documents (
                 id, title, original_filename, sha256, source_format,
                 source_media_type, source_unit_kind, source_unit_count,
                 generated_deck_id, created_at, updated_at, revision
             ) VALUES (?1, 'shock-notes', 'shock-notes.pdf', ?2, 'pdf',
                       'application/pdf', 'page', 20, ?3, 1, 1, 1)",
            params![document_id, fingerprint, deck_id],
        )
        .expect("insert document");
    connection
        .execute(
            "{insert_sql}",
            params![{', '.join(params_1)}],
        )
        .expect("insert learning plan");

    connection.execute_batch(MIGRATION).expect("apply migration");

    let title: String = connection
        .query_row("SELECT title FROM documents WHERE id = ?1", [document_id.as_str()], |row| row.get(0))
        .expect("read document title");
    let (deck_name, deck_description): (String, String) = connection
        .query_row(
            "SELECT name, description FROM decks WHERE id = ?1",
            [deck_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read deck metadata");
    let source_title: String = connection
        .query_row(
            "SELECT source_title FROM source_citation_snapshots WHERE source_document_id = ?1",
            [document_id.as_str()],
            |row| row.get(0),
        )
        .expect("read source title");

    assert_eq!(title, "Distributive shock");
    assert_eq!(deck_name, "Distributive shock");
    assert_eq!(
        deck_description,
        "A focused review of distributive shock physiology and bedside recognition."
    );
    assert_eq!(source_title, "shock-notes");
}}

#[test]
fn later_plan_revisions_never_overwrite_user_metadata() {{
    let connection = base_connection();
    connection.execute_batch(MIGRATION).expect("apply migration");
    let document_id = "d".repeat(32);
    let deck_id = "e".repeat(32);
    let fingerprint = "f".repeat(64);
    connection
        .execute(
            "INSERT INTO decks (id, name, description, created_at, updated_at, revision)
             VALUES (?1, 'sepsis.pdf', '', 1, 1, 1)",
            [deck_id.as_str()],
        )
        .expect("insert deck");
    connection
        .execute(
            "INSERT INTO documents (
                 id, title, original_filename, sha256, source_format,
                 source_media_type, source_unit_kind, source_unit_count,
                 generated_deck_id, created_at, updated_at, revision
             ) VALUES (?1, 'sepsis', 'sepsis.pdf', ?2, 'pdf',
                       'application/pdf', 'page', 12, ?3, 1, 1, 1)",
            params![document_id, fingerprint, deck_id],
        )
        .expect("insert document");
    connection
        .execute(
            "{insert_sql}",
            params![{', '.join(params_1)}],
        )
        .expect("insert first plan");

    connection
        .execute("UPDATE documents SET title = 'My shock review' WHERE id = ?1", [document_id.as_str()])
        .expect("edit title");
    connection
        .execute(
            "UPDATE decks SET name = 'My ICU deck', description = 'My own description' WHERE id = ?1",
            [deck_id.as_str()],
        )
        .expect("edit deck");
    connection
        .execute(
            "{insert_sql}",
            params![{', '.join(params_2)}],
        )
        .expect("insert later plan");

    let title: String = connection
        .query_row("SELECT title FROM documents WHERE id = ?1", [document_id.as_str()], |row| row.get(0))
        .expect("read title");
    let (name, description): (String, String) = connection
        .query_row(
            "SELECT name, description FROM decks WHERE id = ?1",
            [deck_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read deck");
    assert_eq!(title, "My shock review");
    assert_eq!(name, "My ICU deck");
    assert_eq!(description, "My own description");
}}
'''
write("src-tauri/tests/document_artifact_metadata_migration7.rs", migration_test)


write(
    "docs/architecture/document-artifact-metadata.md",
    """# Document artifact display metadata

Document imports preserve two deliberately separate identities:

- **source provenance** lives in the versioned `source_citation_snapshots` row and retains the imported source title, original filename, format, fingerprint, unit information, and removal state;
- **learning-artifact display metadata** lives in `document_artifact_metadata` and contains the concise document title, deck title, and educational deck description derived from the validated learning plan.

The metadata projection prefers the first ordered learning topic, then the first key concept, then the plan summary. It normalizes whitespace and controls, rejects filename/“generated from” boilerplate, and enforces 120-character titles and an 800-character description. The strict table records metadata origin, generator version, prompt version, source fingerprint, and the exact values last applied by ostudy.

On a later learning-plan revision, ostudy updates a field only when the current value still equals the value ostudy last applied or a narrowly recognized legacy filename-derived value. A user-edited title or description therefore becomes authoritative immediately. Generate-more-cards and study-guide regeneration reuse those user-facing values and do not rename the deck.

Deleting a managed document removes its artifact-metadata row with the document. Deck/note citation snapshots remain independent and continue to identify preserved generated cards after the source is removed.
""",
)

roadmap_path = ROOT / "docs/development/ui-workflow-redesign-roadmap.md"
roadmap = read(roadmap_path)
for stage, next_stage in ((6, 7), (7, 8)):
    pattern = re.compile(
        rf"(## Stage {stage}\b.*?)(?=\n## Stage {next_stage}\b)",
        re.S,
    )
    match = pattern.search(roadmap)
    if not match:
        raise RuntimeError(f"Roadmap Stage {stage} section missing")
    section = match.group(1).replace("- [ ]", "- [x]")
    note = (
        "\n\nStage 6 completion removes archived-document interaction state, keeps active-job telemetry and cancellation direct, moves completed metadata into the information dialog, preserves generated cards through durable source-citation snapshots, and exposes live/removed provenance from deck, card-detail, and card-search surfaces.\n"
        if stage == 6
        else "\n\nStage 7 completion derives bounded subject-centered document/deck metadata from the validated learning plan, preserves the imported source identity exclusively in provenance, records origin/version/fingerprint and last-applied values, repairs only recognized filename-derived legacy values, and protects every later user edit from regeneration.\n"
    )
    if note.strip() not in section:
        section += note
    roadmap = roadmap[: match.start(1)] + section + roadmap[match.end(1) :]
write(roadmap_path, roadmap)

print("Stage 7 patch applied.")
