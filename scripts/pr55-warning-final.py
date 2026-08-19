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
# Remove the legacy automatic-only wrapper by its stable function boundaries;
# formatter/type-alias changes must not make this cleanup brittle.
providers = ROOT / "src-tauri/src/ai/providers.rs"
source = providers.read_text()
legacy = "pub(crate) fn openrouter_provider_preferences("
routed = "pub(crate) fn openrouter_provider_preferences_with_routing("
start = source.find(legacy)
if start >= 0:
    cfg_start = source.rfind("#[cfg(test)]\n", 0, start)
    remove_start = cfg_start if cfg_start >= 0 and source[cfg_start:start].strip() == "#[cfg(test)]" else start
    routed_start = source.find(routed, start)
    if routed_start < 0:
        raise RuntimeError("src-tauri/src/ai/providers.rs: routed OpenRouter helper missing")
    source = source[:remove_start] + source[routed_start:]
if legacy in source:
    raise RuntimeError("src-tauri/src/ai/providers.rs: legacy OpenRouter preference wrapper still present")
providers.write_text(source)

SELF.unlink()
