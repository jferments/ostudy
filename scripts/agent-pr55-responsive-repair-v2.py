#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
PATH = ROOT / "scripts/agent-pr55-responsive-completion.py"
text = PATH.read_text()

old_import = """if \"LayoutDiagnosticsCard\" not in text:
    first_import_end = text.find(\"\\n\", text.find(\"import \"))
    insertion = 'import { LayoutDiagnosticsCard } from \"./LayoutDiagnosticsCard\";\\n'
    text = text[: first_import_end + 1] + insertion + text[first_import_end + 1 :]
    closing_candidates = [text.rfind(\"</div>\\n  );\"), text.rfind(\"</>\\n  );\")]
"""
new_import = """if \"LayoutDiagnosticsCard\" not in text:
    imports = list(re.finditer(r\"^import[\\s\\S]*?;\\n\", text, flags=re.M))
    if not imports:
        raise RuntimeError(f\"{path}: import boundary not found\")
    import_end = imports[-1].end()
    insertion = 'import { LayoutDiagnosticsCard } from \"./LayoutDiagnosticsCard\";\\n'
    text = text[:import_end] + insertion + text[import_end:]
    closing_candidates = [text.rfind(\"</div>\\n  );\"), text.rfind(\"</>\\n  );\")]
"""
if old_import in text:
    text = text.replace(old_import, new_import, 1)
elif new_import not in text:
    raise RuntimeError("responsive applicator import repair anchor not found")

old_doc = """path = \"docs/architecture/ui.md\"
text = read(path)
addition = '''
"""
new_doc = """path = \"docs/architecture/ui.md\"
doc_path = ROOT / path
text = doc_path.read_text() if doc_path.exists() else \"# UI architecture\\n\"
addition = '''
"""
if old_doc in text:
    text = text.replace(old_doc, new_doc, 1)
elif new_doc not in text:
    raise RuntimeError("responsive applicator documentation repair anchor not found")

PATH.write_text(text)
