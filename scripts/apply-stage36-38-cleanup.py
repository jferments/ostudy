#!/usr/bin/env python3
"""Remove every temporary Stage 3.6–3.8 applicator and its workflow, including this file."""
from pathlib import Path
import subprocess
root=Path(__file__).resolve().parents[1]
for path in [root/'.github/workflows/stage36-38-apply.yml',*root.glob('scripts/apply-stage36-38-*.py')]:
    if path.exists(): path.unlink()
print('Removed temporary Stage 3.6–3.8 development tooling.')
