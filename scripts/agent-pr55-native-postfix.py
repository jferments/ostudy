#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
# qualification-trigger: anti-heading-flashcard-prompt-contract

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

# The authoritative workflow generates/migrates frontend and native tests after
# the product applicators. Run final compatibility, portability, compile,
# prompt-contract restoration, semantic Clippy refactor, structural warning
# cleanup, verifier cleanup, and schema-contract alignment last while every
# one-shot helper still exists.
test_migration = ROOT / "scripts/agent-pr55-python-test-migrations.py"
test_source = test_migration.read_text()
hook = '''\n\n# Final native contract/test alignment runs after all generated test rewrites.\nimport runpy\nrunpy.run_path(str(ROOT / "scripts/agent-pr55-native-final-compat.py"), run_name="__main__")\nrunpy.run_path(str(ROOT / "scripts/agent-pr55-native-preset-sync.py"), run_name="__main__")\nrunpy.run_path(str(ROOT / "scripts/pr55-native-compile-final.py"), run_name="__main__")\nrunpy.run_path(str(ROOT / "scripts/pr55-prompt-contract-final.py"), run_name="__main__")\nrunpy.run_path(str(ROOT / "scripts/pr55-clippy-final.py"), run_name="__main__")\nrunpy.run_path(str(ROOT / "scripts/pr55-warning-final.py"), run_name="__main__")\nrunpy.run_path(str(ROOT / "scripts/pr55-verifier-final.py"), run_name="__main__")\nrunpy.run_path(str(ROOT / "scripts/pr55-schema-contract-final.py"), run_name="__main__")\n'''
if "agent-pr55-native-final-compat.py" not in test_source:
    test_migration.write_text(test_source.rstrip() + hook)
else:
    additions = []
    for script in [
        "agent-pr55-native-preset-sync.py",
        "pr55-native-compile-final.py",
        "pr55-prompt-contract-final.py",
        "pr55-clippy-final.py",
        "pr55-warning-final.py",
        "pr55-verifier-final.py",
        "pr55-schema-contract-final.py",
    ]:
        if script not in test_source:
            additions.append(
                f'runpy.run_path(str(ROOT / "scripts/{script}"), run_name="__main__")'
            )
    if additions:
        test_migration.write_text(test_source.rstrip() + "\n" + "\n".join(additions) + "\n")
