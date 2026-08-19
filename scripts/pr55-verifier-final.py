#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
SELF = pathlib.Path(__file__).resolve()
verifier = ROOT / "scripts/verify-ui-ai-overhaul.mjs"
source = verifier.read_text()

import_line = 'import fs from "node:fs";'
# The shebang must remain the first bytes in an executable Node ESM script.
source = source.replace(
    import_line + "\n\n#!/usr/bin/env node\n",
    "#!/usr/bin/env node\n\n" + import_line + "\n",
    1,
)
if import_line not in source:
    if source.startswith("#!/usr/bin/env node\n"):
        boundary = len("#!/usr/bin/env node\n")
        source = source[:boundary] + "\n" + import_line + "\n" + source[boundary:]
    else:
        source = import_line + "\n\n" + source

# Keep the added architecture checks self-contained. Reuse requireText when the
# generated verifier already supplies it; otherwise define the same minimal
# boolean contract immediately after imports.
if "function requireText(" not in source and "const requireText =" not in source:
    anchor = import_line + "\n"
    helper = '''\nfunction requireText(condition, message) {\n  if (!condition) throw new Error(message);\n}\n'''
    source = source.replace(anchor, anchor + helper, 1)

for message in [
    "AI setup must remain ephemeral until model commit",
    "AI task policy and prompt presets require dedicated native tables",
    "semantic target coverage must be durable",
    "AI task policy must not be persisted as generic app_settings rows",
    "Generate More must use durable semantic coverage",
]:
    marker = f', "{message}");'
    index = source.find(marker)
    if index < 0:
        continue
    line_start = source.rfind("\n", 0, index) + 1
    if source.startswith("assert(", line_start):
        source = source[:line_start] + "requireText(" + source[line_start + len("assert("):]

verifier.write_text(source)
SELF.unlink()
