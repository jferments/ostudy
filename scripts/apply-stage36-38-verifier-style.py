#!/usr/bin/env python3
"""Include the dedicated Stage 3.6–3.8 stylesheet in the Stage 2 UI verifier."""
from pathlib import Path
root=Path(__file__).resolve().parents[1]
path=root/'scripts/verify-stage2.mjs'
text=path.read_text(encoding='utf-8')
old='  read("src","card-organization.css"),'
new='  readJoined(["src","card-organization.css"],["src","stage36-38.css"]),'
if old not in text:
    raise SystemExit('Stage 2 stylesheet verifier anchor changed')
path.write_text(text.replace(old,new,1),encoding='utf-8')
print('Included Stage 3.6–3.8 projection styles in the repository verifier.')
