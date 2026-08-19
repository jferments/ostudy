#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]

migration = ROOT / "src-tauri/migrations/0002_ai_task_policy_learning_coverage.sql"
text = migration.read_text()
text = text.replace(
    "'comparison', 'equation', 'application', 'misconception', 'prerequisite'",
    "'comparison', 'equation', 'application', 'misconception', 'prerequisite', 'section'",
)
migration.write_text(text)
checksum = hashlib.sha256(text.encode()).hexdigest()

registry = ROOT / "src-tauri/src/database.rs"
source = registry.read_text()
pattern = re.compile(
    r'(name: "ai_task_policy_learning_coverage",\n\s*checksum: ")[0-9a-f]{64}("[,])'
)
source, count = pattern.subn(rf'\g<1>{checksum}\g<2>', source, count=1)
if count != 1:
    raise RuntimeError("database.rs: migration-2 checksum entry not found")
registry.write_text(source)
