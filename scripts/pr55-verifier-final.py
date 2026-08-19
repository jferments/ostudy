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
verifier.write_text(source)
SELF.unlink()
