#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
SELF = pathlib.Path(__file__).resolve()
path = ROOT / "scripts/verify-project-manifests.mjs"
source = path.read_text()
old = 'equal("public schema fixture version", publicBaselineManifest.schemaVersion, 1);'
new = 'equal("public schema fixture version", publicBaselineManifest.schemaVersion, latest);'
if old in source:
    source = source.replace(old, new, 1)
elif new not in source:
    raise RuntimeError("scripts/verify-project-manifests.mjs: public schema fixture version contract missing")
path.write_text(source)
SELF.unlink()
