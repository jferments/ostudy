-- Subject-centered, user-edit-safe metadata for document-generated learning artifacts.
--
-- The durable source citation remains the source-identity authority. This table
-- owns only user-facing display metadata plus the values ostudy actually applied,
-- allowing later planning passes to distinguish generated values from user edits.

CREATE TABLE document_artifact_metadata (
    document_id               TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    deck_id                   TEXT REFERENCES decks(id) ON DELETE SET NULL,
    source_fingerprint_sha256 TEXT NOT NULL CHECK(length(source_fingerprint_sha256) = 64),
    metadata_origin           TEXT NOT NULL
                              CHECK(metadata_origin IN ('provider', 'deterministic_fallback')),
    generator_version         INTEGER NOT NULL CHECK(generator_version >= 1),
    schema_version            INTEGER NOT NULL CHECK(schema_version >= 1),
    prompt_version            INTEGER NOT NULL CHECK(prompt_version >= 1),
    document_title            TEXT NOT NULL
                              CHECK(length(trim(document_title)) BETWEEN 1 AND 120),
    deck_title                TEXT NOT NULL
                              CHECK(length(trim(deck_title)) BETWEEN 1 AND 120),
    deck_description          TEXT NOT NULL
                              CHECK(length(trim(deck_description)) BETWEEN 1 AND 800),
    applied_document_title    TEXT
                              CHECK(applied_document_title IS NULL OR
                                    length(trim(applied_document_title)) BETWEEN 1 AND 120),
    applied_deck_title        TEXT
                              CHECK(applied_deck_title IS NULL OR
                                    length(trim(applied_deck_title)) BETWEEN 1 AND 120),
    applied_deck_description  TEXT
                              CHECK(applied_deck_description IS NULL OR
                                    length(trim(applied_deck_description)) BETWEEN 1 AND 800),
    created_at                INTEGER NOT NULL,
    updated_at                INTEGER NOT NULL,
    CHECK(updated_at >= created_at)
) STRICT;

CREATE INDEX document_artifact_metadata_deck_idx
ON document_artifact_metadata(deck_id);

-- A source citation captures import-time identity. Later display-title changes
-- must never rewrite that identity when the ordinary citation upsert refreshes
-- source counts or removal state.
CREATE TRIGGER source_citation_snapshot_title_immutable
AFTER UPDATE OF source_title ON source_citation_snapshots
WHEN NEW.source_title <> OLD.source_title
BEGIN
    UPDATE source_citation_snapshots
    SET source_title = OLD.source_title
    WHERE id = NEW.id AND source_title = NEW.source_title;
END;

-- Backfill one deterministic candidate per document from the active learning
-- plan. The first plan topic is preferred, then the first key concept, then the
-- bounded whole-document summary.
WITH raw_candidates AS (
    SELECT
        document.id AS document_id,
        document.generated_deck_id AS deck_id,
        document.sha256 AS source_fingerprint_sha256,
        document.created_at AS created_at,
        MAX(document.updated_at, plan.updated_at) AS updated_at,
        COALESCE(
            NULLIF((
                SELECT trim(topic.title)
                FROM document_learning_plan_topics AS topic
                WHERE topic.plan_id = plan.id
                ORDER BY topic.sort_index
                LIMIT 1
            ), ''),
            NULLIF((
                SELECT trim(value)
                FROM json_each(plan.key_concepts_json)
                WHERE json_type(value) = 'text' AND trim(value) <> ''
                LIMIT 1
            ), ''),
            NULLIF(trim(plan.summary_text), ''),
            'Study material'
        ) AS raw_title
    FROM documents AS document
    JOIN document_learning_plans AS plan
      ON plan.document_id = document.id AND plan.state = 'active'
),
normalized_candidates AS (
    SELECT
        document_id,
        deck_id,
        source_fingerprint_sha256,
        created_at,
        updated_at,
        substr(
            trim(
                replace(replace(replace(replace(replace(replace(replace(
                    raw_title,
                    char(0), ''
                ), char(9), ' '), char(10), ' '), char(13), ' '),
                '  ', ' '), '  ', ' '), '  ', ' '),
                ' .,:;-–—|'
            ),
            1,
            120
        ) AS title
    FROM raw_candidates
)
INSERT INTO document_artifact_metadata (
    document_id, deck_id, source_fingerprint_sha256, metadata_origin,
    generator_version, schema_version, prompt_version,
    document_title, deck_title, deck_description,
    applied_document_title, applied_deck_title, applied_deck_description,
    created_at, updated_at
)
SELECT
    document_id,
    deck_id,
    source_fingerprint_sha256,
    'deterministic_fallback',
    1,
    1,
    1,
    CASE WHEN title = '' THEN 'Study material' ELSE title END,
    CASE WHEN title = '' THEN 'Study material' ELSE title END,
    substr(
        'Review ' || CASE WHEN title = '' THEN 'this study material' ELSE title END ||
        ' with source-grounded explanations and linked flashcards.',
        1,
        800
    ),
    NULL,
    NULL,
    NULL,
    created_at,
    updated_at
