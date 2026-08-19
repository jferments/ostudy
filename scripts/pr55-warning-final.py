#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SELF = pathlib.Path(__file__).resolve()

# The pre-v2 settings source imports the reasoning mirror primitives in a compact
# grouped `use` statement. Once the mirror is removed, keep only HashSet.
settings = ROOT / "src-tauri/src/settings.rs"
source = settings.read_text()
source = source.replace(
    "use std::{collections::HashSet, sync::{LazyLock, RwLock}};\n",
    "use std::collections::HashSet;\n",
    1,
)
source = source.replace(
    "use std::{collections::HashSet, sync::{RwLock, LazyLock}};\n",
    "use std::collections::HashSet;\n",
    1,
)
settings.write_text(source)

# Production OpenRouter document requests use the policy-aware routing helper.
# The old automatic-only wrapper is now unused even in tests, so remove it
# rather than suppressing dead-code warnings.
providers = ROOT / "src-tauri/src/ai/providers.rs"
source = providers.read_text()
source, count = re.subn(
    r'''\n#\[cfg\(test\)\]\npub\(crate\) fn openrouter_provider_preferences\(\n\s*require_parameters: bool,\n\s*require_zero_data_retention: bool,\n\) -> serde_json::Value \{\n\s*openrouter::provider_preferences\(require_parameters, require_zero_data_retention\)\n\}\n''',
    "\n",
    source,
    count=1,
)
if count == 0:
    source, count = re.subn(
        r'''\npub\(crate\) fn openrouter_provider_preferences\(\n\s*require_parameters: bool,\n\s*require_zero_data_retention: bool,\n\) -> serde_json::Value \{\n\s*openrouter::provider_preferences\(require_parameters, require_zero_data_retention\)\n\}\n''',
        "\n",
        source,
        count=1,
    )
if count == 0 and "openrouter_provider_preferences(" in source:
    raise RuntimeError("src-tauri/src/ai/providers.rs: legacy OpenRouter preference wrapper still present")
providers.write_text(source)

SELF.unlink()
