#!/usr/bin/env python3
"""One-shot PR #54 helper retained only until temporary CI cleanup.

The initial real-provider transformation has already been committed. Remaining
acceptance corrections are applied by the focused cleanup/cache helpers before
this temporary automation is removed.
"""
from pathlib import Path

required = {
    "src-tauri/src/task_cancellation.rs": "current_window_end",
    "src-tauri/src/ai/gateway.rs": "DOCUMENT_STREAM_PROGRESS_TIMEOUT_SECONDS",
    "src/screens/DocumentsScreen.tsx": "Active windows",
}
for path, marker in required.items():
    if marker not in Path(path).read_text():
        raise SystemExit(f"Initial PR #54 resilience change is missing: {path} lacks {marker}")

print("Initial PR #54 document resilience transformation already applied.")