FROM normalized_candidates;

-- Narrowly repair only exact import/source-title forms. Ambiguous document
-- titles are left untouched.
UPDATE documents
SET title = (
        SELECT metadata.document_title
        FROM document_artifact_metadata AS metadata
        WHERE metadata.document_id = documents.id
    ),
    updated_at = MAX(
        updated_at,
        (SELECT metadata.updated_at
         FROM document_artifact_metadata AS metadata
         WHERE metadata.document_id = documents.id)
    ),
    revision = revision + 1
WHERE EXISTS (
        SELECT 1
        FROM document_artifact_metadata AS metadata
        WHERE metadata.document_id = documents.id
    )
  AND title <> (
        SELECT metadata.document_title
        FROM document_artifact_metadata AS metadata
        WHERE metadata.document_id = documents.id
    )
  AND (
      lower(trim(title)) = lower(trim(original_filename))
      OR lower(trim(title)) = lower(trim(
          CASE
              WHEN lower(original_filename) LIKE '%.' || lower(source_format)
              THEN substr(original_filename, 1, length(original_filename) - length(source_format) - 1)
              ELSE original_filename
          END
      ))
      OR lower(trim(title)) = lower(trim(replace(
          CASE
              WHEN lower(original_filename) LIKE '%.' || lower(source_format)
              THEN substr(original_filename, 1, length(original_filename) - length(source_format) - 1)
              ELSE original_filename
          END,
          '_',
          ' '
      )))
      OR lower(trim(title)) = lower(trim(replace(
          CASE
              WHEN lower(original_filename) LIKE '%.' || lower(source_format)
              THEN substr(original_filename, 1, length(original_filename) - length(source_format) - 1)
              ELSE original_filename
          END,
          '-',
          ' '
      )))
      OR lower(trim(title)) = lower(trim(COALESCE((
          SELECT citation.source_title
          FROM source_citation_snapshots AS citation
          WHERE citation.source_document_id = documents.id
      ), '')))
  );

