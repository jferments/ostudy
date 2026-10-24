-- Release migration 22: documents left out of their subject's study set.
--
-- A learner picks which of a subject's documents its study materials come
-- from. A left-out document stays filed in the subject, but the subject's
-- study set skips it, the way a study range skips the pages outside it:
-- the subject's study guide, its lessons and a Tutor scoped to the subject
-- all leave it out. Moving the document to another subject, or out of
-- every subject, brings it back in.
--
-- The upgrade creates the table and writes no rows.
CREATE TABLE document_study_exclusions (
    document_id  TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE
                 CHECK(length(document_id) = 32),
    created_at   INTEGER NOT NULL CHECK(created_at >= 0)
) STRICT;

CREATE TRIGGER document_study_exclusions_follow_filing
AFTER UPDATE OF subject_id ON documents
WHEN OLD.subject_id IS NOT NEW.subject_id
BEGIN
    DELETE FROM document_study_exclusions WHERE document_id = NEW.id;
END;
CREATE TRIGGER knowledge_collection_view_study_exclusion_insert
AFTER INSERT ON document_study_exclusions BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id = (SELECT subject_id FROM documents WHERE id = NEW.document_id);
END;
CREATE TRIGGER knowledge_collection_view_study_exclusion_delete
BEFORE DELETE ON document_study_exclusions BEGIN
    UPDATE knowledge_collection_views SET stale = 1
    WHERE collection_id = (SELECT subject_id FROM documents WHERE id = OLD.document_id);
END;
