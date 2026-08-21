#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
export OSTUDY_QUALIFICATION_COMMAND="bash ./scripts/smoke-startup-compatibility.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/build-state.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/diagnostics.bash"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

ostudy_parse_diagnostic_args "$@"
if (( ${#OSTUDY_DIAGNOSTIC_REMAINDER[@]} > 0 )); then
  printf 'Usage: bash ./scripts/smoke-startup-compatibility.sh [diagnostic options]\n' >&2
  ostudy_diagnostic_usage >&2
  exit 2
fi

cd "$ROOT_DIR"
ostudy_require_current_build
for command_name in sqlite3 python3 timeout; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'ERROR: %s is required for the startup compatibility smoke test.\n' "$command_name" >&2
    exit 1
  }
done

if [[ -z "${DISPLAY:-}" && "${OSTUDY_IN_STARTUP_XVFB:-0}" != "1" ]]; then
  command -v xvfb-run >/dev/null 2>&1 || {
    printf 'ERROR: xvfb-run is required for a headless startup compatibility smoke test.\n' >&2
    exit 1
  }
  exec env OSTUDY_IN_STARTUP_XVFB=1 xvfb-run -a bash "$0" "$@"
fi

TEMP_ROOT="$(mktemp -d -t ostudy-startup-compat.XXXXXXXX)"
cleanup() { rm -rf -- "$TEMP_ROOT"; }
trap cleanup EXIT

DATA_HOME="$TEMP_ROOT/data"
CONFIG_HOME="$TEMP_ROOT/config"
CACHE_HOME="$TEMP_ROOT/cache"
STATE_HOME="$TEMP_ROOT/state"
APP_DATA="$DATA_HOME/io.github.jferments.ostudy"
DATABASE="$APP_DATA/library.sqlite3"
APPLICATION_LOG="$APP_DATA/diagnostics/ostudy.jsonl"
COMMAND_LOG="$TEMP_ROOT/startup.log"
mkdir -p "$APP_DATA" "$CONFIG_HOME" "$CACHE_HOME" "$STATE_HOME"

python3 - "$ROOT_DIR" "$DATABASE" <<'PY'
from __future__ import annotations

import importlib.util
from pathlib import Path
import sqlite3
import sys

root = Path(sys.argv[1])
database = Path(sys.argv[2])
module_path = root / "scripts" / "restore_pre_public_backup.py"
spec = importlib.util.spec_from_file_location("ostudy_pre_public_restore", module_path)
if spec is None or spec.loader is None:
    raise RuntimeError("Could not load pre-public migration metadata")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

baseline = (root / "src-tauri" / "migrations" / "0001_public_baseline.sql").read_text()
connection = sqlite3.connect(database)
connection.executescript(baseline)
connection.executescript(
    r'''
PRAGMA foreign_keys = OFF;
BEGIN IMMEDIATE;

CREATE TABLE ai_provider_profiles_legacy (
    id                      TEXT PRIMARY KEY CHECK(length(id) = 32),
    display_name            TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 100),
    normalized_name         TEXT NOT NULL CHECK(length(normalized_name) BETWEEN 1 AND 100),
    provider_kind           TEXT NOT NULL
                            CHECK(provider_kind IN (
                                'llama_cpp', 'deepseek', 'ollama',
                                'openai', 'gemini', 'claude'
                            )),
    base_url                TEXT NOT NULL CHECK(length(base_url) BETWEEN 1 AND 2048),
    model_id                TEXT NOT NULL DEFAULT '' CHECK(length(model_id) <= 300),
    auth_mode               TEXT NOT NULL CHECK(auth_mode IN ('none', 'bearer')),
    enabled                 INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    allow_insecure_http     INTEGER NOT NULL DEFAULT 0 CHECK(allow_insecure_http IN (0, 1)),
    request_timeout_ms      INTEGER NOT NULL DEFAULT 60000
                            CHECK(request_timeout_ms BETWEEN 1000 AND 300000),
    credential_storage      TEXT NOT NULL DEFAULT 'none'
                            CHECK(credential_storage IN ('none', 'secure', 'session')),
    credential_epoch        INTEGER NOT NULL DEFAULT 0 CHECK(credential_epoch >= 0),
    revision                INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    deleted_at              INTEGER
) STRICT;

INSERT INTO ai_provider_profiles_legacy (
    id, display_name, normalized_name, provider_kind, base_url, model_id,
    auth_mode, enabled, allow_insecure_http, request_timeout_ms,
    credential_storage, credential_epoch, revision, created_at, updated_at, deleted_at
)
SELECT
    id, display_name, normalized_name, provider_kind, base_url, model_id,
    auth_mode, enabled, allow_insecure_http, request_timeout_ms,
    credential_storage, credential_epoch, revision, created_at, updated_at, deleted_at
FROM ai_provider_profiles;

DROP TABLE ai_provider_profiles;
ALTER TABLE ai_provider_profiles_legacy RENAME TO ai_provider_profiles;
CREATE UNIQUE INDEX ai_provider_profiles_active_name_idx
    ON ai_provider_profiles(normalized_name) WHERE deleted_at IS NULL;
CREATE INDEX ai_provider_profiles_active_updated_idx
    ON ai_provider_profiles(updated_at DESC) WHERE deleted_at IS NULL;

DROP TABLE schema_metadata;
DELETE FROM schema_migrations;
DELETE FROM data_repairs;

INSERT INTO decks (
    id, name, description, sort_order, created_at, updated_at,
    archived_at, deleted_at, revision, folder_id
) VALUES (
    '00000000000000000000000000000001', 'Preserved deck', '', 0,
    1000, 1000, NULL, NULL, 1, NULL
);

INSERT INTO notes (
    id, deck_id, note_type, content_format, content_schema_version,
    front_doc, back_doc, front_text, back_text, created_at, updated_at,
    deleted_at, revision, duplicate_hash, reverse_enabled, cloze_source, cloze_mode
) VALUES (
    '00000000000000000000000000000002',
    '00000000000000000000000000000001',
    'basic', 'tiptap-json', 1,
    '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Question"}]}]}',
    '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Answer"}]}]}',
    'Question', 'Answer', 1000, 1000, NULL, 1, '', 0, '', 'one_at_a_time'
);

INSERT INTO cards (
    id, note_id, deck_id, direction, template_ordinal,
    created_at, updated_at, suspended_at, deleted_at,
    buried_until, archive_suspended_at
) VALUES (
    '00000000000000000000000000000003',
    '00000000000000000000000000000002',
    '00000000000000000000000000000001',
    'forward', 0, 1000, 1000, NULL, NULL, NULL, NULL
);

INSERT INTO card_schedules (
    card_id, state, due_at, stability, difficulty, elapsed_days,
    scheduled_days, reps, lapses, last_review_at,
    scheduler_name, scheduler_version, updated_at, revision
) VALUES (
    '00000000000000000000000000000003',
    2, 2000, 1.0, 5.0, 1, 1, 1, 0, 1000,
    'fsrs', 'rs-fsrs-1.2.1', 1000, 1
);

INSERT INTO review_events (
    id, submission_id, session_id, card_id, reviewed_at, rating,
    answer_correct, confidence, response_time_ms, state_before, state_after,
    due_before, due_after, stability_before, stability_after,
    difficulty_before, difficulty_after, scheduled_days_before,
    scheduled_days_after, scheduler_name, scheduler_version, created_at
) VALUES (
    '00000000000000000000000000000004',
    '00000000000000000000000000000005', NULL,
    '00000000000000000000000000000003',
    1000, 3, 1, 'somewhat_confident', 1200, 1, 2,
    1000, 2000, 0.5, 1.0, 5.0, 5.0, 0, 1,
    'fsrs', 'rs-fsrs-1.2.1', 1000
);

INSERT INTO review_event_attributions (
    review_event_id, reviewed_at, review_local_date, time_zone_id,
    utc_offset_minutes, card_id, note_id, note_revision,
    owner_deck_id_at_review, captured_deck_name, direction, card_type,
    card_label, presentation_variant, question_excerpt, question_text_hash,
    question_was_truncated, attribution_source
) VALUES (
    '00000000000000000000000000000004', 1000, '2026-08-17', 'UTC', 0,
    '00000000000000000000000000000003',
    '00000000000000000000000000000002', 1,
    '00000000000000000000000000000001', 'Preserved deck',
    'forward', 'basic', 'Basic', '', 'Question',
    '0000000000000000000000000000000000000000000000000000000000000000',
    0, 'captured'
);
'''
)
for version, name, checksum in module.EXPECTED_MIGRATIONS:
    connection.execute(
        "INSERT INTO schema_migrations(version, name, checksum, applied_at) "
        "VALUES (?, ?, ?, 0)",
        (version, name, checksum),
    )
for version, name, checksum in module.EXPECTED_REPAIRS:
    connection.execute(
        "INSERT INTO data_repairs(version, name, checksum, applied_at, changed_rows) "
        "VALUES (?, ?, ?, 0, 0)",
        (version, name, checksum),
    )
connection.execute(
    f"PRAGMA user_version = {module.HISTORICAL_SQLITE_USER_VERSION}"
)
connection.commit()
connection.execute("PRAGMA foreign_keys = ON")
if connection.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
    raise RuntimeError("legacy fixture integrity check failed")
if connection.execute("PRAGMA foreign_key_check").fetchone() is not None:
    raise RuntimeError("legacy fixture foreign-key check failed")
connection.close()
PY

before_decks="$(sqlite3 "$DATABASE" 'SELECT COUNT(*) FROM decks;')"
before_cards="$(sqlite3 "$DATABASE" 'SELECT COUNT(*) FROM cards;')"
before_reviews="$(sqlite3 "$DATABASE" 'SELECT COUNT(*) FROM review_events;')"
before_attributions="$(sqlite3 "$DATABASE" 'SELECT COUNT(*) FROM review_event_attributions;')"

printf '\n==> Built-binary final pre-public database adoption boundary\n'
set +e
XDG_DATA_HOME="$DATA_HOME" \
XDG_CONFIG_HOME="$CONFIG_HOME" \
XDG_CACHE_HOME="$CACHE_HOME" \
XDG_STATE_HOME="$STATE_HOME" \
OSTUDY_LOG='warn,ostudy.app=debug,ostudy.database=debug' \
  timeout --signal=TERM 8s "$OSTUDY_BINARY" >"$COMMAND_LOG" 2>&1
status=$?
set -e

if [[ "$status" -ne 124 ]]; then
  cat "$COMMAND_LOG" >&2
  printf 'ERROR: adopted-data startup exited with status %s instead of remaining alive as a usable application.\n' "$status" >&2
  ostudy_capture_failure_artifacts \
    "Native startup compatibility smoke" \
    "$status" \
    "$COMMAND_LOG" \
    "$APPLICATION_LOG"
  exit 1
fi

[[ -f "$APPLICATION_LOG" ]] || {
  cat "$COMMAND_LOG" >&2
  printf 'ERROR: startup compatibility smoke did not create the structured application log.\n' >&2
  exit 1
}

grep -F '"event":"pre_public_database_adoption_completed"' "$APPLICATION_LOG" >/dev/null || {
  cat "$COMMAND_LOG" >&2
  printf 'ERROR: exact v26 startup did not complete data-preserving adoption.\n' >&2
  exit 1
}
grep -F '"event":"application_started"' "$APPLICATION_LOG" >/dev/null || {
  cat "$COMMAND_LOG" >&2
  printf 'ERROR: the application did not finish startup after pre-public adoption.\n' >&2
  exit 1
}

if grep -Eq 'application_startup_blocked|panicked at|Failed to setup app|panic_captured' "$COMMAND_LOG" "$APPLICATION_LOG"; then
  cat "$COMMAND_LOG" >&2
  printf 'ERROR: an adoptable final pre-public database reached a blocked or panic path.\n' >&2
  exit 1
fi

lineage="$(sqlite3 "$DATABASE" "SELECT value FROM schema_metadata WHERE key='schema_lineage';")"
schema_version="$(sqlite3 "$DATABASE" 'PRAGMA user_version;')"
migrations="$(sqlite3 -separator '|' "$DATABASE" 'SELECT version, name, checksum FROM schema_migrations ORDER BY version;')"
repairs="$(sqlite3 -separator '|' "$DATABASE" 'SELECT version, name, checksum, changed_rows FROM data_repairs ORDER BY version;')"
expected_migrations="$(cat <<'EOF'
1|public_baseline|2d978f8be026ecb820e71ba77a8585e1de51cb08fc2820714526a33284167f85
2|canonical_documents|dba2a5fd43b9544c1a4da9e060106f157ca5ac839a5c87ca831d7ceef8bb7733
EOF
)"
expected_repairs="$(cat <<'EOF'
1|canonical_semantic_chunk_hashes|779d93574a75f1e694fc4476bbf278ce7a540155cc4c35cf113f9ddb03d2a46f|0
2|canonical_block_hashes|02a07faa3635627246ef361160e94fe80a4634f7d36c97dbc3d6cb0819846085|0
EOF
)"
after_decks="$(sqlite3 "$DATABASE" 'SELECT COUNT(*) FROM decks;')"
after_cards="$(sqlite3 "$DATABASE" 'SELECT COUNT(*) FROM cards;')"
after_reviews="$(sqlite3 "$DATABASE" 'SELECT COUNT(*) FROM review_events;')"
after_attributions="$(sqlite3 "$DATABASE" 'SELECT COUNT(*) FROM review_event_attributions;')"

