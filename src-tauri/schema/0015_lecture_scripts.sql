-- Release migration 15: lecture scripts.
--
-- A teacher's lecture for one saved lesson revision: what the teacher says
-- over each of the revision's lecture slides, and the few questions they stop
-- to ask the class, each with the teacher's reply to every option. The AI
-- writes a script once; the native owner validates it against the revision's
-- slides before it is kept and again whenever it is read. Writing the lecture
-- again replaces the revision's script. Deleting the lesson or the revision
-- deletes its script. The upgrade creates the table and writes no rows.
CREATE TABLE lecture_scripts (
    lesson_id        TEXT NOT NULL,
    revision_number  INTEGER NOT NULL,
    script_json      TEXT NOT NULL CHECK(
                         json_valid(script_json)
                         AND json_type(script_json) = 'object'
                         AND length(script_json) BETWEEN 2 AND 262144
                     ),
    provenance_json  TEXT NOT NULL CHECK(
                         json_valid(provenance_json) AND length(provenance_json) <= 65536
                     ),
    created_at       INTEGER NOT NULL CHECK(created_at >= 0),
    PRIMARY KEY(lesson_id, revision_number),
    FOREIGN KEY(lesson_id, revision_number)
        REFERENCES lesson_revisions(lesson_id, revision_number) ON DELETE CASCADE
) STRICT;
