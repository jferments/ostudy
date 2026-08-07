#!/usr/bin/env python3
from pathlib import Path

path = Path("src-tauri/src/cards.rs")
text = path.read_text(encoding="utf-8")

unused_import = "use rusqlite::Connection;\n"
if text.count(unused_import) != 1:
    raise SystemExit("expected one unused rusqlite::Connection import")
text = text.replace(unused_import, "", 1)

helper = '''    fn tempfile_path() -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("ostudy-card-test-{suffix}"))
    }

    use std::time::{SystemTime, UNIX_EPOCH};
'''
if text.count(helper) != 1:
    raise SystemExit("expected one obsolete tempfile_path helper block")
text = text.replace(helper, "", 1)

if "tempfile_path" in text or "SystemTime" in text or "UNIX_EPOCH" in text:
    raise SystemExit("stale temporary-path test helper residue remains")

path.write_text(text, encoding="utf-8")
print("Removed Stage 3.12 Clippy-only stale card test residue.")