[[ "$lineage" == "ostudy-public-v1" ]] || {
  printf 'ERROR: adopted database has unexpected lineage: %s\n' "$lineage" >&2
  exit 1
}
[[ "$schema_version" == "2" ]] || {
  printf 'ERROR: adopted database has unexpected SQLite schema version: %s\n' "$schema_version" >&2
  exit 1
}
[[ "$migrations" == "$expected_migrations" ]] || {
  printf 'ERROR: adopted database has unexpected current migration ledger:\n%s\n' "$migrations" >&2
  exit 1
}
[[ "$repairs" == "$expected_repairs" ]] || {
  printf 'ERROR: adopted database has unexpected canonical repair ledger:\n%s\n' "$repairs" >&2
  exit 1
}
[[ "$before_decks" == "$after_decks" && "$before_cards" == "$after_cards" ]] || {
  printf 'ERROR: decks or cards changed during pre-public adoption.\n' >&2
  exit 1
}
[[ "$before_reviews" == "$after_reviews" && "$before_attributions" == "$after_attributions" ]] || {
  printf 'ERROR: review or analytics attribution rows changed during pre-public adoption.\n' >&2
  exit 1
}

backup_count="$(find "$APP_DATA" -maxdepth 1 -type f -name 'library.sqlite3.pre-public-v26-*.backup' | wc -l)"
[[ "$backup_count" -eq 1 ]] || {
  printf 'ERROR: expected one consistent pre-public SQLite backup, found %s.\n' "$backup_count" >&2
  exit 1
}

printf 'Native startup compatibility smoke passed: the built application backed up and adopted the actual historical final-v26 database, migrated it through the complete current public lineage, preserved decks/cards/reviews/analytics, completed startup, and did not enter a reset or panic path.\n'