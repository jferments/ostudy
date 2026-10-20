-- Release migration 12: how far a learner has read each document study guide.
--
-- document_study_guide_section_reads keeps one row for each guide section the
-- learner marked as read, keyed by the section's topic, with the hash of the
-- section text they read. Remaking a guide keeps a topic's mark only while its
-- section text is unchanged, so a rewritten section reads as unread again
-- without rewriting the row. document_study_guide_positions keeps the section a
-- learner last opened in each document's guide, so the reader can reopen it.
--
-- Rows belong to the document and its topics rather than to one guide revision,
-- and go with them. Deleting a guide clears them in the same transaction.
-- Reading is not a review: nothing here changes a schedule or a review event.
-- The upgrade creates the tables and writes no rows.
CREATE TABLE document_study_guide_section_reads (
    document_id   TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    topic_id      TEXT NOT NULL REFERENCES document_learning_topics(id) ON DELETE CASCADE,
    content_hash  TEXT NOT NULL CHECK(length(content_hash) = 64),
    read_at       INTEGER NOT NULL CHECK(read_at >= 0),
    PRIMARY KEY(document_id, topic_id)
) STRICT;

CREATE INDEX document_study_guide_section_reads_by_topic
    ON document_study_guide_section_reads(topic_id);

CREATE TABLE document_study_guide_positions (
    document_id   TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    topic_id      TEXT NOT NULL REFERENCES document_learning_topics(id) ON DELETE CASCADE,
    opened_at     INTEGER NOT NULL CHECK(opened_at >= 0)
) STRICT;

CREATE INDEX document_study_guide_positions_by_opened
    ON document_study_guide_positions(opened_at);

CREATE INDEX document_study_guide_positions_by_topic
    ON document_study_guide_positions(topic_id);
