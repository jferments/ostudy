from pathlib import Path

path = Path('.github/pr41-quality-pass.py')
text = path.read_text()
old = "Cloze syntax, tags, duplicate fingerprints, fresh schedules, and transactional deck/card creation. There is no generated-card review queue."
new = "Cloze syntax, subject-matter tags, duplicate fingerprints, fresh schedules, and transactional deck/card creation. There is no generated-card review queue. Document provenance remains in the managed-document/deck relationship and never becomes learner-facing tags such as `PDF` or `AI generated`; defensive normalization strips those legacy provenance labels if a model returns them anyway."
if old not in text:
    raise SystemExit('expected architecture-doc matcher not found in PR41 helper')
path.write_text(text.replace(old, new, 1))
