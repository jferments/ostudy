-- Tutor project and conversation organization. Projects are local learner-owned
-- organization/prompt preferences; conversation source authority remains in
-- tutor_conversations.scope_json and per-turn capturedScope snapshots.

CREATE TABLE tutor_projects (
    id TEXT PRIMARY KEY CHECK(length(id) = 32),
    title TEXT NOT NULL CHECK(length(trim(title)) BETWEEN 1 AND 160),
    description TEXT NOT NULL DEFAULT '' CHECK(length(description) <= 4000),
    instructions TEXT NOT NULL DEFAULT '' CHECK(length(instructions) <= 8000),
    default_scope_json TEXT NOT NULL
        CHECK(json_valid(default_scope_json) AND length(default_scope_json) <= 4096),
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    archived_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;
CREATE INDEX tutor_projects_active_idx
    ON tutor_projects(archived_at, title COLLATE NOCASE, id);

ALTER TABLE tutor_conversations ADD COLUMN project_id TEXT
    REFERENCES tutor_projects(id) ON DELETE SET NULL;
ALTER TABLE tutor_conversations ADD COLUMN organization_revision INTEGER NOT NULL DEFAULT 1
    CHECK(organization_revision >= 1);
ALTER TABLE tutor_conversations ADD COLUMN archived_at INTEGER;
CREATE INDEX tutor_conversations_project_idx
    ON tutor_conversations(project_id, archived_at, updated_at DESC, id);

ALTER TABLE tutor_turns ADD COLUMN captured_scope_json TEXT
    CHECK(captured_scope_json IS NULL OR
          (json_valid(captured_scope_json) AND length(captured_scope_json) <= 1024));
UPDATE tutor_turns
SET captured_scope_json = (
    SELECT scope_json FROM tutor_conversations
    WHERE tutor_conversations.id = tutor_turns.conversation_id
)
WHERE captured_scope_json IS NULL;

CREATE TABLE tutor_organization_operations (
    operation_id TEXT PRIMARY KEY CHECK(length(operation_id) = 32),
    action TEXT NOT NULL CHECK(action IN (
        'create_project', 'update_project', 'archive_project', 'restore_project',
        'delete_project', 'rename_conversation', 'move_conversation',
        'archive_conversation', 'restore_conversation', 'delete_conversation'
    )),
    target_kind TEXT NOT NULL CHECK(target_kind IN ('project', 'conversation')),
    target_id TEXT NOT NULL CHECK(length(target_id) = 32),
    request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
    result_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(result_json) AND length(result_json) <= 16384),
    created_at INTEGER NOT NULL
) STRICT;
CREATE INDEX tutor_organization_operations_target_idx
    ON tutor_organization_operations(target_kind, target_id, created_at);