-- Apply deterministic metadata to existing generated decks only when their
-- values exactly match known generated/source templates.
UPDATE decks
SET name = CASE
        WHEN (
            lower(trim(name)) = lower(trim(COALESCE((
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
            OR lower(trim(name)) = lower(trim(replace(COALESCE((
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
            ), ''), '')))
            OR lower(trim(name)) = lower('flashcards from ' || trim(replace(COALESCE((
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
            ), ''), '')))
            OR lower(trim(name)) = lower(trim(replace(COALESCE((
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
            ), ''), '')) || ' flashcards')
        )
        THEN (
            SELECT metadata.deck_title
            FROM document_artifact_metadata AS metadata
            WHERE metadata.deck_id = decks.id
        )
        ELSE name
    END,
    description = CASE
        WHEN trim(description) = ''
          OR description = 'AI-generated flashcards from ' || COALESCE((
                SELECT citation.original_filename
                FROM document_artifact_metadata AS metadata
                JOIN source_citation_snapshots AS citation
                  ON citation.source_document_id = metadata.document_id
                WHERE metadata.deck_id = decks.id
            ), '') || '. Edit or delete these cards through the normal deck tools.'
        THEN (
            SELECT metadata.deck_description
            FROM document_artifact_metadata AS metadata
            WHERE metadata.deck_id = decks.id
        )
        ELSE description
    END,
    updated_at = MAX(
        updated_at,
        (SELECT metadata.updated_at
         FROM document_artifact_metadata AS metadata
         WHERE metadata.deck_id = decks.id)
    ),
    revision = revision + 1
WHERE deleted_at IS NULL
  AND EXISTS (
      SELECT 1
      FROM document_artifact_metadata AS metadata
      WHERE metadata.deck_id = decks.id
  )
  AND (
      lower(trim(name)) = lower(trim(COALESCE((
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
      OR lower(trim(name)) = lower(trim(replace(COALESCE((
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
      ), ''), '')))
      OR lower(trim(name)) = lower('flashcards from ' || trim(replace(COALESCE((
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
      ), ''), '')))
      OR lower(trim(name)) = lower(trim(replace(COALESCE((
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
      ), ''), '')) || ' flashcards')
      OR trim(description) = ''
      OR description = 'AI-generated flashcards from ' || COALESCE((
          SELECT citation.original_filename
          FROM document_artifact_metadata AS metadata
          JOIN source_citation_snapshots AS citation
            ON citation.source_document_id = metadata.document_id
          WHERE metadata.deck_id = decks.id
      ), '') || '. Edit or delete these cards through the normal deck tools.'
  );

UPDATE document_artifact_metadata
SET applied_document_title = CASE
        WHEN (SELECT document.title
              FROM documents AS document
              WHERE document.id = document_artifact_metadata.document_id) = document_title
        THEN document_title
        ELSE applied_document_title
    END,
    applied_deck_title = CASE
        WHEN (SELECT deck.name
              FROM decks AS deck
              WHERE deck.id = document_artifact_metadata.deck_id) = deck_title
        THEN deck_title
        ELSE applied_deck_title
    END,
    applied_deck_description = CASE
        WHEN (SELECT deck.description
              FROM decks AS deck
              WHERE deck.id = document_artifact_metadata.deck_id) = deck_description
        THEN deck_description
        ELSE applied_deck_description
    END;

-- Initial flashcard generation creates its deck after the shared learning plan
-- and metadata have been persisted. Link and apply the saved metadata when that
-- generated deck ID becomes available, again using only exact templates.
CREATE TRIGGER document_artifact_metadata_after_generated_deck_link
AFTER UPDATE OF generated_deck_id ON documents
WHEN NEW.generated_deck_id IS NOT NULL
 AND NEW.generated_deck_id IS NOT OLD.generated_deck_id
BEGIN
    UPDATE document_artifact_metadata
    SET deck_id = NEW.generated_deck_id,
        updated_at = MAX(updated_at, NEW.updated_at)
    WHERE document_id = NEW.id;

    UPDATE decks
    SET name = CASE
            WHEN lower(trim(name)) = lower(trim(COALESCE((
                    SELECT citation.source_title
                    FROM source_citation_snapshots AS citation
                    WHERE citation.source_document_id = NEW.id
                ), '')))
              OR lower(trim(name)) = lower(trim(COALESCE((
                    SELECT citation.original_filename
                    FROM source_citation_snapshots AS citation
                    WHERE citation.source_document_id = NEW.id
                ), '')))
              OR lower(trim(name)) = lower(trim(replace(COALESCE((
                    SELECT citation.original_filename
                    FROM source_citation_snapshots AS citation
                    WHERE citation.source_document_id = NEW.id
                ), ''), '.' || COALESCE((
                    SELECT citation.source_format
                    FROM source_citation_snapshots AS citation
                    WHERE citation.source_document_id = NEW.id
                ), ''), '')))
            THEN (SELECT metadata.deck_title
                  FROM document_artifact_metadata AS metadata
                  WHERE metadata.document_id = NEW.id)
            ELSE name
        END,
        description = CASE
            WHEN trim(description) = ''
              OR description = 'AI-generated flashcards from ' || COALESCE((
                    SELECT citation.original_filename
                    FROM source_citation_snapshots AS citation
                    WHERE citation.source_document_id = NEW.id
                ), '') || '. Edit or delete these cards through the normal deck tools.'
            THEN (SELECT metadata.deck_description
                  FROM document_artifact_metadata AS metadata
                  WHERE metadata.document_id = NEW.id)
            ELSE description
        END,
        updated_at = MAX(updated_at, NEW.updated_at),
        revision = revision + 1
    WHERE id = NEW.generated_deck_id
      AND deleted_at IS NULL
      AND EXISTS (
          SELECT 1
          FROM document_artifact_metadata AS metadata
          WHERE metadata.document_id = NEW.id
      )
      AND (
          lower(trim(name)) = lower(trim(COALESCE((
              SELECT citation.source_title
              FROM source_citation_snapshots AS citation
              WHERE citation.source_document_id = NEW.id
          ), '')))
          OR lower(trim(name)) = lower(trim(COALESCE((
              SELECT citation.original_filename
              FROM source_citation_snapshots AS citation
              WHERE citation.source_document_id = NEW.id
          ), '')))
          OR lower(trim(name)) = lower(trim(replace(COALESCE((
              SELECT citation.original_filename
              FROM source_citation_snapshots AS citation
              WHERE citation.source_document_id = NEW.id
          ), ''), '.' || COALESCE((
              SELECT citation.source_format
              FROM source_citation_snapshots AS citation
              WHERE citation.source_document_id = NEW.id
          ), ''), '')))
          OR trim(description) = ''
          OR description = 'AI-generated flashcards from ' || COALESCE((
              SELECT citation.original_filename
              FROM source_citation_snapshots AS citation
              WHERE citation.source_document_id = NEW.id
          ), '') || '. Edit or delete these cards through the normal deck tools.'
      );

    UPDATE document_artifact_metadata
    SET applied_deck_title = CASE
            WHEN (SELECT deck.name FROM decks AS deck WHERE deck.id = NEW.generated_deck_id) = deck_title
            THEN deck_title
            ELSE applied_deck_title
        END,
        applied_deck_description = CASE
            WHEN (SELECT deck.description FROM decks AS deck WHERE deck.id = NEW.generated_deck_id) = deck_description
            THEN deck_description
            ELSE applied_deck_description
        END
    WHERE document_id = NEW.id;
END;
