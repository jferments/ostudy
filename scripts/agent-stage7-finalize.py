#!/usr/bin/env python3
"""Run the Stage 6/7 one-shot patches and normalize migration backfill SQL."""

from __future__ import annotations

import runpy
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

stage6 = ROOT / "scripts/agent-stage6-complete.py"
if stage6.is_file():
    subprocess.run(["python3", str(stage6)], cwd=ROOT, check=True)

stage7_globals = runpy.run_path(str(ROOT / "scripts/agent-stage7-complete.py"))
plan_columns = set(stage7_globals.get("plan_columns", set()))

migration_path = ROOT / "src-tauri/migrations/0007_document_artifact_metadata.sql"
migration = migration_path.read_text(encoding="utf-8")

# The generated trigger body is intentionally reused for one document at a
# time. Initial migration repair is set-based instead, which avoids aliases
# that exist only inside NEW-trigger scope.
start_marker = "\n    INSERT INTO document_artifact_metadata ("
end_marker = "\nCREATE TRIGGER document_artifact_metadata_after_plan_insert"
start = migration.find(start_marker)
end = migration.find(end_marker)
if start < 0 or end < 0 or end <= start:
    raise RuntimeError("Could not locate generated initial metadata-application block")

backfill = r'''

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
WHERE EXISTS (
        SELECT 1 FROM document_artifact_metadata AS metadata
        WHERE metadata.document_id = documents.id
    )
  AND title <> (SELECT document_title FROM document_artifact_metadata WHERE document_id = documents.id)
  AND (
      title = (SELECT applied_document_title FROM document_artifact_metadata WHERE document_id = documents.id)
      OR lower(trim(title)) = lower(trim(original_filename))
      OR lower(trim(title)) = lower(trim(
          CASE
              WHEN instr(original_filename, '.') > 0
              THEN substr(original_filename, 1, length(original_filename) - length(source_format) - 1)
              ELSE original_filename
          END
      ))
      OR lower(trim(title)) = lower(trim(replace(
          CASE
              WHEN instr(original_filename, '.') > 0
              THEN substr(original_filename, 1, length(original_filename) - length(source_format) - 1)
              ELSE original_filename
          END,
          '_', ' '
      )))
      OR lower(trim(title)) = lower(trim(replace(
          CASE
              WHEN instr(original_filename, '.') > 0
              THEN substr(original_filename, 1, length(original_filename) - length(source_format) - 1)
              ELSE original_filename
          END,
          '-', ' '
      )))
      OR lower(trim(title)) = lower(trim(COALESCE(
          (SELECT source_title FROM source_citation_snapshots WHERE source_document_id = documents.id),
          ''
      )))
  );

UPDATE decks
SET name = CASE
        WHEN name = (
                SELECT metadata.applied_deck_title
                FROM document_artifact_metadata AS metadata
                WHERE metadata.deck_id = decks.id
            )
          OR lower(trim(name)) = lower(trim(COALESCE((
                SELECT citation.source_title
                FROM document_artifact_metadata AS metadata
                JOIN source_citation_snapshots AS citation
                  ON citation.source_document_id = metadata.document_id
                WHERE metadata.deck_id = decks.id
            ), '')))
          OR lower(trim(name)) = lower(trim(COALESCE((
                SELECT citation.original_filename
                FROM document_artifact_metadata AS metadata
                JOIN source_citation_snapshots AS citation
                  ON citation.source_document_id = metadata.document_id
                WHERE metadata.deck_id = decks.id
            ), '')))
          OR lower(trim(name)) LIKE 'flashcards from %'
          OR lower(trim(name)) LIKE '% flashcards'
        THEN (SELECT deck_title FROM document_artifact_metadata WHERE deck_id = decks.id)
        ELSE name
    END,
    description = CASE
        WHEN description = (
                SELECT metadata.applied_deck_description
                FROM document_artifact_metadata AS metadata
                WHERE metadata.deck_id = decks.id
            )
          OR trim(description) = ''
          OR lower(trim(description)) LIKE 'ai-generated%'
          OR lower(trim(description)) LIKE 'generated from%'
        THEN (SELECT deck_description FROM document_artifact_metadata WHERE deck_id = decks.id)
        ELSE description
    END,
    updated_at = MAX(
        updated_at,
        (SELECT metadata.updated_at FROM document_artifact_metadata AS metadata WHERE metadata.deck_id = decks.id)
    ),
    revision = revision + 1
WHERE deleted_at IS NULL
  AND EXISTS (
      SELECT 1 FROM document_artifact_metadata AS metadata
      WHERE metadata.deck_id = decks.id
  )
  AND (
      name = (SELECT applied_deck_title FROM document_artifact_metadata WHERE deck_id = decks.id)
      OR lower(trim(name)) LIKE 'flashcards from %'
      OR lower(trim(name)) LIKE '% flashcards'
      OR trim(description) = ''
      OR lower(trim(description)) LIKE 'ai-generated%'
      OR lower(trim(description)) LIKE 'generated from%'
      OR description = (SELECT applied_deck_description FROM document_artifact_metadata WHERE deck_id = decks.id)
      OR lower(trim(name)) = lower(trim(COALESCE((
          SELECT citation.source_title
          FROM document_artifact_metadata AS metadata
          JOIN source_citation_snapshots AS citation
            ON citation.source_document_id = metadata.document_id
          WHERE metadata.deck_id = decks.id
      ), '')))
  );

UPDATE document_artifact_metadata
SET applied_document_title = CASE
        WHEN (SELECT title FROM documents WHERE id = document_artifact_metadata.document_id) = document_title
        THEN document_title ELSE applied_document_title END,
    applied_deck_title = CASE
        WHEN (SELECT name FROM decks WHERE id = document_artifact_metadata.deck_id) = deck_title
        THEN deck_title ELSE applied_deck_title END,
    applied_deck_description = CASE
        WHEN (SELECT description FROM decks WHERE id = document_artifact_metadata.deck_id) = deck_description
        THEN deck_description ELSE applied_deck_description END;
'''

migration = migration[:start] + backfill + migration[end:]
trigger_start = migration.find(end_marker)
if trigger_start < 0:
    raise RuntimeError("Stage 7 trigger section missing after backfill replacement")
head = migration[:trigger_start]
triggers = migration[trigger_start:]
triggers = triggers.replace("document.original_filename", "documents.original_filename")
triggers = triggers.replace("document.source_format", "documents.source_format")
migration = head + triggers
if "updated_at" not in plan_columns:
    migration = migration.replace(
        "MAX(document.updated_at, plan.updated_at) AS updated_at",
        "document.updated_at AS updated_at",
    )
migration_path.write_text(migration, encoding="utf-8")

print("Stage 6 and Stage 7 patches finalized.")
