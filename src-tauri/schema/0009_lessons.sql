-- Release migration 9: lessons.
--
-- A subject's lessons are its course, in sort_order. A lesson is a short
-- teaching sequence written from the subject's knowledge units. Its text lives
-- in immutable revisions: writing a lesson again adds a revision, and the
-- highest revision is the one a learner reads. Each revision records the
-- knowledge units it explains and their content hashes when it was written, so
-- a later change to the source shows the lesson as out of date without
-- rewriting it. Units are not foreign keys: a revision stays readable after its
-- units are re-extracted or evicted.
--
-- Progress is a place to resume, not mastery: it never writes a review or
-- touches a schedule. The native owner validates every payload before it is
-- written. The upgrade creates the tables and writes no rows.
CREATE TABLE lessons (
    id          TEXT PRIMARY KEY
                CHECK(length(id) = 32 AND id NOT GLOB '*[^0-9a-f]*'),
    subject_id  TEXT NOT NULL REFERENCES document_collections(id) ON DELETE CASCADE
                CHECK(length(subject_id) = 32),
    sort_order  INTEGER NOT NULL CHECK(sort_order >= 0),
    title       TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 160),
    revision    INTEGER NOT NULL CHECK(revision >= 1),
    created_at  INTEGER NOT NULL CHECK(created_at >= 0),
    updated_at  INTEGER NOT NULL CHECK(updated_at >= created_at)
) STRICT;

CREATE INDEX lessons_by_subject ON lessons(subject_id, sort_order, id);

CREATE TABLE lesson_revisions (
    lesson_id        TEXT NOT NULL REFERENCES lessons(id) ON DELETE CASCADE,
    revision_number  INTEGER NOT NULL CHECK(revision_number >= 1),
    origin           TEXT NOT NULL CHECK(origin IN ('authored', 'generated')),
    content_json     TEXT NOT NULL CHECK(
                         json_valid(content_json)
                         AND length(content_json) BETWEEN 2 AND 1048576
                     ),
    content_hash     TEXT NOT NULL
                     CHECK(length(content_hash) = 64 AND content_hash NOT GLOB '*[^0-9a-f]*'),
    provenance_json  TEXT CHECK(
                         provenance_json IS NULL
                         OR (json_valid(provenance_json) AND length(provenance_json) <= 65536)
                     ),
    created_at       INTEGER NOT NULL CHECK(created_at >= 0),
    PRIMARY KEY(lesson_id, revision_number),
    CHECK((origin = 'authored') = (provenance_json IS NULL))
) STRICT;

CREATE TABLE lesson_revision_units (
    lesson_id          TEXT NOT NULL,
    revision_number    INTEGER NOT NULL,
    unit_id            TEXT NOT NULL CHECK(length(unit_id) = 32),
    unit_content_hash  TEXT NOT NULL CHECK(length(unit_content_hash) = 64),
    PRIMARY KEY(lesson_id, revision_number, unit_id),
    FOREIGN KEY(lesson_id, revision_number)
        REFERENCES lesson_revisions(lesson_id, revision_number) ON DELETE CASCADE
) STRICT;

CREATE INDEX lesson_revision_units_by_unit ON lesson_revision_units(unit_id);

CREATE TABLE lesson_progress (
    lesson_id        TEXT PRIMARY KEY REFERENCES lessons(id) ON DELETE CASCADE,
    revision_number  INTEGER NOT NULL,
    current_step     INTEGER NOT NULL CHECK(current_step >= 0),
    furthest_step    INTEGER NOT NULL CHECK(furthest_step >= current_step),
    checks_json      TEXT NOT NULL CHECK(
                         json_valid(checks_json) AND json_type(checks_json) = 'object'
                         AND length(checks_json) <= 1048576
                     ),
    revision         INTEGER NOT NULL CHECK(revision >= 1),
    started_at       INTEGER NOT NULL CHECK(started_at >= 0),
    updated_at       INTEGER NOT NULL CHECK(updated_at >= started_at),
    finished_at      INTEGER CHECK(finished_at IS NULL OR finished_at >= started_at),
    FOREIGN KEY(lesson_id, revision_number)
        REFERENCES lesson_revisions(lesson_id, revision_number) ON DELETE CASCADE
) STRICT;
